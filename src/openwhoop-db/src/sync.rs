use std::collections::HashMap;
use std::fmt;

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use openwhoop_entities::{activities, heart_rate, sleep_cycles};
use sea_orm::{
    ActiveValue::{NotSet, Set},
    ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect,
    sea_query::{Expr, OnConflict},
};

// SQLite limits to 999 SQL variables, so batch sizes must respect:
// heart_rate: 10 Set columns -> max 99 rows
// sleep_cycles: 11 Set columns -> max 90 rows
// activities: 4 Set columns -> max 249 rows
const HEART_RATE_BATCH: u64 = 90;
const SLEEP_CYCLES_BATCH: u64 = 80;
const ACTIVITIES_BATCH: u64 = 160;

pub struct DatabaseSync<'a> {
    local: &'a DatabaseConnection,
    remote: &'a DatabaseConnection,
}

pub struct SyncReport {
    pub sleep_cycles_synced: usize,
    pub activities_synced: usize,
    pub heart_rate_synced: usize,
}

impl fmt::Display for SyncReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Sync complete:")?;
        writeln!(f, "  sleep_cycles: {}", self.sleep_cycles_synced)?;
        writeln!(f, "  activities:   {}", self.activities_synced)?;
        write!(f, "  heart_rate:   {}", self.heart_rate_synced)
    }
}

fn bar_style() -> ProgressStyle {
    ProgressStyle::with_template("{prefix:>20} [{wide_bar:.cyan/dim}] {percent_precise}% ({elapsed}/{duration}, {eta} remaining)")
        .unwrap()
        .progress_chars("=>-")
}

impl<'a> DatabaseSync<'a> {
    pub fn new(local: &'a DatabaseConnection, remote: &'a DatabaseConnection) -> Self {
        Self { local, remote }
    }

    pub async fn run(&self) -> anyhow::Result<SyncReport> {
        let mp = MultiProgress::new();

        // 1. sleep_cycles (no FK dependencies)
        let sc_lr = self
            .sync_sleep_cycles(self.local, self.remote, &mp, "sleep_cycles L->R")
            .await?;
        let sc_rl = self
            .sync_sleep_cycles(self.remote, self.local, &mp, "sleep_cycles R->L")
            .await?;
        let sleep_cycles_synced = sc_lr + sc_rl;

        // 2. activities (FK -> sleep_cycles via period_id)
        let act_lr = self
            .sync_activities(self.local, self.remote, &mp, "activities L->R")
            .await?;
        let act_rl = self
            .sync_activities(self.remote, self.local, &mp, "activities R->L")
            .await?;
        let activities_synced = act_lr + act_rl;

        // 3. heart_rate (largest table, no FK)
        let hr_lr = self
            .sync_heart_rate(self.local, self.remote, &mp, "heart_rate L->R")
            .await?;
        let hr_rl = self
            .sync_heart_rate(self.remote, self.local, &mp, "heart_rate R->L")
            .await?;
        let heart_rate_synced = hr_lr + hr_rl;

        let report = SyncReport {
            sleep_cycles_synced,
            activities_synced,
            heart_rate_synced,
        };
        println!("{report}");
        Ok(report)
    }

    async fn sync_sleep_cycles(
        &self,
        source: &DatabaseConnection,
        target: &DatabaseConnection,
        mp: &MultiProgress,
        label: &str,
    ) -> anyhow::Result<usize> {
        let unsynced = sleep_cycles::Entity::find()
            .filter(sleep_cycles::Column::Synced.eq(false));

        let total = unsynced.clone().count(source).await?;
        let pb = mp.add(ProgressBar::new(total));
        pb.set_style(bar_style());
        pb.set_prefix(label.to_string());

        if total == 0 {
            pb.finish();
            return Ok(0);
        }

        let mut synced = 0usize;

        loop {
            let rows = unsynced
                .clone()
                .order_by_asc(sleep_cycles::Column::SleepId)
                .limit(Some(SLEEP_CYCLES_BATCH))
                .all(source)
                .await?;

            if rows.is_empty() {
                break;
            }

            let batch_len = u64::try_from(rows.len())?;

            // Deduplicate by sleep_id
            let mut deduped: HashMap<chrono::NaiveDate, sleep_cycles::Model> = HashMap::new();
            for row in &rows {
                deduped.insert(row.sleep_id, row.clone());
            }

            let ids: Vec<_> = deduped.values().map(|m| m.id).collect();

            let models: Vec<sleep_cycles::ActiveModel> = deduped
                .into_values()
                .map(|m| sleep_cycles::ActiveModel {
                    id: Set(m.id),
                    sleep_id: Set(m.sleep_id),
                    start: Set(m.start),
                    end: Set(m.end),
                    min_bpm: Set(m.min_bpm),
                    max_bpm: Set(m.max_bpm),
                    avg_bpm: Set(m.avg_bpm),
                    min_hrv: Set(m.min_hrv),
                    max_hrv: Set(m.max_hrv),
                    avg_hrv: Set(m.avg_hrv),
                    score: Set(m.score),
                    synced: Set(true),
                })
                .collect();

            let count = models.len();

            sleep_cycles::Entity::insert_many(models)
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
                        .value(
                            sleep_cycles::Column::Score,
                            Expr::cust("COALESCE(excluded.score, sleep_cycles.score)"),
                        )
                        .update_column(sleep_cycles::Column::Synced)
                        .to_owned(),
                )
                .exec(target)
                .await?;

            // Mark as synced on source
            sleep_cycles::Entity::update_many()
                .col_expr(sleep_cycles::Column::Synced, Expr::value(true))
                .filter(sleep_cycles::Column::Id.is_in(ids))
                .exec(source)
                .await?;

            synced += count;
            pb.inc(batch_len);

            if batch_len < SLEEP_CYCLES_BATCH {
                break;
            }
        }

