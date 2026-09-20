use std::path::{Path, PathBuf};

use super::geom::{GeomDisk, find_geom_file, load_geom_disks};
use super::sesutil::{SesBay, SesEnclosure, parse_sesutil_named};
use super::zfs::{ZfsUsage, find_zfs_file, gpt_label_from_path, kernel_leaf, load_zpool_status};
use crate::error::{Error, Result};

/// Snapshot of SES + geom + ZFS, from a fixture directory or a live probe.
pub struct FixtureProbe {
    pub source: PathBuf,
    pub enclosures: Vec<SesEnclosure>,
    pub disks: Vec<GeomDisk>,
    pub geom_probed: bool,
    pub zfs: Vec<ZfsUsage>,
    pub zfs_probed: bool,
    pub warnings: Vec<String>,
}

impl FixtureProbe {
    pub fn load(dir: &Path) -> Result<Self> {
        let map_path = dir.join("map.json");
        let show_path = dir.join("show.json");
        let status_path = dir.join("status.json");
        let map = read(&map_path)?;
        let show = read(&show_path)?;
        let status = read(&status_path)?;
        let mut enclosures =
            parse_sesutil_named(&map, &show, &status, &map_path, &show_path, &status_path)?;
        let (disks, geom_probed) = match find_geom_file(dir) {
            Some(path) => (load_geom_disks(&path)?, true),
            None => (Vec::new(), false),
        };
        if geom_probed {
            attach_geom(&mut enclosures, &disks);
        }
        let (zfs, zfs_probed) = match find_zfs_file(dir) {
            Some(path) => (load_zpool_status(&path)?, true),
            None => (Vec::new(), false),
        };
        if zfs_probed {
            attach_zfs(&mut enclosures, &zfs);
        }
        Ok(Self {
            source: dir.to_path_buf(),
            enclosures,
            disks,
            geom_probed,
            zfs,
            zfs_probed,
            warnings: Vec::new(),
        })
    }

    pub fn live() -> Result<Self> {
        super::freebsd::live_probe()
    }

    pub fn is_live(&self) -> bool {
        self.source.as_os_str() == "live"
    }

    pub fn by_id(&self, id: &str) -> Option<&SesEnclosure> {
        self.enclosures.iter().find(|e| e.id == id)
    }

    pub fn by_unit(&self, unit: &str) -> Option<&SesEnclosure> {
        self.enclosures.iter().find(|e| e.enc == unit)
    }
}

pub(crate) fn attach_geom(enclosures: &mut [SesEnclosure], disks: &[GeomDisk]) {
    use std::collections::HashMap;
    let by_name: HashMap<&str, &GeomDisk> = disks.iter().map(|d| (d.name.as_str(), d)).collect();
    let by_ident: HashMap<&str, &GeomDisk> = disks
        .iter()
        .filter_map(|d| d.ident.as_deref().map(|id| (id, d)))
        .collect();
    for enc in enclosures {
        for bay in &mut enc.bays {
            let disk = bay
                .serial
                .as_deref()
                .and_then(|s| by_ident.get(s).copied())
                .or_else(|| {
                    bay.kernel_disk
                        .as_deref()
                        .and_then(|n| by_name.get(n).copied())
                });
            let Some(disk) = disk else {
                continue;
            };
            bay.geom_known = true;
            bay.wwn = disk.lunid.clone();
            bay.gpt_scheme = disk.scheme.clone();
            bay.gpt_partitions = disk.partitions.clone();
        }
    }
}

pub(crate) fn attach_zfs(enclosures: &mut [SesEnclosure], usages: &[ZfsUsage]) {
    for enc in enclosures {
        for bay in &mut enc.bays {
            bay.zfs = usages
                .iter()
                .filter(|u| zfs_matches_bay(bay, u))
                .cloned()
                .collect();
        }
    }
}

fn zfs_matches_bay(bay: &SesBay, usage: &ZfsUsage) -> bool {
    if let Some(label) = gpt_label_from_path(&usage.path) {
        return bay
            .gpt_partitions
            .iter()
            .any(|p| p.label.as_deref() == Some(label));
    }
    if let Some(disk) = kernel_leaf(&usage.path) {
        return bay.kernel_disk.as_deref() == Some(disk);
    }
    false
}

