#[macro_use]
extern crate log;

use std::{str::FromStr, time::Duration};

use anyhow::anyhow;
use btleplug::{
    api::{BDAddr, Central, Manager as _, Peripheral as _, ScanFilter},
    platform::{Adapter, Manager, Peripheral},
};
use chrono::{DateTime, Local, NaiveDateTime, NaiveTime, TimeDelta, Utc};
use clap::{Parser, Subcommand};
use db_entities::packets;
use dotenv::dotenv;
use openwhoop::{
    algo::{ExerciseMetrics, SleepConsistencyAnalyzer},
    types::activities::{ActivityType, SearchActivityPeriods},
    DatabaseHandler, OpenWhoop, WhoopDevice,
};
use tokio::time::sleep;
use whoop::{constants::WHOOP_SERVICE, WhoopPacket};

#[cfg(target_os = "linux")]
pub type DeviceId = BDAddr;

#[cfg(target_os = "macos")]
pub type DeviceId = String;

#[derive(Parser)]
pub struct OpenWhoopCli {
    #[arg(env, long)]
    pub database_url: String,
    #[cfg(target_os = "linux")]
    #[arg(env, long)]
    pub ble_interface: Option<String>,
    #[clap(subcommand)]
    pub subcommand: OpenWhoopCommand,
}

#[derive(Subcommand)]
pub enum OpenWhoopCommand {
    ///
    /// Scan for Whoop devices
    ///
    Scan,
    ///
    /// Download history data from whoop devices
    ///
    DownloadHistory {
        #[arg(long, env)]
        whoop: DeviceId,
    },
    ///
    /// Get the name of the whoop device
    ///
    GetName {
        #[arg(long, env)]
        whoop: DeviceId,
    },
    /// Reruns the packet processing on stored packets
    /// This is used after new more of packets get handled
    ///
    ReRun,
    ///
    /// Detects sleeps and exercises
    ///
    DetectEvents,
    ///
    /// Print sleep statistics for all time and last week
    ///
    SleepStats,
    ///
    /// Print activity statistics for all time and last week
    ///
    ExerciseStats,
    ///
    /// Calculate stress for historical data
    ///
    CalculateStress,
    ///
    /// Set alarm
    ///
    SetAlarm {
        #[arg(long, env)]
        whoop: DeviceId,
        alarm_time: AlarmTime,
    },
    ///
    /// Reboot whoop
    ///
    Reboot {
        #[arg(long, env)]
        whoop: DeviceId,
    },
    ///
    /// Copy packets from one database into another
    ///
    Merge { from: String },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if let Err(error) = dotenv() {
        println!("{}", error);
    }

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .filter_module("sqlx::query", log::LevelFilter::Off)
        .filter_module("sea_orm_migration::migrator", log::LevelFilter::Off)
        .filter_module("bluez_async", log::LevelFilter::Off)
        .init();

    let cli = OpenWhoopCli::parse();

    let adapter = cli.create_ble_adapter().await?;
    let db_handler = DatabaseHandler::new(cli.database_url).await;

