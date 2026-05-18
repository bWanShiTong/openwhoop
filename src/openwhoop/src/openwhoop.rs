use chrono::{DateTime, Local, NaiveDate, TimeDelta};
use openwhoop_codec::{
    HistoryReading, WhoopData, WhoopPacket,
    constants::{
        CMD_FROM_STRAP_GEN4, CMD_FROM_STRAP_GEN5, DATA_FROM_STRAP_GEN4, DATA_FROM_STRAP_GEN5,
        MetadataType, WhoopGeneration,
    },
};
use openwhoop_db::{DailyInfo, DailyStats, DailyStatsAverage, DatabaseHandler, SearchHistory};
use openwhoop_entities::packets;

use crate::{
    algo::{
        ActivityPeriod, MAX_SLEEP_PAUSE, SleepCycle, StressCalculator, helpers::format_hm::FormatHM,
    },
    ble::BleNotification,
    types::activities,
};

pub struct OpenWhoop {
    pub database: DatabaseHandler,
    pub packet: Option<WhoopPacket>,
    pub last_history_packet: Option<HistoryReading>,
    pub history_packets: Vec<HistoryReading>,
    pub generation: WhoopGeneration,
}

impl OpenWhoop {
    pub fn new(database: DatabaseHandler, generation: WhoopGeneration) -> Self {
        Self {
            database,
            packet: None,
            last_history_packet: None,
            history_packets: Vec::new(),
            generation,
        }
    }

    pub async fn store_packet(
        &self,
        notification: BleNotification,
    ) -> anyhow::Result<packets::Model> {
        let packet = self
            .database
            .create_packet(notification.uuid, self.generation, notification.value)
            .await?;

        Ok(packet)
    }

