use crate::db::Db;
use crate::state::AppState;
use crate::types::{AlertSettings, RankedRoute};
use anyhow::Result;
use chrono::{Duration, Utc};
use tauri::AppHandle;

pub async fn should_fire(
    db: &Db,
    user_id: &str,
    settings: &AlertSettings,
    r: &RankedRoute,
) -> Result<bool> {
    if !settings.desktop_enabled && settings.webhook_url.is_none() {
        return Ok(false);
    }
    if r.cr_per_hour < settings.min_cr_per_hour {
        return Ok(false);
    }
    let worst_ppt = r.legs.iter().map(|l| l.profit_per_ton).min().unwrap_or(0);
    if worst_ppt < settings.min_profit_per_ton {
        return Ok(false);
    }
    let total_distance: f64 = r.legs.iter().map(|l| l.distance_ly).sum();
    if total_distance > settings.max_distance_ly {
        return Ok(false);
    }

    let cutoff = (Utc::now() - Duration::minutes(settings.cooldown_minutes as i64)).to_rfc3339();
    let recent: Option<i64> = match db {
        Db::Sqlite(p) => sqlx::query_scalar(
            "SELECT id FROM alert_log WHERE user_id = ? AND route_hash = ? AND fired_at > ? ORDER BY fired_at DESC LIMIT 1",
        )
        .bind(user_id)
        .bind(&r.route_hash)
        .bind(&cutoff)
        .fetch_optional(p)
        .await?,
        Db::Postgres(p) => sqlx::query_scalar::<_, i64>(
            "SELECT id FROM alert_log WHERE user_id = $1 AND route_hash = $2 AND fired_at > $3::timestamptz ORDER BY fired_at DESC LIMIT 1",
        )
        .bind(user_id)
        .bind(&r.route_hash)
        .bind(&cutoff)
        .fetch_optional(p)
        .await?,
    };
    Ok(recent.is_none())
}

pub async fn record_fire(
    db: &Db,
    user_id: &str,
    r: &RankedRoute,
    channel: &str,
) -> Result<()> {
    let worst_ppt = r.legs.iter().map(|l| l.profit_per_ton).min().unwrap_or(0);
    let now = Utc::now();
    match db {
        Db::Sqlite(p) => {
            sqlx::query(
                "INSERT INTO alert_log (user_id, route_hash, profit_per_ton, fired_at, channel) VALUES (?, ?, ?, ?, ?)",
            )
            .bind(user_id)
            .bind(&r.route_hash)
            .bind(worst_ppt)
            .bind(now.to_rfc3339())
            .bind(channel)
            .execute(p)
            .await?;
        }
        Db::Postgres(p) => {
            sqlx::query(
                "INSERT INTO alert_log (user_id, route_hash, profit_per_ton, fired_at, channel) VALUES ($1, $2, $3, $4, $5)",
            )
            .bind(user_id)
            .bind(&r.route_hash)
            .bind(worst_ppt)
            .bind(now)
            .bind(channel)
            .execute(p)
            .await?;
        }
    }
    Ok(())
}

pub async fn dispatch(app: &AppHandle, state: &AppState, route: &RankedRoute) {
    let settings = state.settings.read().await.alerts.clone();
    let user_id = state.user_id.as_str();
    match should_fire(&state.db, user_id, &settings, route).await {
        Ok(true) => {
            if settings.desktop_enabled {
                super::toast::fire(app, route);
                let _ = record_fire(&state.db, user_id, route, "toast").await;
            }
            if let Some(url) = &settings.webhook_url {
                if super::webhook::fire(url, route).await.is_ok() {
                    let _ = record_fire(&state.db, user_id, route, "webhook").await;
                }
            }
            crate::events::emit_alert(app, route);
        }
        Ok(false) => {}
        Err(e) => tracing::warn!(error = %e, "alert dispatch failed"),
    }
}
