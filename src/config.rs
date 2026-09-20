use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::{Error, Result};
use crate::layout::{Fill, Geometry, Origin};

#[derive(Clone, Debug, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub host: Host,
    #[serde(default)]
    pub enclosure: Vec<EnclosureConfig>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Host {
    pub name: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Face {
    Front,
    Back,
    #[default]
    Unknown,
}

#[derive(Clone, Debug, Deserialize)]
pub struct EnclosureConfig {
    pub name: String,
    pub chassis: Option<String>,
    #[serde(default)]
    pub face: Face,
    pub ses_device: Option<String>,
    pub enclosure_id: Option<String>,
    pub ncols: u32,
    pub nrows: u32,
    pub origin: Origin,
    pub fill: Fill,
    pub slot_index_base: u32,
}

impl EnclosureConfig {
    pub fn geometry(&self) -> Geometry {
        Geometry {
            ncols: self.ncols,
            nrows: self.nrows,
            origin: self.origin,
            fill: self.fill,
            slot_index_base: self.slot_index_base,
        }
    }

    /// Normalized SES unit name, e.g. `ses0`.
    pub fn ses_unit(&self) -> Option<String> {
        self.ses_device.as_deref().map(normalize_ses_device)
    }
}

pub fn normalize_ses_device(raw: &str) -> String {
    raw.trim()
        .strip_prefix("/dev/")
        .unwrap_or(raw.trim())
        .to_string()
}

impl Config {
    pub fn load_from_path(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path).map_err(|source| Error::Io {
            path: path.to_path_buf(),
            source,
        })?;
        let cfg: Self = toml::from_str(&text).map_err(|source| Error::Toml {
            path: path.to_path_buf(),
            source,
        })?;
        cfg.validate()?;
        Ok(cfg)
    }

    pub fn find_and_load(explicit: Option<&Path>) -> Result<(PathBuf, Self)> {
        if let Some(path) = explicit {
            return Ok((path.to_path_buf(), Self::load_from_path(path)?));
        }
        let candidates = default_config_paths();
        for path in &candidates {
            if path.is_file() {
                return Ok((path.clone(), Self::load_from_path(path)?));
            }
        }
        Err(Error::Config(format!(
            "no config file found (tried {})",
            candidates
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        )))
    }

    fn validate(&self) -> Result<()> {
        if self.enclosure.is_empty() {
            return Err(Error::Config("config has no [[enclosure]] tables".into()));
        }
        let mut names = HashSet::new();
        let mut chassis_faces = HashSet::new();
        for enc in &self.enclosure {
            if enc.name.is_empty() {
                return Err(Error::Config("enclosure name must not be empty".into()));
            }
            if !names.insert(enc.name.clone()) {
                return Err(Error::Config(format!(
                    "duplicate enclosure name {:?}",
                    enc.name
                )));
            }
            if enc.ncols < 1 || enc.nrows < 1 {
                return Err(Error::Config(format!(
                    "enclosure {:?}: ncols and nrows must be >= 1",
                    enc.name
                )));
            }
            if enc.slot_index_base > 1 {
                return Err(Error::Config(format!(
                    "enclosure {:?}: slot_index_base must be 0 or 1",
                    enc.name
                )));
            }
            if enc.ses_device.is_none() && enc.enclosure_id.is_none() {
                return Err(Error::Config(format!(
                    "enclosure {:?}: set ses_device and/or enclosure_id",
                    enc.name
                )));
            }
            if let Some(chassis) = &enc.chassis {
                let key = (chassis.clone(), enc.face);
                if enc.face != Face::Unknown && !chassis_faces.insert(key) {
                    return Err(Error::Config(format!(
                        "duplicate chassis {:?} face {:?}",
                        chassis, enc.face
                    )));
                }
            }
            enc.geometry().slot_to_cell(enc.slot_index_base)?;
        }
        Ok(())
    }
}

fn default_config_paths() -> Vec<PathBuf> {
    let mut paths = vec![PathBuf::from("diskmgr.toml")];
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        paths.push(PathBuf::from(xdg).join("diskmgr/config.toml"));
    } else if let Some(home) = std::env::var_os("HOME") {
        paths.push(PathBuf::from(home).join(".config/diskmgr/config.toml"));
    }
    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn example_config_loads() {
        let cfg = Config::load_from_path(Path::new("examples/diskmgr.toml")).unwrap();
        assert_eq!(cfg.host.name.as_deref(), Some("storage1"));
        assert_eq!(cfg.enclosure.len(), 5);
        let rear = &cfg.enclosure[1];
        assert_eq!(rear.chassis.as_deref(), Some("sc846p-0c1f"));
        assert_eq!(rear.face, Face::Back);
        assert_eq!(rear.ncols, 4);
        assert_eq!(rear.nrows, 3);
        assert_eq!(rear.ses_unit().as_deref(), Some("ses1"));
    }

    #[test]
    fn rejects_missing_identity() {
        let toml = r#"
            [[enclosure]]
            name = "x"
            ncols = 1
            nrows = 1
            origin = "TL"
            fill = "row"
            slot_index_base = 0
        "#;
        let cfg: Config = toml::from_str(toml).unwrap();
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn normalize_dev_prefix() {
        assert_eq!(normalize_ses_device("/dev/ses2"), "ses2");
        assert_eq!(normalize_ses_device("ses2"), "ses2");
    }
}
