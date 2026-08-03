//! Audio device enumeration + resolution for meeting recording.
//!
//! **Windows only** for this phase. System-loopback capture is done by
//! opening an *input* stream on an *output*-role device — this only works
//! thanks to the patched `cpal` fork pinned in `Cargo.toml`'s
//! `[patch.crates-io]` (stock cpal does not support WASAPI loopback). On
//! non-Windows platforms every function here returns an explicit "not yet
//! implemented" error — no pretend support, per project policy (see
//! CLAUDE.md: "surface the limitation, not ship an init script"-style
//! guidance generalized to platform gaps).
//!
//! [`configure_windows_audio`] / [`get_windows_device`] port Meetily's
//! `audio/devices/platform/windows.rs` logic faithfully (enumerate via the
//! WASAPI host, fall back to the default host, fall back to the system
//! default device).

use anyhow::{anyhow, Result};
use serde::Serialize;

#[cfg(target_os = "windows")]
use cpal::traits::{DeviceTrait, HostTrait};

const LOG_PREFIX: &str = "[meeting-recorder:devices]";

/// Whether a logical [`AudioDevice`] is a microphone (`Input`) or a
/// system-loopback candidate (`Output`). Loopback candidates are
/// enumerated from the *output* device list — see the module doc.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceType {
    Input,
    Output,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AudioDevice {
    pub name: String,
    pub device_type: DeviceType,
}

/// Enumerate capturable audio devices for the current platform.
///
/// Mic devices come back as `DeviceType::Input`; system-loopback
/// candidates come back as `DeviceType::Output` (see module doc for why).
pub fn configure_platform_audio(host: &cpal::Host) -> Result<Vec<AudioDevice>> {
    #[cfg(target_os = "windows")]
    {
        configure_windows_audio(host)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = host;
        Err(anyhow!(
            "Audio device capture not yet implemented for this platform"
        ))
    }
}

/// Resolve a cpal `Device` + negotiated `SupportedStreamConfig` for the
/// given logical [`AudioDevice`].
pub fn get_device(
    audio_device: &AudioDevice,
) -> Result<(cpal::Device, cpal::SupportedStreamConfig)> {
    #[cfg(target_os = "windows")]
    {
        get_windows_device(audio_device)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = audio_device;
        Err(anyhow!(
            "Audio device capture not yet implemented for this platform"
        ))
    }
}

#[cfg(target_os = "windows")]
fn configure_windows_audio(default_host: &cpal::Host) -> Result<Vec<AudioDevice>> {
    let mut devices = match cpal::host_from_id(cpal::HostId::Wasapi) {
        Ok(wasapi_host) => {
            let found = enumerate_host(&wasapi_host);
            if found.is_empty() {
                log::warn!(
                    "{LOG_PREFIX} WASAPI enumeration returned no devices, falling back to default host"
                );
                enumerate_host(default_host)
            } else {
                found
            }
        }
        Err(err) => {
            log::warn!(
                "{LOG_PREFIX} WASAPI host unavailable ({err}), falling back to default host"
            );
            enumerate_host(default_host)
        }
    };

    if devices.is_empty() {
        log::warn!(
            "{LOG_PREFIX} no devices found via host enumeration, falling back to system defaults"
        );
        if let Some(d) = default_host.default_input_device() {
            if let Ok(name) = d.name() {
                devices.push(AudioDevice {
                    name,
                    device_type: DeviceType::Input,
                });
            }
        }
        if let Some(d) = default_host.default_output_device() {
            if let Ok(name) = d.name() {
                devices.push(AudioDevice {
                    name,
                    device_type: DeviceType::Output,
                });
            }
        }
    }

    if devices.is_empty() {
        return Err(anyhow!(
            "no audio devices found via WASAPI, default host, or system defaults"
        ));
    }

    log::debug!("{LOG_PREFIX} enumerated {} device(s)", devices.len());
    Ok(devices)
}

#[cfg(target_os = "windows")]
fn enumerate_host(host: &cpal::Host) -> Vec<AudioDevice> {
    let mut out = Vec::new();
    match host.output_devices() {
        Ok(iter) => {
            for d in iter {
                if let Ok(name) = d.name() {
                    out.push(AudioDevice {
                        name,
                        device_type: DeviceType::Output,
                    });
                }
            }
        }
        Err(err) => log::debug!("{LOG_PREFIX} output_devices() enumeration failed: {err}"),
    }
    match host.input_devices() {
        Ok(iter) => {
            for d in iter {
                if let Ok(name) = d.name() {
                    out.push(AudioDevice {
                        name,
                        device_type: DeviceType::Input,
                    });
                }
            }
        }
        Err(err) => log::debug!("{LOG_PREFIX} input_devices() enumeration failed: {err}"),
    }
    out
}

