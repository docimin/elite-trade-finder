use crate::state::AppState;
use crate::types::UserState;
use anyhow::Result;
use chrono::Utc;
use notify::{Event as NotifyEvent, EventKind, RecursiveMode, Watcher};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use tokio::io::{AsyncBufReadExt, AsyncSeekExt, BufReader, SeekFrom};

#[derive(Debug, Clone)]
pub enum JournalEvent {
    LoadGame { credits: i64 },
    Loadout { ship: String, cargo_capacity: i32, max_jump_range: f64 },
    Location { star_system: String, star_pos: Option<[f64; 3]>, station_name: Option<String> },
    FsdJump { star_system: String, star_pos: Option<[f64; 3]> },
    Docked { star_system: String, station_name: String, market_id: i64, station_type: Option<String>, max_pad_size: Option<String> },
    Market { market_id: i64 },
    Cargo { inventory: Vec<CargoItem> },
    Undocked,
    Other,
}

#[derive(Debug, Clone)]
pub struct CargoItem {
    pub symbol: String,
    pub count: i32,
}

#[derive(Debug, Clone)]
pub struct MarketFile {
    pub market_id: i64,
    pub star_system: String,
    pub station_name: String,
    pub timestamp: String,
    pub items: Vec<MarketItem>,
}

#[derive(Debug, Clone)]
pub struct MarketItem {
    pub commodity_id: i64,
    pub symbol: String,
    pub buy_price: Option<i32>,
    pub sell_price: Option<i32>,
    pub supply: i32,
    pub demand: i32,
}

#[derive(Debug, Deserialize)]
struct Raw {
    event: String,
    #[serde(rename = "Credits")]
    credits: Option<i64>,
    #[serde(rename = "Ship")]
    ship: Option<String>,
    #[serde(rename = "CargoCapacity")]
    cargo_capacity: Option<i32>,
    #[serde(rename = "MaxJumpRange")]
    max_jump_range: Option<f64>,
    #[serde(rename = "StarSystem")]
    star_system: Option<String>,
    #[serde(rename = "StarPos")]
    star_pos: Option<[f64; 3]>,
    #[serde(rename = "StationName")]
    station_name: Option<String>,
    #[serde(rename = "StationType")]
    station_type: Option<String>,
    #[serde(rename = "MarketID")]
    market_id: Option<i64>,
    #[serde(rename = "LandingPads")]
    landing_pads: Option<serde_json::Value>,
    #[serde(rename = "Inventory")]
    inventory: Option<Vec<RawCargo>>,
}

#[derive(Debug, Deserialize)]
struct RawCargo {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Count")]
    count: i32,
}

pub fn parse_line(line: &str) -> Option<JournalEvent> {
    if line.trim().is_empty() {
        return None;
    }
    let raw: Raw = serde_json::from_str(line).ok()?;
    Some(match raw.event.as_str() {
        "LoadGame" => JournalEvent::LoadGame {
            credits: raw.credits.unwrap_or(0),
        },
        "Loadout" => JournalEvent::Loadout {
            ship: raw.ship.unwrap_or_default(),
            cargo_capacity: raw.cargo_capacity.unwrap_or(0),
            max_jump_range: raw.max_jump_range.unwrap_or(0.0),
        },
        "Location" => JournalEvent::Location {
            star_system: raw.star_system.unwrap_or_default(),
            star_pos: raw.star_pos,
            station_name: raw.station_name,
        },
        "FSDJump" => JournalEvent::FsdJump {
            star_system: raw.star_system.unwrap_or_default(),
            star_pos: raw.star_pos,
        },
        "Docked" => JournalEvent::Docked {
            star_system: raw.star_system.unwrap_or_default(),
            station_name: raw.station_name.unwrap_or_default(),
            market_id: raw.market_id.unwrap_or(0),
            station_type: raw.station_type,
            max_pad_size: raw.landing_pads.as_ref().and_then(infer_max_pad),
        },
        "Market" => JournalEvent::Market {
            market_id: raw.market_id.unwrap_or(0),
        },
        "Cargo" => JournalEvent::Cargo {
            inventory: raw
                .inventory
                .unwrap_or_default()
                .into_iter()
                .map(|r| CargoItem {
                    symbol: r.name,
                    count: r.count,
                })
                .collect(),
        },
        "Undocked" => JournalEvent::Undocked,
        _ => JournalEvent::Other,
    })
}

fn infer_max_pad(pads: &serde_json::Value) -> Option<String> {
    let large = pads.get("Large").and_then(|v| v.as_i64()).unwrap_or(0);
    let medium = pads.get("Medium").and_then(|v| v.as_i64()).unwrap_or(0);
    let small = pads.get("Small").and_then(|v| v.as_i64()).unwrap_or(0);
    if large > 0 {
        Some("L".into())
    } else if medium > 0 {
        Some("M".into())
    } else if small > 0 {
        Some("S".into())
    } else {
        None
    }
}

