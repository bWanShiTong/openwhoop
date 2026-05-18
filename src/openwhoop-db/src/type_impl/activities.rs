use std::str::FromStr;

use openwhoop_entities::{activities, sleep_cycles};
use openwhoop_types::activities::{ActivityPeriod, ActivityType, SearchActivityPeriods};
use sea_orm::{
    ColumnTrait, Condition, EntityTrait, NotSet, QueryFilter, QueryOrder, Set,
    sea_query::OnConflict,
};

use crate::DatabaseHandler;

impl DatabaseHandler {
    pub async fn create_activity(&self, activity: ActivityPeriod) -> anyhow::Result<()> {
        if let Some(to) = activity.to {
            if to <= activity.from {
                return Ok(());
            }
        }

        if self.activity_overlaps_existing(activity).await?
            || self.activity_overlaps_sleep(activity).await?
        {
            return Ok(());
        }

        let model = activities::ActiveModel {
            id: NotSet,
            period_id: Set(activity.period_id),
            start: Set(activity.from),
            end: Set(activity.to),
            activity: Set(activity.activity.to_string()),
            strain: Set(activity.strain),
            synced: NotSet,
        };

        activities::Entity::insert(model)
            .on_conflict(
                OnConflict::column(activities::Column::Start)
                    .update_column(activities::Column::End)
                    .update_column(activities::Column::Activity)
                    .update_column(activities::Column::Strain)
                    .to_owned(),
            )
            .exec(&self.db)
            .await?;

        Ok(())
    }

    async fn activity_overlaps_existing(&self, activity: ActivityPeriod) -> anyhow::Result<bool> {
        Ok(activities::Entity::find()
            .filter(overlap_condition(
                activities::Column::Start,
                activities::Column::End,
                activity.from,
                activity.to,
            ))
            .one(&self.db)
            .await?
            .is_some())
    }

    async fn activity_overlaps_sleep(&self, activity: ActivityPeriod) -> anyhow::Result<bool> {
        Ok(sleep_cycles::Entity::find()
            .filter(overlap_condition(
                sleep_cycles::Column::Start,
                sleep_cycles::Column::End,
                activity.from,
                activity.to,
            ))
            .one(&self.db)
            .await?
            .is_some())
    }

    pub async fn search_activities(
        &self,
        options: SearchActivityPeriods,
    ) -> anyhow::Result<Vec<ActivityPeriod>> {
        let activities = activities::Entity::find()
            .filter(search_activity_periods_query(options))
            .all(&self.db)
            .await?
            .into_iter()
            .map(map_activity_period)
            .collect();

        Ok(activities)
    }

    pub async fn get_latest_activity(&self) -> anyhow::Result<Option<ActivityPeriod>> {
        Ok(activities::Entity::find()
            .order_by_desc(activities::Column::Start)
            .one(&self.db)
            .await?
            .map(map_activity_period))
    }

    pub async fn get_unfinished_activity(&self) -> anyhow::Result<Option<ActivityPeriod>> {
        Ok(activities::Entity::find()
            .filter(activities::Column::End.is_null())
            .order_by_desc(activities::Column::Start)
            .one(&self.db)
            .await?
            .map(map_activity_period))
    }
}

fn map_activity_period(value: activities::Model) -> ActivityPeriod {
    ActivityPeriod {
        period_id: value.period_id,
        from: value.start,
        to: value.end,
        activity: ActivityType::from_str(value.activity.as_str()).unwrap(),
        strain: value.strain,
    }
}

fn search_activity_periods_query(query: SearchActivityPeriods) -> Condition {
    Condition::all()
        .add_option(query.from.map(|from| activities::Column::Start.gt(from)))
        .add_option(query.to.map(|to| {
            Condition::all()
                .add(activities::Column::End.is_not_null())
                .add(activities::Column::End.lt(to))
        }))
        .add_option(
            query
                .activity
                .map(|activity| activities::Column::Activity.eq(activity.to_string())),
        )
}

