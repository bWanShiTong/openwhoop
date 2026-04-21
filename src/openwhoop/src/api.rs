use std::path::Path;

use anyhow::{Context, bail};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use serde::{Deserialize, Serialize};

const API_BASE: &str = "https://api.prod.whoop.com";

#[derive(Serialize)]
struct SignInRequest<'a> {
    username: &'a str,
    password: &'a str,
}

#[derive(Deserialize)]
struct SignInResponse {
    access_token: String,
    #[allow(dead_code)]
    access_token_expires_in: Option<u64>,
}

#[derive(Serialize)]
struct FirmwareRequest {
    current_chip_firmwares: Vec<ChipFirmware>,
    chip_firmwares_of_upgrade: Vec<ChipFirmware>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ChipFirmware {
    pub chip_name: String,
    pub version: String,
}

#[derive(Deserialize)]
struct FirmwareResponse {
    firmware_zip_file: Option<String>,
    firmware_file: Option<String>,
    desired_device_firmware_config: Option<DeviceFirmwareConfig>,
}

#[derive(Deserialize)]
struct DeviceFirmwareConfig {
    hardware_device: Option<String>,
    chip_firmwares: Option<Vec<ChipFirmwareInfo>>,
    force_update: Option<bool>,
}

#[derive(Deserialize)]
struct ChipFirmwareInfo {
    chip_name: String,
    version: String,
}

pub struct WhoopApiClient {
    client: reqwest::Client,
    token: String,
}

impl WhoopApiClient {
    pub async fn sign_in(email: &str, password: &str) -> anyhow::Result<Self> {
        let client = reqwest::Client::new();

        let resp = client
            .post(format!("{API_BASE}/auth-service/v2/whoop/sign-in"))
            .json(&SignInRequest {
                username: email,
                password,
            })
            .send()
            .await
            .context("failed to reach WHOOP auth endpoint")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            bail!("authentication failed ({status}): {body}");
        }

        let auth: SignInResponse = resp.json().await.context("invalid auth response")?;
        if let Some(expires) = auth.access_token_expires_in {
            log::info!("authenticated (token expires in {expires}s)");
        }

        Ok(Self {
            client,
            token: auth.access_token,
        })
    }

    pub async fn download_firmware(
        &self,
        device_name: &str,
        current_versions: Vec<ChipFirmware>,
        upgrade_versions: Vec<ChipFirmware>,
    ) -> anyhow::Result<String> {
        let resp = self
            .client
            .post(format!("{API_BASE}/firmware-service/v4/firmware/version"))
            .query(&[("deviceName", device_name)])
            .header("Authorization", format!("Bearer {}", self.token))
            .header("X-WHOOP-Device-Platform", "ANDROID")
            .json(&FirmwareRequest {
                current_chip_firmwares: current_versions,
                chip_firmwares_of_upgrade: upgrade_versions,
            })
            .send()
            .await
            .context("failed to reach firmware endpoint")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            bail!("firmware download failed ({status}): {body}");
        }

        let fw: FirmwareResponse = resp.json().await.context("invalid firmware response")?;

        if let Some(cfg) = &fw.desired_device_firmware_config {
            log::info!(
                "server config (device: {})",
                cfg.hardware_device.as_deref().unwrap_or("?")
            );
            if let Some(chips) = &cfg.chip_firmwares {
                for c in chips {
                    log::info!("  {}: {}", c.chip_name, c.version);
                }
            }
            if cfg.force_update == Some(true) {
                log::info!("  force_update: true");
            }
        }

        fw.firmware_zip_file
            .or(fw.firmware_file)
            .context("no firmware file found in response")
    }
}

pub fn decode_and_extract(firmware_b64: &str, output_dir: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(output_dir)
        .with_context(|| format!("failed to create output dir {}", output_dir.display()))?;

    let zip_bytes = BASE64
        .decode(firmware_b64)
        .context("failed to base64-decode firmware")?;

    log::info!(
        "decoded firmware ZIP: {} bytes ({:.1} KB)",
        zip_bytes.len(),
        zip_bytes.len() as f64 / 1024.0
    );

    let zip_path = output_dir.join("firmware.zip");
    std::fs::write(&zip_path, &zip_bytes)
        .with_context(|| format!("failed to write {}", zip_path.display()))?;
    log::info!("saved ZIP to {}", zip_path.display());

    let cursor = std::io::Cursor::new(&zip_bytes);
    let mut archive = zip::ZipArchive::new(cursor).context("invalid ZIP archive")?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let name = file.name().to_string();
        let out_path = output_dir.join(&name);

        if file.is_dir() {
            std::fs::create_dir_all(&out_path)?;
        } else {
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut out_file = std::fs::File::create(&out_path)?;
            std::io::copy(&mut file, &mut out_file)?;
            log::info!("  {} ({} bytes)", name, file.size());
        }
    }

    log::info!("firmware files saved to {}/", output_dir.display());
    Ok(())
}
