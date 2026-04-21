use crate::db::Db;
use anyhow::{Context, Result};
use futures::StreamExt;
use serde::Deserialize;
use std::path::Path;
use tokio::io::AsyncWriteExt;

const GALAXY_URL: &str = "https://downloads.spansh.co.uk/galaxy_populated.json.gz";
const STATIONS_URL: &str = "https://downloads.spansh.co.uk/galaxy_stations.json.gz";

#[derive(Deserialize)]
#[allow(non_snake_case)]
struct System {
    id64: i64,
    name: String,
    coords: Coords,
    #[serde(default)]
    allegiance: Option<String>,
    #[serde(default)]
    government: Option<String>,
    #[serde(default)]
    primaryEconomy: Option<String>,
    #[serde(default)]
    security: Option<String>,
}

#[derive(Deserialize)]
struct Coords {
    x: f64,
    y: f64,
    z: f64,
}

pub async fn download_stations(
    dest: &Path,
    on_progress: impl Fn(u64, Option<u64>) + Send + Sync,
) -> Result<()> {
    download_url(STATIONS_URL, dest, on_progress).await
}

async fn download_url(
    url: &str,
    dest: &Path,
    on_progress: impl Fn(u64, Option<u64>) + Send + Sync,
) -> Result<()> {
    let client = reqwest::Client::builder().build()?;
    let resp = client.get(url).send().await?.error_for_status()?;
    let total = resp.content_length();
    let mut file = tokio::fs::File::create(dest).await?;
    let mut stream = resp.bytes_stream();
    let mut downloaded = 0u64;
    while let Some(chunk) = stream.next().await {
        let bytes = chunk?;
        file.write_all(&bytes).await?;
        downloaded += bytes.len() as u64;
        on_progress(downloaded, total);
    }
    file.flush().await?;
    Ok(())
}

pub async fn download(
    dest: &Path,
    on_progress: impl Fn(u64, Option<u64>) + Send + Sync,
) -> Result<()> {
    let client = reqwest::Client::builder().build()?;
    let resp = client.get(GALAXY_URL).send().await?.error_for_status()?;
    let total = resp.content_length();
    let mut file = tokio::fs::File::create(dest).await?;
    let mut stream = resp.bytes_stream();
    let mut downloaded = 0u64;
    while let Some(chunk) = stream.next().await {
        let bytes = chunk?;
        file.write_all(&bytes).await?;
        downloaded += bytes.len() as u64;
        on_progress(downloaded, total);
    }
    file.flush().await?;
    Ok(())
}

const BATCH_SIZE: usize = 5000;

async fn bulk_mode_on(db: &Db) -> Result<()> {
    match db {
        Db::Sqlite(p) => {
            // Rerunning the import is idempotent, so losing sync guarantees
            // during bulk load is fine.
            sqlx::query("PRAGMA synchronous = OFF").execute(p).await.ok();
            sqlx::query("PRAGMA journal_mode = MEMORY").execute(p).await.ok();
            sqlx::query("PRAGMA temp_store = MEMORY").execute(p).await.ok();
            sqlx::query("PRAGMA cache_size = -200000").execute(p).await.ok();
        }
        Db::Postgres(p) => {
            sqlx::query("SET synchronous_commit = off").execute(p).await.ok();
        }
    }
    Ok(())
}

async fn bulk_mode_off(db: &Db) -> Result<()> {
    match db {
        Db::Sqlite(p) => {
            sqlx::query("PRAGMA synchronous = NORMAL").execute(p).await.ok();
            sqlx::query("PRAGMA journal_mode = WAL").execute(p).await.ok();
        }
        Db::Postgres(p) => {
            sqlx::query("SET synchronous_commit = on").execute(p).await.ok();
        }
    }
    Ok(())
}

async fn drop_system_indexes(db: &Db) -> Result<()> {
    match db {
        Db::Sqlite(p) => {
            sqlx::query("DROP INDEX IF EXISTS ix_systems_name").execute(p).await?;
            sqlx::query("DROP INDEX IF EXISTS ix_systems_coords").execute(p).await?;
        }
        Db::Postgres(p) => {
            sqlx::query("DROP INDEX IF EXISTS ix_systems_name").execute(p).await?;
            sqlx::query("DROP INDEX IF EXISTS ix_systems_coords").execute(p).await?;
        }
    }
    Ok(())
}

