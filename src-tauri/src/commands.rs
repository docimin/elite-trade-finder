use crate::db::retention;
use crate::db::Db;
use crate::ingest::spansh;
use crate::settings_store;
use crate::state::AppState;
use crate::types::*;
use tauri::{Manager, State};

#[tauri::command]
pub async fn get_top_routes(
    filter: RouteFilter,
    state: State<'_, AppState>,
) -> Result<Vec<RankedRoute>, String> {
    let all = state.top_routes.read().await.clone();
    let limit = filter.limit.max(1) as usize;
    Ok(all
        .into_iter()
        .filter(|r| {
            filter
                .modes
                .as_ref()
                .map_or(true, |m| m.contains(&r.mode))
        })
        .filter(|r| {
            filter
                .min_cr_per_hour
                .map_or(true, |min| r.cr_per_hour >= min)
        })
        .filter(|r| filter.max_jumps.map_or(true, |mx| r.total_jumps <= mx))
        .filter(|r| !filter.require_fleet_carrier || r.touches_fleet_carrier)
        .filter(|r| !filter.exclude_fleet_carrier || !r.touches_fleet_carrier)
        .filter(|r| {
            filter.max_profit_per_ton.map_or(true, |cap| {
                r.legs.iter().all(|l| l.profit_per_ton <= cap)
            })
        })
        .take(limit)
        .collect())
}

#[tauri::command]
pub async fn get_user_state(state: State<'_, AppState>) -> Result<UserState, String> {
    Ok(state.user_state.read().await.clone())
}

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<Settings, String> {
    Ok(state.settings.read().await.clone())
}

#[tauri::command]
pub async fn set_settings(
    new_settings: Settings,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    settings_store::save(&state.db, &state.user_id, &new_settings)
        .await
        .map_err(|e| e.to_string())?;

    // bootstrap() reads `database_url` from the LOCAL SQLite file so the user
    // can flip backends. If we only wrote to the current (Postgres) backend,
    // clicking "Use SQLite" would persist `null` to Postgres but leave the old
    // URL in SQLite — on next boot we'd dutifully reconnect to Postgres.
    // Mirror the full settings row to the local SQLite whenever the current
    // backend isn't SQLite, so it stays the canonical source for bootstrap.
    if !matches!(&state.db, crate::db::Db::Sqlite(_)) {
        let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
        let sqlite_url = crate::db::default_sqlite_url(&app_data_dir);
        match crate::db::connect(&sqlite_url).await {
            Ok(local_db) => {
                let _ = crate::db::migrations::run(&local_db).await;
                if let Err(e) =
                    settings_store::save(&local_db, &state.user_id, &new_settings).await
                {
                    tracing::warn!(
                        error = %format!("{:#}", e),
                        "failed to mirror settings to local SQLite; backend switch may not persist"
                    );
                }
            }
            Err(e) => {
                tracing::warn!(
                    error = %format!("{:#}", e),
                    "failed to open local SQLite for settings mirror"
                );
            }
        }
    }

    *state.settings.write().await = new_settings;
    Ok(())
}

#[tauri::command]
pub async fn manual_override_ship(
    ship: Option<ShipSpec>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    *state.override_ship.write().await = ship;
    Ok(())
}

#[tauri::command]
pub async fn force_prune(state: State<'_, AppState>) -> Result<(i64, i64), String> {
    let d = retention::prune_once(&state.db)
        .await
        .map_err(|e| e.to_string())?;
    Ok((d.snapshots as i64, d.alerts as i64))
}

#[tauri::command]
pub async fn rebuild_latest_market(
    state: State<'_, AppState>,
) -> Result<i64, String> {
    crate::ingest::ingestor::rebuild_latest_market(&state.db)
        .await
        .map_err(|e| format!("{:#}", e))?;
    let n: i64 = match &state.db {
        Db::Sqlite(p) => sqlx::query_scalar("SELECT COUNT(*) FROM latest_market")
            .fetch_one(p)
            .await
            .map_err(|e| format!("{:#}", e))?,
        Db::Postgres(p) => sqlx::query_scalar("SELECT COUNT(*) FROM latest_market")
            .fetch_one(p)
            .await
            .map_err(|e| format!("{:#}", e))?,
    };
    Ok(n)
}

