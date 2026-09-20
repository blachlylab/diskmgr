//! GPT / geom disk inventory.
//!
//! Fixture path: JSON (`examples/geom/disks.json`).
//! Live FreeBSD: `gpart list` and `geom disk list` are **text** on 13.x
//! (`--libxo json` is ignored).

use std::path::Path;

use serde::Deserialize;

use crate::error::{Error, Result};

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
pub struct GptPartition {
    pub provider: String,
    pub index: u32,
    #[serde(rename = "type")]
    pub type_name: String,
    pub label: Option<String>,
    pub length: Option<u64>,
}

impl GptPartition {
    /// `gpt/zfs0` when labeled, otherwise the provider name (`da0p1`).
    pub fn display_name(&self) -> String {
        match self
            .label
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            Some(label) => format!("gpt/{label}"),
            None => self.provider.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct GeomDisk {
    pub name: String,
    pub ident: Option<String>,
    pub lunid: Option<String>,
    pub descr: Option<String>,
    pub mediasize: Option<u64>,
    pub scheme: Option<String>,
    #[serde(default)]
    pub partitions: Vec<GptPartition>,
}

impl GeomDisk {
    pub fn has_gpt(&self) -> bool {
        self.scheme
            .as_deref()
            .is_some_and(|s| s.eq_ignore_ascii_case("GPT"))
            || self.partitions.iter().any(|p| p.label.is_some())
    }

    pub fn gpt_labels(&self) -> Vec<String> {
        self.partitions.iter().map(|p| p.display_name()).collect()
    }
}

#[derive(Deserialize)]
struct GeomFile {
    disks: Vec<GeomDisk>,
}

pub fn load_geom_disks(path: &Path) -> Result<Vec<GeomDisk>> {
    let text = std::fs::read_to_string(path).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let file: GeomFile = serde_json::from_str(&text).map_err(|source| Error::Json {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(file.disks)
}

/// Merge `geom disk list` identity with `gpart list` partition tables.
pub fn merge_disks(ident_disks: Vec<GeomDisk>, tables: Vec<GeomDisk>) -> Vec<GeomDisk> {
    use std::collections::HashMap;
    let mut by_name: HashMap<String, GeomDisk> = ident_disks
        .into_iter()
        .map(|d| (d.name.clone(), d))
        .collect();
    for table in tables {
        match by_name.get_mut(&table.name) {
            Some(disk) => {
                if table.scheme.is_some() {
                    disk.scheme = table.scheme;
                }
                if !table.partitions.is_empty() {
                    disk.partitions = table.partitions;
                }
            }
            None => {
                by_name.insert(table.name.clone(), table);
            }
        }
    }
    let mut disks: Vec<_> = by_name.into_values().collect();
    disks.sort_by(|a, b| a.name.cmp(&b.name));
    disks
}

pub fn parse_geom_disk_list(text: &str) -> Vec<GeomDisk> {
    let mut disks = Vec::new();
    let mut cur: Option<GeomDisk> = None;
    for line in text.lines() {
        if let Some(name) = prefix_value(line, "Geom name:") {
            if let Some(disk) = cur.take() {
                disks.push(disk);
            }
            cur = Some(GeomDisk {
                name,
                ident: None,
                lunid: None,
                descr: None,
                mediasize: None,
                scheme: None,
                partitions: Vec::new(),
            });
            continue;
        }
        let Some(disk) = cur.as_mut() else {
            continue;
        };
        if let Some(v) = indented_value(line, "ident") {
            disk.ident = nonempty_opt(&v);
        } else if let Some(v) = indented_value(line, "lunid") {
            disk.lunid = nonempty_opt(&v);
        } else if let Some(v) = indented_value(line, "descr") {
            disk.descr = nonempty_opt(&v);
        } else if let Some(v) = indented_value(line, "Mediasize") {
            disk.mediasize = parse_leading_u64(&v);
        }
    }
    if let Some(disk) = cur {
        disks.push(disk);
    }
    disks
}

pub fn parse_gpart_list(text: &str) -> Vec<GeomDisk> {
    let mut disks = Vec::new();
    let mut cur: Option<GeomDisk> = None;
    let mut in_providers = false;
    for line in text.lines() {
        if let Some(name) = prefix_value(line, "Geom name:") {
            if let Some(disk) = cur.take() {
                disks.push(disk);
            }
            cur = Some(GeomDisk {
                name,
                ident: None,
                lunid: None,
                descr: None,
                mediasize: None,
                scheme: None,
                partitions: Vec::new(),
            });
            in_providers = false;
            continue;
        }
        let trimmed = line.trim();
        if trimmed == "Providers:" {
            in_providers = true;
            continue;
        }
        if trimmed == "Consumers:" {
            in_providers = false;
            continue;
        }
        let Some(disk) = cur.as_mut() else {
            continue;
        };
        if let Some(scheme) = prefix_value(line, "scheme:") {
            disk.scheme = nonempty_opt(&scheme);
            continue;
        }
        if in_providers {
            if let Some(name) = provider_name(line) {
                disk.partitions.push(GptPartition {
                    provider: name,
                    index: 0,
                    type_name: String::new(),
                    label: None,
                    length: None,
                });
                continue;
            }
            if let Some(part) = disk.partitions.last_mut() {
                if let Some(v) = indented_value(line, "label") {
                    part.label = nonempty_label(&v);
                } else if let Some(v) = indented_value(line, "type") {
                    part.type_name = v;
                } else if let Some(v) = indented_value(line, "index") {
                    part.index = v.parse().unwrap_or(0);
                } else if let Some(v) = indented_value(line, "length") {
                    part.length = parse_leading_u64(&v);
                } else if part.length.is_none() {
                    if let Some(v) = indented_value(line, "Mediasize") {
                        part.length = parse_leading_u64(&v);
                    }
                }
            }
        }
    }
    if let Some(disk) = cur {
        disks.push(disk);
    }
    disks
}

/// Fallback when `gpart list` is unavailable.
pub fn parse_gpart_show_lp(text: &str) -> Vec<GeomDisk> {
    let mut disks = Vec::new();
    let mut cur: Option<GeomDisk> = None;
    for line in text.lines() {
        if let Some(rest) = line.trim_start().strip_prefix("=>") {
            if let Some(disk) = cur.take() {
                disks.push(disk);
            }
            let cols: Vec<&str> = rest.split_whitespace().collect();
            // 40  500118112  ada0  GPT  (238G)
            if cols.len() >= 4 {
                cur = Some(GeomDisk {
                    name: cols[2].to_string(),
                    ident: None,
                    lunid: None,
                    descr: None,
                    mediasize: None,
                    scheme: nonempty_opt(cols[3]),
                    partitions: Vec::new(),
                });
            }
            continue;
        }
        let Some(disk) = cur.as_mut() else {
            continue;
        };
        if line.contains("- free -") {
            continue;
        }
        let cols: Vec<&str> = line.split_whitespace().collect();
        // 40  1024  ada0p1  gptboot0  (512K)
        if cols.len() >= 4 && !cols[2].starts_with('(') {
            disk.partitions.push(GptPartition {
                provider: cols[2].to_string(),
                index: disk.partitions.len() as u32 + 1,
                type_name: String::new(),
                label: nonempty_label(cols[3]),
                length: None,
            });
        }
    }
    if let Some(disk) = cur {
        disks.push(disk);
    }
    disks
}

fn prefix_value(line: &str, prefix: &str) -> Option<String> {
    line.trim_start()
        .strip_prefix(prefix)
        .map(|v| v.trim().to_string())
}

fn indented_value(line: &str, key: &str) -> Option<String> {
    let trimmed = line.trim();
    let (k, v) = trimmed.split_once(':')?;
    if k.trim() == key {
        Some(v.trim().to_string())
    } else {
        None
    }
}

fn provider_name(line: &str) -> Option<String> {
    let trimmed = line.trim();
    let rest = trimmed.split_once("Name:")?.1.trim();
    if rest.is_empty() {
        None
    } else {
        Some(rest.to_string())
    }
}

fn nonempty_opt(s: &str) -> Option<String> {
    let s = s.trim();
    if s.is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}

fn nonempty_label(s: &str) -> Option<String> {
    let s = s.trim();
    if s.is_empty() || s.eq_ignore_ascii_case("(null)") {
        None
    } else {
        Some(s.to_string())
    }
}

fn parse_leading_u64(s: &str) -> Option<u64> {
    s.split_whitespace().next()?.parse().ok()
}

pub fn find_geom_file(fixture_dir: &Path) -> Option<std::path::PathBuf> {
    let candidates = [
        fixture_dir.join("disks.json"),
        fixture_dir.join("geom/disks.json"),
        fixture_dir
            .parent()
            .map(|p| p.join("geom/disks.json"))
            .unwrap_or_default(),
    ];
    candidates.into_iter().find(|p| p.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lab_geom_fixture_loads() {
        let disks = load_geom_disks(Path::new("examples/geom/disks.json")).unwrap();
        assert_eq!(disks.len(), 61);
        let ada0 = disks.iter().find(|d| d.name == "ada0").unwrap();
        assert!(ada0.has_gpt());
        assert_eq!(
            ada0.gpt_labels(),
            vec!["gpt/gptboot0", "gpt/swap0", "gpt/zfs0"]
        );
        let da0 = disks.iter().find(|d| d.name == "da0").unwrap();
        assert!(!da0.has_gpt());
        let da33 = disks.iter().find(|d| d.name == "da33").unwrap();
        assert_eq!(da33.gpt_labels(), vec!["gpt/hog-FGH6UV3S-spare"]);
        let da57 = disks.iter().find(|d| d.name == "da57").unwrap();
        assert_eq!(da57.gpt_labels(), vec!["gpt/hog-TMKTXFDQ"]);
        assert!(da0.lunid.is_some());
        let text = std::fs::read_to_string("examples/geom/disks.json").unwrap();
        assert!(!text.contains("JEG9TZ7N"));
        assert!(!text.contains("VLKPN58V"));
        assert!(!text.contains("WWZCQLXA"));
    }

    #[test]
    fn parse_geom_disk_list_ident_and_lunid() {
        let text = "\
Geom name: ada0
Providers:
1. Name: ada0
   ident: ABC123
   lunid: 5000aabbccddeeff
   descr: Micron TEST
   Mediasize: 256000 (256K)

Geom name: da0
Providers:
1. Name: da0
   ident: SER0001
   lunid: 5000cafebabe0001
   descr: HGST TEST
   Mediasize: 8000000 (8.0M)
";
        let disks = parse_geom_disk_list(text);
        assert_eq!(disks.len(), 2);
        assert_eq!(disks[0].name, "ada0");
        assert_eq!(disks[0].ident.as_deref(), Some("ABC123"));
        assert_eq!(disks[0].lunid.as_deref(), Some("5000aabbccddeeff"));
        assert_eq!(disks[0].mediasize, Some(256000));
    }

    #[test]
    fn parse_gpart_list_partitions() {
        let text = "\
Geom name: ada0
scheme: GPT
Providers:
1. Name: ada0p1
   label: gptboot0
   type: freebsd-boot
   index: 1
   length: 524288
2. Name: ada0p2
   label: (null)
   type: freebsd-swap
   index: 2
   length: 2147483648
Consumers:
1. Name: ada0
   Mediasize: 256000 (256K)
";
        let disks = parse_gpart_list(text);
        assert_eq!(disks.len(), 1);
        assert_eq!(disks[0].scheme.as_deref(), Some("GPT"));
        assert_eq!(disks[0].partitions.len(), 2);
        assert_eq!(disks[0].partitions[0].label.as_deref(), Some("gptboot0"));
        assert_eq!(disks[0].partitions[1].label, None);
        assert_eq!(disks[0].partitions[1].type_name, "freebsd-swap");
    }

    #[test]
    fn merge_prefers_gpart_table_on_matching_name() {
        let ident = parse_geom_disk_list(
            "Geom name: ada0\nProviders:\n1. Name: ada0\n   ident: ABC123\n   lunid: 00aa\n",
        );
        let tables = parse_gpart_list(
            "Geom name: ada0\nscheme: GPT\nProviders:\n1. Name: ada0p1\n   label: zfs0\n   type: freebsd-zfs\n   index: 1\n   length: 10\nConsumers:\n1. Name: ada0\n",
        );
        let merged = merge_disks(ident, tables);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].ident.as_deref(), Some("ABC123"));
        assert_eq!(merged[0].gpt_labels(), vec!["gpt/zfs0"]);
    }

    #[test]
    fn parse_gpart_show_lp_fallback() {
        let text = "\
=>       40  500118112    ada0  GPT  (238G)
         40       1024  ada0p1  gptboot0  (512K)
       1064        984          - free -  (492K)
       2048    4194304  ada0p2  swap0  (2.0G)
";
        let disks = parse_gpart_show_lp(text);
        assert_eq!(disks[0].name, "ada0");
        assert_eq!(disks[0].scheme.as_deref(), Some("GPT"));
        assert_eq!(disks[0].gpt_labels(), vec!["gpt/gptboot0", "gpt/swap0"]);
    }
}