async fn create_system_indexes(db: &Db) -> Result<()> {
    match db {
        Db::Sqlite(p) => {
            sqlx::query("CREATE INDEX IF NOT EXISTS ix_systems_name ON systems(name)").execute(p).await?;
            sqlx::query("CREATE INDEX IF NOT EXISTS ix_systems_coords ON systems(x, y, z)").execute(p).await?;
        }
        Db::Postgres(p) => {
            sqlx::query("CREATE INDEX IF NOT EXISTS ix_systems_name ON systems(name)").execute(p).await?;
            sqlx::query("CREATE INDEX IF NOT EXISTS ix_systems_coords ON systems(x, y, z)").execute(p).await?;
        }
    }
    Ok(())
}

pub async fn import_into_systems(
    db: &Db,
    path: &Path,
    on_progress: impl Fn(u64) + Send + Sync,
) -> Result<u64> {
    use flate2::read::GzDecoder;
    use std::io::BufRead;

    bulk_mode_on(db).await?;
    drop_system_indexes(db).await?;

    let f = std::fs::File::open(path).context("opening dump")?;
    let dec = GzDecoder::new(f);
    let reader = std::io::BufReader::new(dec);

    let mut imported = 0u64;
    let mut batch: Vec<System> = Vec::with_capacity(BATCH_SIZE);

    let result: Result<u64> = async {
        for line in reader.lines() {
            let line = match line {
                Ok(l) => l,
                Err(_) => continue,
            };
            let trimmed = line.trim_end_matches(',').trim();
            if trimmed.is_empty() || trimmed == "[" || trimmed == "]" {
                continue;
            }
            match serde_json::from_str::<System>(trimmed) {
                Ok(s) => batch.push(s),
                Err(_) => continue,
            }
            if batch.len() >= BATCH_SIZE {
                imported += flush_batch(db, &batch).await?;
                batch.clear();
                on_progress(imported);
            }
        }
        if !batch.is_empty() {
            imported += flush_batch(db, &batch).await?;
            on_progress(imported);
        }
        Ok(imported)
    }
    .await;

    // Always attempt to put indexes back + restore durability, even on failure.
    let _ = create_system_indexes(db).await;
    let _ = bulk_mode_off(db).await;

    result
}

// Each row binds 9 parameters. SQLite defaults to a max of 999 bound params
// per statement; Postgres allows 65535. We chunk to 100 rows per multi-row
// statement to stay well under both limits.
const CHUNK_ROWS: usize = 100;

#[derive(Deserialize)]
#[allow(non_snake_case)]
struct StationsDumpSystem {
    #[serde(default)]
    id64: Option<i64>,
    name: String,
    #[serde(default)]
    coords: Option<Coords>,
    #[serde(default)]
    stations: Vec<StationsDumpStation>,
}

#[derive(Deserialize)]
#[allow(non_snake_case)]
struct StationsDumpStation {
    #[serde(default)]
    id: Option<i64>,
    name: String,
    #[serde(default, rename = "type")]
    station_type: Option<String>,
    #[serde(default)]
    landingPads: Option<LandingPads>,
    #[serde(default)]
    distanceToArrival: Option<f64>,
    #[serde(default)]
    market: Option<StationsDumpMarket>,
}

#[derive(Deserialize)]
#[allow(non_snake_case)]
struct LandingPads {
    #[serde(default)]
    small: Option<i32>,
    #[serde(default)]
    medium: Option<i32>,
    #[serde(default)]
    large: Option<i32>,
}

#[derive(Deserialize)]
#[allow(non_snake_case)]
struct StationsDumpMarket {
    #[serde(default)]
    updateTime: Option<String>,
    #[serde(default)]
    commodities: Vec<StationsDumpCommodity>,
}

#[derive(Deserialize)]
#[allow(non_snake_case)]
struct StationsDumpCommodity {
    #[serde(default)]
    symbol: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    buyPrice: i32,
    #[serde(default)]
    sellPrice: i32,
    #[serde(default)]
    supply: i32,
    #[serde(default)]
    demand: i32,
}

