use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use sea_orm_migration::prelude::*;

use crate::m20250202_085524_activities::Activities;

const OPEN_ACTIVITY_INDEX: &str = "activities-one-open-activity";

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Sqlite => migrate_sqlite_up(manager).await?,
            DatabaseBackend::Postgres => migrate_postgres_up(manager).await?,
            other => {
                return Err(DbErr::Migration(format!(
                    "unsupported database backend for activity nullable end migration: {other:?}"
                )));
            }
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Sqlite => migrate_sqlite_down(manager).await?,
            DatabaseBackend::Postgres => migrate_postgres_down(manager).await?,
            other => {
                return Err(DbErr::Migration(format!(
                    "unsupported database backend for activity nullable end migration: {other:?}"
                )));
            }
        }

        Ok(())
    }
}

async fn migrate_postgres_up(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .alter_table(
            Table::alter()
                .table(Activities::Table)
                .modify_column(ColumnDef::new(Activities::End).date_time().null())
                .to_owned(),
        )
        .await?;

    manager
        .get_connection()
        .execute(Statement::from_string(
            DatabaseBackend::Postgres,
            format!(
                "CREATE UNIQUE INDEX \"{OPEN_ACTIVITY_INDEX}\" ON \"activities\" ((1)) WHERE \"end\" IS NULL"
            ),
        ))
        .await?;

    Ok(())
}

async fn migrate_postgres_down(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute(Statement::from_string(
            DatabaseBackend::Postgres,
            format!("DROP INDEX IF EXISTS \"{OPEN_ACTIVITY_INDEX}\""),
        ))
        .await?;

    manager
        .get_connection()
        .execute(Statement::from_string(
            DatabaseBackend::Postgres,
            "UPDATE \"activities\" SET \"end\" = \"start\" WHERE \"end\" IS NULL".to_string(),
        ))
        .await?;

    manager
        .alter_table(
            Table::alter()
                .table(Activities::Table)
                .modify_column(ColumnDef::new(Activities::End).date_time().not_null())
                .to_owned(),
        )
        .await?;

    Ok(())
}

async fn migrate_sqlite_up(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let db = manager.get_connection();

    db.execute_unprepared("PRAGMA foreign_keys = OFF").await?;
    db.execute_unprepared(
        r#"
        CREATE TABLE "activities_new" (
            "id" integer NOT NULL PRIMARY KEY AUTOINCREMENT,
            "period_id" date NOT NULL,
            "start" text NOT NULL UNIQUE,
            "end" text NULL,
            "activity" varchar(64) NOT NULL,
            "strain" double NULL,
            "synced" boolean NOT NULL DEFAULT 0,
            CONSTRAINT "fk_activities_sleep_cycles"
                FOREIGN KEY ("period_id")
                REFERENCES "sleep_cycles" ("sleep_id")
                ON DELETE CASCADE
                ON UPDATE CASCADE
        )
        "#,
    )
    .await?;
    db.execute_unprepared(
        r#"
        INSERT INTO "activities_new" ("id", "period_id", "start", "end", "activity", "strain", "synced")
        SELECT "id", "period_id", "start", "end", "activity", "strain", "synced"
        FROM "activities"
        "#,
    )
    .await?;
    db.execute_unprepared(r#"DROP TABLE "activities""#).await?;
    db.execute_unprepared(r#"ALTER TABLE "activities_new" RENAME TO "activities""#)
        .await?;
    db.execute_unprepared(&format!(
        r#"CREATE UNIQUE INDEX "{OPEN_ACTIVITY_INDEX}" ON "activities" (1) WHERE "end" IS NULL"#
    ))
    .await?;
    db.execute_unprepared("PRAGMA foreign_keys = ON").await?;

    Ok(())
}

async fn migrate_sqlite_down(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let db = manager.get_connection();

    db.execute_unprepared("PRAGMA foreign_keys = OFF").await?;
    db.execute_unprepared(&format!(r#"DROP INDEX IF EXISTS "{OPEN_ACTIVITY_INDEX}""#))
        .await?;
    db.execute_unprepared(r#"UPDATE "activities" SET "end" = "start" WHERE "end" IS NULL"#)
        .await?;
    db.execute_unprepared(
        r#"
        CREATE TABLE "activities_old" (
            "id" integer NOT NULL PRIMARY KEY AUTOINCREMENT,
            "period_id" date NOT NULL,
            "start" text NOT NULL UNIQUE,
            "end" text NOT NULL,
            "activity" varchar(64) NOT NULL,
            "strain" double NULL,
            "synced" boolean NOT NULL DEFAULT 0,
            CONSTRAINT "fk_activities_sleep_cycles"
                FOREIGN KEY ("period_id")
                REFERENCES "sleep_cycles" ("sleep_id")
                ON DELETE CASCADE
                ON UPDATE CASCADE
        )
        "#,
    )
    .await?;
    db.execute_unprepared(
        r#"
        INSERT INTO "activities_old" ("id", "period_id", "start", "end", "activity", "strain", "synced")
        SELECT "id", "period_id", "start", "end", "activity", "strain", "synced"
        FROM "activities"
        "#,
    )
    .await?;
    db.execute_unprepared(r#"DROP TABLE "activities""#).await?;
    db.execute_unprepared(r#"ALTER TABLE "activities_old" RENAME TO "activities""#)
        .await?;
    db.execute_unprepared("PRAGMA foreign_keys = ON").await?;

    Ok(())
}
