use chrono::NaiveDateTime;
use db_entities::heart_rate;
use sea_orm::{ColumnTrait, Condition, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use whoop::{Activity, ParsedHistoryReading};

use crate::DatabaseHandler;

#[derive(Default, Debug)]
pub struct SearchHistory {
    pub from: Option<NaiveDateTime>,
    pub to: Option<NaiveDateTime>,
    pub limit: Option<u64>,
}

impl SearchHistory {
    pub(crate) fn conditions(self) -> Condition {
        Condition::all()
            .add_option(self.from.map(|from| heart_rate::Column::Time.gt(from)))
            .add_option(self.to.map(|to| heart_rate::Column::Time.lt(to)))
    }
}

impl DatabaseHandler {
    pub async fn search_history(
        &self,
        options: SearchHistory,
    ) -> anyhow::Result<Vec<ParsedHistoryReading>> {
        let limit = options.limit;
        let history = heart_rate::Entity::find()
            .filter(options.conditions())
            .filter(heart_rate::Column::Activity.is_not_null())
            .limit(limit)
            .order_by_asc(heart_rate::Column::Time)
            .all(&self.db)
            .await?
            .into_iter()
            .map(Self::parse_reading)
            .collect();

        Ok(history)
    }

    fn parse_reading(model: heart_rate::Model) -> ParsedHistoryReading {
        ParsedHistoryReading {
            time: model.time,
            bpm: model.bpm.try_into().unwrap_or(u8::MAX),
            rr: model
                .rr_intervals
                .split(',')
                .filter_map(|rr| rr.parse().ok())
                .collect(),
            activity: model.activity.map(Activity::from).unwrap(),
            imu_data: {
                if let Some(data) = model.imu_data {
                    serde_json::from_value(data).unwrap()
                } else {
                    Default::default()
                }
            },
        }
    }
}