fn infer_max_pad(pads: &Option<LandingPads>) -> Option<String> {
    if let Some(p) = pads.as_ref() {
        if p.large.unwrap_or(0) > 0 {
            return Some("L".into());
        }
        if p.medium.unwrap_or(0) > 0 {
            return Some("M".into());
        }
        if p.small.unwrap_or(0) > 0 {
            return Some("S".into());
        }
    }
    None
}

fn pad_from_station_type(t: Option<&str>) -> Option<String> {
    let t = t?.to_lowercase();
    // Fleet carriers + large starports always have L pads.
    if t.contains("carrier") {
        return Some("L".into());
    }
    if t.contains("starport") || t.contains("asteroid base") || t.contains("mega ship") {
        return Some("L".into());
    }
    // Planetary surface ports: most are M, some are L. Without extra info assume M.
    if t.contains("planetary port") {
        return Some("L".into());
    }
    if t.contains("outpost") {
        return Some("M".into());
    }
    if t.contains("planetary outpost") {
        return Some("M".into());
    }
    None
}

fn is_fleet_carrier_type(t: Option<&str>) -> bool {
    t.map(|s| s.to_lowercase().contains("carrier")).unwrap_or(false)
}

pub struct StationImportStats {
    pub stations: u64,
    pub snapshots: u64,
}

pub async fn import_stations_and_markets(
    db: &Db,
    path: &Path,
    on_progress: impl Fn(u64, u64) + Send + Sync,
) -> Result<StationImportStats> {
    use flate2::read::GzDecoder;
    use std::io::BufRead;

    bulk_mode_on(db).await?;

    let f = std::fs::File::open(path).context("opening stations dump")?;
    let dec = GzDecoder::new(f);
    let reader = std::io::BufReader::new(dec);

    let mut stations_done = 0u64;
    let mut snapshots_done = 0u64;
    let mut station_batch: Vec<PendingStation> = Vec::with_capacity(500);
    let mut snapshot_batch: Vec<PendingSnapshot> = Vec::with_capacity(10000);

    let result: Result<()> = async {
        for line in reader.lines() {
            let line = match line {
                Ok(l) => l,
                Err(_) => continue,
            };
            let trimmed = line.trim_end_matches(',').trim();
            if trimmed.is_empty() || trimmed == "[" || trimmed == "]" {
                continue;
            }
            let sys: StationsDumpSystem = match serde_json::from_str(trimmed) {
                Ok(s) => s,
                Err(_) => continue,
            };
            let system_name = sys.name;
            let system_id64 = sys.id64;

            for st in sys.stations {
                let Some(market_id) = st.id else { continue };
                let pad = infer_max_pad(&st.landingPads)
                    .or_else(|| pad_from_station_type(st.station_type.as_deref()));
                let is_fc = is_fleet_carrier_type(st.station_type.as_deref());
                let last_seen_at = st
                    .market
                    .as_ref()
                    .and_then(|m| m.updateTime.clone())
                    .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());

                station_batch.push(PendingStation {
                    station_id: market_id,
                    system_name: system_name.clone(),
                    system_id64,
                    station_name: st.name,
                    pad_size: pad,
                    station_type: st.station_type,
                    is_fleet_carrier: is_fc,
                    distance_to_star: st.distanceToArrival,
                    last_seen_at: last_seen_at.clone(),
                });

                if let Some(market) = st.market {
                    let recorded_at = market
                        .updateTime
                        .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());
                    for c in market.commodities {
                        let Some(symbol) = c.symbol.or(c.name) else {
                            continue;
                        };
                        let has_buy = c.buyPrice > 0;
                        let has_sell = c.sellPrice > 0;
                        if !has_buy && !has_sell {
                            continue;
                        }
                        snapshot_batch.push(PendingSnapshot {
                            station_id: market_id,
                            symbol,
                            buy_price: if has_buy { Some(c.buyPrice) } else { None },
                            sell_price: if has_sell { Some(c.sellPrice) } else { None },
                            supply: c.supply,
                            demand: c.demand,
                            recorded_at: recorded_at.clone(),
                        });
                    }
                }
            }

            if station_batch.len() >= 500 {
                stations_done += flush_station_batch(db, &station_batch).await?;
                station_batch.clear();
            }
            if snapshot_batch.len() >= 10_000 {
                snapshots_done += flush_snapshot_batch(db, &snapshot_batch).await?;
                snapshot_batch.clear();
                on_progress(stations_done, snapshots_done);
            }
        }
        if !station_batch.is_empty() {
            stations_done += flush_station_batch(db, &station_batch).await?;
        }
        if !snapshot_batch.is_empty() {
            snapshots_done += flush_snapshot_batch(db, &snapshot_batch).await?;
        }
        on_progress(stations_done, snapshots_done);
        Ok(())
    }
    .await;

    let _ = bulk_mode_off(db).await;
    result?;

    Ok(StationImportStats {
        stations: stations_done,
        snapshots: snapshots_done,
    })
}