pub fn parse_market_file(blob: &str) -> Result<MarketFile> {
    #[derive(Deserialize)]
    struct RawFile {
        timestamp: String,
        #[serde(rename = "MarketID")]
        market_id: i64,
        #[serde(rename = "StarSystem")]
        star_system: String,
        #[serde(rename = "StationName")]
        station_name: String,
        #[serde(rename = "Items")]
        items: Vec<RawItem>,
    }
    #[derive(Deserialize)]
    struct RawItem {
        id: i64,
        #[serde(rename = "Name")]
        name: String,
        #[serde(rename = "BuyPrice")]
        buy_price: i32,
        #[serde(rename = "SellPrice")]
        sell_price: i32,
        #[serde(rename = "Stock")]
        stock: i32,
        #[serde(rename = "Demand")]
        demand: i32,
    }
    let raw: RawFile = serde_json::from_str(blob)?;
    Ok(MarketFile {
        market_id: raw.market_id,
        star_system: raw.star_system,
        station_name: raw.station_name,
        timestamp: raw.timestamp,
        items: raw
            .items
            .into_iter()
            .map(|i| MarketItem {
                commodity_id: i.id,
                symbol: i
                    .name
                    .trim_start_matches('$')
                    .trim_end_matches(';')
                    .trim_end_matches("_name")
                    .to_string(),
                buy_price: if i.buy_price > 0 {
                    Some(i.buy_price)
                } else {
                    None
                },
                sell_price: if i.sell_price > 0 {
                    Some(i.sell_price)
                } else {
                    None
                },
                supply: i.stock,
                demand: i.demand,
            })
            .collect(),
    })
}

pub fn default_journal_dir() -> PathBuf {
    #[cfg(windows)]
    {
        if let Some(profile) = std::env::var_os("USERPROFILE") {
            return PathBuf::from(profile)
                .join("Saved Games")
                .join("Frontier Developments")
                .join("Elite Dangerous");
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home)
                .join("Library/Application Support/Frontier Developments/Elite Dangerous");
        }
    }
    #[cfg(target_os = "linux")]
    {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(".local/share/Frontier Developments/Elite Dangerous");
        }
    }
    PathBuf::from(".")
}

/// Maps the FDev internal ship name from the Loadout event to the pad size
/// the ship requires (smallest pad it fits on). This is what the route engine
/// uses to filter out stations that are too small for the current ship.
fn ship_required_pad(ship: &str) -> Option<String> {
    match ship.to_lowercase().as_str() {
        // Small pad
        "sidewinder" | "eagle" | "hauler" | "adder" | "empire_courier" | "empire_eagle"
        | "viper" | "viper_mkiv" | "cobramkiii" | "cobramkiv" | "cobra_mk_v"
        | "diamondback" | "dolphin" | "vulture" => Some("S".into()),
        // Medium pad
        "asp_scout" | "asp" | "diamondbackxl" | "federation_dropship"
        | "federation_dropship_mkii" | "federation_gunship" | "ferdelance"
        | "krait_mkii" | "krait_light" | "mamba" | "python" | "python_nx"
        | "type6" | "type7" | "typex" | "typex_2" | "typex_3"
        | "independant_trader" | "corsair" => Some("M".into()),
        // Large pad
        "anaconda" | "belugaliner" | "cutter" | "federation_corvette"
        | "type9" | "type9_military" | "orca" | "empire_trader" => Some("L".into()),
        _ => None,
    }
}

pub fn apply_event(s: &mut UserState, ev: &JournalEvent) {
    match ev {
        JournalEvent::LoadGame { credits } => {
            s.credits = Some(*credits);
        }
        JournalEvent::Loadout {
            ship,
            cargo_capacity,
            max_jump_range,
        } => {
            s.ship_type = Some(ship.clone());
            s.cargo_capacity = Some(*cargo_capacity);
            s.jump_range_ly = Some(*max_jump_range);
            // Pad is a property of the SHIP, not the last station.
            if let Some(pad) = ship_required_pad(ship) {
                s.pad_size_max = Some(pad);
            }
        }
        JournalEvent::Location {
            star_system,
            station_name,
            ..
        } => {
            s.current_system = Some(star_system.clone());
            s.current_station = station_name.clone();
        }
        JournalEvent::FsdJump { star_system, .. } => {
            s.current_system = Some(star_system.clone());
            s.current_station = None;
        }
        JournalEvent::Docked {
            star_system,
            station_name,
            ..
        } => {
            s.current_system = Some(star_system.clone());
            s.current_station = Some(station_name.clone());
            // Deliberately do NOT set pad_size_max from the station
        }
        JournalEvent::Undocked => {
            s.current_station = None;
        }
        _ => {}
    }
    s.updated_at = Utc::now();
}