    pub async fn handle_packet(
        &mut self,
        packet: packets::Model,
    ) -> anyhow::Result<Option<WhoopPacket>> {
        let generation = if matches!(self.generation, WhoopGeneration::Placeholder) {
            packet
                .generation
                .parse()
                .ok()
                .or_else(|| match packet.uuid {
                    DATA_FROM_STRAP_GEN4 | CMD_FROM_STRAP_GEN4 => Some(WhoopGeneration::Gen4),
                    DATA_FROM_STRAP_GEN5 | CMD_FROM_STRAP_GEN5 => Some(WhoopGeneration::Gen5),
                    _ => None,
                })
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "unable to determine packet generation for row {}",
                        packet.id
                    )
                })?
        } else {
            self.generation
        };

        let parse_packet = match generation {
            WhoopGeneration::Placeholder => {
                return Err(anyhow::anyhow!(
                    "cannot decode packets with placeholder generation"
                ));
            }
            WhoopGeneration::Gen4 => WhoopPacket::from_data,
            WhoopGeneration::Gen5 => WhoopPacket::from_data_maverick,
        };
        let data_from_packet = match generation {
            WhoopGeneration::Placeholder => {
                return Err(anyhow::anyhow!(
                    "cannot decode packets with placeholder generation"
                ));
            }
            WhoopGeneration::Gen4 => WhoopData::from_packet_gen4,
            WhoopGeneration::Gen5 => WhoopData::from_packet_gen5,
        };

        let data = match packet.uuid {
            DATA_FROM_STRAP_GEN4 | DATA_FROM_STRAP_GEN5 => {
                let packet = if let Some(mut whoop_packet) = self.packet.take() {
                    whoop_packet.data.extend_from_slice(&packet.bytes);

                    if whoop_packet.data.len() + 3 >= whoop_packet.size {
                        whoop_packet
                    } else {
                        self.packet = Some(whoop_packet);
                        return Ok(None);
                    }
                } else {
                    let packet = parse_packet(packet.bytes)?;
                    if packet.partial {
                        self.packet = Some(packet);
                        return Ok(None);
                    }
                    packet
                };

                match data_from_packet(packet) {
                    Ok(data) => data,
                    Err(e) => {
                        trace!(target: "handle_packet", "DATA unhandled: {}", e);
                        return Ok(None);
                    }
                }
            }
            CMD_FROM_STRAP_GEN4 | CMD_FROM_STRAP_GEN5 => {
                let packet = parse_packet(packet.bytes)?;
                match data_from_packet(packet) {
                    Ok(data) => data,
                    Err(e) => {
                        trace!(target: "handle_packet", "CMD unhandled: {}", e);
                        return Ok(None);
                    }
                }
            }
            _ => return Ok(None),
        };

        self.handle_data(data).await
    }

    async fn handle_data(&mut self, data: WhoopData) -> anyhow::Result<Option<WhoopPacket>> {
        match data {
            WhoopData::HistoryReading(hr) if hr.is_valid() => {
                if let Some(last_packet) = self.last_history_packet.as_mut() {
                    if last_packet.unix == hr.unix && last_packet.bpm == hr.bpm {
                        return Ok(None);
                    } else {
                        last_packet.unix = hr.unix;
                        last_packet.bpm = hr.bpm;
                    }
                } else {
                    self.last_history_packet = Some(hr.clone());
                }

                let ptime = DateTime::from_timestamp_millis(i64::try_from(hr.unix)?)
                    .unwrap()
                    .with_timezone(&Local)
                    .format("%Y-%m-%d %H:%M:%S");

                if hr.imu_data.is_empty() {
                    info!(target: "HistoryReading", "time: {}", ptime);
                } else {
                    info!(target: "HistoryReading", "time: {}, (IMU)", ptime);
                }

                self.history_packets.push(hr);
            }
            WhoopData::HistoryMetadata { end_data, cmd, .. } => match cmd {
                MetadataType::HistoryComplete => {}
                MetadataType::HistoryStart => {}
                MetadataType::HistoryEnd => {
                    self.database
                        .create_readings(std::mem::take(&mut self.history_packets))
                        .await?;

                    let packet = WhoopPacket::history_end(end_data);
                    return Ok(Some(packet));
                }
            },
            WhoopData::ConsoleLog { log, .. } => {
                trace!(target: "ConsoleLog", "{}", log);
            }
            WhoopData::RunAlarm { .. } => {}
            WhoopData::AlarmInfo { .. } => {}
            WhoopData::Event { .. } => {}
            WhoopData::UnknownEvent { .. } => {}
            WhoopData::CommandResponse(_) => {}
            WhoopData::VersionInfo { harvard, boylston } => {
                info!("version harvard {} boylston {}", harvard, boylston);
            }
            WhoopData::HistoryReading(_) => {}
            WhoopData::RealtimeHr { unix, bpm } => {
                info!(target: "RealtimeHr", "time: {}, bpm: {}", unix, bpm);
            }
        }

        Ok(None)
    }

    pub async fn get_latest_sleep(&self) -> anyhow::Result<Option<SleepCycle>> {
        self.database.get_latest_sleep().await
    }

    pub async fn get_sleep_for_date(&self, date: NaiveDate) -> anyhow::Result<Option<SleepCycle>> {
        self.database.get_sleep_for_date(date).await
    }

    pub async fn calculate_latest_strain(&self) -> anyhow::Result<()> {
        self.database.calculate_latest_strain().await?;

        Ok(())
    }

    pub async fn get_latest_strain(
        &self,
    ) -> anyhow::Result<Option<openwhoop_entities::strain::Model>> {
        self.database.get_latest_strain().await
    }

    pub async fn get_strain_for_date(
        &self,
        date: NaiveDate,
    ) -> anyhow::Result<Option<openwhoop_entities::strain::Model>> {
        self.database.get_strain_for_date(date).await
    }

    pub async fn get_daily_info(&self, date: NaiveDate) -> anyhow::Result<DailyInfo> {
        self.database.get_daily_info(date).await
    }

    pub async fn get_latest_daily_stats(&self) -> anyhow::Result<Option<DailyStats>> {
        self.database.get_latest_daily_stats().await
    }

    pub async fn get_last_7_day_daily_stats_average(&self) -> anyhow::Result<DailyStatsAverage> {
        self.database.get_last_7_day_daily_stats_average().await
    }

    pub async fn detect_events(&self) -> anyhow::Result<()> {
        Ok(())
    }

    /// TODO: add handling for data splits
    pub async fn detect_sleeps(&self) -> anyhow::Result<()> {
        'a: loop {
            let last_sleep = self.get_latest_sleep().await?;

            let options = SearchHistory {
                from: last_sleep.map(|s| s.end),
                limit: Some(86400 * 2),
                ..Default::default()
            };

            let mut history = self.database.search_history(options).await?;
            let mut periods = ActivityPeriod::detect_from_gravity(&history);

            while let Some(mut sleep) = ActivityPeriod::find_sleep(&mut periods) {
                if let Some(last_sleep) = last_sleep {
                    let diff = sleep.start - last_sleep.end;

                    if diff < MAX_SLEEP_PAUSE {
                        history = self
                            .database
                            .search_history(SearchHistory {
                                from: Some(last_sleep.start),
                                to: Some(sleep.end),
                                ..Default::default()
                            })
                            .await?;

                        sleep.start = last_sleep.start;
                        sleep.duration = sleep.end - sleep.start;
                    } else {
                        let this_sleep_id = sleep.end.date();
                        let last_sleep_id = last_sleep.end.date();

                        if this_sleep_id == last_sleep_id {
                            if sleep.duration < last_sleep.duration() {
                                let nap = activities::ActivityPeriod {
                                    period_id: last_sleep.id,
                                    from: sleep.start,
                                    to: Some(sleep.end),
                                    activity: activities::ActivityType::Nap,
                                    strain: None,
                                };
                                self.database.create_activity(nap).await?;
                                continue;
                            } else {
                                let nap = activities::ActivityPeriod {
                                    period_id: last_sleep.id - TimeDelta::days(1),
                                    from: last_sleep.start,
                                    to: Some(last_sleep.end),
                                    activity: activities::ActivityType::Nap,
                                    strain: None,
                                };
                                self.database.create_activity(nap).await?;
                            }
                        }
                    }
                }

                let sleep_cycle = SleepCycle::from_event(sleep, &history)?;

                info!(
                    "Detected sleep from {} to {}, duration: {}",
                    sleep.start,
                    sleep.end,
                    sleep.duration.format_hm()
                );
                self.database.create_sleep(sleep_cycle).await?;
                continue 'a;
            }

            break;
        }

        Ok(())
    }

    pub async fn calculate_stress(&self) -> anyhow::Result<()> {
        loop {
            let last_stress = self.database.last_stress_time().await?;
            let options = SearchHistory {
                from: last_stress.map(|t| {
                    t - TimeDelta::seconds(
                        i64::try_from(StressCalculator::MIN_READING_PERIOD).unwrap_or(0),
                    )
                }),
                to: None,
                limit: Some(86400),
            };

            let history = self.database.search_history(options).await?;
            if history.is_empty() || history.len() <= StressCalculator::MIN_READING_PERIOD {
                break;
            }

            let stress_scores = history
                .windows(StressCalculator::MIN_READING_PERIOD)
                .filter_map(StressCalculator::calculate_stress);

            for stress in stress_scores {
                self.database.update_stress_on_reading(stress).await?;
            }
        }

        Ok(())
    }
}
