#[macro_use]
extern crate log;

use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use btleplug::{
    api::{BDAddr, Central, Manager as _, Peripheral as _, ScanFilter},
    platform::{Adapter, Manager, Peripheral},
};
use clap::{Parser, Subcommand};
use dialoguer::Select;
use dotenv::dotenv;
use openwhoop::{DatabaseHandler, OpenWhoop, WhoopDevice};
use std::collections::HashSet;
use std::env;
use tokio::time::{sleep, Instant};
use whoop::{constants::WHOOP_SERVICE, WhoopPacket};

pub const DEFAULT_SCAN_DURATION: Duration = Duration::from_secs(10);
pub const POLL_INTERVAL: Duration = Duration::from_secs(1);

mod Platform;
mod utils;

use crate::utils::{get_adapter, initialize_platform_specific, sanitize_name};
use crate::Platform::port;

#[derive(Parser)]
pub struct OpenWhoopCli {
    #[arg(env, long)]
    pub database_url: String,
    #[arg(env, long)]
    pub ble_interface: Option<String>,
    #[clap(subcommand)]
    pub subcommand: OpenWhoopCommand,
}

#[derive(Subcommand)]
pub enum OpenWhoopCommand {
    Scan,
    DownloadHistory {
        #[arg(long, alias = "whoop-name", env)]
        device_identifier: Option<String>,
        #[arg(long)]
        interactive: bool,
    },
    ReRun,
    DetectEvents,
}

async fn initialize_stuff() -> Result<(OpenWhoopCli, DatabaseHandler, Adapter)> {
    dotenv().ok();
    initialize_platform_specific();

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .filter_module("sqlx::query", log::LevelFilter::Off)
        .init();

    let cli = OpenWhoopCli::parse();
    let db_handler = DatabaseHandler::new(cli.database_url.clone()).await;
    let manager = Manager::new()
        .await
        .with_context(|| "Failed to create BLE manager")?;
    let adapter = get_adapter(manager, cli.ble_interface.clone()).await?;

    Ok((cli, db_handler, adapter))
}

#[tokio::main]
async fn main() -> Result<()> {
    let (cli, db_handler, adapter) = initialize_stuff().await?;

    match cli.subcommand {
        OpenWhoopCommand::Scan => {
            let _device = port::download_history_scan_device(adapter, None, false).await?;
            Ok(())
        }
        OpenWhoopCommand::DownloadHistory {
            device_identifier,
            interactive,
        } => {
            let peripheral =
                port::download_history_scan_device(adapter, device_identifier, interactive).await?;
            let mut whoop_device = WhoopDevice::new(peripheral, db_handler);
            whoop_device.connect().await?;
            whoop_device.initialize().await?;
            if let Err(e) = whoop_device.sync_history().await {
                error!("Error during history sync: {}", e);
            }
            while !whoop_device.is_connected().await.unwrap_or(false) {
                whoop_device.connect().await?;
                sleep(Duration::from_secs(1)).await;
            }
            whoop_device
                .send_command(WhoopPacket::exit_high_freq_sync())
                .await?;
            Ok(())
        }
        OpenWhoopCommand::ReRun => {
            let whoop = OpenWhoop::new(db_handler.clone());
            let mut last_packet_id = 0;
            loop {
                let packets = db_handler.get_packets(last_packet_id).await?;
                if packets.is_empty() {
                    break;
                }
                for packet in packets {
                    last_packet_id = packet.id;
                    whoop.handle_packet(packet).await?;
                }
                println!("Processed up to packet id: {}", last_packet_id);
            }
            Ok(())
        }
        OpenWhoopCommand::DetectEvents => {
            let whoop = OpenWhoop::new(db_handler);
            whoop.detect_sleeps().await?;
            whoop.detect_events().await?;
            Ok(())
        }
    }
}
