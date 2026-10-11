//! `GET /v1/me/player/devices` and the body of `PUT /v1/me/player`.

use serde::Deserialize;

use super::error::WebError;
use crate::music::devices::{sort, web_kind, Device};
use crate::music::state::DEVICE_NAME;

#[derive(Deserialize)]
struct Raw {
    #[serde(default)]
    devices: Vec<Option<RawDevice>>,
}

#[derive(Deserialize)]
struct RawDevice {
    id: Option<String>,
    name: Option<String>,
    #[serde(rename = "type")]
    kind: Option<String>,
    #[serde(default)]
    is_active: bool,
    volume_percent: Option<u32>,
}

/// The devices with an id. This one is the entry with `own_id`, else the
/// first computer named "Oculus", and carries `own_id` either way.
pub(super) fn parse(body: &str, own_id: &str) -> Result<Vec<Device>, WebError> {
    let raw: Raw = serde_json::from_str(body).map_err(|e| WebError::Unreadable(e.to_string()))?;
    let mut devices: Vec<Device> = raw
        .devices
        .into_iter()
        .flatten()
        .filter_map(|d| {
            let id = d.id.filter(|id| !id.is_empty())?;
            Some(Device {
                name: d
                    .name
                    .filter(|n| !n.is_empty())
                    .unwrap_or_else(|| id.clone()),
                id,
                kind: web_kind(d.kind.as_deref().unwrap_or("")),
                active: d.is_active,
                volume: d.volume_percent.map(|v| v.min(100) as u8),
                is_this: false,
            })
        })
        .collect();
    let this = devices.iter().position(|d| d.id == own_id).or_else(|| {
        devices
            .iter()
            .position(|d| d.name == DEVICE_NAME && d.kind == "computer")
    });
    if let Some(i) = this {
        devices[i].is_this = true;
        devices[i].id = own_id.to_string();
    }
    sort(&mut devices);
    Ok(devices)
}

pub(super) fn transfer_body(device_id: &str) -> String {
    serde_json::json!({ "device_ids": [device_id], "play": true }).to_string()
}
