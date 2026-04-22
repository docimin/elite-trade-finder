use super::eddn::CommodityMsg;
use crate::db::Db;
use crate::events;
use crate::state::AppState;
use crate::types::FirehoseTick;
use anyhow::Result;
use once_cell::sync::Lazy;
use std::sync::atomic::{AtomicI64, Ordering};
use tauri::AppHandle;
use tokio::sync::mpsc;

static SYNTHETIC_ID_CURSOR: Lazy<AtomicI64> = Lazy::new(|| AtomicI64::new(900_000_000));

/// Seed the synthetic commodity_id cursor from the current DB max so we don't
/// re-issue IDs already assigned in a prior session and collide with the PK.
/// Call once on bootstrap after migrations + seed.
pub async fn init_synthetic_cursor(db: &Db) -> Result<()> {
    let max: i64 = match db {
        Db::Sqlite(p) => {
            sqlx::query_scalar::<_, Option<i64>>(
                "SELECT MAX(commodity_id) FROM commodities",
            )
            .fetch_one(p)
            .await?
            .unwrap_or(0)
        }
        Db::Postgres(p) => sqlx::query_scalar::<_, Option<i32>>(
            "SELECT MAX(commodity_id) FROM commodities",
        )
        .fetch_one(p)
        .await?
        .map(|n| n as i64)
        .unwrap_or(0),
    };
    let start = max.max(900_000_000) + 1;
    SYNTHETIC_ID_CURSOR.store(start, Ordering::SeqCst);
    tracing::info!(synthetic_cursor_start = start, "seeded synthetic commodity_id cursor");
    Ok(())
}

async fn resolve_commodity_id(db: &Db, symbol: &str) -> Result<i64> {
    match db {
        Db::Sqlite(p) => {
            if let Some(id) = sqlx::query_scalar::<_, i64>(
                "SELECT commodity_id FROM commodities WHERE symbol = ?",
            )
            .bind(symbol)
            .fetch_optional(p)
            .await?
            {
                return Ok(id);
            }
            let new_id = SYNTHETIC_ID_CURSOR.fetch_add(1, Ordering::SeqCst);
            sqlx::query("INSERT OR IGNORE INTO commodities (commodity_id, symbol, display_name) VALUES (?, ?, ?)")
                .bind(new_id).bind(symbol).bind(symbol)
                .execute(p).await?;
            match sqlx::query_scalar::<_, i64>(
                "SELECT commodity_id FROM commodities WHERE symbol = ?",
            )
            .bind(symbol)
            .fetch_optional(p)
            .await?
            {
                Some(id) => Ok(id),
                None => anyhow::bail!(
                    "failed to resolve commodity_id for symbol '{}' after insert",
                    symbol
                ),
            }
        }
        Db::Postgres(p) => {
            if let Some(id) = sqlx::query_scalar::<_, i32>(
                "SELECT commodity_id FROM commodities WHERE symbol = $1",
            )
            .bind(symbol)
            .fetch_optional(p)
            .await?
            {
                return Ok(id as i64);
            }
            let new_id = SYNTHETIC_ID_CURSOR.fetch_add(1, Ordering::SeqCst) as i32;
            sqlx::query("INSERT INTO commodities (commodity_id, symbol, display_name) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING")
                .bind(new_id).bind(symbol).bind(symbol)
                .execute(p).await?;
            match sqlx::query_scalar::<_, i32>(
                "SELECT commodity_id FROM commodities WHERE symbol = $1",
            )
            .bind(symbol)
            .fetch_optional(p)
            .await?
            {
                Some(id) => Ok(id as i64),
                None => anyhow::bail!(
                    "failed to resolve commodity_id for symbol '{}' after insert",
                    symbol
                ),
            }
        }
    }
}

