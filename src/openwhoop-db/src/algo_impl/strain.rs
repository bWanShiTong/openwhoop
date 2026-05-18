use chrono::{Days, NaiveDate, NaiveDateTime};
use openwhoop_algos::{StrainCalculator, StrainScore};
use openwhoop_entities::{heart_rate, sleep_cycles, strain};
use openwhoop_migration::OnConflict;
use openwhoop_types::activities::ActivityPeriod;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect, Set};
use uuid::Uuid;

use crate::{ActivityHeartRateStats, DatabaseHandler, SearchHistory};

impl DatabaseHandler {
    pub async fn get_latest_strain(&self) -> anyhow::Result<Option<strain::Model>> {
        Ok(strain::Entity::find()
            .order_by_desc(strain::Column::Date)
            .one(&self.db)
            .await?)
    }

    pub async fn calculate_latest_strain(&self) -> anyhow::Result<()> {
        let Some(first_date) = self.get_first_reading_date().await? else {
            return Ok(());
        };
        let Some(last_date) = self.get_latest_reading_date().await? else {
            return Ok(());
        };

        let next_unsaved_date = self
            .get_latest_strain()
            .await?
            .map(|row| row.date.checked_add_days(Days::new(1)).unwrap_or(row.date))
            .unwrap_or(first_date);
        let recalc_from = last_date
            .checked_sub_days(Days::new(1))
            .unwrap_or(last_date);
        let start_date = next_unsaved_date.min(recalc_from).max(first_date);

        if start_date > last_date {
            return Ok(());
        }

        let mut date = start_date;
        while date <= last_date {
            let from = date.and_hms_opt(0, 0, 0).expect("valid start of day");
            let to = date
                .checked_add_days(Days::new(1))
                .and_then(|next| next.and_hms_opt(0, 0, 0))
                .expect("valid next day");

            let history = self
                .search_history(SearchHistory {
                    from: Some(from - chrono::TimeDelta::milliseconds(1)),
                    to: Some(to),
                    limit: None,
                })
                .await?;

            let Some(max_hr) = self.get_max_hr_before(from, to).await? else {
                date = match date.checked_add_days(Days::new(1)) {
                    Some(next) => next,
                    None => break,
                };
                continue;
            };

            let Some(resting_hr) = self.get_resting_hr_before(from).await? else {
                date = match date.checked_add_days(Days::new(1)) {
                    Some(next) => next,
                    None => break,
                };
                continue;
            };
            let calculator = StrainCalculator::new(max_hr, resting_hr);

            if let Some(StrainScore(score)) = calculator.calculate(&history) {
                self.create_or_update_strain(date, score).await?;
            }

            date = match date.checked_add_days(Days::new(1)) {
                Some(next) => next,
                None => break,
            };
        }

        Ok(())
    }

    pub async fn calculate_strain_for_activity(
        &self,
        activity: ActivityPeriod,
    ) -> anyhow::Result<Option<f64>> {
        let to = match activity.to {
            Some(to) => to,
            None => match self.get_latest_reading_time().await? {
                Some(to) => to,
                None => return Ok(None),
            },
        };

        let history = self
            .search_history(SearchHistory {
                from: Some(activity.from - chrono::TimeDelta::milliseconds(1)),
                to: activity.to,
                limit: None,
            })
            .await?;

        let Some(max_hr) = self.get_max_hr_before(activity.from, to).await? else {
            return Ok(None);
        };

        let Some(resting_hr) = self.get_resting_hr_before(activity.from).await? else {
            return Ok(None);
        };

        let calculator = StrainCalculator::new(max_hr, resting_hr);
        Ok(calculator
            .calculate(&history)
            .map(|StrainScore(score)| score))
    }

    pub async fn get_heart_rate_stats_for_activity(
        &self,
        activity: ActivityPeriod,
    ) -> anyhow::Result<Option<ActivityHeartRateStats>> {
        let history = self
            .search_history(SearchHistory {
                from: Some(activity.from),
                to: activity.to,
                limit: None,
            })
            .await?;

        if history.is_empty() {
            return Ok(None);
        }

        let (min_hr, max_hr, sum_hr, count) = history.iter().fold(
            (u8::MAX, u8::MIN, 0_u64, 0_u64),
            |(min_hr, max_hr, sum_hr, count), reading| {
                (
                    min_hr.min(reading.bpm),
                    max_hr.max(reading.bpm),
                    sum_hr + u64::from(reading.bpm),
                    count + 1,
                )
            },
        );

        Ok(Some(ActivityHeartRateStats {
            min_hr,
            max_hr,
            avg_hr: sum_hr as f64 / count as f64,
        }))
    }