#[cfg(target_os = "windows")]
fn get_windows_device(
    audio_device: &AudioDevice,
) -> Result<(cpal::Device, cpal::SupportedStreamConfig)> {
    let host = match cpal::host_from_id(cpal::HostId::Wasapi) {
        Ok(h) => h,
        Err(err) => {
            log::warn!(
                "{LOG_PREFIX} WASAPI host unavailable for device lookup ({err}), using default host"
            );
            cpal::default_host()
        }
    };

    let found = match audio_device.device_type {
        DeviceType::Input => find_device_by_name(host.input_devices(), &audio_device.name),
        DeviceType::Output => find_device_by_name(host.output_devices(), &audio_device.name),
    };

    let device = match found {
        Some(d) => d,
        None => {
            log::warn!(
                "{LOG_PREFIX} device '{}' not found via WASAPI host, falling back to system default",
                audio_device.name
            );
            let default_host = cpal::default_host();
            match audio_device.device_type {
                DeviceType::Input => default_host
                    .default_input_device()
                    .ok_or_else(|| anyhow!("no default input device available"))?,
                DeviceType::Output => default_host
                    .default_output_device()
                    .ok_or_else(|| anyhow!("no default output device available"))?,
            }
        }
    };

    // NOTE: for the loopback (Output) case we still deliberately probe the
    // *output* config space even though we go on to open an *input*
    // stream on this device — WASAPI loopback mirrors the device's render
    // (output) mix format, so the output config space is what actually
    // describes the data we'll receive. This mirrors Meetily's original
    // (slightly surprising, but correct) windows.rs logic exactly.
    let config = match audio_device.device_type {
        DeviceType::Input => resolve_input_config(&device)?,
        DeviceType::Output => resolve_output_config(&device)?,
    };

    Ok((device, config))
}

#[cfg(target_os = "windows")]
fn find_device_by_name<I>(
    devices: Result<I, cpal::DevicesError>,
    name: &str,
) -> Option<cpal::Device>
where
    I: Iterator<Item = cpal::Device>,
{
    let mut contains_match: Option<cpal::Device> = None;
    let devices = devices.ok()?;
    for d in devices {
        match d.name() {
            Ok(n) if n == name => return Some(d),
            Ok(n) if contains_match.is_none() && (n.contains(name) || name.contains(&n)) => {
                contains_match = Some(d);
            }
            _ => {}
        }
    }
    contains_match
}

#[cfg(target_os = "windows")]
fn resolve_input_config(device: &cpal::Device) -> Result<cpal::SupportedStreamConfig> {
    if let Ok(cfg) = device.default_input_config() {
        return Ok(cfg);
    }
    let mut configs: Vec<_> = device
        .supported_input_configs()
        .map_err(|e| anyhow!("supported_input_configs failed: {e}"))?
        .collect();
    pick_preferred_config(&mut configs)
        .ok_or_else(|| anyhow!("no usable input stream config found"))
}

#[cfg(target_os = "windows")]
fn resolve_output_config(device: &cpal::Device) -> Result<cpal::SupportedStreamConfig> {
    if let Ok(cfg) = device.default_output_config() {
        return Ok(cfg);
    }
    let mut configs: Vec<_> = device
        .supported_output_configs()
        .map_err(|e| anyhow!("supported_output_configs failed: {e}"))?
        .collect();
    pick_preferred_config(&mut configs)
        .ok_or_else(|| anyhow!("no usable output stream config found"))
}

/// Prefer F32 stereo, then F32 mono, then whatever's first.
#[cfg(target_os = "windows")]
fn pick_preferred_config(
    configs: &mut [cpal::SupportedStreamConfigRange],
) -> Option<cpal::SupportedStreamConfig> {
    if configs.is_empty() {
        return None;
    }
    let idx = configs
        .iter()
        .position(|c| c.sample_format() == cpal::SampleFormat::F32 && c.channels() == 2)
        .or_else(|| {
            configs
                .iter()
                .position(|c| c.sample_format() == cpal::SampleFormat::F32 && c.channels() == 1)
        })
        .unwrap_or(0);
    Some(configs[idx].clone().with_max_sample_rate())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_type_serializes_snake_case() {
        assert_eq!(
            serde_json::to_string(&DeviceType::Input).unwrap(),
            "\"input\""
        );
        assert_eq!(
            serde_json::to_string(&DeviceType::Output).unwrap(),
            "\"output\""
        );
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn configure_platform_audio_stub_errors_on_non_windows() {
        let host = cpal::default_host();
        let err = configure_platform_audio(&host).unwrap_err();
        assert!(err.to_string().contains("not yet implemented"));
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn get_device_stub_errors_on_non_windows() {
        let device = AudioDevice {
            name: "anything".to_string(),
            device_type: DeviceType::Input,
        };
        let err = get_device(&device).unwrap_err();
        assert!(err.to_string().contains("not yet implemented"));
    }
}
