use anyhow::{anyhow, Result};
use btleplug::api::PeripheralProperties;
use std::env;

#[cfg(not(target_os = "macos"))]
use {btleplug::api::BDAddr, std::str::FromStr};

#[cfg(target_os = "macos")]
#[derive(Debug)]
pub struct Platform {
    identifier: String, // Device name
}

#[cfg(not(target_os = "macos"))]
#[derive(Debug)]
pub struct Platform {
    identifier: BDAddr, // BLE address
}

impl Platform {
    pub fn initialize() -> Result<()> {
        #[cfg(target_os = "macos")]
        {
            env::remove_var("BLE_INTERFACE");
            env::remove_var("WHOOP_ADDR");
        }

        #[cfg(target_os = "linux")]
        {
            env::remove_var("WHOOP_NAME");
        }

        #[cfg(target_os = "windows")]
        {
            // btleplug on windows does not display local_name, it also does not connect to the device successfully
            // While these can be worked-around using lower level windows BLE interfaces it defeats the purpose of cross-platform ble library
            // so instead we error and do not support windows at this time
            // https://github.com/deviceplug/btleplug/issues/267
            // https://github.com/deviceplug/btleplug/issues/301
            // https://github.com/deviceplug/btleplug/issues/260

            return Err(anyhow!(
                "Windows is not supported due to compatibility issues"
            ));
        }

        Ok(())
    }

    pub fn from_env() -> Result<Self> {
        #[cfg(target_os = "macos")]
        {
            env::var("WHOOP_NAME")
                .map(|name| Self { identifier: name })
                .map_err(|_| anyhow!("WHOOP_NAME environment variable required"))
        }

        #[cfg(not(target_os = "macos"))]
        {
            env::var("WHOOP_ADDR")
                .map_err(|_| anyhow!("WHOOP_ADDR environment variable required"))
                .and_then(|addr| {
                    BDAddr::from_str(&addr).map_err(|_| anyhow!("Invalid BLE address format"))
                })
                .map(|addr| Self { identifier: addr })
        }
    }

    pub fn matches(&self, properties: &PeripheralProperties) -> bool {
        #[cfg(target_os = "macos")]
        {
            properties
                .local_name
                .as_deref()
                .map(|name| name.eq_ignore_ascii_case(&self.identifier))
                .unwrap_or(false)
        }

        #[cfg(not(target_os = "macos"))]
        {
            properties.address == self.identifier
        }
    }

    pub fn to_string(&self) -> String {
        #[cfg(target_os = "macos")]
        {
            self.identifier.clone()
        }

        #[cfg(not(target_os = "macos"))]
        {
            self.identifier.to_string()
        }
    }

    pub fn format_device_info(
        properties: &PeripheralProperties,
        name: &Option<String>,
    ) -> Vec<String> {
        let info = vec![
            format!(
                "Name: {}",
                name.as_ref().unwrap_or(&String::from("Unknown"))
            ),
            format!("RSSI: {}", properties.rssi.unwrap_or(0)),
        ];

        #[cfg(not(target_os = "macos"))]
        return [vec![format!("Address: {}", properties.address)], info].concat();

        #[cfg(target_os = "macos")]
        info
    }
}
