pub mod db;
pub mod types;
pub mod state;
pub mod commands;
pub mod settings_store;
pub mod events;
pub mod ingest;
pub mod engine;
pub mod alerts;

use state::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Linux white-screen workarounds for WebKitGTK. Three layers:
    //
    // 1. Disable the DMA-BUF renderer — fixes most NVIDIA + some Mesa stacks
    //    that hit "EGL_BAD_PARAMETER" on dmabuf buffer negotiation.
    // 2. Disable GPU compositing as a further fallback for older/broken GL.
    // 3. On Wayland, force XWayland. KDE Plasma 6 + Mesa + WebKitGTK has
    //    known brokenness; X11 is the reliable fallback.
    //
    // Users can override any of these by exporting the var themselves before
    // launch (e.g. `GDK_BACKEND=wayland` to try native Wayland anyway).
    #[cfg(target_os = "linux")]
    {
        let set_if_unset = |k: &str, v: &str| {
            if std::env::var_os(k).is_none() {
                std::env::set_var(k, v);
            }
        };
        set_if_unset("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        set_if_unset("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
        let is_wayland = std::env::var("XDG_SESSION_TYPE")
            .map(|v| v.eq_ignore_ascii_case("wayland"))
            .unwrap_or(false)
            || std::env::var_os("WAYLAND_DISPLAY").is_some();
        if is_wayland {
            set_if_unset("GDK_BACKEND", "x11");
        }
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = bootstrap(&handle).await {
                    tracing::error!(error = %e, "bootstrap failed");
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_top_routes,
            commands::get_user_state,
            commands::get_settings,
            commands::set_settings,
            commands::manual_override_ship,
            commands::force_prune,
            commands::get_diagnostics,
            commands::download_spansh_galaxy,
            commands::test_database_url,
            commands::debug_route_pipeline,
            commands::import_spansh_markets,
            commands::rebuild_latest_market,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

async fn bootstrap(app: &tauri::AppHandle) -> anyhow::Result<()> {
    let app_data_dir = app.path().app_data_dir()?;
    std::fs::create_dir_all(&app_data_dir)?;

    let user_id = db::user_id::load_or_create(&app_data_dir)?;
    tracing::info!(%user_id, "loaded per-install user id");

    // Resolution order for the DB URL:
    //   1. DATABASE_URL env var (dev / explicit override)
    //   2. settings.data_sources.database_url (UI-configured)
    //   3. default SQLite file in app data dir
    // We need settings from SQLite first to resolve step 2, so we always
    // read the SQLite-backed settings file even if the user has opted in to
    // Postgres — that way settings persist across backends.
    let sqlite_url = db::default_sqlite_url(&app_data_dir);
    let cfg_settings = match db::connect(&sqlite_url).await {
        Ok(cfg_db) => {
            let _ = db::migrations::run(&cfg_db).await;
            settings_store::load(&cfg_db, &user_id).await.unwrap_or_else(|_| types::Settings {
                score_weights: Default::default(),
                alerts: Default::default(),
                data_sources: Default::default(),
            })
        }
        Err(_) => types::Settings {
            score_weights: Default::default(),
            alerts: Default::default(),
            data_sources: Default::default(),
        },
    };

    let url = std::env::var("DATABASE_URL")
        .ok()
        .or_else(|| cfg_settings.data_sources.database_url.clone())
        .unwrap_or_else(|| sqlite_url.clone());
    tracing::info!(%url, "connecting db");

    let dbh = db::connect(&url).await?;
    db::migrations::run(&dbh).await?;
    db::seed::commodities(&dbh).await?;
    if let Err(e) = db::cleanup::dedupe_systems(&dbh).await {
        tracing::warn!(error = %format!("{:#}", e), "dedupe_systems failed");
    }
    if let Err(e) = db::cleanup::fix_fleet_carrier_flags(&dbh).await {
        tracing::warn!(error = %format!("{:#}", e), "fix_fleet_carrier_flags failed");
    }

    let state = AppState::new(dbh.clone(), user_id.clone());
    let loaded = settings_store::load(&dbh, &user_id).await.unwrap_or(cfg_settings);
    *state.settings.write().await = loaded;

    // If we have snapshots but the materialized cache is empty, rebuild it
    // in the background. This handles the case where migration 0003's
    // INSERT-SELECT timed out on a remote DB, leaving latest_market empty.
    {
        let db_for_task = dbh.clone();
        tokio::spawn(async move {
            let should_rebuild = match &db_for_task {
                db::Db::Sqlite(p) => {
                    let (lm,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM latest_market")
                        .fetch_one(p).await.unwrap_or((0,));
                    let (snaps,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM market_snapshots")
                        .fetch_one(p).await.unwrap_or((0,));
                    lm == 0 && snaps > 0
                }
                db::Db::Postgres(p) => {
                    let (lm,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM latest_market")
                        .fetch_one(p).await.unwrap_or((0,));
                    let (snaps,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM market_snapshots")
                        .fetch_one(p).await.unwrap_or((0,));
                    lm == 0 && snaps > 0
                }
            };
            if should_rebuild {
                tracing::info!("latest_market is empty but history exists — rebuilding in background");
                if let Err(e) = ingest::ingestor::rebuild_latest_market(&db_for_task).await {
                    tracing::warn!(error = %format!("{:#}", e), "background rebuild_latest_market failed");
                } else {
                    tracing::info!("background latest_market rebuild complete");
                }
            }
        });
    }

    app.manage(state.clone());

    db::retention::spawn_hourly(dbh.clone());

    let ds = state.settings.read().await.data_sources.clone();
    let journal_dir = ds
        .journal_dir
        .map(std::path::PathBuf::from)
        .unwrap_or_else(ingest::journal::default_journal_dir);
    ingest::journal::spawn_watcher(app.clone(), state.clone(), journal_dir).await?;

    let (eddn_tx, eddn_rx) = tokio::sync::mpsc::channel::<ingest::eddn::CommodityMsg>(1024);
    let relay = state.settings.read().await.data_sources.eddn_relay_url.clone();
    ingest::eddn::spawn(state.clone(), relay, eddn_tx);
    ingest::ingestor::spawn_forwarder(app.clone(), state.clone(), eddn_rx);

    engine::orchestrator::spawn_periodic(app.clone(), state.clone());

    Ok(())
}