struct PendingStation {
    station_id: i64,
    system_name: String,
    system_id64: Option<i64>,
    station_name: String,
    pad_size: Option<String>,
    station_type: Option<String>,
    is_fleet_carrier: bool,
    distance_to_star: Option<f64>,
    last_seen_at: String,
}

struct PendingSnapshot {
    station_id: i64,
    symbol: String,
    buy_price: Option<i32>,
    sell_price: Option<i32>,
    supply: i32,
    demand: i32,
    recorded_at: String,
}

async fn flush_station_batch(db: &Db, batch: &[PendingStation]) -> Result<u64> {
    let mut count = 0u64;
    match db {
        Db::Sqlite(p) => {
            let mut tx = p.begin().await?;
            for s in batch {
                sqlx::query(
                    "INSERT INTO stations (station_id, system_name, system_id64, station_name, pad_size, station_type, is_fleet_carrier, distance_to_star, last_seen_at) \
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?) \
                     ON CONFLICT(station_id) DO UPDATE SET \
                       system_name=excluded.system_name, \
                       system_id64=COALESCE(excluded.system_id64, stations.system_id64), \
                       station_name=excluded.station_name, \
                       pad_size=COALESCE(excluded.pad_size, stations.pad_size), \
                       station_type=COALESCE(excluded.station_type, stations.station_type), \
                       is_fleet_carrier=excluded.is_fleet_carrier, \
                       distance_to_star=COALESCE(excluded.distance_to_star, stations.distance_to_star), \
                       last_seen_at=MAX(stations.last_seen_at, excluded.last_seen_at)",
                )
                .bind(s.station_id)
                .bind(&s.system_name)
                .bind(s.system_id64)
                .bind(&s.station_name)
                .bind(s.pad_size.as_deref())
                .bind(s.station_type.as_deref())
                .bind(s.is_fleet_carrier as i32)
                .bind(s.distance_to_star)
                .bind(&s.last_seen_at)
                .execute(&mut *tx)
                .await?;
                count += 1;
            }
            tx.commit().await?;
        }
        Db::Postgres(p) => {
            let mut tx = p.begin().await?;
            for s in batch {
                sqlx::query(
                    "INSERT INTO stations (station_id, system_name, system_id64, station_name, pad_size, station_type, is_fleet_carrier, distance_to_star, last_seen_at) \
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9::timestamptz) \
                     ON CONFLICT(station_id) DO UPDATE SET \
                       system_name=EXCLUDED.system_name, \
                       system_id64=COALESCE(EXCLUDED.system_id64, stations.system_id64), \
                       station_name=EXCLUDED.station_name, \
                       pad_size=COALESCE(EXCLUDED.pad_size, stations.pad_size), \
                       station_type=COALESCE(EXCLUDED.station_type, stations.station_type), \
                       is_fleet_carrier=EXCLUDED.is_fleet_carrier, \
                       distance_to_star=COALESCE(EXCLUDED.distance_to_star, stations.distance_to_star), \
                       last_seen_at=GREATEST(stations.last_seen_at, EXCLUDED.last_seen_at)",
                )
                .bind(s.station_id)
                .bind(&s.system_name)
                .bind(s.system_id64)
                .bind(&s.station_name)
                .bind(s.pad_size.as_deref())
                .bind(s.station_type.as_deref())
                .bind(s.is_fleet_carrier)
                .bind(s.distance_to_star)
                .bind(&s.last_seen_at)
                .execute(&mut *tx)
                .await?;
                count += 1;
            }
            tx.commit().await?;
        }
    }
    Ok(count)
}