fn newest_journal(dir: &Path) -> Option<PathBuf> {
    let rd = std::fs::read_dir(dir).ok()?;
    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    for entry in rd.flatten() {
        let p = entry.path();
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if !name.starts_with("Journal.") || !name.ends_with(".log") {
            continue;
        }
        let meta = match entry.metadata() {
            Ok(m) => m,
            _ => continue,
        };
        let mt = match meta.modified() {
            Ok(t) => t,
            _ => continue,
        };
        if best.as_ref().map_or(true, |(bt, _)| mt > *bt) {
            best = Some((mt, p));
        }
    }
    best.map(|(_, p)| p)
}

async fn persist_user_state(db: &crate::db::Db, user_id: &str, us: &UserState) {
    use crate::db::Db;
    let ts = us.updated_at.to_rfc3339();
    let result = match db {
        Db::Sqlite(p) => sqlx::query(
                "INSERT INTO user_state (user_id, current_system, current_station, ship_type, cargo_capacity, jump_range_ly, credits, pad_size_max, updated_at) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?) \
                 ON CONFLICT(user_id) DO UPDATE SET \
                   current_system=excluded.current_system, \
                   current_station=excluded.current_station, \
                   ship_type=COALESCE(excluded.ship_type, user_state.ship_type), \
                   cargo_capacity=COALESCE(excluded.cargo_capacity, user_state.cargo_capacity), \
                   jump_range_ly=COALESCE(excluded.jump_range_ly, user_state.jump_range_ly), \
                   credits=COALESCE(excluded.credits, user_state.credits), \
                   pad_size_max=COALESCE(excluded.pad_size_max, user_state.pad_size_max), \
                   updated_at=excluded.updated_at",
            )
            .bind(user_id)
            .bind(&us.current_system)
            .bind(&us.current_station)
            .bind(&us.ship_type)
            .bind(us.cargo_capacity)
            .bind(us.jump_range_ly)
            .bind(us.credits)
            .bind(&us.pad_size_max)
            .bind(&ts)
            .execute(p)
            .await
            .map(|_| ()),
        Db::Postgres(p) => sqlx::query(
                "INSERT INTO user_state (user_id, current_system, current_station, ship_type, cargo_capacity, jump_range_ly, credits, pad_size_max, updated_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) \
                 ON CONFLICT(user_id) DO UPDATE SET \
                   current_system=EXCLUDED.current_system, \
                   current_station=EXCLUDED.current_station, \
                   ship_type=COALESCE(EXCLUDED.ship_type, user_state.ship_type), \
                   cargo_capacity=COALESCE(EXCLUDED.cargo_capacity, user_state.cargo_capacity), \
                   jump_range_ly=COALESCE(EXCLUDED.jump_range_ly, user_state.jump_range_ly), \
                   credits=COALESCE(EXCLUDED.credits, user_state.credits), \
                   pad_size_max=COALESCE(EXCLUDED.pad_size_max, user_state.pad_size_max), \
                   updated_at=EXCLUDED.updated_at",
            )
            .bind(user_id)
            .bind(&us.current_system)
            .bind(&us.current_station)
            .bind(&us.ship_type)
            .bind(us.cargo_capacity)
            .bind(us.jump_range_ly)
            .bind(us.credits)
            .bind(&us.pad_size_max)
            .bind(us.updated_at)
            .execute(p)
            .await
            .map(|_| ()),
    };
    if let Err(e) = result {
        tracing::warn!(error = %format!("{:#}", e), "persist_user_state failed");
    }
}

async fn upsert_system(db: &crate::db::Db, name: &str, pos: Option<[f64; 3]>) {
    let Some([x, y, z]) = pos else { return };
    use crate::db::Db;
    // Try to update an existing row first (matched by name). If no row was
    // updated, insert a new row with a synthetic id64. This avoids creating
    // duplicate rows when Spansh already populated the same system with its
    // real id64.
    let updated: u64 = match db {
        Db::Sqlite(p) => sqlx::query(
            "UPDATE systems SET x = ?, y = ?, z = ? WHERE name = ?",
        )
        .bind(x).bind(y).bind(z).bind(name)
        .execute(p).await
        .map(|r| r.rows_affected()).unwrap_or(0),
        Db::Postgres(p) => sqlx::query(
            "UPDATE systems SET x = $1, y = $2, z = $3 WHERE name = $4",
        )
        .bind(x).bind(y).bind(z).bind(name)
        .execute(p).await
        .map(|r| r.rows_affected()).unwrap_or(0),
    };
    if updated == 0 {
        match db {
            Db::Sqlite(p) => {
                let _ = sqlx::query(
                    "INSERT INTO systems (id64, name, x, y, z) VALUES ((SELECT COALESCE(MAX(id64), 0) + 1 FROM systems), ?, ?, ?, ?) \
                     ON CONFLICT(id64) DO NOTHING",
                )
                .bind(name).bind(x).bind(y).bind(z)
                .execute(p).await;
            }
            Db::Postgres(p) => {
                let _ = sqlx::query(
                    "INSERT INTO systems (id64, name, x, y, z) VALUES ((SELECT COALESCE(MAX(id64), 0) + 1 FROM systems), $1, $2, $3, $4) \
                     ON CONFLICT(id64) DO NOTHING",
                )
                .bind(name).bind(x).bind(y).bind(z)
                .execute(p).await;
            }
        }
    }
}