    match cli.subcommand {
        OpenWhoopCommand::Scan => {
            scan_command(adapter, None).await?;
            Ok(())
        }
        OpenWhoopCommand::DownloadHistory { whoop } => {
            let peripheral = scan_command(adapter, Some(whoop)).await?;
            let mut whoop = WhoopDevice::new(peripheral, db_handler);

            whoop.connect().await?;
            whoop.initialize().await?;

            let result = whoop.sync_history().await;
            if let Err(e) = result {
                error!("{}", e);
            }

            loop {
                if let Ok(true) = whoop.is_connected().await {
                    whoop
                        .send_command(WhoopPacket::exit_high_freq_sync())
                        .await?;
                    break;
                } else {
                    whoop.connect().await?;
                    sleep(Duration::from_secs(1)).await;
                }
            }

            Ok(())
        }
        OpenWhoopCommand::GetName { whoop } => {
            let peripheral = scan_command(adapter, Some(whoop)).await?;
            let mut whoop = WhoopDevice::new(peripheral, db_handler);

            whoop.connect_and_initialize().await?;
            whoop.init().await?;

            let result = whoop.get_name().await;
            if let Err(e) = result {
                error!("{}", e);
            }

            loop {
                if let Ok(true) = whoop.is_connected().await {
                    whoop
                        .send_command(WhoopPacket::exit_high_freq_sync())
                        .await?;
                    break;
                } else {
                    whoop.connect().await?;
                    sleep(Duration::from_secs(1)).await;
                }
            }

            Ok(())
        }
        OpenWhoopCommand::ReRun => {
            let whoop = OpenWhoop::new(db_handler.clone());
            let mut id = 0;
            loop {
                let packets = db_handler.get_packets(id).await?;
                if packets.is_empty() {
                    break;
                }

                for packet in packets {
                    id = packet.id;
                    whoop.handle_packet(packet).await?;
                }

                println!("{}", id);
            }

            Ok(())
        }
        OpenWhoopCommand::DetectEvents => {
            let whoop = OpenWhoop::new(db_handler);
            whoop.detect_sleeps().await?;
            whoop.detect_events().await?;
            Ok(())
        }
        OpenWhoopCommand::SleepStats => {
            let whoop = OpenWhoop::new(db_handler);
            let sleep_records = whoop.database.get_sleep_cycles().await?;

            if sleep_records.is_empty() {
                println!("No sleep records found, exiting now");
                return Ok(());
            }

            let mut last_week = sleep_records
                .iter()
                .rev()
                .take(7)
                .copied()
                .collect::<Vec<_>>();

            last_week.reverse();
            let analyzer = SleepConsistencyAnalyzer::new(sleep_records);
            let metrics = analyzer.calculate_consistency_metrics();
            println!("All time: \n{}", metrics);
            let analyzer = SleepConsistencyAnalyzer::new(last_week);
            let metrics = analyzer.calculate_consistency_metrics();
            println!("\nWeek: \n{}", metrics);

            Ok(())
        }
        OpenWhoopCommand::ExerciseStats => {
            let whoop = OpenWhoop::new(db_handler);
            let exercises = whoop
                .database
                .search_activities(
                    SearchActivityPeriods::default().with_activity(ActivityType::Activity),
                )
                .await?;

            if exercises.is_empty() {
                println!("No activities found, exiting now");
                return Ok(());
            };

            let last_week = exercises
                .iter()
                .rev()
                .take(7)
                .copied()
                .rev()
                .collect::<Vec<_>>();

            let metrics = ExerciseMetrics::new(exercises);
            let last_week = ExerciseMetrics::new(last_week);

            println!("All time: \n{}", metrics);
            println!("Last week: \n{}", last_week);
            Ok(())
        }
        OpenWhoopCommand::CalculateStress => {
            let whoop = OpenWhoop::new(db_handler);
            whoop.calculate_stress().await?;
            Ok(())
        }
        OpenWhoopCommand::SetAlarm { whoop, alarm_time } => {
            let peripheral = scan_command(adapter, Some(whoop)).await?;
            let mut whoop = WhoopDevice::new(peripheral, db_handler);
            whoop.connect().await?;

            let time = alarm_time.unix();
            let now = Utc::now();

            if time < now {
                error!(
                    "Time {} is in past, current time: {}",
                    time.format("%Y-%m-%d %H:%M:%S"),
                    now.format("%Y-%m-%d %H:%M:%S")
                );
                return Ok(());
            }

            let packet = WhoopPacket::alarm_time(time.timestamp() as u32);
            whoop.send_command(packet).await?;
            let time = time.with_timezone(&Local);

            println!("Alarm time set for: {}", time.format("%Y-%m-%d %H:%M:%S"));
            Ok(())
        }

        OpenWhoopCommand::Reboot { whoop } => {
            let peripheral = scan_command(adapter, Some(whoop)).await?;
            let mut whoop = WhoopDevice::new(peripheral, db_handler);
            whoop.connect().await?;
            whoop.initialize().await?;

            let packet = WhoopPacket::reboot();
            whoop.send_command(packet).await?;

            println!("Reboot commmend sent");
            Ok(())
        }

        OpenWhoopCommand::Merge { from } => {
            let from_db = DatabaseHandler::new(from).await;

            let mut id = 0;
            loop {
                let packets = from_db.get_packets(id).await?;
                if packets.is_empty() {
                    break;
                }

                for packets::Model {
                    uuid,
                    bytes,
                    id: c_id,
                } in packets
                {
                    id = c_id;
                    db_handler.create_packet(uuid, bytes).await?;
                }

                println!("{}", id);
            }

            Ok(())
        }
    }
}