async fn ensure_commodity(db: &Db, symbol: &str) -> Result<i64> {
    match db {
        Db::Sqlite(p) => {
            if let Some(id) =
                sqlx::query_scalar::<_, i64>("SELECT commodity_id FROM commodities WHERE symbol = ?")
                    .bind(symbol)
                    .fetch_optional(p)
                    .await?
            {
                return Ok(id);
            }
            let next: i64 = sqlx::query_scalar(
                "SELECT COALESCE(MAX(commodity_id), 900000000) + 1 FROM commodities",
            )
            .fetch_one(p)
            .await?;
            sqlx::query(
                "INSERT INTO commodities (commodity_id, symbol, display_name) VALUES (?, ?, ?) ON CONFLICT(symbol) DO NOTHING",
            )
            .bind(next)
            .bind(symbol)
            .bind(symbol)
            .execute(p)
            .await?;
            Ok(sqlx::query_scalar::<_, i64>("SELECT commodity_id FROM commodities WHERE symbol = ?")
                .bind(symbol)
                .fetch_one(p)
                .await?)
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
            let next: i32 = sqlx::query_scalar(
                "SELECT COALESCE(MAX(commodity_id), 900000000) + 1 FROM commodities",
            )
            .fetch_one(p)
            .await?;
            sqlx::query(
                "INSERT INTO commodities (commodity_id, symbol, display_name) VALUES ($1, $2, $3) ON CONFLICT(symbol) DO NOTHING",
            )
            .bind(next)
            .bind(symbol)
            .bind(symbol)
            .execute(p)
            .await?;
            Ok(sqlx::query_scalar::<_, i32>(
                "SELECT commodity_id FROM commodities WHERE symbol = $1",
            )
            .bind(symbol)
            .fetch_one(p)
            .await? as i64)
        }
    }
}

async fn flush_snapshot_batch(db: &Db, batch: &[PendingSnapshot]) -> Result<u64> {
    use std::collections::HashMap;
    // Resolve commodity IDs once per symbol rather than once per row.
    let mut symbol_to_id: HashMap<String, i64> = HashMap::new();
    for s in batch {
        if !symbol_to_id.contains_key(&s.symbol) {
            let id = ensure_commodity(db, &s.symbol).await?;
            symbol_to_id.insert(s.symbol.clone(), id);
        }
    }

    let mut count = 0u64;
    match db {
        Db::Sqlite(p) => {
            let mut tx = p.begin().await?;
            for chunk in batch.chunks(100) {
                let mut sql = String::from(
                    "INSERT INTO market_snapshots (station_id, commodity_id, buy_price, sell_price, supply, demand, recorded_at, source) VALUES ",
                );
                for i in 0..chunk.len() {
                    if i > 0 {
                        sql.push(',');
                    }
                    sql.push_str("(?,?,?,?,?,?,?,'spansh')");
                }
                sql.push_str(" ON CONFLICT(station_id, commodity_id, recorded_at) DO NOTHING");
                let mut q = sqlx::query(&sql);
                for s in chunk {
                    let cid = *symbol_to_id.get(&s.symbol).unwrap();
                    q = q
                        .bind(s.station_id)
                        .bind(cid)
                        .bind(s.buy_price)
                        .bind(s.sell_price)
                        .bind(s.supply)
                        .bind(s.demand)
                        .bind(&s.recorded_at);
                }
                q.execute(&mut *tx).await?;
                count += chunk.len() as u64;
            }
            tx.commit().await?;
        }
        Db::Postgres(p) => {
            let mut tx = p.begin().await?;
            for chunk in batch.chunks(100) {
                let mut sql = String::from(
                    "INSERT INTO market_snapshots (station_id, commodity_id, buy_price, sell_price, supply, demand, recorded_at, source) VALUES ",
                );
                let mut n = 1;
                for i in 0..chunk.len() {
                    if i > 0 {
                        sql.push(',');
                    }
                    sql.push('(');
                    for j in 0..7 {
                        if j > 0 {
                            sql.push(',');
                        }
                        sql.push_str(&format!("${}", n));
                        n += 1;
                    }
                    sql.push_str(",'spansh')");
                }
                sql.push_str(" ON CONFLICT(station_id, commodity_id, recorded_at) DO NOTHING");
                let mut q = sqlx::query(&sql);
                for s in chunk {
                    let cid = *symbol_to_id.get(&s.symbol).unwrap() as i32;
                    q = q
                        .bind(s.station_id)
                        .bind(cid)
                        .bind(s.buy_price)
                        .bind(s.sell_price)
                        .bind(s.supply)
                        .bind(s.demand)
                        .bind(&s.recorded_at);
                }
                q.execute(&mut *tx).await?;
                count += chunk.len() as u64;
            }
            tx.commit().await?;
        }
    }
    Ok(count)
}