    async fn create_or_update_strain(
        &self,
        date: NaiveDate,
        score: f64,
    ) -> anyhow::Result<strain::Model> {
        let model = strain::ActiveModel {
            id: Set(Uuid::new_v4()),
            date: Set(date),
            strain: Set(score),
        };

        strain::Entity::insert(model)
            .on_conflict(
                OnConflict::column(strain::Column::Date)
                    .update_column(strain::Column::Strain)
                    .to_owned(),
            )
            .exec(&self.db)
            .await?;

        strain::Entity::find()
            .filter(strain::Column::Date.eq(date))
            .one(&self.db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("strain row missing after upsert"))
    }

    async fn get_first_reading_date(&self) -> anyhow::Result<Option<NaiveDate>> {
        Ok(self
            .get_boundary_reading_time(true)
            .await?
            .map(|time| time.date()))
    }

    async fn get_latest_reading_date(&self) -> anyhow::Result<Option<NaiveDate>> {
        Ok(self
            .get_boundary_reading_time(false)
            .await?
            .map(|time| time.date()))
    }

    async fn get_boundary_reading_time(
        &self,
        earliest: bool,
    ) -> anyhow::Result<Option<NaiveDateTime>> {
        let query = heart_rate::Entity::find().select_only();

        let query = if earliest {
            query.expr(heart_rate::Column::Time.min())
        } else {
            query.expr(heart_rate::Column::Time.max())
        };

        Ok(query.into_tuple().one(&self.db).await?)
    }

    async fn get_max_hr_before(
        &self,
        before: NaiveDateTime,
        fallback_to: NaiveDateTime,
    ) -> anyhow::Result<Option<u8>> {
        let max_hr = self
            .get_max_hr_in_range(None, before)
            .await?
            .or(self.get_max_hr_in_range(Some(before), fallback_to).await?);

        Ok(max_hr.and_then(|value| u8::try_from(value).ok()))
    }

    async fn get_max_hr_in_range(
        &self,
        from: Option<NaiveDateTime>,
        to: NaiveDateTime,
    ) -> anyhow::Result<Option<i16>> {
        let max_hr: Option<i16> = heart_rate::Entity::find()
            .select_only()
            .filter({
                let mut condition = sea_orm::Condition::all().add(heart_rate::Column::Time.lt(to));
                if let Some(from) = from {
                    condition = condition.add(heart_rate::Column::Time.gte(from));
                }
                condition
            })
            .column(heart_rate::Column::Bpm)
            .order_by_desc(heart_rate::Column::Bpm)
            .limit(1)
            .into_tuple()
            .one(&self.db)
            .await?;

        Ok(max_hr)
    }

