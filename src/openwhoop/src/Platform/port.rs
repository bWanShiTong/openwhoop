use anyhow::{anyhow, Context, Result};
use btleplug::{
    api::BDAddr,
    platform::{Adapter, Peripheral},
};

#[cfg(target_os = "linux")]
use crate::Platform::linux_scanner;
#[cfg(target_os = "macos")]
use crate::Platform::macos_scanner;

#[cfg(target_os = "linux")]
pub async fn download_history_scan_device(
    adapter: Adapter,
    device_identifier: Option<String>,
    interactive: bool,
) -> Result<Peripheral> {
    linux_scanner::scan_device(adapter, device_identifier, interactive).await
}

#[cfg(target_os = "macos")]
pub async fn download_history_scan_device(
    adapter: Adapter,
    device_identifier: Option<String>,
    interactive: bool,
) -> Result<Peripheral> {
    macos_scanner::scan_device(adapter, device_identifier, interactive).await
}

//TODO: Add Android, iOS, Windows, ...
