pub mod engine;
mod exclusive;
mod resample;

use cpal::traits::{DeviceTrait, HostTrait};
use cpal::Device;
use serde::Serialize;

#[derive(Serialize, Clone)]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
}

#[derive(Serialize)]
pub struct Devices {
    pub inputs: Vec<DeviceInfo>,
    pub outputs: Vec<DeviceInfo>,
    pub default_input: Option<String>,
    pub default_output: Option<String>,
}

fn info(d: &Device) -> Option<DeviceInfo> {
    let id = d.id().ok()?.to_string();
    let name = d.description().map(|x| x.name().to_string()).unwrap_or_else(|_| id.clone());
    Some(DeviceInfo { id, name })
}

pub fn list_devices() -> Result<Devices, String> {
    let host = cpal::default_host();
    let collect = |it: Result<Vec<Device>, String>| it.map(|v| v.iter().filter_map(info).collect::<Vec<_>>());
    Ok(Devices {
        inputs: collect(host.input_devices().map(|i| i.collect()).map_err(|e| e.to_string()))?,
        outputs: collect(host.output_devices().map(|i| i.collect()).map_err(|e| e.to_string()))?,
        default_input: host.default_input_device().as_ref().and_then(info).map(|d| d.id),
        default_output: host.default_output_device().as_ref().and_then(info).map(|d| d.id),
    })
}

pub fn default_output_id() -> Option<String> {
    cpal::default_host().default_output_device().as_ref().and_then(info).map(|d| d.id)
}

pub fn find_device(id: Option<&str>, input: bool) -> Result<Device, String> {
    let host = cpal::default_host();
    let dev = match id {
        Some(id) => {
            let parsed = id.parse().map_err(|_| format!("bad device id: {id}"))?;
            host.device_by_id(&parsed)
        }
        None if input => host.default_input_device(),
        None => host.default_output_device(),
    };
    dev.ok_or_else(|| format!("{} device not found", if input { "input" } else { "output" }))
}
