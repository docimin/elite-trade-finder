use super::scoring::{compute_score, cr_per_hour, estimate_cycle_seconds, ScoreInputs};
use crate::db::Db;
use crate::types::{RankedRoute, RouteLeg, RouteMode, ScoreWeights, Sustainability};
use anyhow::Result;
use chrono::{DateTime, Utc};

#[derive(sqlx::FromRow, Clone)]
struct RareRow {
    system_name: String,
    station_name: String,
    symbol: String,
    buy_price: i32,
    supply: i32,
    recorded_at: String,
    x: Option<f64>,
    y: Option<f64>,
    z: Option<f64>,
}

#[derive(sqlx::FromRow)]
struct HubRow {
    _station_id: i64,
    system_name: String,
    station_name: String,
    symbol: String,
    sell_price: i32,
    demand: i32,
    _recorded_at: String,
    x: Option<f64>,
    y: Option<f64>,
    z: Option<f64>,
}

pub async fn find(
    db: &Db,
    user_id: &str,
    weights: &ScoreWeights,
    limit: i32,
) -> Result<Vec<RankedRoute>> {
    let row: Option<(Option<String>, Option<i32>, Option<f64>)> = match db {
        Db::Sqlite(p) => sqlx::query_as(
            "SELECT current_system, cargo_capacity, jump_range_ly FROM user_state WHERE user_id = ?",
        )
        .bind(user_id)
        .fetch_optional(p)
        .await?,
        Db::Postgres(p) => sqlx::query_as(
            "SELECT current_system, cargo_capacity, jump_range_ly FROM user_state WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_optional(p)
        .await?,
    };
    let Some((user_system, cargo, jump_range)) = row else {
        return Ok(vec![]);
    };
    let (Some(user_system), Some(cargo), Some(jump_range)) = (user_system, cargo, jump_range)
    else {
        return Ok(vec![]);
    };
    let tour_radius = jump_range * 15.0;

    let rares: Vec<RareRow> = match db {
        Db::Sqlite(p) => sqlx::query_as(
            "SELECT s.system_name, s.station_name, c.symbol, m.buy_price, m.supply, m.recorded_at, sys.x, sys.y, sys.z \
             FROM latest_market m \
             JOIN commodities c ON c.commodity_id = m.commodity_id AND c.is_rare = 1 \
             JOIN stations s ON s.station_id = m.station_id \
             LEFT JOIN systems sys ON sys.name = s.system_name \
             CROSS JOIN (SELECT x AS ux, y AS uy, z AS uz FROM systems WHERE name = ?) u \
             WHERE m.buy_price > 0 AND m.supply > 0 \
               AND (sys.x IS NULL OR ((sys.x-u.ux)*(sys.x-u.ux)+(sys.y-u.uy)*(sys.y-u.uy)+(sys.z-u.uz)*(sys.z-u.uz)) <= ?*?) \
             ORDER BY m.recorded_at DESC",
        )
        .bind(&user_system)
        .bind(tour_radius)
        .bind(tour_radius)
        .fetch_all(p)
        .await?,
        Db::Postgres(p) => sqlx::query_as(
            "SELECT s.system_name, s.station_name, c.symbol, m.buy_price, m.supply, m.recorded_at::text AS recorded_at, sys.x, sys.y, sys.z \
             FROM latest_market m \
             JOIN commodities c ON c.commodity_id = m.commodity_id AND c.is_rare = TRUE \
             JOIN stations s ON s.station_id = m.station_id \
             LEFT JOIN systems sys ON sys.name = s.system_name \
             CROSS JOIN (SELECT (SELECT x FROM systems WHERE name = $1 LIMIT 1) AS ux, \
                                (SELECT y FROM systems WHERE name = $1 LIMIT 1) AS uy, \
                                (SELECT z FROM systems WHERE name = $1 LIMIT 1) AS uz) u \
             WHERE m.buy_price > 0 AND m.supply > 0 \
               AND (u.ux IS NULL OR sys.x IS NULL OR ((sys.x-u.ux)*(sys.x-u.ux)+(sys.y-u.uy)*(sys.y-u.uy)+(sys.z-u.uz)*(sys.z-u.uz)) <= $2*$2) \
             ORDER BY m.recorded_at DESC",
        )
        .bind(&user_system)
        .bind(tour_radius)
        .fetch_all(p)
        .await?,
    };

    let hubs: Vec<HubRow> = match db {
        Db::Sqlite(p) => sqlx::query_as(
            "SELECT m.station_id AS _station_id, s.system_name, s.station_name, c.symbol, m.sell_price, m.demand, m.recorded_at AS _recorded_at, sys.x, sys.y, sys.z \
             FROM latest_market m \
             JOIN commodities c ON c.commodity_id = m.commodity_id AND c.is_rare = 1 \
             JOIN stations s ON s.station_id = m.station_id \
             LEFT JOIN systems sys ON sys.name = s.system_name \
             WHERE m.sell_price > 0 AND m.demand > 0",
        )
        .fetch_all(p)
        .await?,
        Db::Postgres(p) => sqlx::query_as(
            "SELECT m.station_id AS _station_id, s.system_name, s.station_name, c.symbol, m.sell_price, m.demand, m.recorded_at::text AS _recorded_at, sys.x, sys.y, sys.z \
             FROM latest_market m \
             JOIN commodities c ON c.commodity_id = m.commodity_id AND c.is_rare = TRUE \
             JOIN stations s ON s.station_id = m.station_id \
             LEFT JOIN systems sys ON sys.name = s.system_name \
             WHERE m.sell_price > 0 AND m.demand > 0",
        )
        .fetch_all(p)
        .await?,
    };

    let mut results = Vec::new();
    for hub in &hubs {
        let matching: Vec<&RareRow> =
            rares.iter().filter(|r| r.symbol == hub.symbol).collect();
        if matching.is_empty() {
            continue;
        }

        let (hx, hy, hz) = match (hub.x, hub.y, hub.z) {
            (Some(x), Some(y), Some(z)) => (x, y, z),
            _ => continue,
        };
        let buyer = matching.iter().find_map(|r| {
            let (x, y, z) = (r.x?, r.y?, r.z?);
            let d = ((hx - x).powi(2) + (hy - y).powi(2) + (hz - z).powi(2)).sqrt();
            if d >= 150.0 {
                Some(((*r).clone(), d))
            } else {
                None
            }
        });
        let Some((src, dist)) = buyer else {
            continue;
        };

        let units = src.supply.min(cargo).min(hub.demand);
        if units <= 0 {
            continue;
        }
        let profit_per_ton = hub.sell_price - src.buy_price;
        let profit = (profit_per_ton as i64) * (units as i64);

        let jumps = (dist / jump_range).ceil().max(1.0) as i32;
        let cycle_seconds = estimate_cycle_seconds(&[(jumps, 5000.0), (jumps, 1000.0)]);
        let cphr = cr_per_hour(profit, cycle_seconds);

        let recorded: DateTime<Utc> = DateTime::parse_from_rfc3339(&src.recorded_at)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());
        let age = (Utc::now() - recorded).num_seconds() as f64 / 60.0;
        let score = compute_score(
            &ScoreInputs {
                profit_per_ton,
                cargo_units: units,
                age_minutes: age,
                traffic_percentile: 0.3,
                touches_fleet_carrier: false,
                trip_jumps: jumps * 2,
            },
            weights,
        );

        let leg = RouteLeg {
            from_system: src.system_name.clone(),
            from_station: src.station_name.clone(),
            to_system: hub.system_name.clone(),
            to_station: hub.station_name.clone(),
            commodity: src.symbol.clone(),
            buy_price: src.buy_price,
            sell_price: hub.sell_price,
            profit_per_ton,
            supply: src.supply,
            demand: hub.demand,
            jumps,
            distance_ly: dist,
            recorded_at: recorded,
        };

        let route_hash = format!(
            "rare:{}:{}->{}",
            src.symbol, src.station_name, hub.station_name
        );

        results.push(RankedRoute {
            mode: RouteMode::RareChain,
            legs: vec![leg],
            cr_per_hour: cphr,
            profit_per_cycle: profit,
            cycle_seconds,
            total_jumps: jumps * 2,
            sustainability: Sustainability::Decaying {
                estimated_cycles: 3,
            },
            score,
            freshest_age_seconds: (age * 60.0) as i32,
            touches_fleet_carrier: false,
            route_hash,
        });
    }

    results.sort_by(|a, b| b.cr_per_hour.cmp(&a.cr_per_hour));
    results.truncate(limit.max(1) as usize);
    Ok(results)
}
