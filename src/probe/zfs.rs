//! ZFS pool membership from `zpool status -P`.
//!
//! TODO: parse `zpool status -j` (JSON) when the lab host's OpenZFS is new
//! enough. Until then the text format from `-P` is the contract.

use std::path::Path;

use crate::error::{Error, Result};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZfsUsage {
    pub pool: String,
    pub pool_state: String,
    pub vdev: String,
    pub vdev_kind: String,
    pub role: String,
    /// Path as ZFS reported it (`/dev/da9` or `/dev/gpt/…`).
    pub path: String,
    pub state: String,
    pub read_err: u64,
    pub write_err: u64,
    pub cksum_err: u64,
}

impl ZfsUsage {
    pub fn path_is_unstable(&self) -> bool {
        let name = self.path.rsplit('/').next().unwrap_or(&self.path);
        kernel_leaf(name).is_some() && !self.path.contains("/gpt/")
    }

    pub fn has_errors(&self) -> bool {
        self.read_err > 0 || self.write_err > 0 || self.cksum_err > 0
    }
}

pub fn load_zpool_status(path: &Path) -> Result<Vec<ZfsUsage>> {
    let text = std::fs::read_to_string(path).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(parse_zpool_status_p(&text))
}

pub fn find_zfs_file(fixture_dir: &Path) -> Option<std::path::PathBuf> {
    let candidates = [
        fixture_dir.join("status-P.txt"),
        fixture_dir.join("zfs/status-P.txt"),
        fixture_dir
            .parent()
            .map(|p| p.join("zfs/status-P.txt"))
            .unwrap_or_default(),
    ];
    candidates.into_iter().find(|p| p.is_file())
}

pub fn parse_zpool_status_p(text: &str) -> Vec<ZfsUsage> {
    let mut out = Vec::new();
    for block in split_pools(text) {
        out.extend(parse_pool_block(&block));
    }
    out
}

fn split_pools(text: &str) -> Vec<String> {
    let mut pools = Vec::new();
    let mut cur = String::new();
    for line in text.lines() {
        if line.trim_start().starts_with("pool:") && !cur.trim().is_empty() {
            pools.push(std::mem::take(&mut cur));
        }
        cur.push_str(line);
        cur.push('\n');
    }
    if !cur.trim().is_empty() {
        pools.push(cur);
    }
    pools
}

