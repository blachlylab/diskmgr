//! Parser for `sesutil map|show|status --libxo json`.

use std::path::Path;

use serde::Deserialize;

use crate::error::{Error, Result};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SesEnclosure {
    pub enc: String,
    pub name: String,
    pub id: String,
    pub status: Vec<String>,
    pub bays: Vec<SesBay>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SesBay {
    pub element_id: u32,
    pub slot_index: u32,
    pub description: String,
    pub status: String,
    pub kernel_disk: Option<String>,
    pub device_names_raw: Option<String>,
    pub swapped: bool,
    pub locate: bool,
    pub fault: bool,
    pub model: Option<String>,
    pub serial: Option<String>,
    pub size_bytes: Option<u64>,
}

impl SesBay {
    pub fn manufacturer(&self) -> Option<&str> {
        self.model
            .as_deref()
            .and_then(|m| m.split_whitespace().next())
    }
}

#[derive(Deserialize)]
struct SesutilFile<T> {
    sesutil: T,
}

#[derive(Deserialize)]
struct EnclosureList<E> {
    enclosures: Vec<E>,
}

#[derive(Deserialize)]
struct MapEnclosure {
    enc: String,
    name: String,
    id: String,
    #[serde(default)]
    elements: Vec<MapElement>,
}

#[derive(Deserialize)]
struct MapElement {
    id: u32,
    #[serde(rename = "type")]
    elm_type: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    description: String,
    device_names: Option<String>,
    extra_status: Option<ExtraStatus>,
}

#[derive(Deserialize)]
struct ShowEnclosure {
    enc: String,
    id: String,
    #[serde(default)]
    elements: Vec<ShowElement>,
}

#[derive(Deserialize)]
struct ShowElement {
    #[serde(rename = "type")]
    elm_type: String,
    #[serde(default)]
    description: String,
    device_names: Option<String>,
    model: Option<String>,
    serial: Option<String>,
    size: Option<u64>,
    status: Option<String>,
    extra_status: Option<ExtraStatus>,
}

#[derive(Deserialize)]
struct StatusEnclosure {
    enc: String,
    status: EncStatus,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum EncStatus {
    One(String),
    Many(Vec<String>),
}

#[derive(Deserialize, Default)]
struct ExtraStatus {
    swapped: Option<FlexibleBool>,
    locate: Option<FlexibleBool>,
    fault: Option<FlexibleBool>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum FlexibleBool {
    Bool(bool),
    String(String),
}

impl FlexibleBool {
    fn is_true(&self) -> bool {
        match self {
            Self::Bool(b) => *b,
            Self::String(s) => s.eq_ignore_ascii_case("true") || s == "1",
        }
    }
}

pub fn parse_slot_index(description: &str) -> Option<u32> {
    let rest = description.trim().strip_prefix("Slot")?;
    rest.trim().parse().ok()
}

pub fn kernel_disk(device_names: &str) -> Option<String> {
    let mut ada = None;
    let mut da = None;
    for part in device_names.split(',') {
        let p = part.trim();
        if p.is_empty() {
            continue;
        }
        if is_cam_unit(p, "ada") {
            ada = Some(p.to_string());
        } else if is_cam_unit(p, "da") {
            da = Some(p.to_string());
        }
    }
    da.or(ada)
}

fn is_cam_unit(name: &str, prefix: &str) -> bool {
    name.strip_prefix(prefix)
        .is_some_and(|rest| !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit()))
}

fn nonempty(s: Option<&String>) -> Option<String> {
    s.map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

pub fn parse_sesutil_json(map: &str, show: &str, status: &str) -> Result<Vec<SesEnclosure>> {
    parse_sesutil_named(
        map,
        show,
        status,
        Path::new("map"),
        Path::new("show"),
        Path::new("status"),
    )
}

pub fn parse_sesutil_named(
    map: &str,
    show: &str,
    status: &str,
    map_path: &Path,
    show_path: &Path,
    status_path: &Path,
) -> Result<Vec<SesEnclosure>> {
    let map: SesutilFile<EnclosureList<MapEnclosure>> =
        serde_json::from_str(map).map_err(|source| Error::Json {
            path: map_path.to_path_buf(),
            source,
        })?;
    let show: SesutilFile<EnclosureList<ShowEnclosure>> =
        serde_json::from_str(show).map_err(|source| Error::Json {
            path: show_path.to_path_buf(),
            source,
        })?;
    let status: SesutilFile<EnclosureList<StatusEnclosure>> = serde_json::from_str(status)
        .map_err(|source| Error::Json {
            path: status_path.to_path_buf(),
            source,
        })?;

    let mut out = Vec::new();
    for map_enc in map.sesutil.enclosures {
        let show_enc = show
            .sesutil
            .enclosures
            .iter()
            .find(|e| e.id == map_enc.id || e.enc == map_enc.enc);
        let status_enc = status
            .sesutil
            .enclosures
            .iter()
            .find(|e| e.enc == map_enc.enc);

        let mut bays = Vec::new();
        for el in &map_enc.elements {
            if el.elm_type != "Array Device Slot" {
                continue;
            }
            let Some(slot_index) = parse_slot_index(&el.description) else {
                continue;
            };
            let show_el = show_enc.and_then(|se| {
                se.elements.iter().find(|s| {
                    s.elm_type == "device_slot"
                        && parse_slot_index(&s.description) == Some(slot_index)
                })
            });
            let extra = el.extra_status.as_ref();
            let show_extra = show_el.and_then(|s| s.extra_status.as_ref());
            let raw_names = nonempty(el.device_names.as_ref())
                .or_else(|| show_el.and_then(|s| nonempty(s.device_names.as_ref())));
            bays.push(SesBay {
                element_id: el.id,
                slot_index,
                description: el.description.trim().to_string(),
                status: show_el
                    .and_then(|s| nonempty(s.status.as_ref()))
                    .unwrap_or_else(|| el.status.clone()),
                kernel_disk: raw_names.as_deref().and_then(kernel_disk),
                device_names_raw: raw_names,
                swapped: extra
                    .and_then(|e| e.swapped.as_ref())
                    .or_else(|| show_extra.and_then(|e| e.swapped.as_ref()))
                    .is_some_and(FlexibleBool::is_true),
                locate: extra
                    .and_then(|e| e.locate.as_ref())
                    .is_some_and(FlexibleBool::is_true),
                fault: extra
                    .and_then(|e| e.fault.as_ref())
                    .is_some_and(FlexibleBool::is_true),
                model: show_el.and_then(|s| nonempty(s.model.as_ref())),
                serial: show_el.and_then(|s| nonempty(s.serial.as_ref())),
                size_bytes: show_el.and_then(|s| s.size),
            });
        }
        out.push(SesEnclosure {
            enc: map_enc.enc,
            name: map_enc.name,
            id: map_enc.id,
            status: status_enc
                .map(|s| match &s.status {
                    EncStatus::One(v) => vec![v.clone()],
                    EncStatus::Many(v) => v.clone(),
                })
                .unwrap_or_default(),
            bays,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_index_accepts_padded_and_spaced() {
        assert_eq!(parse_slot_index("Slot00"), Some(0));
        assert_eq!(parse_slot_index("Slot00  "), Some(0));
        assert_eq!(parse_slot_index("Slot 00 "), Some(0));
        assert_eq!(parse_slot_index("Slot23"), Some(23));
        assert_eq!(parse_slot_index("ArrayDevicesInSubEnclsr0"), None);
        assert_eq!(parse_slot_index("Drive Slots"), None);
        assert_eq!(parse_slot_index("Drive Connector 00"), None);
    }

    #[test]
    fn kernel_disk_prefers_da_skips_pass() {
        assert_eq!(kernel_disk("da12,pass12").as_deref(), Some("da12"));
        assert_eq!(kernel_disk("pass64,ada0").as_deref(), Some("ada0"));
        assert_eq!(kernel_disk(""), None);
    }
}
