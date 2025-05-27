use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(HeartRate::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(HeartRate::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    // Sqlite and sea orm doesn't have `u8`
                    .col(ColumnDef::new(HeartRate::Bpm).small_integer().not_null())
                    .col(
                        ColumnDef::new(HeartRate::Time)
                            .date_time()
                            .not_null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(HeartRate::RrIntervals).text().not_null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(HeartRate::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
pub enum HeartRate {
    Table,
    Id,
    Bpm,
    Time,
    RrIntervals,
    ImuData,
}
