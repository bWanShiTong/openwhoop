use anyhow::{anyhow, Context, Result};
use btleplug::api::Central as _;
use btleplug::api::Peripheral as _;
use btleplug::api::{Central, Manager as ApiManager, Peripheral as ApiPeripheral};
use btleplug::platform::{Adapter, Manager, Peripheral};
use dialoguer::Select;
use std::collections::HashSet;
use std::env;
use std::time::Duration;
use tokio::time::{sleep, Instant};

pub const DEFAULT_SCAN_DURATION: Duration = Duration::from_secs(10);
pub const POLL_INTERVAL: Duration = Duration::from_secs(1);

/// Sanitize a device name by removing control characters and trimming whitespace.
/// This is the case when the whoop name data was corrupted.
/// Funnily enough, I think this won't work in the following case because you won't have a readable name:
/// 1. You corrupt your name data
/// 2. You don't have access to a change_name functionality like in the official Whoop App
/// Thats why an interactive or automatic detection is important.
pub fn sanitize_name(name: &str) -> String {
    name.chars()
        .filter(|c| !c.is_control())
        .collect::<String>()
        .trim()
        .to_string()
}

/// Returns the desired BLE adapter; if an interface is provided, it filters for it.
pub async fn get_adapter(manager: Manager, ble_interface: Option<String>) -> Result<Adapter> {
    let adapters = manager.adapters().await?;
    if let Some(interface) = ble_interface {
        adapters
            .into_iter()
            .find(|adapter| {
                if let Ok(info) = futures::executor::block_on(adapter.adapter_info()) {
                    info.starts_with(&interface)
                } else {
                    false
                }
            })
            .ok_or_else(|| anyhow!("Adapter `{}` not found", interface))
    } else {
        adapters
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("No BLE adapters found"))
    }
}

/// Performs any platform-specific initialization.
pub fn initialize_platform_specific() {
    #[cfg(target_os = "macos")]
    {
        env::remove_var("BLE_INTERFACE");
        env::remove_var("WHOOP_ADDR");
    }
}

/// Collects all discovered devices from the adapter over the given duration.
pub async fn collect_devices_common(
    adapter: &Adapter,
    duration: Duration,
) -> Result<Vec<Peripheral>> {
    let start = Instant::now();
    let mut seen = HashSet::new();
    let mut devices = Vec::new();
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
                let name =
                    sanitize_name(&props.local_name.unwrap_or_else(|| "Unknown".to_string()));
                let key = (name, peripheral.id());
                if seen.insert(key) {
                    devices.push(peripheral);
                }
            }
        }
        sleep(POLL_INTERVAL).await;
    }
    Ok(devices)
}

/// Presents an interactive selection dialog for the given devices.
pub async fn interactive_selection_common(devices: Vec<Peripheral>) -> Result<Peripheral> {
    let mut items = Vec::new();
    for device in &devices {
        if let Some(props) = device
            .properties()
            .await
            .with_context(|| "Failed to get peripheral properties")?
        {
            let local_name =
                sanitize_name(&props.local_name.unwrap_or_else(|| "Unknown".to_string()));
            items.push(format!("{} ({})", local_name, props.address));
        }
    }
    let selection = tokio::task::spawn_blocking(move || {
        Select::new()
            .with_prompt("Select a device")
            .items(&items)
            .default(0)
            .interact()
    })
    .await??;
    devices
        .into_iter()
        .nth(selection)
        .ok_or_else(|| anyhow!("Invalid selection"))
}
