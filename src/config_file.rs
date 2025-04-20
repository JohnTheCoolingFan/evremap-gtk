use std::path::Path;

// The contents of this file are loosely based on [`evremap`](https://github.com/wez/evremap/blob/master/src/mapping.rs#L116)
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::evdev_utils::{KeyCode, list_keycodes};

#[derive(Debug, Error)]
pub enum ConfigFileError {
    #[error("IO error")]
    Io(#[from] std::io::Error),
    #[error("Parsing error")]
    TomlDeserialize(#[from] toml::de::Error),
    #[error("Serialization error")]
    TomlSerialize(#[from] toml::ser::Error),
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ConfigFile {
    #[serde(default)]
    pub device_name: Option<String>,
    #[serde(default)]
    pub phys: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dual_role: Vec<DualRoleConfig>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub remap: Vec<RemapConfig>,
}

impl ConfigFile {
    pub fn read_from<P: AsRef<Path>>(path: P) -> Result<Self, ConfigFileError> {
        let contents = std::fs::read_to_string(path).map_err(ConfigFileError::Io)?;
        toml::from_str(&contents).map_err(ConfigFileError::TomlDeserialize)
    }

    pub fn save_to<P: AsRef<Path>>(&self, path: P) -> Result<(), ConfigFileError> {
        let contents = toml::to_string_pretty(self).map_err(ConfigFileError::TomlSerialize)?;
        std::fs::write(path, contents).map_err(ConfigFileError::Io)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DualRoleConfig {
    pub input: KeyCode,
    pub hold: Vec<KeyCode>,
    pub tap: Vec<KeyCode>,
}

impl Default for DualRoleConfig {
    fn default() -> Self {
        Self {
            input: list_keycodes()[0],
            hold: vec![],
            tap: vec![],
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct RemapConfig {
    pub input: Vec<KeyCode>,
    pub output: Vec<KeyCode>,
}
