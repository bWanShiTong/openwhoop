use chrono::{Local, NaiveDateTime, TimeZone};
use openwhoop_entities::{packets, sleep_cycles};
use openwhoop_migration::{Migrator, MigratorTrait, OnConflict};
use sea_orm::{
    ActiveModelTrait, ActiveValue::NotSet, ColumnTrait, ConnectOptions, Database,
    DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect, Set,
};
use uuid::Uuid;

use openwhoop_algos::SleepCycle;
use openwhoop_codec::HistoryReading;

#[derive(Clone)]
pub struct DatabaseHandler {
    pub(crate) db: DatabaseConnection,
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
        data: Vec<u8>,
    ) -> anyhow::Result<openwhoop_entities::packets::Model> {
        let packet = openwhoop_entities::packets::ActiveModel {
            id: NotSet,
            uuid: Set(char),
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
            .map(|s| serde_json::to_value(s))
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
                    .map(|s| serde_json::to_value(s))
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

    pub async fn get_latest_sleep(
        &self,
    ) -> anyhow::Result<Option<openwhoop_entities::sleep_cycles::Model>> {
        let sleep = sleep_cycles::Entity::find()
            .order_by_desc(sleep_cycles::Column::End)
            .one(&self.db)
            .await?;

        Ok(sleep)
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

    #[tokio::test]
    async fn create_and_get_packets() {
        let db = DatabaseHandler::new("sqlite::memory:").await;
        let uuid = Uuid::new_v4();
        let data = vec![0xAA, 0xBB, 0xCC];

        let packet = db.create_packet(uuid, data.clone()).await.unwrap();
        assert_eq!(packet.uuid, uuid);
        assert_eq!(packet.bytes, data);

        let packets = db.get_packets(0).await.unwrap();
        assert_eq!(packets.len(), 1);
        assert_eq!(packets[0].uuid, uuid);
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