#[tauri::command]
pub async fn debug_route_pipeline(state: State<'_, AppState>) -> Result<String, String> {
    let mut report = String::new();
    let db = &state.db;

    let user_id = state.user_id.as_str();
    report.push_str(&format!("user_id: {}\n", user_id));

    // user_state
    let us_row: Option<(
        Option<String>,
        Option<String>,
        Option<i32>,
        Option<f64>,
        Option<String>,
    )> = match db {
        Db::Sqlite(p) => sqlx::query_as(
            "SELECT current_system, current_station, cargo_capacity, jump_range_ly, pad_size_max FROM user_state WHERE user_id = ?",
        )
        .bind(user_id)
        .fetch_optional(p)
        .await
        .map_err(|e| format!("{:#}", e))?,
        Db::Postgres(p) => sqlx::query_as(
            "SELECT current_system, current_station, cargo_capacity, jump_range_ly, pad_size_max FROM user_state WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_optional(p)
        .await
        .map_err(|e| format!("{:#}", e))?,
    };
    report.push_str(&format!("user_state: {:?}\n", us_row));

    let counts: Vec<(&str, String)> = {
        let mut v = Vec::<(&str, String)>::new();
        for (label, sql) in [
            ("stations", "SELECT COUNT(*) FROM stations"),
            ("stations_with_pad", "SELECT COUNT(*) FROM stations WHERE pad_size IS NOT NULL"),
            ("systems", "SELECT COUNT(*) FROM systems"),
            ("systems_with_coords", "SELECT COUNT(*) FROM systems WHERE x IS NOT NULL"),
            ("commodities", "SELECT COUNT(*) FROM commodities"),
            ("market_snapshots", "SELECT COUNT(*) FROM market_snapshots"),
            ("latest_market", "SELECT COUNT(*) FROM latest_market"),
            ("latest_buy_rows", "SELECT COUNT(*) FROM latest_market WHERE buy_price > 0 AND supply > 0"),
            ("latest_sell_rows", "SELECT COUNT(*) FROM latest_market WHERE sell_price > 0 AND demand > 0"),
            ("buy_candidates_history", "SELECT COUNT(*) FROM market_snapshots WHERE buy_price > 0 AND supply > 0"),
            ("sell_candidates_history", "SELECT COUNT(*) FROM market_snapshots WHERE sell_price > 0 AND demand > 0"),
        ] {
            let n: i64 = match db {
                Db::Sqlite(p) => sqlx::query_scalar(sql)
                    .fetch_one(p)
                    .await
                    .map_err(|e| format!("{:#}", e))?,
                Db::Postgres(p) => sqlx::query_scalar(sql)
                    .fetch_one(p)
                    .await
                    .map_err(|e| format!("{:#}", e))?,
            };
            v.push((label, n.to_string()));
        }
        v
    };
    for (k, v) in &counts {
        report.push_str(&format!("{}: {}\n", k, v));
    }

    // Profitable pairs without any user filters — does commodity arbitrage exist at all?
    // Runs on latest_market so this finishes in tens of milliseconds instead of
    // the multi-second DISTINCT ON scan of market_snapshots.
    let raw_pairs: i64 = match db {
        Db::Sqlite(p) => sqlx::query_scalar(
            "WITH b AS (SELECT commodity_id, buy_price FROM latest_market WHERE buy_price > 0 AND supply > 0), \
                  s AS (SELECT commodity_id, sell_price FROM latest_market WHERE sell_price > 0 AND demand > 0) \
             SELECT COUNT(*) FROM b JOIN s USING(commodity_id) WHERE s.sell_price > b.buy_price"
        ).fetch_one(p).await.map_err(|e| format!("{:#}", e))?,
        Db::Postgres(p) => sqlx::query_scalar(
            "WITH b AS (SELECT commodity_id, buy_price FROM latest_market WHERE buy_price > 0 AND supply > 0), \
                  s AS (SELECT commodity_id, sell_price FROM latest_market WHERE sell_price > 0 AND demand > 0) \
             SELECT COUNT(*) FROM b JOIN s USING(commodity_id) WHERE s.sell_price > b.buy_price"
        ).fetch_one(p).await.map_err(|e| format!("{:#}", e))?,
    };
    report.push_str(&format!("raw_profitable_pairs: {}\n", raw_pairs));

    let weights = state.settings.read().await.score_weights.clone();
    let ovr_guard = state.override_ship.read().await;
    let ovr = ovr_guard.as_ref();

    // Single-hop
    let singles = crate::engine::single_hop::find(db, user_id, &weights, 10, ovr)
        .await
        .map_err(|e| format!("single_hop error: {:#}", e))?;
    report.push_str(&format!("single_hop: {} routes\n", singles.len()));
    if let Some(r) = singles.first() {
        report.push_str(&format!(
            "  top: {} → {} / {} / {} cr/ton / {:.1}M cr/hr\n",
            r.legs[0].from_station,
            r.legs[0].to_station,
            r.legs[0].commodity,
            r.legs[0].profit_per_ton,
            r.cr_per_hour as f64 / 1_000_000.0
        ));
    }

    // How many distinct station-pairs exist in current single-hops?
    let pair_count: i64 = {
        use std::collections::HashSet;
        let mut set: HashSet<(String, String)> = HashSet::new();
        let wide = crate::engine::single_hop::find(db, user_id, &weights, 1000, ovr)
            .await
            .unwrap_or_default();
        for h in &wide {
            let leg = &h.legs[0];
            set.insert((leg.from_station.clone(), leg.to_station.clone()));
        }
        set.len() as i64
    };
    report.push_str(&format!("distinct from→to pairs in hops: {}\n", pair_count));

    // 2-leg loops
    match crate::engine::loops::find_two_leg(db, user_id, &weights, 10, ovr).await {
        Ok(v) => {
            report.push_str(&format!("loops_two_leg: {} routes\n", v.len()));
            if let Some(r) = v.first() {
                report.push_str(&format!(
                    "  top: {} ↔ {} · {:.1}M cr/hr\n",
                    r.legs[0].from_station,
                    r.legs[0].to_station,
                    r.cr_per_hour as f64 / 1_000_000.0
                ));
            } else {
                report.push_str(
                    "  no 2-leg loops — needs a station-pair where both A→B and B→A are profitable with available commodities\n",
                );
            }
        }
        Err(e) => report.push_str(&format!("loops_two_leg error: {:#}\n", e)),
    }

    // 3-4 leg loops
    match crate::engine::loops::find_multi_leg(db, user_id, &weights, 4, 10, ovr).await {
        Ok(v) => {
            report.push_str(&format!("loops_multi_leg: {} routes\n", v.len()));
            if v.is_empty() {
                report.push_str(
                    "  no 3-4 leg cycles in the single-hop graph. Cycles need 3+ unique station pairs forming a closed loop.\n",
                );
            }
        }
        Err(e) => report.push_str(&format!("loops_multi_leg error: {:#}\n", e)),
    }

    // Rare chains
    let rare_commodity_count: i64 = match db {
        Db::Sqlite(p) => sqlx::query_scalar(
            "SELECT COUNT(*) FROM commodities WHERE is_rare = 1",
        )
        .fetch_one(p)
        .await
        .map_err(|e| format!("{:#}", e))?,
        Db::Postgres(p) => sqlx::query_scalar(
            "SELECT COUNT(*) FROM commodities WHERE is_rare = TRUE",
        )
        .fetch_one(p)
        .await
        .map_err(|e| format!("{:#}", e))?,
    };
    report.push_str(&format!(
        "rare_commodities_flagged: {}\n",
        rare_commodity_count
    ));

    let rare_with_data: i64 = match db {
        Db::Sqlite(p) => sqlx::query_scalar(
            "SELECT COUNT(DISTINCT m.commodity_id) FROM market_snapshots m \
             JOIN commodities c ON c.commodity_id = m.commodity_id WHERE c.is_rare = 1",
        )
        .fetch_one(p)
        .await
        .map_err(|e| format!("{:#}", e))?,
        Db::Postgres(p) => sqlx::query_scalar(
            "SELECT COUNT(DISTINCT m.commodity_id) FROM market_snapshots m \
             JOIN commodities c ON c.commodity_id = m.commodity_id WHERE c.is_rare = TRUE",
        )
        .fetch_one(p)
        .await
        .map_err(|e| format!("{:#}", e))?,
    };
    report.push_str(&format!("rare_commodities_in_feed: {}\n", rare_with_data));

    match crate::engine::rare_chains::find(db, user_id, &weights, 10, ovr).await {
        Ok(v) => report.push_str(&format!("rare_chains: {} routes\n", v.len())),
        Err(e) => report.push_str(&format!("rare_chains error: {:#}\n", e)),
    }

    Ok(report)
}

#[tauri::command]
pub async fn import_spansh_markets(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<u64, String> {
    use crate::events;
    use std::sync::Mutex;
    use std::time::Instant;

    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let dest = dir.join("spansh-galaxy-stations.json.gz");

    let emit = |phase: SpanshPhase,
                done: i64,
                total: Option<i64>,
                imported: i64,
                msg: Option<String>| {
        events::emit_spansh(
            &app,
            &SpanshProgress {
                phase,
                bytes_done: done,
                bytes_total: total,
                systems_imported: imported,
                message: msg,
            },
        );
    };

    emit(
        SpanshPhase::Downloading,
        0,
        None,
        0,
        Some("Downloading galaxy_stations dump…".into()),
    );

    let last_dl = Mutex::new(Instant::now());
    let app_dl = app.clone();
    if let Err(e) = spansh::download_stations(&dest, |done, total| {
        let mut last = last_dl.lock().unwrap();
        if last.elapsed() >= std::time::Duration::from_millis(250) {
            *last = Instant::now();
            drop(last);
            events::emit_spansh(
                &app_dl,
                &SpanshProgress {
                    phase: SpanshPhase::Downloading,
                    bytes_done: done as i64,
                    bytes_total: total.map(|t| t as i64),
                    systems_imported: 0,
                    message: Some("Downloading galaxy_stations dump…".into()),
                },
            );
        }
    })
    .await
    {
        emit(SpanshPhase::Failed, 0, None, 0, Some(format!("{:#}", e)));
        return Err(format!("{:#}", e));
    }

    let dest_size = std::fs::metadata(&dest).ok().map(|m| m.len() as i64);
    emit(
        SpanshPhase::Importing,
        dest_size.unwrap_or(0),
        dest_size,
        0,
        Some("Parsing and importing markets…".into()),
    );

    let last_im = Mutex::new(Instant::now());
    let app_im = app.clone();
    let stats = spansh::import_stations_and_markets(&state.db, &dest, move |stations, snaps| {
        let mut last = last_im.lock().unwrap();
        if last.elapsed() >= std::time::Duration::from_millis(500) {
            *last = Instant::now();
            drop(last);
            events::emit_spansh(
                &app_im,
                &SpanshProgress {
                    phase: SpanshPhase::Importing,
                    bytes_done: dest_size.unwrap_or(0),
                    bytes_total: dest_size,
                    systems_imported: snaps as i64,
                    message: Some(format!(
                        "{} stations · {} market snapshots",
                        stations, snaps
                    )),
                },
            );
        }
    })
    .await
    .map_err(|e| {
        events::emit_spansh(
            &app,
            &SpanshProgress {
                phase: SpanshPhase::Failed,
                bytes_done: 0,
                bytes_total: None,
                systems_imported: 0,
                message: Some(format!("{:#}", e)),
            },
        );
        format!("{:#}", e)
    })?;

    emit(
        SpanshPhase::Importing,
        dest_size.unwrap_or(0),
        dest_size,
        stats.snapshots as i64,
        Some("Rebuilding latest-market cache…".into()),
    );
    if let Err(e) = crate::ingest::ingestor::rebuild_latest_market(&state.db).await {
        tracing::warn!(error = %format!("{:#}", e), "rebuild_latest_market failed");
    }

    emit(
        SpanshPhase::Done,
        dest_size.unwrap_or(0),
        dest_size,
        stats.snapshots as i64,
        Some(format!(
            "Imported {} stations and {} market snapshots",
            stats.stations, stats.snapshots
        )),
    );
    Ok(stats.snapshots)
}

#[tauri::command]
pub async fn test_database_url(url: String) -> Result<String, String> {
    let db = crate::db::connect(&url)
        .await
        .map_err(|e| format!("{:#}", e))?;
    crate::db::migrations::run(&db)
        .await
        .map_err(|e| format!("{:#}", e))?;
    Ok(format!("connected ({})", db.dialect()))
}

#[tauri::command]
pub async fn download_spansh_galaxy(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<u64, String> {
    use crate::events;
    use std::sync::Mutex;
    use std::time::Instant;

    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let dest = dir.join("spansh-galaxy.json.gz");

    let emit = |phase: SpanshPhase, done: i64, total: Option<i64>, imported: i64, msg: Option<String>| {
        events::emit_spansh(
            &app,
            &SpanshProgress {
                phase,
                bytes_done: done,
                bytes_total: total,
                systems_imported: imported,
                message: msg,
            },
        );
    };

    emit(SpanshPhase::Downloading, 0, None, 0, None);

    let last_emit_dl = Mutex::new(Instant::now());
    let app_dl = app.clone();
    let dl_res = spansh::download(&dest, |done, total| {
        let mut last = last_emit_dl.lock().unwrap();
        if last.elapsed() >= std::time::Duration::from_millis(250) {
            *last = Instant::now();
            drop(last);
            events::emit_spansh(
                &app_dl,
                &SpanshProgress {
                    phase: SpanshPhase::Downloading,
                    bytes_done: done as i64,
                    bytes_total: total.map(|t| t as i64),
                    systems_imported: 0,
                    message: None,
                },
            );
        }
    })
    .await;

    if let Err(e) = dl_res {
        emit(SpanshPhase::Failed, 0, None, 0, Some(e.to_string()));
        return Err(e.to_string());
    }

    let dest_size = std::fs::metadata(&dest).ok().map(|m| m.len() as i64);
    emit(SpanshPhase::Importing, dest_size.unwrap_or(0), dest_size, 0, None);

    let last_emit_im = Mutex::new(Instant::now());
    let app_im = app.clone();
    let import_res = spansh::import_into_systems(&state.db, &dest, |imported| {
        let mut last = last_emit_im.lock().unwrap();
        if last.elapsed() >= std::time::Duration::from_millis(500) {
            *last = Instant::now();
            drop(last);
            events::emit_spansh(
                &app_im,
                &SpanshProgress {
                    phase: SpanshPhase::Importing,
                    bytes_done: dest_size.unwrap_or(0),
                    bytes_total: dest_size,
                    systems_imported: imported as i64,
                    message: None,
                },
            );
        }
    })
    .await;

    let n = match import_res {
        Ok(n) => n,
        Err(e) => {
            emit(SpanshPhase::Failed, 0, None, 0, Some(e.to_string()));
            return Err(e.to_string());
        }
    };

    {
        let mut s = state.settings.write().await;
        s.data_sources.spansh_galaxy_downloaded = true;
        settings_store::save(&state.db, &state.user_id, &s)
            .await
            .map_err(|e| e.to_string())?;
    }

    emit(SpanshPhase::Done, dest_size.unwrap_or(0), dest_size, n as i64, None);
    Ok(n)
}

#[tauri::command]
pub async fn get_diagnostics(state: State<'_, AppState>) -> Result<Diagnostics, String> {
    let eddn = state.eddn_status.read().await.clone();
    let journal_status = state.journal_status.read().await.clone();
    let (snap_count, oldest, newest) = match &state.db {
        Db::Sqlite(p) => {
            let (c,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM market_snapshots")
                .fetch_one(p)
                .await
                .map_err(|e| e.to_string())?;
            let oldest: Option<String> =
                sqlx::query_scalar("SELECT MIN(recorded_at) FROM market_snapshots")
                    .fetch_one(p)
                    .await
                    .ok()
                    .flatten();
            let newest: Option<String> =
                sqlx::query_scalar("SELECT MAX(recorded_at) FROM market_snapshots")
                    .fetch_one(p)
                    .await
                    .ok()
                    .flatten();
            (
                c,
                oldest
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                    .map(|d| d.with_timezone(&chrono::Utc)),
                newest
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                    .map(|d| d.with_timezone(&chrono::Utc)),
            )
        }
        Db::Postgres(p) => {
            let (c,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM market_snapshots")
                .fetch_one(p)
                .await
                .map_err(|e| e.to_string())?;
            let oldest: Option<chrono::DateTime<chrono::Utc>> =
                sqlx::query_scalar("SELECT MIN(recorded_at) FROM market_snapshots")
                    .fetch_one(p)
                    .await
                    .ok()
                    .flatten();
            let newest: Option<chrono::DateTime<chrono::Utc>> =
                sqlx::query_scalar("SELECT MAX(recorded_at) FROM market_snapshots")
                    .fetch_one(p)
                    .await
                    .ok()
                    .flatten();
            (c, oldest, newest)
        }
    };
    Ok(Diagnostics {
        db_dialect: state.db.dialect().into(),
        db_bytes: 0,
        snapshot_count: snap_count,
        oldest_snapshot: oldest,
        newest_snapshot: newest,
        eddn_connected: eddn.connected,
        eddn_msgs_per_sec: eddn.msgs_per_sec,
        eddn_last_msg_at: eddn.last_msg_at,
        journal_status,
    })
}
