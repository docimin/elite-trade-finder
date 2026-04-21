use super::Db;
use anyhow::Result;
use chrono::{Duration, Utc};

pub const RETENTION_DAYS: i64 = 7;

pub struct Deleted {
    pub snapshots: u64,
    pub alerts: u64,
}

pub async fn prune_once(db: &Db) -> Result<Deleted> {
    let cutoff = (Utc::now() - Duration::days(RETENTION_DAYS)).to_rfc3339();
    match db {
        Db::Sqlite(pool) => {
            let s = sqlx::query("DELETE FROM market_snapshots WHERE recorded_at < ?")
                .bind(&cutoff)
                .execute(pool)
                .await?;
            let a = sqlx::query("DELETE FROM alert_log WHERE fired_at < ?")
                .bind(&cutoff)
                .execute(pool)
                .await?;
            Ok(Deleted {
                snapshots: s.rows_affected(),
                alerts: a.rows_affected(),
            })
        }
        Db::Postgres(pool) => {
            let s = sqlx::query(
                "DELETE FROM market_snapshots WHERE recorded_at < NOW() - INTERVAL '7 days'",
            )
            .execute(pool)
            .await?;
            let a = sqlx::query("DELETE FROM alert_log WHERE fired_at < NOW() - INTERVAL '7 days'")
                .execute(pool)
                .await?;
            Ok(Deleted {
                snapshots: s.rows_affected(),
                alerts: a.rows_affected(),
            })
        }
    }
}

pub fn spawn_hourly(db: Db) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600));
        interval.tick().await;
        loop {
            interval.tick().await;
            match prune_once(&db).await {
                Ok(d) => tracing::info!(
                    snapshots = d.snapshots,
                    alerts = d.alerts,
                    "pruned"
                ),
                Err(e) => tracing::error!(error = %e, "prune failed"),
            }
        }
    })
}