fn overlap_condition<C>(
    start_column: C,
    end_column: C,
    from: chrono::NaiveDateTime,
    to: Option<chrono::NaiveDateTime>,
) -> Condition
where
    C: ColumnTrait,
{
    match to {
        Some(to) => Condition::all().add(start_column.lt(to)).add(
            Condition::any()
                .add(end_column.is_null())
                .add(end_column.gt(from)),
        ),
        None => Condition::all().add(
            Condition::any()
                .add(end_column.is_null())
                .add(end_column.gt(from)),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, Timelike};

    fn make_activity(hour: u32) -> ActivityPeriod {
        let base = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
        ActivityPeriod {
            period_id: base,
            from: base.and_hms_opt(hour, 0, 0).unwrap(),
            to: Some(base.and_hms_opt(hour + 1, 0, 0).unwrap()),
            activity: ActivityType::Running,
            strain: None,
        }
    }

    #[test]
    fn map_activity_period_converts() {
        let model = activities::Model {
            id: 1,
            period_id: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            start: NaiveDate::from_ymd_opt(2025, 1, 1)
                .unwrap()
                .and_hms_opt(8, 0, 0)
                .unwrap(),
            end: Some(
                NaiveDate::from_ymd_opt(2025, 1, 1)
                    .unwrap()
                    .and_hms_opt(9, 0, 0)
                    .unwrap(),
            ),
            activity: "Running".to_string(),
            strain: Some(7.25),
            synced: false,
        };
        let period = map_activity_period(model);
        assert!(matches!(period.activity, ActivityType::Running));
        assert_eq!(period.strain, Some(7.25));
    }

    #[test]
    fn search_query_no_filters() {
        let query = SearchActivityPeriods::default();
        let _ = search_activity_periods_query(query);
    }

    #[test]
    fn search_query_with_activity_filter() {
        let query = SearchActivityPeriods::default().with_activity(ActivityType::Running);
        let _ = search_activity_periods_query(query);
    }

    #[tokio::test]
    async fn create_and_search_activities() {
        let db = DatabaseHandler::new("sqlite::memory:").await;

        // Must create a sleep cycle first (FK constraint)
        let sleep_date = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
        let sleep = openwhoop_algos::SleepCycle {
            id: sleep_date,
            start: sleep_date.and_hms_opt(22, 0, 0).unwrap(),
            end: NaiveDate::from_ymd_opt(2025, 1, 2)
                .unwrap()
                .and_hms_opt(6, 0, 0)
                .unwrap(),
            min_bpm: 50,
            max_bpm: 70,
            avg_bpm: 60,
            min_hrv: 30,
            max_hrv: 80,
            avg_hrv: 55,
            score: 100.0,
        };
        db.create_sleep(sleep).await.unwrap();

        let activity = make_activity(8);
        db.create_activity(activity).await.unwrap();

        let results = db
            .search_activities(SearchActivityPeriods::default())
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert!(matches!(results[0].activity, ActivityType::Running));
    }

    #[tokio::test]
    async fn get_latest_activity_empty() {
        let db = DatabaseHandler::new("sqlite::memory:").await;
        assert!(db.get_latest_activity().await.unwrap().is_none());
    }

    #[tokio::test]
    async fn get_latest_activity_returns_most_recent() {
        let db = DatabaseHandler::new("sqlite::memory:").await;

        let sleep_date = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
        let sleep = openwhoop_algos::SleepCycle {
            id: sleep_date,
            start: sleep_date.and_hms_opt(22, 0, 0).unwrap(),
            end: NaiveDate::from_ymd_opt(2025, 1, 2)
                .unwrap()
                .and_hms_opt(6, 0, 0)
                .unwrap(),
            min_bpm: 50,
            max_bpm: 70,
            avg_bpm: 60,
            min_hrv: 30,
            max_hrv: 80,
            avg_hrv: 55,
            score: 100.0,
        };
        db.create_sleep(sleep).await.unwrap();

        db.create_activity(make_activity(8)).await.unwrap();
        db.create_activity(make_activity(14)).await.unwrap();

        let latest = db.get_latest_activity().await.unwrap().unwrap();
        assert_eq!(latest.from.hour(), 14);
    }

    #[tokio::test]
    async fn create_activity_skips_overlap_with_existing_activity() {
        let db = DatabaseHandler::new("sqlite::memory:").await;

        let sleep_date = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
        db.create_sleep(openwhoop_algos::SleepCycle {
            id: sleep_date,
            start: NaiveDate::from_ymd_opt(2024, 12, 31)
                .unwrap()
                .and_hms_opt(22, 0, 0)
                .unwrap(),
            end: sleep_date.and_hms_opt(6, 0, 0).unwrap(),
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

        db.create_activity(ActivityPeriod {
            period_id: sleep_date,
            from: sleep_date.and_hms_opt(8, 0, 0).unwrap(),
            to: Some(sleep_date.and_hms_opt(9, 0, 0).unwrap()),
            activity: ActivityType::Running,
            strain: None,
        })
        .await
        .unwrap();

        db.create_activity(ActivityPeriod {
            period_id: sleep_date,
            from: sleep_date.and_hms_opt(8, 30, 0).unwrap(),
            to: Some(sleep_date.and_hms_opt(9, 30, 0).unwrap()),
            activity: ActivityType::Cycling,
            strain: None,
        })
        .await
        .unwrap();

        let results = db
            .search_activities(SearchActivityPeriods::default())
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert!(matches!(results[0].activity, ActivityType::Running));
    }

    #[tokio::test]
    async fn create_activity_skips_overlap_with_sleep() {
        let db = DatabaseHandler::new("sqlite::memory:").await;

        let sleep_date = NaiveDate::from_ymd_opt(2025, 1, 2).unwrap();
        db.create_sleep(openwhoop_algos::SleepCycle {
            id: sleep_date,
            start: NaiveDate::from_ymd_opt(2025, 1, 1)
                .unwrap()
                .and_hms_opt(22, 0, 0)
                .unwrap(),
            end: sleep_date.and_hms_opt(6, 0, 0).unwrap(),
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

        db.create_activity(ActivityPeriod {
            period_id: sleep_date,
            from: sleep_date.and_hms_opt(5, 30, 0).unwrap(),
            to: Some(sleep_date.and_hms_opt(6, 30, 0).unwrap()),
            activity: ActivityType::Running,
            strain: None,
        })
        .await
        .unwrap();

        let results = db
            .search_activities(SearchActivityPeriods::default())
            .await
            .unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn get_unfinished_activity_returns_activity_in_progress() {
        let db = DatabaseHandler::new("sqlite::memory:").await;

        let now = chrono::Local::now().naive_local();
        let sleep_date = now.date();
        db.create_sleep(openwhoop_algos::SleepCycle {
            id: sleep_date,
            start: now - chrono::TimeDelta::days(1) - chrono::TimeDelta::hours(8),
            end: now - chrono::TimeDelta::days(1),
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

        let active = ActivityPeriod {
            period_id: sleep_date,
            from: now - chrono::TimeDelta::minutes(30),
            to: None,
            activity: ActivityType::Running,
            strain: None,
        };

        db.create_activity(active).await.unwrap();

        let unfinished = db.get_unfinished_activity().await.unwrap().unwrap();
        assert_eq!(unfinished.from, active.from);
        assert_eq!(unfinished.to, active.to);
        assert!(matches!(unfinished.activity, ActivityType::Running));
    }

    #[tokio::test]
    async fn get_unfinished_activity_returns_none_when_all_finished() {
        let db = DatabaseHandler::new("sqlite::memory:").await;

        let now = chrono::Local::now().naive_local();
        let sleep_date = now.date();
        db.create_sleep(openwhoop_algos::SleepCycle {
            id: sleep_date,
            start: now - chrono::TimeDelta::days(1) - chrono::TimeDelta::hours(8),
            end: now - chrono::TimeDelta::days(1),
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

        db.create_activity(ActivityPeriod {
            period_id: sleep_date,
            from: now - chrono::TimeDelta::hours(2),
            to: Some(now - chrono::TimeDelta::hours(1)),
            activity: ActivityType::Running,
            strain: None,
        })
        .await
        .unwrap();

        assert!(db.get_unfinished_activity().await.unwrap().is_none());
    }
}
