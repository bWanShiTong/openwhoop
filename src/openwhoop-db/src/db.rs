use chrono::{Duration, Local, NaiveDate, NaiveDateTime, TimeZone, Timelike};
use openwhoop_entities::{packets, sleep_cycles, strain};
use openwhoop_migration::{Migrator, MigratorTrait, OnConflict};
use openwhoop_types::activities::SearchActivityPeriods;
use sea_orm::{
    ActiveModelTrait, ActiveValue::NotSet, ColumnTrait, ConnectOptions, Database,
    DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect, Set,
};
use uuid::Uuid;

use openwhoop_algos::SleepCycle;
use openwhoop_codec::{HistoryReading, constants::WhoopGeneration};

#[derive(Clone)]
pub struct DatabaseHandler {
    pub(crate) db: DatabaseConnection,
}

#[derive(Debug, Clone)]
pub struct DailyInfo {
    pub date: NaiveDate,
    pub sleep: Option<SleepCycle>,
    pub strain: Option<strain::Model>,
    pub activities: Vec<openwhoop_types::activities::ActivityPeriod>,
    pub stress: Option<DailyStressInfo>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DailyStressInfo {
    pub latest: StressReading,
    pub minute_averages: Vec<StressReading>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DailyStats {
    pub hrv: Option<f64>,
    pub rhr: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DailyStatsAverage {
    Average(DailyStats),
    MissingDays(u8),
}

#[derive(Debug, Clone, PartialEq)]
pub struct StressReading {
    pub time: NaiveDateTime,
    pub stress: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ActivityHeartRateStats {
    pub min_hr: u8,
    pub max_hr: u8,
    pub avg_hr: f64,
}

impl DatabaseHandler {
    pub fn connection(&self) -> &DatabaseConnection {
        &self.db
    }

    pub async fn new<C>(path: C) -> Self
    where
        C: Into<ConnectOptions>,
    {
        let db = Database::connect(path)
            .await
            .expect("Unable to connect to db");

        Migrator::up(&db, None)
            .await
            .expect("Error running migrations");

        Self { db }
    }

    pub async fn create_packet(
        &self,
        char: Uuid,
        generation: WhoopGeneration,
        data: Vec<u8>,
    ) -> anyhow::Result<openwhoop_entities::packets::Model> {
        let packet = openwhoop_entities::packets::ActiveModel {
            id: NotSet,
            uuid: Set(char),
            generation: Set(generation.to_string()),
            bytes: Set(data),
        };

        let packet = packet.insert(&self.db).await?;
        Ok(packet)
    }

    pub async fn create_reading(&self, reading: HistoryReading) -> anyhow::Result<()> {
        let time = timestamp_to_local(reading.unix)?;

        let sensor_json = reading
            .sensor_data
            .as_ref()
            .map(serde_json::to_value)
            .transpose()?;

        let packet = openwhoop_entities::heart_rate::ActiveModel {
            id: NotSet,
            bpm: Set(i16::from(reading.bpm)),
            time: Set(time),
            rr_intervals: Set(rr_to_string(reading.rr)),
            activity: NotSet,
            stress: NotSet,
            spo2: NotSet,
            skin_temp: NotSet,
            imu_data: Set(Some(serde_json::to_value(reading.imu_data)?)),
            sensor_data: Set(sensor_json),
            synced: NotSet,
        };

        let _model = openwhoop_entities::heart_rate::Entity::insert(packet)
            .on_conflict(
                OnConflict::column(openwhoop_entities::heart_rate::Column::Time)
                    .update_column(openwhoop_entities::heart_rate::Column::Bpm)
                    .update_column(openwhoop_entities::heart_rate::Column::RrIntervals)
                    .update_column(openwhoop_entities::heart_rate::Column::SensorData)
                    .to_owned(),
            )
            .exec(&self.db)
            .await?;

        Ok(())
    }

    pub async fn create_readings(&self, readings: Vec<HistoryReading>) -> anyhow::Result<()> {
        if readings.is_empty() {
            return Ok(());
        }
        let payloads = readings
            .into_iter()
            .map(|r| {
                let time = timestamp_to_local(r.unix)?;
                let sensor_json = r
                    .sensor_data
                    .as_ref()
                    .map(serde_json::to_value)
                    .transpose()?;
                Ok(openwhoop_entities::heart_rate::ActiveModel {
                    id: NotSet,
                    bpm: Set(i16::from(r.bpm)),
                    time: Set(time),
                    rr_intervals: Set(rr_to_string(r.rr)),
                    activity: NotSet,
                    stress: NotSet,
                    spo2: NotSet,
                    skin_temp: NotSet,
                    imu_data: Set(Some(serde_json::to_value(r.imu_data)?)),
                    sensor_data: Set(sensor_json),
                    synced: NotSet,
                })
            })
            .collect::<anyhow::Result<Vec<_>>>()?;

        // SQLite limits to 999 SQL variables per statement.
        // heart_rate has 11 columns, so max 90 rows per batch.
        for chunk in payloads.chunks(90) {
            openwhoop_entities::heart_rate::Entity::insert_many(chunk.to_vec())
                .on_conflict(
                    OnConflict::column(openwhoop_entities::heart_rate::Column::Time)
                        .update_column(openwhoop_entities::heart_rate::Column::Bpm)
                        .update_column(openwhoop_entities::heart_rate::Column::RrIntervals)
                        .update_column(openwhoop_entities::heart_rate::Column::SensorData)
                        .to_owned(),
                )
                .exec(&self.db)
                .await?;
        }

        Ok(())
    }

    pub async fn get_packets(&self, id: i32) -> anyhow::Result<Vec<packets::Model>> {
        let stream = packets::Entity::find()
            .filter(packets::Column::Id.gt(id))
            .order_by_asc(packets::Column::Id)
            .limit(10_000)
            .all(&self.db)
            .await?;

        Ok(stream)
    }

    pub async fn get_latest_sleep(&self) -> anyhow::Result<Option<SleepCycle>> {
        Ok(sleep_cycles::Entity::find()
            .order_by_desc(sleep_cycles::Column::End)
            .one(&self.db)
            .await?
            .map(SleepCycle::from))
    }

    pub async fn get_sleep_for_date(&self, date: NaiveDate) -> anyhow::Result<Option<SleepCycle>> {
        Ok(sleep_cycles::Entity::find()
            .filter(sleep_cycles::Column::SleepId.eq(date))
            .one(&self.db)
            .await?
            .map(SleepCycle::from))
    }

    pub async fn get_strain_for_date(
        &self,
        date: NaiveDate,
    ) -> anyhow::Result<Option<strain::Model>> {
        Ok(strain::Entity::find()
            .filter(strain::Column::Date.eq(date))
            .one(&self.db)
            .await?)
    }

    pub async fn get_daily_info(&self, date: NaiveDate) -> anyhow::Result<DailyInfo> {
        Ok(DailyInfo {
            date,
            sleep: self.get_sleep_for_date(date - Duration::days(1)).await?,
            strain: self.get_strain_for_date(date).await?,
            activities: self
                .search_activities(SearchActivityPeriods {
                    from: Some(date.and_hms_opt(0, 0, 0).unwrap() - chrono::TimeDelta::seconds(1)),
                    to: Some(
                        date.succ_opt().unwrap().and_hms_opt(0, 0, 0).unwrap()
                            + chrono::TimeDelta::seconds(1),
                    ),
                    activity: None,
                })
                .await?,
            stress: self.get_daily_stress_info(date).await?,
        })
    }

    async fn get_daily_stress_info(
        &self,
        date: NaiveDate,
    ) -> anyhow::Result<Option<DailyStressInfo>> {
        use openwhoop_entities::heart_rate;

        let day_start = date.and_hms_opt(0, 0, 0).unwrap();
        let day_end = date.succ_opt().unwrap().and_hms_opt(0, 0, 0).unwrap();

        let latest = heart_rate::Entity::find()
            .filter(heart_rate::Column::Time.gte(day_start))
            .filter(heart_rate::Column::Time.lt(day_end))
            .filter(heart_rate::Column::Stress.is_not_null())
            .order_by_desc(heart_rate::Column::Time)
            .one(&self.db)
            .await?;

        let Some(latest) = latest else {
            return Ok(None);
        };

        let latest_time = latest.time;
        let latest_minute = latest_time
            .with_second(0)
            .unwrap()
            .with_nanosecond(0)
            .unwrap();
        let window_start = latest_minute - chrono::TimeDelta::minutes(9);

        let readings = heart_rate::Entity::find()
            .filter(heart_rate::Column::Time.gte(window_start))
            .filter(heart_rate::Column::Time.lt(latest_minute + chrono::TimeDelta::minutes(1)))
            .filter(heart_rate::Column::Stress.is_not_null())
            .order_by_asc(heart_rate::Column::Time)
            .all(&self.db)
            .await?;

        let minute_averages = (0..10)
            .map(|offset| {
                let minute = window_start + chrono::TimeDelta::minutes(offset);
                let values = readings
                    .iter()
                    .filter(|reading| {
                        reading.time >= minute
                            && reading.time < minute + chrono::TimeDelta::minutes(1)
                    })
                    .filter_map(|reading| reading.stress)
                    .collect::<Vec<_>>();

                let stress = if values.is_empty() {
                    None
                } else {
                    Some(values.iter().sum::<f64>() / values.len() as f64)
                };

                StressReading {
                    time: minute,
                    stress,
                }
            })
            .collect();

        Ok(Some(DailyStressInfo {
            latest: StressReading {
                time: latest_time,
                stress: latest.stress,
            },
            minute_averages,
        }))
    }

    pub async fn create_sleep(&self, sleep: SleepCycle) -> anyhow::Result<()> {
        let model = sleep_cycles::ActiveModel {
            id: Set(Uuid::new_v4()),
            sleep_id: Set(sleep.id),
            start: Set(sleep.start),
            end: Set(sleep.end),
            min_bpm: Set(sleep.min_bpm.into()),
            max_bpm: Set(sleep.max_bpm.into()),
            avg_bpm: Set(sleep.avg_bpm.into()),
            min_hrv: Set(sleep.min_hrv.into()),
            max_hrv: Set(sleep.max_hrv.into()),
            avg_hrv: Set(sleep.avg_hrv.into()),
            score: Set(sleep.score.into()),
            synced: NotSet,
        };

        let _r = sleep_cycles::Entity::insert(model)
            .on_conflict(
                OnConflict::column(sleep_cycles::Column::SleepId)
                    .update_columns([
                        sleep_cycles::Column::Start,
                        sleep_cycles::Column::End,
                        sleep_cycles::Column::MinBpm,
                        sleep_cycles::Column::MaxBpm,
                        sleep_cycles::Column::AvgBpm,
                        sleep_cycles::Column::MinHrv,
                        sleep_cycles::Column::MaxHrv,
                        sleep_cycles::Column::AvgHrv,
                    ])
                    .to_owned(),
            )
            .exec(&self.db)
            .await?;

        Ok(())
    }
}

fn timestamp_to_local(unix: u64) -> anyhow::Result<NaiveDateTime> {
    let millis = i64::try_from(unix)?;
    let dt = Local
        .timestamp_millis_opt(millis)
        .single()
        .ok_or_else(|| anyhow::anyhow!("ambiguous or invalid unix timestamp: {}", millis))?;

    Ok(dt.naive_local())
}

fn rr_to_string(rr: Vec<u16>) -> String {
    rr.iter().map(u16::to_string).collect::<Vec<_>>().join(",")
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::ActiveModelTrait;

    #[tokio::test]
    async fn create_and_get_packets() {
        let db = DatabaseHandler::new("sqlite::memory:").await;
        let uuid = Uuid::new_v4();
        let data = vec![0xAA, 0xBB, 0xCC];

        let packet = db
            .create_packet(uuid, WhoopGeneration::Gen5, data.clone())
            .await
            .unwrap();
        assert_eq!(packet.uuid, uuid);
        assert_eq!(packet.generation, WhoopGeneration::Gen5.to_string());
        assert_eq!(packet.bytes, data);

        let packets = db.get_packets(0).await.unwrap();
        assert_eq!(packets.len(), 1);
        assert_eq!(packets[0].uuid, uuid);
        assert_eq!(packets[0].generation, WhoopGeneration::Gen5.to_string());
    }

    #[tokio::test]
    async fn create_reading_and_search_history() {
        let db = DatabaseHandler::new("sqlite::memory:").await;

        let reading = HistoryReading {
            unix: 1735689600000, // 2025-01-01 00:00:00 UTC in millis
            bpm: 72,
            rr: vec![833, 850],
            imu_data: vec![],
            sensor_data: None,
        };

        db.create_reading(reading).await.unwrap();

        let history = db
            .search_history(crate::SearchHistory::default())
            .await
            .unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].bpm, 72);
        assert_eq!(history[0].rr, vec![833, 850]);
    }

    #[tokio::test]
    async fn create_readings_batch() {
        let db = DatabaseHandler::new("sqlite::memory:").await;

        let readings: Vec<HistoryReading> = (0..5)
            .map(|i| HistoryReading {
                unix: 1735689600000 + i * 1000,
                bpm: 70 + u8::try_from(i).unwrap(),
                rr: vec![850],
                imu_data: vec![],
                sensor_data: None,
            })
            .collect();

        db.create_readings(readings).await.unwrap();

        let history = db
            .search_history(crate::SearchHistory::default())
            .await
            .unwrap();
        assert_eq!(history.len(), 5);
    }

    #[tokio::test]
    async fn create_and_get_sleep() {
        let db = DatabaseHandler::new("sqlite::memory:").await;

        let start = chrono::NaiveDate::from_ymd_opt(2025, 1, 1)
            .unwrap()
            .and_hms_opt(22, 0, 0)
            .unwrap();
        let end = chrono::NaiveDate::from_ymd_opt(2025, 1, 2)
            .unwrap()
            .and_hms_opt(6, 0, 0)
            .unwrap();

        let sleep = SleepCycle {
            id: end.date(),
            start,
            end,
            min_bpm: 50,
            max_bpm: 70,
            avg_bpm: 60,
            min_hrv: 30,
            max_hrv: 80,
            avg_hrv: 55,
            score: 100.0,
        };

        db.create_sleep(sleep).await.unwrap();

        let latest = db.get_latest_sleep().await.unwrap();
        assert!(latest.is_some());
        let latest = latest.unwrap();
        assert_eq!(latest.min_bpm, 50);
        assert_eq!(latest.avg_bpm, 60);
    }

    #[tokio::test]
    async fn get_sleep_for_date_returns_matching_sleep() {
        let db = DatabaseHandler::new("sqlite::memory:").await;

        let start = chrono::NaiveDate::from_ymd_opt(2025, 1, 1)
            .unwrap()
            .and_hms_opt(22, 0, 0)
            .unwrap();
        let end = chrono::NaiveDate::from_ymd_opt(2025, 1, 2)
            .unwrap()
            .and_hms_opt(6, 0, 0)
            .unwrap();

        db.create_sleep(SleepCycle {
            id: end.date(),
            start,
            end,
            min_bpm: 50,
            max_bpm: 70,
            avg_bpm: 60,
            min_hrv: 30,
            max_hrv: 80,
            avg_hrv: 55,
            score: 100.0,
        })
        .await
        .unwrap();

        let sleep = db.get_sleep_for_date(end.date()).await.unwrap().unwrap();
        assert_eq!(sleep.id, end.date());
        assert_eq!(sleep.min_bpm, 50);
    }

    #[tokio::test]
    async fn get_daily_info_returns_sleep_strain_and_activities_for_date() {
        let db = DatabaseHandler::new("sqlite::memory:").await;
        let date = chrono::NaiveDate::from_ymd_opt(2025, 1, 2).unwrap();

        db.create_sleep(SleepCycle {
            id: date,
            start: chrono::NaiveDate::from_ymd_opt(2025, 1, 1)
                .unwrap()
                .and_hms_opt(22, 0, 0)
                .unwrap(),
            end: date.and_hms_opt(6, 0, 0).unwrap(),
            min_bpm: 50,
            max_bpm: 70,
            avg_bpm: 60,
            min_hrv: 30,
            max_hrv: 80,
            avg_hrv: 55,
            score: 100.0,
        })
        .await
        .unwrap();

        openwhoop_entities::strain::ActiveModel {
            id: Set(Uuid::new_v4()),
            date: Set(date),
            strain: Set(12.5),
        }
        .insert(&db.db)
        .await
        .unwrap();

        for (time, stress) in [
            (date.and_hms_opt(11, 51, 10).unwrap(), 1.0),
            (date.and_hms_opt(11, 51, 40).unwrap(), 3.0),
            (date.and_hms_opt(11, 55, 0).unwrap(), 5.0),
            (date.and_hms_opt(11, 59, 5).unwrap(), 7.0),
            (date.and_hms_opt(12, 0, 15).unwrap(), 9.0),
            (date.and_hms_opt(12, 0, 45).unwrap(), 11.0),
        ] {
            openwhoop_entities::heart_rate::ActiveModel {
                id: NotSet,
                bpm: Set(70),
                time: Set(time),
                rr_intervals: Set("850".to_string()),
                activity: NotSet,
                stress: Set(Some(stress)),
                spo2: NotSet,
                skin_temp: NotSet,
                imu_data: Set(Some(serde_json::to_value(Vec::<u8>::new()).unwrap())),
                sensor_data: NotSet,
                synced: Set(false),
            }
            .insert(&db.db)
            .await
            .unwrap();
        }

        db.create_activity(openwhoop_types::activities::ActivityPeriod {
            period_id: date,
            from: date.and_hms_opt(10, 0, 0).unwrap(),
            to: Some(date.and_hms_opt(11, 0, 0).unwrap()),
            activity: openwhoop_types::activities::ActivityType::Activity,
            strain: Some(8.5),
        })
        .await
        .unwrap();

        let info = db.get_daily_info(date).await.unwrap();
        assert_eq!(info.date, date);
        assert_eq!(info.sleep.unwrap().id, date);
        assert_eq!(info.strain.unwrap().strain, 12.5);
        assert_eq!(info.activities.len(), 1);
        assert_eq!(info.activities[0].from, date.and_hms_opt(10, 0, 0).unwrap());
        assert_eq!(info.activities[0].strain, Some(8.5));
        let stress = info.stress.unwrap();
        assert_eq!(stress.latest.time, date.and_hms_opt(12, 0, 45).unwrap());
        assert_eq!(stress.latest.stress, Some(11.0));
        assert_eq!(stress.minute_averages.len(), 10);
        assert_eq!(
            stress.minute_averages[0],
            StressReading {
                time: date.and_hms_opt(11, 51, 0).unwrap(),
                stress: Some(2.0),
            }
        );
        assert_eq!(stress.minute_averages[1].stress, None);
        assert_eq!(stress.minute_averages[4].stress, Some(5.0));
        assert_eq!(stress.minute_averages[8].stress, Some(7.0));
        assert_eq!(
            stress.minute_averages[9],
            StressReading {
                time: date.and_hms_opt(12, 0, 0).unwrap(),
                stress: Some(10.0),
            }
        );
    }

    #[tokio::test]
    async fn upsert_reading_on_conflict() {
        let db = DatabaseHandler::new("sqlite::memory:").await;

        let reading = HistoryReading {
            unix: 1735689600000,
            bpm: 72,
            rr: vec![833],
            imu_data: vec![],
            sensor_data: None,
        };
        db.create_reading(reading).await.unwrap();

        // Insert again with different bpm - should upsert
        let reading2 = HistoryReading {
            unix: 1735689600000,
            bpm: 80,
            rr: vec![750],
            imu_data: vec![],
            sensor_data: None,
        };
        db.create_reading(reading2).await.unwrap();

        let history = db
            .search_history(crate::SearchHistory::default())
            .await
            .unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].bpm, 80);
    }
}
