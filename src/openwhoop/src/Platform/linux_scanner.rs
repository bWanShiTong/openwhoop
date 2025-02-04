use crate::utils::{collect_devices_common, interactive_selection_common};
use crate::POLL_INTERVAL;
use anyhow::{anyhow, Context, Result};
use btleplug::api::Peripheral as _;
use btleplug::{
    api::{BDAddr, Central},
    platform::{Adapter, Peripheral},
};
use std::str::FromStr;
use tokio::time::sleep;

pub async fn scan_device(
    adapter: Adapter,
    device_identifier: Option<String>,
    interactive: bool,
) -> Result<Peripheral> {
    if interactive {
        sleep(POLL_INTERVAL).await;
        let devices = collect_devices_common(&adapter, std::time::Duration::from_secs(10)).await?;
        if devices.is_empty() {
            return Err(anyhow!("No devices found during scan."));
        }
        interactive_selection_common(devices).await
    } else {
        let id_str = device_identifier.ok_or_else(|| {
            anyhow!("Please provide a device address on Linux in non-interactive mode")
        })?;
        let bd_addr = BDAddr::from_str(&id_str).context("Invalid BLE address")?;
        loop {
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
                    if props.address == bd_addr {
                        return Ok(peripheral);
                    }
                }
            }
            sleep(POLL_INTERVAL).await;
        }
    }
}
