use btleplug::api::ValueNotification;
use db_entities::packets;
use whoop::{
    constants::{MetadataType, DATA_FROM_STRAP},
    HistoryReading, WhoopData, WhoopPacket,
};

use crate::{
    algo::{activity::MAX_SLEEP_PAUSE, ActivityPeriod, SleepCycle},
    DatabaseHandler, SearchHistory,
};

pub struct OpenWhoop {
    pub database: DatabaseHandler,
}

impl OpenWhoop {
    pub fn new(database: DatabaseHandler) -> Self {
        Self { database }
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
        &self,
        packet: packets::Model,
    ) -> anyhow::Result<Option<WhoopPacket>> {
        match packet.uuid {
            DATA_FROM_STRAP => {
                let packet = WhoopPacket::from_data(packet.bytes)?;

                let Ok(data) = WhoopData::from_packet(packet) else {
                    return Ok(None);
                };

                match data {
                    WhoopData::HistoryReading(HistoryReading {
                        unix,
                        bpm,
                        rr,
                        activity,
                    }) => {
                        self.database
                            .create_reading(unix, bpm, rr, activity as i64)
                            .await?;
                    }
                    WhoopData::HistoryMetadata { data, cmd, .. } => match cmd {
                        MetadataType::HistoryComplete => return Ok(None),
                        MetadataType::HistoryStart => {}
                        MetadataType::HistoryEnd => {
                            let packet = WhoopPacket::history_end(data);
                            return Ok(Some(packet));
                        }
                    },
                    WhoopData::ConsoleLog { log, .. } => {
                        trace!(target: "ConsoleLog", "{}", log);
                    }
                    WhoopData::RunAlarm { .. } => {}
                    WhoopData::Event { .. } => {}
                    WhoopData::UnknownEvent { .. } => {}
                }
            }
            _ => {
                // todo!()
            }
        }

        Ok(None)
    }

    pub async fn get_latest_sleep(&self) -> anyhow::Result<Option<SleepCycle>> {
        Ok(self
            .database
            .get_latest_sleep()
            .await?
            .map(SleepCycle::from))
    }

    pub async fn detect_sleeps(&self) -> anyhow::Result<()> {
        'a: loop {
            let last_sleep = self.get_latest_sleep().await?;

            let options = SearchHistory {
                from: last_sleep.map(|s| s.end),
                limit: Some(86400 * 2),
            };

            let mut history = self.database.search_history(options).await?;
            let mut periods = ActivityPeriod::detect(history.as_mut_slice());

            while let Some(mut sleep) = ActivityPeriod::find_sleep(&mut periods) {
                dbg!(sleep);

                if let Some(last_sleep) = last_sleep {
                    let diff = sleep.start - last_sleep.end;

                    if diff < MAX_SLEEP_PAUSE {
                        history = self
                            .database
                            .search_history(SearchHistory {
                                from: Some(last_sleep.start),
                                limit: Some((sleep.end - last_sleep.start).num_seconds() as u64),
                            })
                            .await?;

                        sleep.start = last_sleep.start;
                        sleep.duration = sleep.end - sleep.start;
                    } else {
                        let this_sleep_id = sleep.end.date();
                        let last_sleep_id = last_sleep.end.date();

                        if this_sleep_id == last_sleep_id {
                            if sleep.duration < last_sleep.duration() {
                                // This is an nap create logic for it
                                continue;
                            } else {
                                // this means that previous sleep was an nap
                            }
                        }
                    }
                }

                let sleep_cycle = SleepCycle::from_event(sleep, &history);

                self.database.create_sleep(sleep_cycle).await?;
                continue 'a;
            }

            break;
        }

        Ok(())
    }
}