fn parse_pool_block(block: &str) -> Vec<ZfsUsage> {
    let mut pool = String::new();
    let mut pool_state = String::new();
    let mut in_config = false;
    let mut stack: Vec<(usize, String)> = Vec::new();
    let mut leaves = Vec::new();

    for line in block.lines() {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("pool:") {
            pool = rest.trim().to_string();
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("state:") {
            pool_state = rest.trim().to_string();
            continue;
        }
        if trimmed.starts_with("config:") {
            in_config = true;
            continue;
        }
        if trimmed.starts_with("errors:") {
            in_config = false;
            continue;
        }
        if !in_config {
            continue;
        }
        if trimmed.starts_with("NAME") {
            continue;
        }
        let Some((indent, name, state, read, write, cksum)) = parse_config_line(line) else {
            continue;
        };
        while stack.last().is_some_and(|(i, _)| *i >= indent) {
            stack.pop();
        }
        if is_leaf(&name) {
            let (vdev, vdev_kind, role) = classify(&stack);
            leaves.push(ZfsUsage {
                pool: pool.clone(),
                pool_state: pool_state.clone(),
                vdev,
                vdev_kind,
                role,
                path: name,
                state,
                read_err: read,
                write_err: write,
                cksum_err: cksum,
            });
        } else {
            stack.push((indent, name));
        }
    }
    leaves
}

fn parse_config_line(line: &str) -> Option<(usize, String, String, u64, u64, u64)> {
    let indent = line.chars().take_while(|c| *c == ' ').count();
    let rest = line.trim();
    if rest.is_empty() {
        return None;
    }
    let mut parts = rest.split_whitespace();
    let name = parts.next()?.to_string();
    let state = parts.next()?.to_string();
    match state.as_str() {
        "ONLINE" | "DEGRADED" | "FAULTED" | "OFFLINE" | "UNAVAIL" | "REMOVED" | "AVAIL"
        | "INUSE" => {}
        _ => return None,
    }
    let read = parse_err(parts.next()?)?;
    let write = parse_err(parts.next()?)?;
    let cksum = parse_err(parts.next()?)?;
    Some((indent, name, state, read, write, cksum))
}

fn parse_err(s: &str) -> Option<u64> {
    let s = s.trim();
    if let Some(n) = s.strip_suffix('K').or_else(|| s.strip_suffix('k')) {
        return Some((n.parse::<f64>().ok()? * 1000.0).round() as u64);
    }
    if let Some(n) = s.strip_suffix('M').or_else(|| s.strip_suffix('m')) {
        return Some((n.parse::<f64>().ok()? * 1_000_000.0).round() as u64);
    }
    s.parse().ok()
}

fn is_leaf(name: &str) -> bool {
    let base = name.rsplit('/').next().unwrap_or(name);
    name.contains("/dev/") || kernel_leaf(base).is_some()
}

/// `da12`, `ada0`, `ada0p3` → disk unit `da12` / `ada0`.
pub fn kernel_leaf(name: &str) -> Option<&str> {
    let name = name.trim_start_matches("/dev/");
    let bytes = name.as_bytes();
    let mut i = if name.starts_with("ada") || name.starts_with("nda") {
        3
    } else if name.starts_with("da") {
        2
    } else {
        return None;
    };
    let digits_start = i;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i == digits_start {
        return None;
    }
    // ada0p3 → ada0
    Some(&name[..i])
}

pub fn gpt_label_from_path(path: &str) -> Option<&str> {
    let path = path.trim_start_matches("/dev/");
    path.strip_prefix("gpt/")
}

fn classify(stack: &[(usize, String)]) -> (String, String, String) {
    let mut role = "data";
    for (_, name) in stack {
        match name.as_str() {
            "spares" => role = "spare",
            "logs" | "log" => role = "log",
            "cache" => role = "cache",
            "special" => role = "special",
            "dedup" => role = "dedup",
            _ => {}
        }
    }
    let vdev = stack
        .iter()
        .rev()
        .find(|(_, n)| n.contains('-') || n.starts_with("raidz") || n.starts_with("mirror"))
        .map(|(_, n)| n.clone())
        .or_else(|| stack.last().map(|(_, n)| n.clone()))
        .unwrap_or_else(|| "—".into());
    let vdev_kind = vdev.split('-').next().unwrap_or(&vdev).to_string();
    (vdev, vdev_kind, role.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_lab_status_p() {
        let text = std::fs::read_to_string("examples/zfs/status-P.txt").unwrap();
        let leaves = parse_zpool_status_p(&text);
        assert_eq!(leaves.iter().filter(|l| l.pool == "bigpool").count(), 24);
        assert_eq!(leaves.iter().filter(|l| l.pool == "hog").count(), 32);
        assert_eq!(leaves.iter().filter(|l| l.pool == "zroot").count(), 2);

        let da9 = leaves.iter().find(|l| l.path == "/dev/da9").unwrap();
        assert_eq!(da9.pool, "hog");
        assert_eq!(da9.vdev, "raidz2-1");
        assert_eq!(da9.role, "data");
        assert_eq!(da9.read_err, 246);

        let gpt = leaves
            .iter()
            .find(|l| l.path == "/dev/gpt/hog-VPUN999Y-spare")
            .unwrap();
        assert_eq!(gpt.vdev, "raidz2-3");
        assert_eq!(gpt.role, "data"); // GPT name says spare; zpool role is data
        assert_eq!(gpt.read_err, 3510);
        assert_eq!(gpt.cksum_err, 1);

        let root = leaves.iter().find(|l| l.path == "/dev/ada0p3").unwrap();
        assert_eq!(root.pool, "zroot");
        assert_eq!(root.vdev, "mirror-0");
        assert_eq!(root.vdev_kind, "mirror");
        assert!(root.path_is_unstable());

        let labeled = leaves
            .iter()
            .find(|l| l.path == "/dev/gpt/bigpool-TL42G0L0")
            .unwrap();
        assert!(!labeled.path_is_unstable());
        assert!(!text.contains("ZZ00M8V9"));
        assert!(!text.contains("JEG9TZ7N"));
    }

    #[test]
    fn error_counts() {
        assert_eq!(parse_err("0"), Some(0));
        assert_eq!(parse_err("246"), Some(246));
        assert_eq!(parse_err("3.51K"), Some(3510));
    }

    #[test]
    fn kernel_leaf_strips_partition() {
        assert_eq!(kernel_leaf("ada0p3"), Some("ada0"));
        assert_eq!(kernel_leaf("/dev/da12"), Some("da12"));
        assert_eq!(kernel_leaf("gpt/foo"), None);
    }
}