fn read(path: &Path) -> Result<String> {
    std::fs::read_to_string(path).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::inventory::Inventory;
    use std::path::Path;

    fn load_lab() -> FixtureProbe {
        FixtureProbe::load(Path::new("examples/sesutil")).unwrap()
    }

    #[test]
    fn lab_capture_enclosure_counts() {
        let probe = load_lab();
        assert_eq!(probe.enclosures.len(), 5);

        let ses0 = probe.by_unit("ses0").unwrap();
        assert_eq!(ses0.bays.len(), 24);
        assert_eq!(ses0.name, "SMC SC846P 0c1f");
        assert!(ses0.bays.iter().all(|b| b.kernel_disk.is_some()));
        let slot15 = ses0.bays.iter().find(|b| b.slot_index == 15).unwrap();
        assert_eq!(slot15.kernel_disk.as_deref(), Some("da15"));
        assert!(slot15.swapped);
        assert!(slot15.model.as_deref().unwrap().contains("SEAGATE"));

        let ses1 = probe.by_unit("ses1").unwrap();
        assert_eq!(ses1.bays.len(), 12);
        let empty = ses1.bays.iter().find(|b| b.slot_index == 6).unwrap();
        assert_eq!(empty.status, "Not Installed");
        assert!(empty.kernel_disk.is_none());
        assert!(empty.swapped);
        let slot4 = ses1.bays.iter().find(|b| b.slot_index == 4).unwrap();
        assert_eq!(slot4.kernel_disk.as_deref(), Some("da57"));
        let slot8 = ses1.bays.iter().find(|b| b.slot_index == 8).unwrap();
        assert_eq!(slot8.kernel_disk.as_deref(), Some("da30"));

        let ses2 = probe.by_unit("ses2").unwrap();
        assert_eq!(ses2.bays.len(), 24);
        assert_eq!(
            ses2.bays
                .iter()
                .find(|b| b.slot_index == 0)
                .unwrap()
                .kernel_disk
                .as_deref(),
            Some("da50")
        );
        assert!(ses2.status.iter().any(|s| s == "CRITICAL"));

        let ses4 = probe.by_unit("ses4").unwrap();
        assert_eq!(
            ses4.bays
                .iter()
                .find(|b| b.slot_index == 0)
                .unwrap()
                .kernel_disk
                .as_deref(),
            Some("ada0")
        );

        // Headers and expanders must not become bays.
        assert!(ses0.bays.iter().all(|b| b.element_id >= 1));
    }

    #[test]
    fn inventory_joins_config_on_enclosure_id() {
        let cfg = Config::load_from_path(Path::new("examples/diskmgr.toml")).unwrap();
        let probe = load_lab();
        let inv = Inventory::from_fixture(&cfg, &probe).unwrap();
        assert_eq!(inv.enclosures.len(), 6);
        assert!(inv.enclosures.iter().take(5).all(|e| !e.unmatched));
        assert!(inv.enclosures[5].unmatched);

        let front = &inv.enclosures[0];
        assert_eq!(front.occupied_count(), 24);
        let silk0 = front.slots.iter().find(|s| s.silk == 0).unwrap();
        assert_eq!(silk0.cell.col, 0);
        assert_eq!(silk0.cell.row, front.config.nrows - 1);
        assert_eq!(
            silk0.bay.as_ref().unwrap().kernel_disk.as_deref(),
            Some("da0")
        );

        let rear = &inv.enclosures[1];
        assert_eq!(rear.occupied_count(), 11);
        let silk0 = rear.slots.iter().find(|s| s.silk == 0).unwrap();
        assert_eq!(silk0.cell, crate::layout::Cell { col: 0, row: 2 });
        let empty = rear.slots.iter().find(|s| s.silk == 6).unwrap();
        assert!(empty.bay.as_ref().unwrap().kernel_disk.is_none());
        assert!(inv.geom_probed);

        let da0 = front.slot_by_silk(0).unwrap();
        assert_eq!(da0.gpt_summary().as_deref(), Some("no"));
        assert!(da0.wwn().is_some());
        assert_eq!(da0.zfs_pool(), Some("hog"));

        // ses1 Slot09 is da33, GPT label with scrambled serial; data vdev not a zpool spare
        let spare = rear.slot_by_silk(9).unwrap();
        assert_eq!(
            spare.bay.as_ref().unwrap().kernel_disk.as_deref(),
            Some("da33")
        );
        assert_eq!(
            spare.gpt_summary().as_deref(),
            Some("gpt/hog-FGH6UV3S-spare")
        );
        assert!(inv.zfs_probed);
        assert_eq!(spare.zfs_pool(), Some("hog"));
        assert_eq!(spare.zfs_vdev(), Some("raidz2-2"));
        assert_eq!(spare.zfs_role(), Some("data"));
        let da9 = front.slot_by_silk(9).unwrap();
        assert_eq!(da9.zfs().first().unwrap().read_err, 246);
        let da15 = front.slot_by_silk(15).unwrap();
        assert!(da15.zfs().is_empty());
    }

    #[test]
    fn fixture_serials_are_scrambled() {
        let probe = load_lab();
        let serials: Vec<_> = probe
            .enclosures
            .iter()
            .flat_map(|e| e.bays.iter())
            .filter_map(|b| b.serial.as_deref())
            .collect();
        assert_eq!(serials.len(), 61);
        for original in [
            "VLKPN58V",
            "WWZCQHXY0000R650F2YG",
            "JEG9EWRN",
            "2BJ5BVSD",
            "ZZ00M8V9",
            "171316A2B85A",
        ] {
            assert!(
                !serials.contains(&original),
                "lab serial {original} must not appear in the fixture"
            );
        }
    }
}
