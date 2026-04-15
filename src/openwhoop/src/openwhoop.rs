use btleplug::api::ValueNotification;
use chrono::{DateTime, Local, TimeDelta};
use openwhoop_entities::packets;
use openwhoop_db::{DatabaseHandler, SearchHistory};
use openwhoop_codec::{
    Activity, HistoryReading, WhoopData, WhoopPacket,
    constants::{CMD_FROM_STRAP, DATA_FROM_STRAP, MetadataType},
};

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
}

impl OpenWhoop {
    pub fn new(database: DatabaseHandler) -> Self {
        Self {
            database,
            packet: None,
            last_history_packet: None,
            history_packets: Vec::new(),
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
        let data = match packet.uuid {
            DATA_FROM_STRAP => {
                let packet = if let Some(mut whoop_packet) = self.packet.take() {
                    // TODO: maybe not needed but it would be nice to handle packet length here
                    // so if next packet contains end of one and start of another it is handled

                    whoop_packet.data.extend_from_slice(&packet.bytes);

                    if whoop_packet.data.len() + 3 >= whoop_packet.size {
                        whoop_packet
                    } else {
                        self.packet = Some(whoop_packet);
                        return Ok(None);
                    }
                } else {
                    let packet = WhoopPacket::from_data(packet.bytes)?;
                    if packet.partial {
                        self.packet = Some(packet);
                        return Ok(None);
                    }
                    packet
                };

                let Ok(data) = WhoopData::from_packet(packet) else {
                    return Ok(None);
                };
                data
            }
            CMD_FROM_STRAP => {
                let packet = WhoopPacket::from_data(packet.bytes)?;

                let Ok(data) = WhoopData::from_packet(packet) else {
                    return Ok(None);
                };

                data
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
            WhoopData::HistoryMetadata { data, cmd, .. } => match cmd {
                MetadataType::HistoryComplete => {}
                MetadataType::HistoryStart => {}
                MetadataType::HistoryEnd => {
                    self.database
                        .create_readings(std::mem::take(&mut self.history_packets))
                        .await?;

                    let packet = WhoopPacket::history_end(data);
                    return Ok(Some(packet));
                }
            },
            WhoopData::ConsoleLog { log, .. } => {
                trace!(target: "ConsoleLog", "{}", log);
            }
            WhoopData::RunAlarm { .. } => {}
            WhoopData::AlarmInfo { .. } => {}
            WhoopData::Event { .. } => {}
            WhoopData::VersionInfo { harvard, boylston } => {
                info!("version harvard {} boylston {}", harvard, boylston);
            }
            _ => {}
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
        const BATCH: u64 = 86400;
        let window = SpO2Calculator::WINDOW_SIZE as i64;

        let mut from = self
            .database
            .last_spo2_time()
            .await?
            .map(|t| t - TimeDelta::seconds(window));

        loop {
            let readings = self
                .database
                .search_sensor_readings(SearchHistory {
                    from,
                    to: None,
                    limit: Some(BATCH),
                })
                .await?;

            if readings.len() < SpO2Calculator::WINDOW_SIZE {
                break;
            }

            for w in readings.windows(SpO2Calculator::WINDOW_SIZE) {
                if let Some(score) = SpO2Calculator::calculate(w) {
                    self.database.update_spo2_on_reading(score).await?;
                }
            }

            if (readings.len() as u64) < BATCH {
                break;
            }

            // Cursor must advance independently of whether scores were produced;
            // the prior implementation relied on last_spo2_time(), which only
            // moves when a row gets written. Invalid tail windows then wedged
            // the loop forever.
            let next = Some(readings.last().unwrap().time - TimeDelta::seconds(window));
            if next == from {
                break;
            }
            from = next;
        }

        Ok(())
    }

    pub async fn calculate_skin_temp(&self) -> anyhow::Result<()> {
        const BATCH: u64 = 86400;
        let mut from: Option<chrono::NaiveDateTime> = None;

        loop {
            let readings = self
                .database
                .search_temp_readings(SearchHistory {
                    from,
                    to: None,
                    limit: Some(BATCH),
                })
                .await?;

            if readings.is_empty() {
                break;
            }

            // Capture cursor before processing: search_temp_readings filters by
            // `skin_temp IS NULL`, so any row where convert() returns None
            // (raw < MIN_RAW) would otherwise re-appear forever.
            let last_time = readings.last().unwrap().time;

            for reading in &readings {
                if let Some(score) =
                    SkinTempCalculator::convert(reading.time, reading.skin_temp_raw)
                {
                    self.database.update_skin_temp_on_reading(score).await?;
                }
            }

            if (readings.len() as u64) < BATCH {
                break;
            }
            if Some(last_time) == from {
                break;
            }
            from = Some(last_time);
        }

        Ok(())
    }

    pub async fn calculate_stress(&self) -> anyhow::Result<()> {
        const BATCH: u64 = 86400;
        let window = StressCalculator::MIN_READING_PERIOD as i64;

        let mut from = self
            .database
            .last_stress_time()
            .await?
            .map(|t| t - TimeDelta::seconds(window));

        loop {
            let history = self
                .database
                .search_history(SearchHistory {
                    from,
                    to: None,
                    limit: Some(BATCH),
                })
                .await?;

            if history.len() < StressCalculator::MIN_READING_PERIOD {
                break;
            }

            for w in history.windows(StressCalculator::MIN_READING_PERIOD) {
                if let Some(stress) = StressCalculator::calculate_stress(w) {
                    self.database.update_stress_on_reading(stress).await?;
                }
            }

            if (history.len() as u64) < BATCH {
                break;
            }

            let next = Some(history.last().unwrap().time - TimeDelta::seconds(window));
            if next == from {
                break;
            }
            from = next;
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