async fn flush_batch(db: &Db, batch: &[System]) -> Result<u64> {
    let mut count = 0u64;
    match db {
        Db::Sqlite(p) => {
            let mut tx = p.begin().await?;
            for chunk in batch.chunks(CHUNK_ROWS) {
                let mut sql = String::from(
                    "INSERT INTO systems (id64, name, x, y, z, allegiance, government, primary_economy, security) VALUES ",
                );
                for i in 0..chunk.len() {
                    if i > 0 {
                        sql.push(',');
                    }
                    sql.push_str("(?,?,?,?,?,?,?,?,?)");
                }
                sql.push_str(
                    " ON CONFLICT(id64) DO UPDATE SET name=excluded.name, x=excluded.x, y=excluded.y, z=excluded.z, allegiance=excluded.allegiance, government=excluded.government, primary_economy=excluded.primary_economy, security=excluded.security \
                     WHERE systems.name IS NOT excluded.name OR systems.x IS NOT excluded.x OR systems.y IS NOT excluded.y OR systems.z IS NOT excluded.z OR systems.allegiance IS NOT excluded.allegiance OR systems.government IS NOT excluded.government OR systems.primary_economy IS NOT excluded.primary_economy OR systems.security IS NOT excluded.security",
                );
                let mut q = sqlx::query(&sql);
                for s in chunk {
                    q = q
                        .bind(s.id64)
                        .bind(&s.name)
                        .bind(s.coords.x)
                        .bind(s.coords.y)
                        .bind(s.coords.z)
                        .bind(&s.allegiance)
                        .bind(&s.government)
                        .bind(&s.primaryEconomy)
                        .bind(&s.security);
                }
                q.execute(&mut *tx).await?;
                count += chunk.len() as u64;
            }
            tx.commit().await?;
        }
        Db::Postgres(p) => {
            let mut tx = p.begin().await?;
            for chunk in batch.chunks(CHUNK_ROWS) {
                let mut sql = String::from(
                    "INSERT INTO systems (id64, name, x, y, z, allegiance, government, primary_economy, security) VALUES ",
                );
                let mut n = 1;
                for i in 0..chunk.len() {
                    if i > 0 {
                        sql.push(',');
                    }
                    sql.push('(');
                    for j in 0..9 {
                        if j > 0 {
                            sql.push(',');
                        }
                        sql.push_str(&format!("${}", n));
                        n += 1;
                    }
                    sql.push(')');
                }
                sql.push_str(
                    " ON CONFLICT(id64) DO UPDATE SET name=EXCLUDED.name, x=EXCLUDED.x, y=EXCLUDED.y, z=EXCLUDED.z, allegiance=EXCLUDED.allegiance, government=EXCLUDED.government, primary_economy=EXCLUDED.primary_economy, security=EXCLUDED.security \
                     WHERE systems.name IS DISTINCT FROM EXCLUDED.name OR systems.x IS DISTINCT FROM EXCLUDED.x OR systems.y IS DISTINCT FROM EXCLUDED.y OR systems.z IS DISTINCT FROM EXCLUDED.z OR systems.allegiance IS DISTINCT FROM EXCLUDED.allegiance OR systems.government IS DISTINCT FROM EXCLUDED.government OR systems.primary_economy IS DISTINCT FROM EXCLUDED.primary_economy OR systems.security IS DISTINCT FROM EXCLUDED.security",
                );
                let mut q = sqlx::query(&sql);
                for s in chunk {
                    q = q
                        .bind(s.id64)
                        .bind(&s.name)
                        .bind(s.coords.x)
                        .bind(s.coords.y)
                        .bind(s.coords.z)
                        .bind(&s.allegiance)
                        .bind(&s.government)
                        .bind(&s.primaryEconomy)
                        .bind(&s.security);
                }
                q.execute(&mut *tx).await?;
                count += chunk.len() as u64;
            }
            tx.commit().await?;
        }
    }
    Ok(count)
}
