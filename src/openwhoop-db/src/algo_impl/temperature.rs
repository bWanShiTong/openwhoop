use crate::{DatabaseHandler, SearchHistory};

use chrono::NaiveDateTime;
use openwhoop_algos::SkinTempScore;
use openwhoop_codec::SensorData;
use openwhoop_entities::heart_rate;
use sea_orm::{
    ActiveValue::NotSet, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
    SelectColumns, Set, Unchanged,
};

pub struct TempReading {
    pub time: NaiveDateTime,
    pub skin_temp_raw: u16,
}

impl DatabaseHandler {
    pub async fn last_skin_temp_time(&self) -> anyhow::Result<Option<NaiveDateTime>> {
        let reading = heart_rate::Entity::find()
            .filter(heart_rate::Column::SkinTemp.is_not_null())
            .order_by_desc(heart_rate::Column::Time)
            .select_only()
            .select_column(heart_rate::Column::Time)
            .into_tuple()
            .one(&self.db)
            .await?;

        Ok(reading)
    }

    pub async fn search_temp_readings(
        &self,
        options: SearchHistory,
    ) -> anyhow::Result<Vec<TempReading>> {
        let limit = options.limit;
        let rows = heart_rate::Entity::find()
            .filter(options.conditions())
            .filter(heart_rate::Column::SensorData.is_not_null())
            .filter(heart_rate::Column::SkinTemp.is_null())
            .limit(limit)
            .order_by_asc(heart_rate::Column::Time)
            .all(&self.db)
            .await?;

        let readings = rows
            .into_iter()
            .filter_map(|m| {
                let json = m.sensor_data?;
                let sd: SensorData = serde_json::from_value(json).ok()?;
                Some(TempReading {
                    time: m.time,
                    skin_temp_raw: sd.skin_temp_raw,
                })
            })
            .collect();

        Ok(readings)
    }

    pub async fn update_skin_temp_on_reading(&self, score: SkinTempScore) -> anyhow::Result<()> {
        let model = heart_rate::ActiveModel {
            id: NotSet,
            bpm: NotSet,
            time: Unchanged(score.time),
            rr_intervals: NotSet,
            activity: NotSet,
            stress: NotSet,
            spo2: NotSet,
            skin_temp: Set(Some(score.temp_celsius)),
            imu_data: NotSet,
            sensor_data: NotSet,
            synced: NotSet,
        };

        heart_rate::Entity::update_many()
            .filter(heart_rate::Column::Time.eq(score.time))
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
    async fn last_skin_temp_time_empty() {
        let db = DatabaseHandler::new("sqlite::memory:").await;
        let result = db.last_skin_temp_time().await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn search_temp_readings_empty() {
        let db = DatabaseHandler::new("sqlite::memory:").await;
        let readings = db
            .search_temp_readings(SearchHistory::default())
            .await
            .unwrap();
        assert!(readings.is_empty());
    }

    #[tokio::test]
    async fn update_skin_temp_on_reading_integration() {
        let db = DatabaseHandler::new("sqlite::memory:").await;

        let sensor = openwhoop_codec::SensorData {
            ppg_green: 100,
            ppg_red_ir: 200,
            spo2_red: 3000,
            spo2_ir: 4000,
            skin_temp_raw: 850,
            ambient_light: 50,
            led_drive_1: 10,
            led_drive_2: 20,
            resp_rate_raw: 0,
            signal_quality: 0,
            skin_contact: 1,
            accel_gravity: [0.0, 0.0, 1.0],
        };

        let reading = openwhoop_codec::HistoryReading {
            unix: 1735689600000,
            bpm: 72,
            rr: vec![833],
            imu_data: vec![],
            sensor_data: Some(sensor),
        };
        db.create_reading(reading).await.unwrap();

        // Search temp readings (should find one without skin_temp)
        let readings = db
            .search_temp_readings(SearchHistory::default())
            .await
            .unwrap();
        assert_eq!(readings.len(), 1);
        assert_eq!(readings[0].skin_temp_raw, 850);

        let time = readings[0].time;

        // Update skin temp
        let score = SkinTempScore {
            time,
            temp_celsius: 34.0,
        };
        db.update_skin_temp_on_reading(score).await.unwrap();

        // Verify skin_temp was set
        let last = db.last_skin_temp_time().await.unwrap();
        assert!(last.is_some());
        assert_eq!(last.unwrap(), time);

        // Search again - should be empty since skin_temp is now set
        let readings = db
            .search_temp_readings(SearchHistory::default())
            .await
            .unwrap();
        assert!(readings.is_empty());
    }
}
