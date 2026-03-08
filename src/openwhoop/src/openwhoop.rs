use btleplug::api::ValueNotification;
use chrono::{DateTime, Local, TimeDelta};
use openwhoop_codec::{
    Activity, HistoryReading, WhoopData, WhoopPacket,
    constants::{
        CMD_FROM_STRAP_GEN4, CMD_FROM_STRAP_GEN5, DATA_FROM_STRAP_GEN4, DATA_FROM_STRAP_GEN5,
        MetadataType, WhoopGeneration,
    },
};
use openwhoop_db::{DatabaseHandler, SearchHistory};
use openwhoop_entities::packets;

use crate::{
    algo::{
        ActivityPeriod, MAX_SLEEP_PAUSE, SkinTempCalculator, SleepCycle, SpO2Calculator,
        StressCalculator, helpers::format_hm::FormatHM,
    },
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
        notification: ValueNotification,
    ) -> anyhow::Result<packets::Model> {
        let packet = self
            .database
            .create_packet(notification.uuid, notification.value)
            .await?;

        Ok(packet)
    }

    pub async fn handle_packet(
        &mut self,
        packet: packets::Model,
    ) -> anyhow::Result<Option<WhoopPacket>> {
        let parse_packet = match self.generation {
            WhoopGeneration::Placeholder => todo!(),
            WhoopGeneration::Gen4 => WhoopPacket::from_data,
            WhoopGeneration::Gen5 => WhoopPacket::from_data_maverick,
        };
        let data_from_packet = match self.generation {
            WhoopGeneration::Placeholder => todo!(),
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
        Ok(self.database.get_latest_sleep().await?.map(map_sleep_cycle))
    }

    pub async fn detect_events(&self) -> anyhow::Result<()> {
        let latest_activity = self.database.get_latest_activity().await?;
        let start_from = latest_activity.map(|a| a.from);

        let sleeps = self
            .database
            .get_sleep_cycles(start_from)
            .await?
            .windows(2)
            .map(|sleep| (sleep[0].id, sleep[0].end, sleep[1].start))
            .collect::<Vec<_>>();

        for (cycle_id, start, end) in sleeps {
            let options = SearchHistory {
                from: Some(start),
                to: Some(end),
                ..Default::default()
            };

            let history = self.database.search_history(options).await?;
            let events = ActivityPeriod::detect_from_gravity(&history);

            for event in events {
                let activity = match event.activity {
                    Activity::Active => activities::ActivityType::Activity,
                    Activity::Sleep => activities::ActivityType::Nap,
                    _ => continue,
                };

                let activity = activities::ActivityPeriod {
                    period_id: cycle_id,
                    from: event.start,
                    to: event.end,
                    activity,
                };

                let duration = activity.to - activity.from;
                info!(
                    "Detected activity period from: {} to: {}, duration: {}",
                    activity.from,
                    activity.to,
                    duration.format_hm()
                );
                self.database.create_activity(activity).await?;
            }
        }

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
                                    to: sleep.end,
                                    activity: activities::ActivityType::Nap,
                                };
                                self.database.create_activity(nap).await?;
                                continue;
                            } else {
                                let nap = activities::ActivityPeriod {
                                    period_id: last_sleep.id - TimeDelta::days(1),
                                    from: last_sleep.start,
                                    to: last_sleep.end,
                                    activity: activities::ActivityType::Nap,
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

    pub async fn calculate_spo2(&self) -> anyhow::Result<()> {
        loop {
            let last = self.database.last_spo2_time().await?;
            let options = SearchHistory {
                from: last.map(|t| {
                    t - TimeDelta::seconds(i64::try_from(SpO2Calculator::WINDOW_SIZE).unwrap_or(0))
                }),
                to: None,
                limit: Some(86400),
            };

            let readings = self.database.search_sensor_readings(options).await?;
            if readings.is_empty() || readings.len() <= SpO2Calculator::WINDOW_SIZE {
                break;
            }

            let scores = readings
                .windows(SpO2Calculator::WINDOW_SIZE)
                .filter_map(SpO2Calculator::calculate);

            for score in scores {
                self.database.update_spo2_on_reading(score).await?;
            }
        }

        Ok(())
    }

    pub async fn calculate_skin_temp(&self) -> anyhow::Result<()> {
        loop {
            let readings = self
                .database
                .search_temp_readings(SearchHistory {
                    limit: Some(86400),
                    ..Default::default()
                })
                .await?;

            if readings.is_empty() {
                break;
            }

            for reading in &readings {
                if let Some(score) =
                    SkinTempCalculator::convert(reading.time, reading.skin_temp_raw)
                {
                    self.database.update_skin_temp_on_reading(score).await?;
                }
            }
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

fn map_sleep_cycle(sleep: openwhoop_entities::sleep_cycles::Model) -> SleepCycle {
    SleepCycle {
        id: sleep.end.date(),
        start: sleep.start,
        end: sleep.end,
        min_bpm: sleep.min_bpm.try_into().unwrap(),
        max_bpm: sleep.max_bpm.try_into().unwrap(),
        avg_bpm: sleep.avg_bpm.try_into().unwrap(),
        min_hrv: sleep.min_hrv.try_into().unwrap(),
        max_hrv: sleep.max_hrv.try_into().unwrap(),
        avg_hrv: sleep.avg_hrv.try_into().unwrap(),
        score: sleep
            .score
            .unwrap_or_else(|| SleepCycle::sleep_score(sleep.start, sleep.end)),
    }
}