fn event_system_coords(ev: &JournalEvent) -> Option<(String, Option<[f64; 3]>)> {
    match ev {
        JournalEvent::Location { star_system, star_pos, .. } => {
            Some((star_system.clone(), *star_pos))
        }
        JournalEvent::FsdJump { star_system, star_pos } => {
            Some((star_system.clone(), *star_pos))
        }
        _ => None,
    }
}

pub async fn spawn_watcher(
    app: tauri::AppHandle,
    state: AppState,
    dir: PathBuf,
) -> Result<()> {
    tokio::spawn(async move {
        *state.journal_status.write().await = "starting".into();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<NotifyEvent>();
        let mut watcher = match notify::recommended_watcher(
            move |res: notify::Result<NotifyEvent>| {
                if let Ok(ev) = res {
                    let _ = tx.send(ev);
                }
            },
        ) {
            Ok(w) => w,
            Err(e) => {
                *state.journal_status.write().await =
                    format!("watcher init failed: {}", e);
                return;
            }
        };
        if watcher.watch(&dir, RecursiveMode::NonRecursive).is_err() {
            *state.journal_status.write().await =
                format!("directory not found: {}", dir.display());
            return;
        }
        *state.journal_status.write().await = "connected".into();

        let mut current_path = newest_journal(&dir);
        let mut offset: u64 = 0;

        // Initial replay: read the full current journal so we pick up the
        // last Loadout / Location / FSDJump / Docked etc. that was already
        // written before we started watching.
        if let Some(path) = current_path.clone() {
            if let Ok(f) = tokio::fs::File::open(&path).await {
                let mut reader = BufReader::new(f);
                let mut line = String::new();
                loop {
                    line.clear();
                    match reader.read_line(&mut line).await {
                        Ok(0) => break,
                        Ok(n) => {
                            offset += n as u64;
                            if let Some(je) = parse_line(line.trim_end()) {
                                if let Some((sys, pos)) = event_system_coords(&je) {
                                    upsert_system(&state.db, &sys, pos).await;
                                }
                                let snapshot = {
                                    let mut us = state.user_state.write().await;
                                    apply_event(&mut us, &je);
                                    crate::events::emit_user_state(&app, &us);
                                    us.clone()
                                };
                                persist_user_state(&state.db, &state.user_id, &snapshot).await;
                            }
                        }
                        Err(_) => break,
                    }
                }
            }
        }

        loop {
            tokio::select! {
                Some(ev) = rx.recv() => {
                    if matches!(ev.kind, EventKind::Create(_)) {
                        if let Some(p) = newest_journal(&dir) {
                            if Some(&p) != current_path.as_ref() {
                                current_path = Some(p);
                                offset = 0;
                            }
                        }
                    }
                    if let Some(path) = current_path.clone() {
                        if let Ok(f) = tokio::fs::File::open(&path).await {
                            let mut f = f;
                            if f.seek(SeekFrom::Start(offset)).await.is_ok() {
                                let mut reader = BufReader::new(f);
                                let mut line = String::new();
                                loop {
                                    line.clear();
                                    match reader.read_line(&mut line).await {
                                        Ok(0) => break,
                                        Ok(n) => {
                                            offset += n as u64;
                                            if let Some(je) = parse_line(line.trim_end()) {
                                                if let Some((sys, pos)) = event_system_coords(&je) {
                                                    upsert_system(&state.db, &sys, pos).await;
                                                }
                                                let snapshot = {
                                                    let mut us = state.user_state.write().await;
                                                    apply_event(&mut us, &je);
                                                    crate::events::emit_user_state(&app, &us);
                                                    us.clone()
                                                };
                                                persist_user_state(&state.db, &state.user_id, &snapshot).await;
                                            }
                                        }
                                        Err(_) => break,
                                    }
                                }
                            }
                        }
                    }
                }
                _ = tokio::time::sleep(std::time::Duration::from_secs(60)) => {
                    let newest = newest_journal(&dir);
                    if newest.as_ref() != current_path.as_ref() {
                        current_path = newest;
                        offset = 0;
                    }
                }
            }
        }
    });
    Ok(())
}