pub async fn ingest_commodity(db: &Db, msg: &CommodityMsg) -> Result<usize> {
    let is_fc = msg
        .station_type
        .as_deref()
        .map(|t| {
            let l = t.to_lowercase();
            l.contains("carrier") || l == "fleetcarrier"
        })
        .unwrap_or(false);

    match db {
        Db::Sqlite(p) => {
            sqlx::query(
                "INSERT INTO stations (station_id, system_name, station_name, station_type, is_fleet_carrier, last_seen_at) VALUES (?, ?, ?, ?, ?, ?) \
                 ON CONFLICT(station_id) DO UPDATE SET last_seen_at = excluded.last_seen_at, station_type = COALESCE(stations.station_type, excluded.station_type)",
            )
            .bind(msg.market_id)
            .bind(&msg.system_name)
            .bind(&msg.station_name)
            .bind(msg.station_type.as_deref())
            .bind(is_fc as i32)
            .bind(msg.gateway_timestamp.to_rfc3339())
            .execute(p)
            .await?;
        }
        Db::Postgres(p) => {
            sqlx::query(
                "INSERT INTO stations (station_id, system_name, station_name, station_type, is_fleet_carrier, last_seen_at) VALUES ($1, $2, $3, $4, $5, $6) \
                 ON CONFLICT(station_id) DO UPDATE SET last_seen_at = EXCLUDED.last_seen_at, station_type = COALESCE(stations.station_type, EXCLUDED.station_type)",
            )
            .bind(msg.market_id)
            .bind(&msg.system_name)
            .bind(&msg.station_name)
            .bind(msg.station_type.as_deref())
            .bind(is_fc)
            .bind(msg.gateway_timestamp)
            .execute(p)
            .await?;
        }
    }

    let recorded_at = msg.timestamp.to_rfc3339();
    let mut inserted = 0usize;
    for row in &msg.commodities {
        let cid = resolve_commodity_id(db, &row.name).await?;
        let buy = if row.buy_price > 0 {
            Some(row.buy_price)
        } else {
            None
        };
        let sell = if row.sell_price > 0 {
            Some(row.sell_price)
        } else {
            None
        };
        let done = match db {
            Db::Sqlite(p) => sqlx::query(
                "INSERT OR IGNORE INTO market_snapshots (station_id, commodity_id, buy_price, sell_price, supply, demand, recorded_at, source) VALUES (?, ?, ?, ?, ?, ?, ?, 'eddn')",
            )
            .bind(msg.market_id)
            .bind(cid)
            .bind(buy)
            .bind(sell)
            .bind(row.stock)
            .bind(row.demand)
            .bind(&recorded_at)
            .execute(p)
            .await?
            .rows_affected(),
            Db::Postgres(p) => sqlx::query(
                "INSERT INTO market_snapshots (station_id, commodity_id, buy_price, sell_price, supply, demand, recorded_at, source) VALUES ($1, $2, $3, $4, $5, $6, $7, 'eddn') ON CONFLICT DO NOTHING",
            )
            .bind(msg.market_id)
            .bind(cid as i32)
            .bind(buy)
            .bind(sell)
            .bind(row.stock)
            .bind(row.demand)
            .bind(msg.timestamp)
            .execute(p)
            .await?
            .rows_affected(),
        };
        if done > 0 {
            inserted += 1;
        }
        // Keep the materialized cache up to date.
        upsert_latest(db, msg.market_id, cid, buy, sell, row.stock, row.demand, &msg.timestamp).await?;
    }
    Ok(inserted)
}

pub async fn upsert_latest(
    db: &Db,
    station_id: i64,
    commodity_id: i64,
    buy_price: Option<i32>,
    sell_price: Option<i32>,
    supply: i32,
    demand: i32,
    recorded_at: &chrono::DateTime<chrono::Utc>,
) -> Result<()> {
    match db {
        Db::Sqlite(p) => {
            sqlx::query(
                "INSERT INTO latest_market (station_id, commodity_id, buy_price, sell_price, supply, demand, recorded_at) \
                 VALUES (?, ?, ?, ?, ?, ?, ?) \
                 ON CONFLICT(station_id, commodity_id) DO UPDATE SET \
                   buy_price = excluded.buy_price, \
                   sell_price = excluded.sell_price, \
                   supply = excluded.supply, \
                   demand = excluded.demand, \
                   recorded_at = excluded.recorded_at \
                 WHERE excluded.recorded_at >= latest_market.recorded_at",
            )
            .bind(station_id)
            .bind(commodity_id)
            .bind(buy_price)
            .bind(sell_price)
            .bind(supply)
            .bind(demand)
            .bind(recorded_at.to_rfc3339())
            .execute(p)
            .await?;
        }
        Db::Postgres(p) => {
            sqlx::query(
                "INSERT INTO latest_market (station_id, commodity_id, buy_price, sell_price, supply, demand, recorded_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7) \
                 ON CONFLICT(station_id, commodity_id) DO UPDATE SET \
                   buy_price = EXCLUDED.buy_price, \
                   sell_price = EXCLUDED.sell_price, \
                   supply = EXCLUDED.supply, \
                   demand = EXCLUDED.demand, \
                   recorded_at = EXCLUDED.recorded_at \
                 WHERE EXCLUDED.recorded_at >= latest_market.recorded_at",
            )
            .bind(station_id)
            .bind(commodity_id as i32)
            .bind(buy_price)
            .bind(sell_price)
            .bind(supply)
            .bind(demand)
            .bind(*recorded_at)
            .execute(p)
            .await?;
        }
    }
    Ok(())
}

