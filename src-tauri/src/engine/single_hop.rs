use super::scoring::{compute_score, cr_per_hour, estimate_cycle_seconds, ScoreInputs};
use crate::db::Db;
use crate::types::{RankedRoute, RouteLeg, RouteMode, ScoreWeights, Sustainability};
use anyhow::Result;
use chrono::{DateTime, Utc};

#[derive(sqlx::FromRow)]
struct Pair {
    from_station_id: i64,
    from_system: String,
    from_station: String,
    to_station_id: i64,
    to_system: String,
    to_station: String,
    commodity_symbol: String,
    buy_price: i32,
    sell_price: i32,
    supply: i32,
    demand: i32,
    recorded_at: String,
    touches_fc: i64,
    from_coords_x: Option<f64>,
    from_coords_y: Option<f64>,
    from_coords_z: Option<f64>,
    to_coords_x: Option<f64>,
    to_coords_y: Option<f64>,
    to_coords_z: Option<f64>,
}

pub async fn find(
    db: &Db,
    user_id: &str,
    weights: &ScoreWeights,
    limit: i32,
    override_ship: Option<&crate::types::ShipSpec>,
) -> Result<Vec<RankedRoute>> {
    let row: Option<(Option<String>, Option<i32>, Option<f64>, Option<String>)> = match db {
        Db::Sqlite(p) => sqlx::query_as(
            "SELECT current_system, cargo_capacity, jump_range_ly, pad_size_max FROM user_state WHERE user_id = ?",
        )
        .bind(user_id)
        .fetch_optional(p)
        .await?,
        Db::Postgres(p) => sqlx::query_as(
            "SELECT current_system, cargo_capacity, jump_range_ly, pad_size_max FROM user_state WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_optional(p)
        .await?,
    };
    let Some((user_system, cargo, jump_range, pad)) = row else {
        tracing::debug!("single_hop: user_state row missing (journal hasn't persisted yet)");
        return Ok(vec![]);
    };
    // Override wins when present. Otherwise fall back to journal-derived
    // values with "S" pad as the permissive default.
    let (cargo, jump_range, pad) = if let Some(os) = override_ship {
        (os.cargo_capacity, os.jump_range_ly, os.pad_size_max.clone())
    } else {
        let pad = pad.unwrap_or_else(|| "S".into());
        match (cargo, jump_range) {
            (Some(c), Some(j)) => (c, j, pad),
            _ => {
                tracing::debug!(
                    "single_hop: user_state missing cargo/jump-range — waiting for Loadout"
                );
                return Ok(vec![]);
            }
        }
    };
    let Some(user_system) = user_system else {
        tracing::debug!("single_hop: user_state missing current_system — waiting for Location");
        return Ok(vec![]);
    };
    tracing::debug!(
        user_system,
        cargo,
        jump_range,
        pad,
        "single_hop: computing routes"
    );
    let radius = jump_range * 20.0;

    // Narrow to reachable stations FIRST in a CTE, then do the self-join only
    // within that set. Cuts the self-join surface from ~50k × 50k to typically
    // ~1-5k × 1-5k for a local bubble search.
    let pairs: Vec<Pair> = match db {
        Db::Sqlite(p) => sqlx::query_as(
            "WITH user_coords AS ( \
               SELECT (SELECT x FROM systems WHERE name = ? LIMIT 1) AS ux, \
                      (SELECT y FROM systems WHERE name = ? LIMIT 1) AS uy, \
                      (SELECT z FROM systems WHERE name = ? LIMIT 1) AS uz \
             ), reachable AS ( \
               SELECT s.station_id, s.system_name, s.station_name, s.pad_size, s.is_fleet_carrier, \
                      sys.x AS sx, sys.y AS sy, sys.z AS sz \
               FROM stations s \
               LEFT JOIN systems sys ON sys.name = s.system_name \
               CROSS JOIN user_coords u \
               WHERE (s.pad_size IS NULL OR s.pad_size >= ?) \
                 AND (u.ux IS NULL OR sys.x IS NULL OR ((sys.x-u.ux)*(sys.x-u.ux)+(sys.y-u.uy)*(sys.y-u.uy)+(sys.z-u.uz)*(sys.z-u.uz)) <= ?*?) \
             ) \
             SELECT b.station_id AS from_station_id, ra.system_name AS from_system, ra.station_name AS from_station, \
                    s.station_id AS to_station_id, rb.system_name AS to_system, rb.station_name AS to_station, \
                    c.symbol AS commodity_symbol, b.buy_price, s.sell_price, b.supply, s.demand, b.recorded_at, \
                    (ra.is_fleet_carrier OR rb.is_fleet_carrier) AS touches_fc, \
                    ra.sx AS from_coords_x, ra.sy AS from_coords_y, ra.sz AS from_coords_z, \
                    rb.sx AS to_coords_x, rb.sy AS to_coords_y, rb.sz AS to_coords_z \
             FROM latest_market b \
             JOIN reachable ra ON ra.station_id = b.station_id \
             JOIN latest_market s ON s.commodity_id = b.commodity_id AND s.sell_price > b.buy_price AND s.station_id <> b.station_id \
             JOIN reachable rb ON rb.station_id = s.station_id \
             JOIN commodities c ON c.commodity_id = b.commodity_id \
             WHERE b.buy_price > 0 AND b.supply > 0 \
               AND s.sell_price > 0 AND s.demand > 0 \
             ORDER BY (s.sell_price - b.buy_price) DESC LIMIT ?",
        )
        .bind(&user_system)
        .bind(&user_system)
        .bind(&user_system)
        .bind(&pad)
        .bind(radius)
        .bind(radius)
        .bind(limit.max(1) * 5)
        .fetch_all(p)
        .await?,
        Db::Postgres(p) => sqlx::query_as(
            "WITH user_coords AS ( \
               SELECT (SELECT x FROM systems WHERE name = $1 LIMIT 1) AS ux, \
                      (SELECT y FROM systems WHERE name = $1 LIMIT 1) AS uy, \
                      (SELECT z FROM systems WHERE name = $1 LIMIT 1) AS uz \
             ), reachable AS ( \
               SELECT s.station_id, s.system_name, s.station_name, s.pad_size, s.is_fleet_carrier, \
                      sys.x AS sx, sys.y AS sy, sys.z AS sz \
               FROM stations s \
               LEFT JOIN systems sys ON sys.name = s.system_name \
               CROSS JOIN user_coords u \
               WHERE (s.pad_size IS NULL OR s.pad_size >= $2) \
                 AND (u.ux IS NULL OR sys.x IS NULL OR ((sys.x-u.ux)*(sys.x-u.ux)+(sys.y-u.uy)*(sys.y-u.uy)+(sys.z-u.uz)*(sys.z-u.uz)) <= $3*$3) \
             ) \
             SELECT b.station_id AS from_station_id, ra.system_name AS from_system, ra.station_name AS from_station, \
                    s.station_id AS to_station_id, rb.system_name AS to_system, rb.station_name AS to_station, \
                    c.symbol AS commodity_symbol, b.buy_price, s.sell_price, b.supply, s.demand, b.recorded_at::text AS recorded_at, \
                    CASE WHEN (ra.is_fleet_carrier OR rb.is_fleet_carrier) THEN 1 ELSE 0 END::int8 AS touches_fc, \
                    ra.sx AS from_coords_x, ra.sy AS from_coords_y, ra.sz AS from_coords_z, \
                    rb.sx AS to_coords_x, rb.sy AS to_coords_y, rb.sz AS to_coords_z \
             FROM latest_market b \
             JOIN reachable ra ON ra.station_id = b.station_id \
             JOIN latest_market s ON s.commodity_id = b.commodity_id AND s.sell_price > b.buy_price AND s.station_id <> b.station_id \
             JOIN reachable rb ON rb.station_id = s.station_id \
             JOIN commodities c ON c.commodity_id = b.commodity_id \
             WHERE b.buy_price > 0 AND b.supply > 0 \
               AND s.sell_price > 0 AND s.demand > 0 \
             ORDER BY (s.sell_price - b.buy_price) DESC LIMIT $4",
        )
        .bind(&user_system)
        .bind(&pad)
        .bind(radius)
        .bind(limit.max(1) * 5)
        .fetch_all(p)
        .await?,
    };

    let user_coords = match db {
        Db::Sqlite(p) => sqlx::query_as::<_, (Option<f64>, Option<f64>, Option<f64>)>(
            "SELECT x, y, z FROM systems WHERE name = ?",
        )
        .bind(&user_system)
        .fetch_one(p)
        .await
        .ok(),
        Db::Postgres(p) => sqlx::query_as::<_, (Option<f64>, Option<f64>, Option<f64>)>(
            "SELECT x, y, z FROM systems WHERE name = $1",
        )
        .bind(&user_system)
        .fetch_one(p)
        .await
        .ok(),
    }
    .and_then(|(x, y, z)| Some((x?, y?, z?)));

    let mut out = Vec::<RankedRoute>::new();
    for p in pairs {
        let profit_per_ton = p.sell_price - p.buy_price;
        let units = cargo.min(p.supply).min(p.demand);
        if units <= 0 {
            continue;
        }
        let profit = (profit_per_ton as i64) * (units as i64);

        let dist_ly = match (
            user_coords,
            (p.from_coords_x, p.from_coords_y, p.from_coords_z),
        ) {
            (Some((ux, uy, uz)), (Some(x), Some(y), Some(z))) => {
                ((x - ux).powi(2) + (y - uy).powi(2) + (z - uz).powi(2)).sqrt()
            }
            _ => 0.0,
        };
        let leg1_dist = match (
            (p.from_coords_x, p.from_coords_y, p.from_coords_z),
            (p.to_coords_x, p.to_coords_y, p.to_coords_z),
        ) {
            ((Some(x1), Some(y1), Some(z1)), (Some(x2), Some(y2), Some(z2))) => {
                ((x2 - x1).powi(2) + (y2 - y1).powi(2) + (z2 - z1).powi(2)).sqrt()
            }
            _ => 0.0,
        };
        let jumps_to_pickup = (dist_ly / jump_range).ceil().max(0.0) as i32;
        let jumps_leg1 = (leg1_dist / jump_range).ceil().max(1.0) as i32;
        let total_jumps = jumps_to_pickup + jumps_leg1;

        let cycle_seconds = estimate_cycle_seconds(&[(total_jumps, 1000.0)]);
        let cphr = cr_per_hour(profit, cycle_seconds);

        let recorded: DateTime<Utc> = DateTime::parse_from_rfc3339(&p.recorded_at)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());
        let age_min = (Utc::now() - recorded).num_seconds() as f64 / 60.0;

        let score = compute_score(
            &ScoreInputs {
                profit_per_ton,
                cargo_units: units,
                age_minutes: age_min,
                traffic_percentile: 0.5,
                touches_fleet_carrier: p.touches_fc != 0,
                trip_jumps: total_jumps,
            },
            weights,
        );

        let route_hash = format!(
            "{}:{}:{}",
            p.from_station_id, p.to_station_id, p.commodity_symbol
        );

        let sustainability = if p.demand >= units.max(1) * 10 {
            Sustainability::Sustainable
        } else {
            Sustainability::Decaying {
                estimated_cycles: (p.demand / units.max(1)).max(1),
            }
        };

        out.push(RankedRoute {
            mode: RouteMode::Single,
            legs: vec![RouteLeg {
                from_system: p.from_system,
                from_station: p.from_station,
                to_system: p.to_system,
                to_station: p.to_station,
                commodity: p.commodity_symbol,
                buy_price: p.buy_price,
                sell_price: p.sell_price,
                profit_per_ton,
                supply: p.supply,
                demand: p.demand,
                jumps: jumps_leg1,
                distance_ly: leg1_dist,
                recorded_at: recorded,
            }],
            cr_per_hour: cphr,
            profit_per_cycle: profit,
            cycle_seconds,
            total_jumps,
            sustainability,
            score,
            freshest_age_seconds: (age_min * 60.0) as i32,
            touches_fleet_carrier: p.touches_fc != 0,
            route_hash,
        });
    }

    out.sort_by(|a, b| b.cr_per_hour.cmp(&a.cr_per_hour));
    out.truncate(limit.max(1) as usize);
    Ok(out)
}