        pb.finish();
        Ok(synced)
    }

    async fn sync_activities(
        &self,
        source: &DatabaseConnection,
        target: &DatabaseConnection,
        mp: &MultiProgress,
        label: &str,
    ) -> anyhow::Result<usize> {
        let unsynced = activities::Entity::find()
            .filter(activities::Column::Synced.eq(false));

        let total = unsynced.clone().count(source).await?;
        let pb = mp.add(ProgressBar::new(total));
        pb.set_style(bar_style());
        pb.set_prefix(label.to_string());

        if total == 0 {
            pb.finish();
            return Ok(0);
        }

        let mut synced = 0usize;

        loop {
            let rows = unsynced
                .clone()
                .order_by_asc(activities::Column::Start)
                .limit(Some(ACTIVITIES_BATCH))
                .all(source)
                .await?;

            if rows.is_empty() {
                break;
            }

            let batch_len = u64::try_from(rows.len())?;

            // Deduplicate by start
            let mut deduped: HashMap<chrono::NaiveDateTime, activities::Model> = HashMap::new();
            for row in &rows {
                deduped.insert(row.start, row.clone());
            }

            let ids: Vec<_> = deduped.values().map(|m| m.id).collect();

            let models: Vec<activities::ActiveModel> = deduped
                .into_values()
                .map(|m| activities::ActiveModel {
                    id: NotSet,
                    period_id: Set(m.period_id),
                    start: Set(m.start),
                    end: Set(m.end),
                    activity: Set(m.activity),
                    synced: Set(true),
                })
                .collect();

            let count = models.len();

            activities::Entity::insert_many(models)
                .on_conflict(
                    OnConflict::column(activities::Column::Start)
                        .update_columns([
                            activities::Column::End,
                            activities::Column::Activity,
                            activities::Column::PeriodId,
                            activities::Column::Synced,
                        ])
                        .to_owned(),
                )
                .exec(target)
                .await?;

            // Mark as synced on source
            activities::Entity::update_many()
                .col_expr(activities::Column::Synced, Expr::value(true))
                .filter(activities::Column::Id.is_in(ids))
                .exec(source)
                .await?;

            synced += count;
            pb.inc(batch_len);

            if batch_len < ACTIVITIES_BATCH {
                break;
            }
        }

        pb.finish();
        Ok(synced)
    }

    async fn sync_heart_rate(
        &self,
        source: &DatabaseConnection,
        target: &DatabaseConnection,
        mp: &MultiProgress,
        label: &str,
    ) -> anyhow::Result<usize> {
        let unsynced = heart_rate::Entity::find()
            .filter(heart_rate::Column::Synced.eq(false));

        let total = unsynced.clone().count(source).await?;
        let pb = mp.add(ProgressBar::new(total));
        pb.set_style(bar_style());
        pb.set_prefix(label.to_string());

        if total == 0 {
            pb.finish();
            return Ok(0);
        }

        let mut synced = 0usize;

        loop {
            let rows = unsynced
                .clone()
                .order_by_asc(heart_rate::Column::Time)
                .limit(Some(HEART_RATE_BATCH))
                .all(source)
                .await?;

            if rows.is_empty() {
                break;
            }

            let batch_len = u64::try_from(rows.len())?;

            // Deduplicate by time
            let mut deduped: HashMap<chrono::NaiveDateTime, heart_rate::Model> = HashMap::new();
            for row in &rows {
                deduped.insert(row.time, row.clone());
            }

            let ids: Vec<_> = deduped.values().map(|m| m.id).collect();

            let models: Vec<heart_rate::ActiveModel> = deduped
                .into_values()
                .map(|m| heart_rate::ActiveModel {
                    id: NotSet,
                    bpm: Set(m.bpm),
                    time: Set(m.time),
                    rr_intervals: Set(m.rr_intervals),
                    activity: Set(m.activity),
                    stress: Set(m.stress),
                    spo2: Set(m.spo2),
                    skin_temp: Set(m.skin_temp),
                    imu_data: Set(m.imu_data),
                    sensor_data: Set(m.sensor_data),
                    synced: Set(true),
                })
                .collect();

            let count = models.len();

            heart_rate::Entity::insert_many(models)
                .on_conflict(
                    OnConflict::column(heart_rate::Column::Time)
                        .update_columns([
                            heart_rate::Column::Bpm,
                            heart_rate::Column::RrIntervals,
                        ])
                        .value(
                            heart_rate::Column::Activity,
                            Expr::cust("COALESCE(excluded.activity, heart_rate.activity)"),
                        )
                        .value(
                            heart_rate::Column::Stress,
                            Expr::cust("COALESCE(excluded.stress, heart_rate.stress)"),
                        )
                        .value(
                            heart_rate::Column::Spo2,
                            Expr::cust("COALESCE(excluded.spo2, heart_rate.spo2)"),
                        )
                        .value(
                            heart_rate::Column::SkinTemp,
                            Expr::cust("COALESCE(excluded.skin_temp, heart_rate.skin_temp)"),
                        )
                        .value(
                            heart_rate::Column::ImuData,
                            Expr::cust("COALESCE(excluded.imu_data, heart_rate.imu_data)"),
                        )
                        .update_column(heart_rate::Column::Synced)
                        .to_owned(),
                )
                .exec(target)
                .await?;

            // Mark as synced on source
            heart_rate::Entity::update_many()
                .col_expr(heart_rate::Column::Synced, Expr::value(true))
                .filter(heart_rate::Column::Id.is_in(ids))
                .exec(source)
                .await?;

            synced += count;
            pb.inc(batch_len);

            if batch_len < HEART_RATE_BATCH {
                break;
            }
        }

        pb.finish();
        Ok(synced)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sync_report_display() {
        let report = SyncReport {
            sleep_cycles_synced: 5,
            activities_synced: 10,
            heart_rate_synced: 1000,
        };
        let s = format!("{report}");
        assert!(s.contains("sleep_cycles: 5"));
        assert!(s.contains("activities:   10"));
        assert!(s.contains("heart_rate:   1000"));
    }

    #[tokio::test]
    async fn sync_empty_databases() {
        let db1 = crate::DatabaseHandler::new("sqlite::memory:").await;
        let db2 = crate::DatabaseHandler::new("sqlite::memory:").await;

        let sync = DatabaseSync::new(db1.connection(), db2.connection());
        let report = sync.run().await.unwrap();

        assert_eq!(report.sleep_cycles_synced, 0);
        assert_eq!(report.activities_synced, 0);
        assert_eq!(report.heart_rate_synced, 0);
    }

    #[tokio::test]
    async fn sync_heart_rate_between_databases() {
        let db1 = crate::DatabaseHandler::new("sqlite::memory:").await;
        let db2 = crate::DatabaseHandler::new("sqlite::memory:").await;

        // Insert readings into db1
        for i in 0..5 {
            let reading = openwhoop_codec::HistoryReading {
                unix: 1735689600000 + i * 1000,
                bpm: 70 + u8::try_from(i).unwrap(),
                rr: vec![850],
                imu_data: vec![],
                sensor_data: None,
            };
            db1.create_reading(reading).await.unwrap();
        }

        let sync = DatabaseSync::new(db1.connection(), db2.connection());
        let report = sync.run().await.unwrap();

        assert_eq!(report.heart_rate_synced, 5);

        // Verify db2 has the data
        let history = db2
            .search_history(crate::SearchHistory::default())
            .await
            .unwrap();
        assert_eq!(history.len(), 5);
    }

    #[tokio::test]
    async fn sync_sleep_cycles_between_databases() {
        let db1 = crate::DatabaseHandler::new("sqlite::memory:").await;
        let db2 = crate::DatabaseHandler::new("sqlite::memory:").await;

        let start = chrono::NaiveDate::from_ymd_opt(2025, 1, 1)
            .unwrap()
            .and_hms_opt(22, 0, 0)
            .unwrap();
        let end = chrono::NaiveDate::from_ymd_opt(2025, 1, 2)
            .unwrap()
            .and_hms_opt(6, 0, 0)
            .unwrap();

        db1.create_sleep(openwhoop_algos::SleepCycle {
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

        let sync = DatabaseSync::new(db1.connection(), db2.connection());
        let report = sync.run().await.unwrap();

        assert_eq!(report.sleep_cycles_synced, 1);

        // Verify db2 has the sleep cycle
        let cycles = db2.get_sleep_cycles(None).await.unwrap();
        assert_eq!(cycles.len(), 1);
    }

    #[tokio::test]
    async fn sync_idempotent() {
        let db1 = crate::DatabaseHandler::new("sqlite::memory:").await;
        let db2 = crate::DatabaseHandler::new("sqlite::memory:").await;

        // Insert a reading
        db1.create_reading(openwhoop_codec::HistoryReading {
            unix: 1735689600000,
            bpm: 72,
            rr: vec![833],
            imu_data: vec![],
            sensor_data: None,
        })
        .await
        .unwrap();

        // First sync
        let sync = DatabaseSync::new(db1.connection(), db2.connection());
        let report1 = sync.run().await.unwrap();
        assert_eq!(report1.heart_rate_synced, 1);

        // Second sync - should be 0 since everything is already synced
        let sync = DatabaseSync::new(db1.connection(), db2.connection());
        let report2 = sync.run().await.unwrap();
        assert_eq!(report2.heart_rate_synced, 0);
    }
}
