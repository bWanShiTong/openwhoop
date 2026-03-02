use crate::DatabaseHandler;

use chrono::NaiveDateTime;
use openwhoop_entities::heart_rate;
use openwhoop_algos::StressScore;
use sea_orm::{
    ActiveValue::NotSet, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
    SelectColumns, Set, Unchanged,
};

impl DatabaseHandler {
    pub async fn last_stress_time(&self) -> anyhow::Result<Option<NaiveDateTime>> {
        let reading = heart_rate::Entity::find()
            .filter(heart_rate::Column::Stress.is_not_null())
            .order_by_desc(heart_rate::Column::Time)
            .select_only()
            .select_column(heart_rate::Column::Time)
            .into_tuple()
            .one(&self.db)
            .await?;

        Ok(reading)
    }

    pub async fn update_stress_on_reading(&self, stress: StressScore) -> anyhow::Result<()> {
        let model = heart_rate::ActiveModel {
            id: NotSet,
            bpm: NotSet,
            time: Unchanged(stress.time),
            rr_intervals: NotSet,
            activity: NotSet,
            stress: Set(Some(stress.score)),
            spo2: NotSet,
            skin_temp: NotSet,
            imu_data: NotSet,
            sensor_data: NotSet,
            synced: NotSet,
        };

        heart_rate::Entity::update_many()
            .filter(heart_rate::Column::Time.eq(stress.time))
            .set(model)
            .exec(&self.db)
            .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn last_stress_time_empty() {
        let db = DatabaseHandler::new("sqlite::memory:").await;
        let result = db.last_stress_time().await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn update_stress_on_reading_integration() {
        let db = DatabaseHandler::new("sqlite::memory:").await;

        let reading = openwhoop_codec::HistoryReading {
            unix: 1735689600000,
            bpm: 72,
            rr: vec![833],
            imu_data: vec![],
            sensor_data: None,
        };
        db.create_reading(reading).await.unwrap();

        let history = db
            .search_history(crate::SearchHistory::default())
            .await
            .unwrap();
        let time = history[0].time;

        let stress = StressScore {
            time,
            score: 5.5,
        };
        db.update_stress_on_reading(stress).await.unwrap();

        let last_stress = db.last_stress_time().await.unwrap();
        assert!(last_stress.is_some());
        assert_eq!(last_stress.unwrap(), time);
    }
}