async fn scan_command(adapter: Adapter, device_id: Option<DeviceId>) -> anyhow::Result<Peripheral> {
    adapter
        .start_scan(ScanFilter {
            services: vec![WHOOP_SERVICE],
        })
        .await?;

    loop {
        let peripherals = adapter.peripherals().await?;

        for peripheral in peripherals {
            let Some(properties) = peripheral.properties().await? else {
                continue;
            };

            if !properties.services.contains(&WHOOP_SERVICE) {
                continue;
            }

            let Some(device_id) = device_id.as_ref() else {
                println!("Address: {}", properties.address);
                println!("Name: {:?}", properties.local_name);
                println!("RSSI: {:?}", properties.rssi);
                println!();
                continue;
            };

            #[cfg(target_os = "linux")]
            if properties.address == *device_id {
                return Ok(peripheral);
            }

            #[cfg(target_os = "macos")]
            {
                let Some(name) = properties.local_name else {
                    continue;
                };
                if sanitize_name(&name).starts_with(device_id) {
                    return Ok(peripheral);
                }
            }
        }

        sleep(Duration::from_secs(1)).await;
    }
}

#[derive(Clone, Copy, Debug)]
pub enum AlarmTime {
    DateTime(NaiveDateTime),
    Time(NaiveTime),
    Minute,
    Minute5,
    Minute10,
    Minute15,
    Minute30,
    Hour,
}

impl FromStr for AlarmTime {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Ok(t) = s.parse() {
            return Ok(Self::DateTime(t));
        }

        if let Ok(t) = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S") {
            return Ok(Self::DateTime(t));
        }

        if let Ok(t) = NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S") {
            return Ok(Self::DateTime(t));
        }

        if let Ok(t) = s.parse() {
            return Ok(Self::Time(t));
        }

        match s {
            "minute" | "1min" | "min" => Ok(Self::Minute),
            "5minute" | "5min" => Ok(Self::Minute5),
            "10minute" | "10min" => Ok(Self::Minute10),
            "15minute" | "15min" => Ok(Self::Minute15),
            "30minute" | "30min" => Ok(Self::Minute30),
            "hour" | "h" => Ok(Self::Hour),
            _ => Err(anyhow!("Invalid alarm time")),
        }
    }
}

impl AlarmTime {
    pub fn unix(self) -> DateTime<Utc> {
        let mut now = Utc::now();
        let timezone_df = Local::now().offset().to_owned();

        match self {
            AlarmTime::DateTime(dt) => dt.and_utc() - timezone_df,
            AlarmTime::Time(t) => {
                let current_time = now.time();
                if current_time > t {
                    now += TimeDelta::days(1);
                }

                now.with_time(t).unwrap() - timezone_df
            }
            _ => {
                let offset = self.offset();
                now + offset
            }
        }
    }

    fn offset(self) -> TimeDelta {
        match self {
            AlarmTime::DateTime(_) => todo!(),
            AlarmTime::Time(_) => todo!(),
            AlarmTime::Minute => TimeDelta::minutes(1),
            AlarmTime::Minute5 => TimeDelta::minutes(5),
            AlarmTime::Minute10 => TimeDelta::minutes(10),
            AlarmTime::Minute15 => TimeDelta::minutes(15),
            AlarmTime::Minute30 => TimeDelta::minutes(30),
            AlarmTime::Hour => TimeDelta::hours(1),
        }
    }
}

#[cfg(target_os = "macos")]
pub fn sanitize_name(name: &str) -> String {
    name.chars()
        .filter(|c| !c.is_control())
        .collect::<String>()
        .trim()
        .to_string()
}

impl OpenWhoopCli {
    async fn create_ble_adapter(&self) -> anyhow::Result<Adapter> {
        let manager = Manager::new().await?;

        #[cfg(target_os = "linux")]
        match self.ble_interface.as_ref() {
            Some(interface) => Self::adapter_from_name(&manager, interface).await,
            None => Self::default_adapter(&manager).await,
        }

        #[cfg(target_os = "macos")]
        Self::default_adapter(&manager).await
    }

    async fn adapter_from_name(manager: &Manager, interface: &str) -> anyhow::Result<Adapter> {
        let adapters = manager.adapters().await?;
        let mut c_adapter = Err(anyhow!("Adapter: `{}` not found", interface));
        for adapter in adapters {
            let name = adapter.adapter_info().await?;
            if name.starts_with(interface) {
                c_adapter = Ok(adapter);
                break;
            }
        }

        c_adapter
    }

    async fn default_adapter(manager: &Manager) -> anyhow::Result<Adapter> {
        let adapters = manager.adapters().await?;
        adapters
            .into_iter()
            .next()
            .ok_or(anyhow!("No BLE adapters found"))
    }
}
