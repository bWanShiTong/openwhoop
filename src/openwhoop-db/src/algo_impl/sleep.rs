use chrono::NaiveDateTime;
use openwhoop_algos::SleepCycle;
use openwhoop_entities::sleep_cycles;
use sea_orm::{ColumnTrait, Condition, EntityTrait, QueryFilter, QueryOrder};

use crate::DatabaseHandler;

impl DatabaseHandler {
    pub async fn get_sleep_cycles(
        &self,
        start: Option<NaiveDateTime>,
    ) -> anyhow::Result<Vec<SleepCycle>> {
        let filter = Condition::all().add_option(start.map(|s| sleep_cycles::Column::Start.gte(s)));

        Ok(sleep_cycles::Entity::find()
            .order_by_asc(sleep_cycles::Column::Start)
            .filter(filter)
            .all(&self.db)
            .await?
            .into_iter()
            .map(map_sleep_cycle)
            .collect())
    }
}

fn map_sleep_cycle(value: sleep_cycles::Model) -> SleepCycle {
    SleepCycle {
        id: value.sleep_id,
        start: value.start,
        end: value.end,
        min_bpm: value.min_bpm.try_into().unwrap(),
        max_bpm: value.max_bpm.try_into().unwrap(),
        avg_bpm: value.avg_bpm.try_into().unwrap(),
        min_hrv: value.min_hrv.try_into().unwrap(),
        max_hrv: value.max_hrv.try_into().unwrap(),
        avg_hrv: value.avg_hrv.try_into().unwrap(),
        score: value
            .score
            .unwrap_or(SleepCycle::sleep_score(value.start, value.end)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    #[test]
    fn map_sleep_cycle_with_score() {
        let model = sleep_cycles::Model {
            id: uuid::Uuid::new_v4(),
            sleep_id: NaiveDate::from_ymd_opt(2025, 1, 2).unwrap(),
            start: NaiveDate::from_ymd_opt(2025, 1, 1)
                .unwrap()
                .and_hms_opt(22, 0, 0)
                .unwrap(),
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
            score: Some(95.0),
            synced: false,
        };

        let cycle = map_sleep_cycle(model);
        assert_eq!(cycle.min_bpm, 50);
        assert_eq!(cycle.avg_hrv, 55);
        assert_eq!(cycle.score, 95.0);
    }

    #[test]
    fn map_sleep_cycle_without_score_uses_calculated() {
        let model = sleep_cycles::Model {
            id: uuid::Uuid::new_v4(),
            sleep_id: NaiveDate::from_ymd_opt(2025, 1, 2).unwrap(),
            start: NaiveDate::from_ymd_opt(2025, 1, 1)
                .unwrap()
                .and_hms_opt(22, 0, 0)
                .unwrap(),
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
            score: None, // No score stored
            synced: false,
        };

        let cycle = map_sleep_cycle(model);
        // 8 hours / 8 hours = 1.0 -> 100.0
        assert_eq!(cycle.score, 100.0);
    }

    #[tokio::test]
    async fn get_sleep_cycles_empty() {
        let db = DatabaseHandler::new("sqlite::memory:").await;
        let cycles = db.get_sleep_cycles(None).await.unwrap();
        assert!(cycles.is_empty());
    }

    #[tokio::test]
    async fn get_sleep_cycles_returns_inserted() {
        let db = DatabaseHandler::new("sqlite::memory:").await;

        let start = NaiveDate::from_ymd_opt(2025, 1, 1)
            .unwrap()
            .and_hms_opt(22, 0, 0)
            .unwrap();
        let end = NaiveDate::from_ymd_opt(2025, 1, 2)
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

        let cycles = db.get_sleep_cycles(None).await.unwrap();
        assert_eq!(cycles.len(), 1);
        assert_eq!(cycles[0].min_bpm, 50);
    }

    #[tokio::test]
    async fn get_sleep_cycles_with_start_filter() {
        let db = DatabaseHandler::new("sqlite::memory:").await;

        // Insert two sleep cycles
        for day in [1, 3] {
            let start = NaiveDate::from_ymd_opt(2025, 1, day)
                .unwrap()
                .and_hms_opt(22, 0, 0)
                .unwrap();
            let end = NaiveDate::from_ymd_opt(2025, 1, day + 1)
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
        }

        let filter_start = NaiveDate::from_ymd_opt(2025, 1, 2)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap();

        let cycles = db.get_sleep_cycles(Some(filter_start)).await.unwrap();
        assert_eq!(cycles.len(), 1); // Only the Jan 3 sleep
    }
}
