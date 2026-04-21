use crate::db::Db;
use crate::types::Settings;
use anyhow::Result;

pub async fn load(db: &Db, user_id: &str) -> Result<Settings> {
    let raw: Option<String> = match db {
        Db::Sqlite(p) => sqlx::query_scalar(
            "SELECT value FROM settings WHERE user_id = ? AND key = 'app'",
        )
        .bind(user_id)
        .fetch_optional(p)
        .await?,
        Db::Postgres(p) => {
            sqlx::query_scalar::<_, serde_json::Value>(
                "SELECT value FROM settings WHERE user_id = $1 AND key = 'app'",
            )
            .bind(user_id)
            .fetch_optional(p)
            .await?
            .map(|v| v.to_string())
        }
    };
    match raw {
        Some(s) => Ok(serde_json::from_str(&s).unwrap_or_else(|_| Settings {
            score_weights: Default::default(),
            alerts: Default::default(),
            data_sources: Default::default(),
        })),
        None => Ok(Settings {
            score_weights: Default::default(),
            alerts: Default::default(),
            data_sources: Default::default(),
        }),
    }
}

pub async fn save(db: &Db, user_id: &str, s: &Settings) -> Result<()> {
    let json = serde_json::to_string(s)?;
    match db {
        Db::Sqlite(p) => {
            sqlx::query("INSERT INTO settings (user_id, key, value) VALUES (?, 'app', ?) ON CONFLICT(user_id, key) DO UPDATE SET value = excluded.value")
                .bind(user_id)
                .bind(&json).execute(p).await?;
        }
        Db::Postgres(p) => {
            let v: serde_json::Value = serde_json::from_str(&json)?;
            sqlx::query("INSERT INTO settings (user_id, key, value) VALUES ($1, 'app', $2) ON CONFLICT(user_id, key) DO UPDATE SET value = EXCLUDED.value")
                .bind(user_id)
                .bind(&v).execute(p).await?;
        }
    }
    Ok(())
}
