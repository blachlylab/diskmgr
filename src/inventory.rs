use crate::config::{Config, EnclosureConfig};
use crate::error::Result;
use crate::layout::Cell;
use crate::probe::{FixtureProbe, SesBay, SesEnclosure};

/// Config geometry joined with live/fixture SES occupancy.
#[derive(Clone, Debug)]
pub struct Inventory {
    pub host_name: Option<String>,
    pub enclosures: Vec<MappedEnclosure>,
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
        })
    }
}

impl MappedEnclosure {
    pub fn occupied(&self) -> usize {
        self.slots
            .iter()
            .filter(|s| s.bay.as_ref().is_some_and(|b| b.kernel_disk.is_some()))
            .count()
    }
}
