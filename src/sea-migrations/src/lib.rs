pub use sea_orm_migration::prelude::*;

mod m20220101_000001_create_packets;
pub mod m20250111_123805_heart_rate;
mod m20250126_200014_alter_heart_rate;
pub mod m20250127_195808_sleep_cycles;
mod m20250202_085524_activities;
mod m20250214_202918_alter_heart_rate_stress;
mod m20250402_092421_alter_sleep;
mod m20250527_094353_imu_data;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20220101_000001_create_packets::Migration),
            Box::new(m20250111_123805_heart_rate::Migration),
            Box::new(m20250126_200014_alter_heart_rate::Migration),
            Box::new(m20250127_195808_sleep_cycles::Migration),
            Box::new(m20250202_085524_activities::Migration),
            Box::new(m20250214_202918_alter_heart_rate_stress::Migration),
            Box::new(m20250402_092421_alter_sleep::Migration),
            Box::new(m20250527_094353_imu_data::Migration),
        ]
    }
}
