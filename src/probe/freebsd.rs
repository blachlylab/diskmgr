//! Live probes: `sesutil`, `gpart`/`geom`, `zpool status -P`.
//!
//! Each source is independent. A failure becomes a warning; other sources
//! still populate the snapshot.

use std::process::Command;

use super::fixture::{FixtureProbe, attach_geom, attach_zfs};
use super::geom::{merge_disks, parse_geom_disk_list, parse_gpart_list, parse_gpart_show_lp};
use super::sesutil::parse_sesutil_json;
use super::zfs::parse_zpool_status_p;
use crate::error::{Error, Result};

pub fn live_probe() -> Result<FixtureProbe> {
    if !cfg!(target_os = "freebsd") {
        return Err(Error::Probe(
            "live probe is FreeBSD-only; pass --fixture DIR for recorded data".into(),
        ));
    }
    Ok(collect())
}

fn collect() -> FixtureProbe {
    let mut warnings = Vec::new();

    let enclosures = match probe_ses() {
        Ok(v) => v,
        Err(e) => {
            warnings.push(format!("sesutil: {e}"));
            Vec::new()
        }
    };

    let (disks, geom_probed) = match probe_geom() {
        Ok(v) => (v, true),
        Err(e) => {
            warnings.push(format!("geom: {e}"));
            (Vec::new(), false)
        }
    };

    let (zfs, zfs_probed) = match probe_zfs() {
        Ok(v) => (v, true),
        Err(e) => {
            warnings.push(format!("zpool: {e}"));
            (Vec::new(), false)
        }
    };

    let mut snap = FixtureProbe {
        source: std::path::PathBuf::from("live"),
        enclosures,
        disks,
        geom_probed,
        zfs,
        zfs_probed,
        warnings,
    };
    if snap.geom_probed {
        attach_geom(&mut snap.enclosures, &snap.disks);
    }
    if snap.zfs_probed {
        attach_zfs(&mut snap.enclosures, &snap.zfs);
    }
    snap
}

fn probe_ses() -> Result<Vec<super::sesutil::SesEnclosure>> {
    let map = run(
        &["sesutil", "/usr/sbin/sesutil"],
        &["map", "--libxo", "json"],
    )?;
    let show = run(
        &["sesutil", "/usr/sbin/sesutil"],
        &["show", "--libxo", "json"],
    )?;
    let status = run(
        &["sesutil", "/usr/sbin/sesutil"],
        &["status", "--libxo", "json"],
    )?;
    parse_sesutil_json(&map, &show, &status)
}

fn probe_geom() -> Result<Vec<super::geom::GeomDisk>> {
    let ident_disks = match run(&["geom", "/sbin/geom"], &["disk", "list"]) {
        Ok(text) => parse_geom_disk_list(&text),
        Err(_) => Vec::new(),
    };
    let mut tables = match run(&["gpart", "/sbin/gpart"], &["list"]) {
        Ok(text) => parse_gpart_list(&text),
        Err(_) => Vec::new(),
    };
    if tables.is_empty() {
        if let Ok(text) = run(&["gpart", "/sbin/gpart"], &["show", "-lp"]) {
            tables = parse_gpart_show_lp(&text);
        }
    }
    if ident_disks.is_empty() && tables.is_empty() {
        return Err(Error::Probe(
            "geom disk list and gpart produced no disks".into(),
        ));
    }
    Ok(merge_disks(ident_disks, tables))
}

fn probe_zfs() -> Result<Vec<super::zfs::ZfsUsage>> {
    let text = run(
        &["zpool", "/sbin/zpool", "/usr/local/sbin/zpool"],
        &["status", "-P"],
    )?;
    let leaves = parse_zpool_status_p(&text);
    if leaves.is_empty() && !text.to_lowercase().contains("no pools") {
        // Empty can mean no pools or parse miss; still mark probed.
    }
    Ok(leaves)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_refuses_non_freebsd() {
        if cfg!(not(target_os = "freebsd")) {
            match live_probe() {
                Err(e) => assert!(e.to_string().contains("FreeBSD-only")),
                Ok(_) => panic!("live probe must not succeed off FreeBSD"),
            }
        }
    }
}

fn run(bins: &[&str], args: &[&str]) -> Result<String> {
    let mut last = String::from("not found");
    for bin in bins {
        match Command::new(bin).args(args).output() {
            Ok(out) if out.status.success() => {
                return Ok(String::from_utf8_lossy(&out.stdout).into_owned());
            }
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                last = format!("{bin} {} {}", out.status, stderr.trim().replace('\n', " "));
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                last = format!("{bin} not found");
            }
            Err(e) => {
                last = format!("{bin}: {e}");
            }
        }
    }
    Err(Error::Probe(format!("{} {last}", args.join(" "))))
}