    async fn get_resting_hr_before(&self, before: NaiveDateTime) -> anyhow::Result<Option<u8>> {
        let latest_sleep = sleep_cycles::Entity::find()
            .filter(sleep_cycles::Column::End.lt(before))
            .order_by_desc(sleep_cycles::Column::End)
            .one(&self.db)
            .await?;

        Ok(latest_sleep.and_then(|sleep| u8::try_from(sleep.min_bpm).ok()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use openwhoop_algos::SleepCycle;
    use openwhoop_entities::heart_rate;
    use sea_orm::{ActiveModelTrait, ActiveValue::NotSet, Set};

    fn make_activity(
        date: NaiveDate,
        start_h: u32,
        start_m: u32,
        duration_secs: i64,
    ) -> ActivityPeriod {
        let from = date.and_hms_opt(start_h, start_m, 0).unwrap();
        ActivityPeriod {
            period_id: date,
            from,
            to: Some(from + chrono::TimeDelta::seconds(duration_secs)),
            activity: openwhoop_types::activities::ActivityType::Running,
            strain: None,
        }
    }

    #[tokio::test]
    async fn calculate_strain_for_activity_uses_all_history_from_start_when_activity_is_open_ended()
    {
        let db = DatabaseHandler::new("sqlite::memory:").await;
        let date = NaiveDate::from_ymd_opt(2025, 1, 2).unwrap();

        db.create_sleep(SleepCycle {
            id: date.pred_opt().unwrap(),
            start: date.pred_opt().unwrap().and_hms_opt(23, 0, 0).unwrap(),
            end: date.and_hms_opt(7, 0, 0).unwrap(),
            min_bpm: 60,
            max_bpm: 70,
            avg_bpm: 64,
            min_hrv: 30,
            max_hrv: 60,
            avg_hrv: 45,
            score: 100.0,
        })
        .await
        .unwrap();

        let activity = ActivityPeriod {
            period_id: date,
            from: date.and_hms_opt(10, 0, 0).unwrap(),
            to: None,
            activity: openwhoop_types::activities::ActivityType::Running,
            strain: None,
        };

        for i in 1..=600 {
            heart_rate::ActiveModel {
                id: NotSet,
                bpm: Set(170),
                time: Set(activity.from + chrono::TimeDelta::seconds(i)),
                rr_intervals: Set("800".to_string()),
                activity: NotSet,
                stress: NotSet,
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

        let score = db
            .calculate_strain_for_activity(activity)
            .await
            .unwrap()
            .expect("open-ended activity strain should be calculated");

        assert!(score > 0.0);
    }

    #[tokio::test]
    async fn calculate_strain_for_activity_uses_activity_time_range() {
        let db = DatabaseHandler::new("sqlite::memory:").await;
        let date = NaiveDate::from_ymd_opt(2025, 1, 2).unwrap();

        db.create_sleep(SleepCycle {
            id: date.pred_opt().unwrap(),
            start: date.pred_opt().unwrap().and_hms_opt(23, 0, 0).unwrap(),
            end: date.and_hms_opt(7, 0, 0).unwrap(),
            min_bpm: 60,
            max_bpm: 70,
            avg_bpm: 64,
            min_hrv: 30,
            max_hrv: 60,
            avg_hrv: 45,
            score: 100.0,
        })
        .await
        .unwrap();

        let activity = make_activity(date, 10, 0, 601);

        for i in 1..=600 {
            heart_rate::ActiveModel {
                id: NotSet,
                bpm: Set(170),
                time: Set(activity.from + chrono::TimeDelta::seconds(i)),
                rr_intervals: Set("800".to_string()),
                activity: NotSet,
                stress: NotSet,
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

        let score = db
            .calculate_strain_for_activity(activity)
            .await
            .unwrap()
            .expect("activity strain should be calculated");

        assert!(score > 0.0);
    }

    #[tokio::test]
    async fn get_heart_rate_stats_for_activity_uses_activity_time_range() {
        let db = DatabaseHandler::new("sqlite::memory:").await;
        let date = NaiveDate::from_ymd_opt(2025, 1, 2).unwrap();
        let activity = make_activity(date, 10, 0, 10);

        for (offset_secs, bpm) in [(1, 100), (2, 120), (3, 140), (11, 200)] {
            heart_rate::ActiveModel {
                id: NotSet,
                bpm: Set(bpm),
                time: Set(activity.from + chrono::TimeDelta::seconds(offset_secs)),
                rr_intervals: Set("800".to_string()),
                activity: NotSet,
                stress: NotSet,
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

        let stats = db
            .get_heart_rate_stats_for_activity(activity)
            .await
            .unwrap()
            .expect("activity heart-rate stats should be calculated");

        assert_eq!(stats.min_hr, 100);
        assert_eq!(stats.max_hr, 140);
        assert!((stats.avg_hr - 120.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn get_heart_rate_stats_for_activity_uses_all_history_when_activity_is_open_ended() {
        let db = DatabaseHandler::new("sqlite::memory:").await;
        let date = NaiveDate::from_ymd_opt(2025, 1, 2).unwrap();
        let from = date.and_hms_opt(10, 0, 0).unwrap();
        let activity = ActivityPeriod {
            period_id: date,
            from,
            to: None,
            activity: openwhoop_types::activities::ActivityType::Running,
            strain: None,
        };

        for (offset_secs, bpm) in [(1, 95), (2, 105), (3, 125)] {
            heart_rate::ActiveModel {
                id: NotSet,
                bpm: Set(bpm),
                time: Set(from + chrono::TimeDelta::seconds(offset_secs)),
                rr_intervals: Set("800".to_string()),
                activity: NotSet,
                stress: NotSet,
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

        let stats = db
            .get_heart_rate_stats_for_activity(activity)
            .await
            .unwrap()
            .expect("open-ended activity heart-rate stats should be calculated");

        assert_eq!(stats.min_hr, 95);
        assert_eq!(stats.max_hr, 125);
        assert!((stats.avg_hr - 108.333_333_333_333_33).abs() < 1e-9);
    }
}
