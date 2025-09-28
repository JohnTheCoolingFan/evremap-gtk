// This file is loosely copied from [`evremap`](https://github.com/wez/evremap/blob/master/src/deviceinfo.rs)
use std::{
    cmp::Ordering,
    path::{Path, PathBuf},
};

use evdev_rs::{Device, DeviceWrapper, enums::EventType};
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub name: String,
    pub phys: Option<String>,
    pub path: PathBuf,
    pub supports_remap: bool,
}

#[derive(Debug, Error)]
pub enum DeviceInfoError {
    #[error("Error opening file at {0:?}: {1}")]
    FileOpen(PathBuf, std::io::Error),
    #[error("Filesystem error: {0}")]
    Io(std::io::Error),
    #[error("evdev error: {0}")]
    Evdev(std::io::Error),
}

impl DeviceInfo {
    pub fn with_path(path: PathBuf) -> Result<Self, DeviceInfoError> {
        let f =
            std::fs::File::open(&path).map_err(|e| DeviceInfoError::FileOpen(path.clone(), e))?;
        let input = Device::new_from_file(f).map_err(DeviceInfoError::Evdev)?;

        Ok(Self {
            name: input.name().unwrap_or("").to_string(),
            phys: input.phys().map(|s| s.to_owned()),
            path,
            supports_remap: input.has_event_type(&EventType::EV_KEY),
        })
    }

    pub fn obtain_device_list() -> Result<Vec<DeviceInfo>, DeviceInfoError> {
        let mut devices = vec![];
        for entry in std::fs::read_dir("/dev/input").map_err(DeviceInfoError::Io)? {
            let entry = entry.map_err(DeviceInfoError::Io)?;

            if !entry
                .file_name()
                .to_str()
                .unwrap_or("")
                .starts_with("event")
            {
                continue;
            }
            let path = entry.path();
            if path.is_dir() {
                continue;
            }

            match DeviceInfo::with_path(path) {
                Ok(item) => devices.push(item),
                Err(err) => log::error!("{:#}", err),
            }
        }

        // Order by name, but when multiple devices have the same name,
        // order by the event device unit number
        devices.sort_by(|a, b| match a.name.cmp(&b.name) {
            Ordering::Equal => {
                event_number_from_path(&a.path).cmp(&event_number_from_path(&b.path))
            }
            different => different,
        });
        Ok(devices)
    }
}

fn event_number_from_path(path: &Path) -> u32 {
    match path.to_str() {
        Some(s) => match s.rfind("event") {
            Some(idx) => s[idx + 5..].parse().unwrap_or(0),
            None => 0,
        },
        None => 0,
    }
}
