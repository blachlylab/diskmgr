//! GPT / geom disk inventory (fixture JSON; live text parsers later).

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
}
