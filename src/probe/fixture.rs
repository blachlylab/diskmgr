use std::path::{Path, PathBuf};

use super::sesutil::{SesEnclosure, parse_sesutil_named};
use crate::error::{Error, Result};

/// Replay recorded `sesutil --libxo json` output from a directory.
pub struct FixtureProbe {
    pub source: PathBuf,
    pub enclosures: Vec<SesEnclosure>,
}

impl FixtureProbe {
    pub fn load(dir: &Path) -> Result<Self> {
        let map_path = dir.join("map.json");
        let show_path = dir.join("show.json");
        let status_path = dir.join("status.json");
        let map = read(&map_path)?;
        let show = read(&show_path)?;
        let status = read(&status_path)?;
        let enclosures =
            parse_sesutil_named(&map, &show, &status, &map_path, &show_path, &status_path)?;
        Ok(Self {
            source: dir.to_path_buf(),
            enclosures,
        })
    }

    pub fn by_id(&self, id: &str) -> Option<&SesEnclosure> {
        self.enclosures.iter().find(|e| e.id == id)
    }

    pub fn by_unit(&self, unit: &str) -> Option<&SesEnclosure> {
        self.enclosures.iter().find(|e| e.enc == unit)
    }
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
        assert_eq!(inv.enclosures.len(), 5);
        assert!(inv.enclosures.iter().all(|e| !e.unmatched));

        let front = &inv.enclosures[0];
        assert_eq!(front.occupied(), 24);
        let silk0 = front.slots.iter().find(|s| s.silk == 0).unwrap();
        assert_eq!(silk0.cell.col, 0);
        assert_eq!(silk0.cell.row, 3);
        assert_eq!(
            silk0.bay.as_ref().unwrap().kernel_disk.as_deref(),
            Some("da0")
        );

        let rear = &inv.enclosures[1];
        assert_eq!(rear.occupied(), 11);
        let silk0 = rear.slots.iter().find(|s| s.silk == 0).unwrap();
        assert_eq!(silk0.cell, crate::layout::Cell { col: 0, row: 2 });
        let empty = rear.slots.iter().find(|s| s.silk == 6).unwrap();
        assert!(empty.bay.as_ref().unwrap().kernel_disk.is_none());
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
