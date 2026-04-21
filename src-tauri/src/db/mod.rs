use anyhow::{Context, Result};
use sqlx::{Pool, Postgres, Sqlite};
use std::path::Path;

pub mod migrations;
pub mod seed;
pub mod retention;
pub mod cleanup;
pub mod user_id;

#[derive(Clone)]
pub enum Db {
    Sqlite(Pool<Sqlite>),
    Postgres(Pool<Postgres>),
}

impl Db {
    pub fn dialect(&self) -> &'static str {
        match self {
            Db::Sqlite(_) => "sqlite",
            Db::Postgres(_) => "postgres",
        }
    }
}

pub async fn connect(url: &str) -> Result<Db> {
    if url.starts_with("postgres://") || url.starts_with("postgresql://") {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(8)
            .connect(url)
            .await
            .context("connecting to postgres")?;
        Ok(Db::Postgres(pool))
    } else if url.starts_with("sqlite://") || url.starts_with("sqlite:") {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(4)
            .connect(url)
            .await
            .context("connecting to sqlite")?;
        Ok(Db::Sqlite(pool))
    } else {
        anyhow::bail!("unrecognized DATABASE_URL scheme: {}", url)
    }
}

pub fn default_sqlite_url(app_data_dir: &Path) -> String {
    let path = app_data_dir.join("elite-trade-finder.sqlite");
    format!(
        "sqlite://{}?mode=rwc",
        path.display().to_string().replace('\\', "/")
    )
}
