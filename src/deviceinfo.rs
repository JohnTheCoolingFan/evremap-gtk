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
    FileOpenError(PathBuf, std::io::Error),
    #[error("Filesystem error: {0}")]
    IoError(std::io::Error),
    #[error("evdev error: {0}")]
    EvdevError(std::io::Error),
    #[error("Device with name `{0} not found`")]
    DeviceNotFoundByName(String),
    #[error("Device with name `{0}` and phys `{1}` not found")]
    DeviceNotFound(String, String),
}

impl DeviceInfo {
    pub fn with_path(path: PathBuf) -> Result<Self, DeviceInfoError> {
        let f = std::fs::File::open(&path)
            .map_err(|e| DeviceInfoError::FileOpenError(path.clone(), e))?;
        let input = Device::new_from_file(f).map_err(DeviceInfoError::EvdevError)?;

        Ok(Self {
            name: input.name().unwrap_or("").to_string(),
            phys: input.phys().map(|s| s.to_owned()),
            path,
            supports_remap: input.has_event_type(&EventType::EV_KEY),
        })
    }

    pub fn with_name(name: &str, phys: Option<&str>) -> Result<Self, DeviceInfoError> {
        let mut devices = Self::obtain_device_list()?;

        if let Some(query_phys) = phys {
            match devices.iter().position(|item| {
                item.phys
                    .as_ref()
                    .map(|devphys| devphys == query_phys)
                    .unwrap_or(false)
            }) {
                Some(idx) => return Ok(devices.remove(idx)),
                None => {
                    return Err(DeviceInfoError::DeviceNotFound(
                        name.to_owned(),
                        query_phys.to_owned(),
                    ));
                }
            }
        }

        let mut devices_with_name: Vec<_> = devices
            .into_iter()
            .filter(|item| item.name == name)
            .collect();

        if devices_with_name.is_empty() {
            return Err(DeviceInfoError::DeviceNotFoundByName(name.to_owned()));
        }

        if devices_with_name.len() > 1 {
            log::warn!("The following devices match name `{}`:", name);
            for dev in &devices_with_name {
                log::warn!("{:?}", dev);
            }
            log::warn!(
                "evremap will use the first entry. If you want to \
                       use one of the others, add the corresponding phys \
                       value to your configuration, for example, \
                       `phys = \"{}\"` for the second entry in the list.",
                devices_with_name[1].phys.as_ref().map_or("", |v| v)
            );
        }

        Ok(devices_with_name.remove(0))
    }

    pub fn obtain_device_list() -> Result<Vec<DeviceInfo>, DeviceInfoError> {
        let mut devices = vec![];
        for entry in std::fs::read_dir("/dev/input").map_err(DeviceInfoError::IoError)? {
            let entry = entry.map_err(DeviceInfoError::IoError)?;

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
