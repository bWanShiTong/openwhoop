use sea_orm_migration::prelude::*;

use crate::m20250111_123805_heart_rate::HeartRate;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(HeartRate::Table)
                    .add_column(ColumnDef::new(HeartRate::ImuData).json().null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(HeartRate::Table)
                    .drop_column(HeartRate::ImuData)
                    .to_owned(),
            )
            .await
    }
}
