use crate::utils::{collect_devices_common, interactive_selection_common, sanitize_name};
use crate::{DEFAULT_SCAN_DURATION, POLL_INTERVAL};
use anyhow::{anyhow, Context, Result};
use btleplug::api::Central as _;
use btleplug::api::Peripheral as _;
use btleplug::{
    api::{Central, ScanFilter},
    platform::{Adapter, Peripheral},
};
use std::time::Duration;
use tokio::time::{sleep, Instant};

pub async fn scan_for_device_by_name(
    adapter: &Adapter,
    target: &str,
    duration: Duration,
    interactive: bool,
) -> Result<Vec<Peripheral>> {
    let sanitized_target = sanitize_name(target);
    let start = Instant::now();
    let mut seen = std::collections::HashSet::new();
    let mut matches = Vec::new();
    while Instant::now().duration_since(start) < duration {
        let peripherals = adapter
            .peripherals()
            .await
            .with_context(|| "Failed to retrieve peripherals")?;
        for peripheral in peripherals {
            if let Some(props) = peripheral
                .properties()
                .await
                .with_context(|| "Failed to get peripheral properties")?
            {
                let local_name =
                    sanitize_name(&props.local_name.unwrap_or_else(|| "Unknown".into()));
                let key = (local_name.clone(), peripheral.id());
                if seen.insert(key) && local_name.eq_ignore_ascii_case(&sanitized_target) {
                    println!("Found matching device: {} ({})", local_name, props.address);
                    if !interactive {
                        return Ok(vec![peripheral]);
                    } else {
                        matches.push(peripheral);
                    }
                }
            }
        }
        if interactive && !matches.is_empty() {
            break;
        }
        sleep(POLL_INTERVAL).await;
    }
    Ok(matches)
}

pub async fn scan_device(
    adapter: Adapter,
    device_name: Option<String>,
    interactive: bool,
) -> Result<Peripheral> {
    adapter
        .start_scan(ScanFilter {
            services: vec![whoop::constants::WHOOP_SERVICE],
        })
        .await
        .with_context(|| "Failed to start scan")?;
    if let Some(ref name) = device_name {
        let matches =
            scan_for_device_by_name(&adapter, name, DEFAULT_SCAN_DURATION, interactive).await?;
        if matches.is_empty() {
            Err(anyhow!(
                "No device with name `{}` found after scanning for {} seconds",
                sanitize_name(name),
                DEFAULT_SCAN_DURATION.as_secs()
            ))
        } else if interactive {
            interactive_selection_common(matches).await
        } else {
            Ok(matches.into_iter().next().unwrap())
        }
    } else {
        let devices = collect_devices_common(&adapter, DEFAULT_SCAN_DURATION).await?;
        if devices.is_empty() {
            Err(anyhow!("No devices found during scan. Ensure your Whoop device is powered on and advertising"))
        } else {
            interactive_selection_common(devices).await
        }
    }
}