/// Rebuild `latest_market` from the full `market_snapshots` history. Cheap
/// because it's one big indexed insert; useful from tests and after bulk
/// imports.
pub async fn rebuild_latest_market(db: &Db) -> Result<()> {
    match db {
        Db::Sqlite(p) => {
            let mut tx = p.begin().await?;
            sqlx::query("DELETE FROM latest_market").execute(&mut *tx).await?;
            // ON CONFLICT tolerates the race where the EDDN ingestor writes a
            // row after our DELETE but before our INSERT. The WHERE clause
            // prefers whichever record has the newer recorded_at.
            sqlx::query(
                "INSERT INTO latest_market (station_id, commodity_id, buy_price, sell_price, supply, demand, recorded_at) \
                 SELECT m.station_id, m.commodity_id, m.buy_price, m.sell_price, m.supply, m.demand, m.recorded_at \
                 FROM market_snapshots m \
                 INNER JOIN ( \
                   SELECT station_id, commodity_id, MAX(recorded_at) AS max_rec \
                   FROM market_snapshots \
                   GROUP BY station_id, commodity_id \
                 ) l ON l.station_id = m.station_id AND l.commodity_id = m.commodity_id AND l.max_rec = m.recorded_at \
                 ON CONFLICT(station_id, commodity_id) DO UPDATE SET \
                   buy_price = excluded.buy_price, \
                   sell_price = excluded.sell_price, \
                   supply = excluded.supply, \
                   demand = excluded.demand, \
                   recorded_at = excluded.recorded_at \
                 WHERE excluded.recorded_at >= latest_market.recorded_at",
            )
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
            sqlx::query("ANALYZE").execute(p).await.ok();
        }
        Db::Postgres(p) => {
            let mut tx = p.begin().await?;
            sqlx::query("DELETE FROM latest_market").execute(&mut *tx).await?;
            sqlx::query(
                "INSERT INTO latest_market (station_id, commodity_id, buy_price, sell_price, supply, demand, recorded_at) \
                 SELECT DISTINCT ON (m.station_id, m.commodity_id) \
                        m.station_id, m.commodity_id, m.buy_price, m.sell_price, m.supply, m.demand, m.recorded_at \
                 FROM market_snapshots m \
                 ORDER BY m.station_id, m.commodity_id, m.recorded_at DESC \
                 ON CONFLICT(station_id, commodity_id) DO UPDATE SET \
                   buy_price = EXCLUDED.buy_price, \
                   sell_price = EXCLUDED.sell_price, \
                   supply = EXCLUDED.supply, \
                   demand = EXCLUDED.demand, \
                   recorded_at = EXCLUDED.recorded_at \
                 WHERE EXCLUDED.recorded_at >= latest_market.recorded_at",
            )
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
            sqlx::query("ANALYZE latest_market").execute(p).await.ok();
            sqlx::query("ANALYZE stations").execute(p).await.ok();
            sqlx::query("ANALYZE systems").execute(p).await.ok();
        }
    }
    Ok(())
}

pub fn spawn_forwarder(
    app: AppHandle,
    state: AppState,
    mut rx: mpsc::Receiver<CommodityMsg>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            match ingest_commodity(&state.db, &msg).await {
                Ok(n) => {
                    let tick = FirehoseTick {
                        at: chrono::Utc::now(),
                        system: msg.system_name.clone(),
                        station: msg.station_name.clone(),
                        commodities_updated: n as i32,
                    };
                    events::emit_firehose(&app, &tick);
                }
                Err(e) => tracing::warn!(error = %e, "ingest failed"),
            }
        }
    })
}
