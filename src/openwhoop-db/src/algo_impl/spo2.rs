use crate::DatabaseHandler;
use crate::SearchHistory;

use chrono::NaiveDateTime;
use openwhoop_algos::{SpO2Reading, SpO2Score};
use openwhoop_codec::SensorData;
use openwhoop_entities::heart_rate;
use sea_orm::{
    ActiveValue::NotSet, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
    SelectColumns, Set, Unchanged,
};

impl DatabaseHandler {
    pub async fn last_spo2_time(&self) -> anyhow::Result<Option<NaiveDateTime>> {
        let reading = heart_rate::Entity::find()
            .filter(heart_rate::Column::Spo2.is_not_null())
            .order_by_desc(heart_rate::Column::Time)
            .select_only()
            .select_column(heart_rate::Column::Time)
            .into_tuple()
            .one(&self.db)
            .await?;

        Ok(reading)
    }

    pub async fn search_sensor_readings(
        &self,
        options: SearchHistory,
    ) -> anyhow::Result<Vec<SpO2Reading>> {
        let limit = options.limit;
        let rows = heart_rate::Entity::find()
            .filter(options.conditions())
            .filter(heart_rate::Column::SensorData.is_not_null())
            .limit(limit)
            .order_by_asc(heart_rate::Column::Time)
            .all(&self.db)
            .await?;

        let readings = rows
            .into_iter()
            .filter_map(|m| {
                let json = m.sensor_data?;
                let sd: SensorData = serde_json::from_value(json).ok()?;
                Some(SpO2Reading {
                    time: m.time,
                    spo2_red: sd.spo2_red,
                    spo2_ir: sd.spo2_ir,
                })
            })
            .collect();

        Ok(readings)
    }

    pub async fn update_spo2_on_reading(&self, score: SpO2Score) -> anyhow::Result<()> {
        let model = heart_rate::ActiveModel {
            id: NotSet,
            bpm: NotSet,
            time: Unchanged(score.time),
            rr_intervals: NotSet,
            activity: NotSet,
            stress: NotSet,
            spo2: Set(Some(score.spo2_percentage)),
            skin_temp: NotSet,
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
    async fn last_spo2_time_empty() {
        let db = DatabaseHandler::new("sqlite::memory:").await;
        let result = db.last_spo2_time().await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn update_spo2_on_reading_integration() {
        let db = DatabaseHandler::new("sqlite::memory:").await;

        let sensor = openwhoop_codec::SensorData {
            ppg_green: 100,
            ppg_red_ir: 200,
            spo2_red: 3000,
            spo2_ir: 4000,
            skin_temp_raw: 500,
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

        // Search sensor readings
        let readings = db
            .search_sensor_readings(crate::SearchHistory::default())
            .await
            .unwrap();
        assert_eq!(readings.len(), 1);
        assert_eq!(readings[0].spo2_red, 3000);
        assert_eq!(readings[0].spo2_ir, 4000);

        let time = readings[0].time;

        // Update spo2
        let score = SpO2Score {
            time,
            spo2_percentage: 97.5,
        };
        db.update_spo2_on_reading(score).await.unwrap();

        // Verify spo2 was set
        let last_spo2 = db.last_spo2_time().await.unwrap();
        assert!(last_spo2.is_some());
        assert_eq!(last_spo2.unwrap(), time);
    }
}
