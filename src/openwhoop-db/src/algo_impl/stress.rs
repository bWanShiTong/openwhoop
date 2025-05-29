use crate::DatabaseHandler;

use chrono::NaiveDateTime;
use db_entities::heart_rate;
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
            imu_data: NotSet,
        };

        heart_rate::Entity::update_many()
            .filter(heart_rate::Column::Time.eq(stress.time))
            .set(model)
            .exec(&self.db)
            .await?;

        Ok(())
    }
}
