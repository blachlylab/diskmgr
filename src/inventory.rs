use crate::config::{Config, EnclosureConfig};
use crate::error::Result;
use crate::layout::Cell;
use crate::probe::{FixtureProbe, GptPartition, SesBay, SesEnclosure};

/// Config geometry joined with live/fixture SES occupancy.
#[derive(Clone, Debug)]
pub struct Inventory {
    pub host_name: Option<String>,
    pub enclosures: Vec<MappedEnclosure>,
    pub geom_probed: bool,
}

#[derive(Clone, Debug)]
pub struct MappedEnclosure {
    pub config: EnclosureConfig,
    pub ses: Option<SesEnclosure>,
    pub unmatched: bool,
    pub slots: Vec<MappedSlot>,
}

#[derive(Clone, Debug)]
pub struct MappedSlot {
    pub silk: u32,
    pub cell: Cell,
    pub bay: Option<SesBay>,
}

impl Inventory {
    pub fn from_fixture(config: &Config, probe: &FixtureProbe) -> Result<Self> {
        let mut enclosures = Vec::new();
        for enc_cfg in &config.enclosure {
            let ses = enc_cfg
                .enclosure_id
                .as_deref()
                .and_then(|id| probe.by_id(id))
                .or_else(|| enc_cfg.ses_unit().as_deref().and_then(|u| probe.by_unit(u)))
                .cloned();
            let unmatched = ses.is_none();
            let geom = enc_cfg.geometry();
            let mut slots = Vec::new();
            for silk in geom.slots() {
                let cell = geom.slot_to_cell(silk)?;
                let desc_index = silk.saturating_sub(geom.slot_index_base);
                let bay = ses
                    .as_ref()
                    .and_then(|s| s.bays.iter().find(|b| b.slot_index == desc_index).cloned());
                slots.push(MappedSlot { silk, cell, bay });
            }
            enclosures.push(MappedEnclosure {
                config: enc_cfg.clone(),
                ses,
                unmatched,
                slots,
            });
        }
        Ok(Self {
            host_name: config.host.name.clone(),
            enclosures,
            geom_probed: probe.geom_probed,
        })
    }
}

impl MappedEnclosure {
    pub fn occupied_count(&self) -> usize {
        self.slots.iter().filter(|s| s.occupied()).count()
    }

    pub fn slot_by_silk(&self, silk: u32) -> Option<&MappedSlot> {
        self.slots.iter().find(|s| s.silk == silk)
    }

    pub fn slot_at_cell(&self, col: u32, row: u32) -> Option<&MappedSlot> {
        self.slots
            .iter()
            .find(|s| s.cell.col == col && s.cell.row == row)
    }
}

impl MappedSlot {
    pub fn occupied(&self) -> bool {
        self.bay
            .as_ref()
            .is_some_and(|b| b.kernel_disk.is_some() || b.serial.is_some() || b.model.is_some())
    }

    pub fn locate(&self) -> bool {
        self.bay.as_ref().is_some_and(|b| b.locate)
    }

    pub fn fault(&self) -> bool {
        self.bay.as_ref().is_some_and(|b| b.fault)
    }

    pub fn wwn(&self) -> Option<&str> {
        self.bay.as_ref().and_then(|b| b.wwn.as_deref())
    }

    pub fn gpt_partitions(&self) -> &[GptPartition] {
        self.bay
            .as_ref()
            .map(|b| b.gpt_partitions.as_slice())
            .unwrap_or(&[])
    }

    pub fn gpt_summary(&self) -> Option<String> {
        let bay = self.bay.as_ref()?;
        if !bay.geom_known {
            return None;
        }
        let labels: Vec<String> = bay
            .gpt_partitions
            .iter()
            .map(|p| p.display_name())
            .collect();
        if !labels.is_empty() {
            return Some(labels.join(", "));
        }
        if bay
            .gpt_scheme
            .as_deref()
            .is_some_and(|s| s.eq_ignore_ascii_case("GPT"))
        {
            Some("GPT (unlabeled)".into())
        } else {
            Some("no".into())
        }
    }
}
