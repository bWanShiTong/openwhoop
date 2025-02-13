use anyhow::Result;
use chrono::{Local, NaiveDateTime, TimeZone};
use csv::Writer;
use db_entities::{packets, sleep_cycles};
use migration::{Migrator, MigratorTrait, OnConflict};
use sea_orm::{
    prelude::Expr, ActiveModelTrait, ActiveValue::NotSet, ColumnTrait, Condition, Database,
    DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect, Set,
};
use std::fs::File;
use uuid::Uuid;

mod history;
pub use history::SearchHistory;
use whoop::constants::DATA_FROM_STRAP;

use crate::algo::SleepCycle;

#[derive(Clone)]
pub struct DatabaseHandler {
    pub(crate) db: DatabaseConnection,
}

impl DatabaseHandler {
    pub async fn new(path: String) -> Self {
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
    ) -> anyhow::Result<db_entities::packets::Model> {
        let packet = db_entities::packets::ActiveModel {
            id: NotSet,
            uuid: Set(char),
            bytes: Set(data),
        };

        let packet = packet.insert(&self.db).await?;
        Ok(packet)
    }

    pub async fn create_reading(
        &self,
        unix: u32,
        bpm: u8,
        rr: Vec<u16>,
        activity: i64,
    ) -> anyhow::Result<()> {
        let time = timestamp_to_local(unix);
        info!(target: "HistoryReading", "time: {}, bpm: {}", time, bpm);

        let packet = db_entities::heart_rate::ActiveModel {
            id: NotSet,
            bpm: Set(bpm as i16),
            time: Set(time),
            rr_intervals: Set(rr_to_string(rr)),
            activity: Set(Some(activity)),
        };

        let _model = db_entities::heart_rate::Entity::insert(packet)
            .on_conflict(
                OnConflict::column(db_entities::heart_rate::Column::Time)
                    .update_column(db_entities::heart_rate::Column::Bpm)
                    .update_column(db_entities::heart_rate::Column::RrIntervals)
                    .update_column(db_entities::heart_rate::Column::Activity)
                    .to_owned(),
            )
            .exec(&self.db)
            .await?;

        Ok(())
    }

    pub async fn get_packets(&self, id: i32) -> anyhow::Result<Vec<packets::Model>> {
        let stream = packets::Entity::find()
            .filter(packets::Column::Id.gt(id))
            .filter(packets::Column::Uuid.eq(DATA_FROM_STRAP))
            .filter(
                Condition::any()
                    .add(Expr::cust("LOWER(HEX(bytes))").like("aa6400a1%"))
                    .add(Expr::cust("LOWER(HEX(bytes))").like("aa5c00f0%")),
            )
            .order_by_asc(packets::Column::Id)
            .limit(10_000)
            .all(&self.db)
            .await?;

        Ok(stream)
    }

    pub async fn get_latest_sleep(
        &self,
    ) -> anyhow::Result<Option<db_entities::sleep_cycles::Model>> {
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
            avg_hrv: Set(sleep.max_hrv.into()),
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
    pub async fn export_heart_rate_to_csv(&self, file_path: &str) -> Result<()> {
        let file = File::create(file_path)?;
        let mut wtr = Writer::from_writer(file);

        wtr.write_record(&["time", "bpm", "rr_intervals", "activity"])?;

        let heart_rate_data = db_entities::heart_rate::Entity::find()
            .order_by_desc(db_entities::heart_rate::Column::Time)
            .all(&self.db)
            .await?;

        for record in heart_rate_data {
            wtr.write_record(&[
                record.time.to_string(),
                record.bpm.to_string(),
                record.rr_intervals,
                record.activity.map(|a| a.to_string()).unwrap_or_default(),
            ])?;
        }

        wtr.flush()?;
        Ok(())
    }

    pub async fn export_sleep_cycles_to_csv(&self, file_path: &str) -> Result<()> {
        let file = File::create(file_path)?;
        let mut wtr = Writer::from_writer(file);

        //Heders
        wtr.write_record(&[
            "sleep_id", "start", "end", "min_bpm", "max_bpm", "avg_bpm", "min_hrv", "max_hrv",
            "avg_hrv",
        ])?;

        // Fetch sleep cycles data from the database
        let sleep_cycles_data = db_entities::sleep_cycles::Entity::find()
            .all(&self.db)
            .await?;

        for record in sleep_cycles_data {
            wtr.write_record(&[
                record.sleep_id.to_string(),
                record.start.to_string(),
                record.end.to_string(),
                record.min_bpm.to_string(),
                record.max_bpm.to_string(),
                record.avg_bpm.to_string(),
                record.min_hrv.to_string(),
                record.max_hrv.to_string(),
                record.avg_hrv.to_string(),
            ])?;
        }

        wtr.flush()?;
        Ok(())
    }
}

fn timestamp_to_local(unix: u32) -> NaiveDateTime {
    let dt = Local
        .timestamp_opt(unix as i64, 0)
        .single()
        .expect("I don't know");

    dt.naive_local()
}

fn rr_to_string(rr: Vec<u16>) -> String {
    rr.iter().map(u16::to_string).collect::<Vec<_>>().join(",")
}
