use super::Db;
use anyhow::{Context, Result};

pub async fn run(db: &Db) -> Result<()> {
    match db {
        Db::Sqlite(pool) => {
            sqlx::migrate!("./migrations/sqlite")
                .run(pool)
                .await
                .context("running sqlite migrations")?;
        }
        Db::Postgres(pool) => {
            sqlx::migrate!("./migrations/postgres")
                .run(pool)
                .await
                .context("running postgres migrations")?;
        }
    }
    Ok(())
}
