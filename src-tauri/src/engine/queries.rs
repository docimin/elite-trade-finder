use crate::db::Db;
use anyhow::Result;

#[derive(Debug, Clone)]
pub struct Candidate {
    pub station_id: i64,
    pub system_name: String,
    pub station_name: String,
    pub pad_size: Option<String>,
    pub is_fleet_carrier: bool,
    pub coords: Option<(f64, f64, f64)>,
}

#[derive(sqlx::FromRow)]
struct RowSqlite {
    station_id: i64,
    system_name: String,
    station_name: String,
    pad_size: Option<String>,
    is_fleet_carrier: i64,
    x: Option<f64>,
    y: Option<f64>,
    z: Option<f64>,
}

fn pad_rank(pad: &str) -> i32 {
    match pad {
        "L" => 3,
        "M" => 2,
        "S" => 1,
        _ => 0,
    }
}

pub async fn reachable_stations(
    db: &Db,
    user_id: &str,
    max_jumps: i32,
) -> Result<Vec<Candidate>> {
    let row: Option<(Option<String>, Option<f64>, Option<String>)> = match db {
        Db::Sqlite(p) => sqlx::query_as(
            "SELECT current_system, jump_range_ly, pad_size_max FROM user_state WHERE user_id = ?",
        )
        .bind(user_id)
        .fetch_optional(p)
        .await?,
        Db::Postgres(p) => sqlx::query_as(
            "SELECT current_system, jump_range_ly, pad_size_max FROM user_state WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_optional(p)
        .await?,
    };
    let Some((system, jump_range, pad)) = row else {
        return Ok(vec![]);
    };
    let (Some(system), Some(jr), Some(pad)) = (system, jump_range, pad) else {
        return Ok(vec![]);
    };
    let min_rank = pad_rank(&pad);
    let radius = jr * (max_jumps as f64);

    let candidates: Vec<Candidate> = match db {
        Db::Sqlite(p) => {
            let rows: Vec<RowSqlite> = sqlx::query_as(
                "SELECT s.station_id, s.system_name, s.station_name, s.pad_size, s.is_fleet_carrier, sys.x, sys.y, sys.z \
                 FROM stations s \
                 LEFT JOIN systems sys ON sys.name = s.system_name \
                 CROSS JOIN (SELECT (SELECT x FROM systems WHERE name = ? LIMIT 1) AS ux, \
                                    (SELECT y FROM systems WHERE name = ? LIMIT 1) AS uy, \
                                    (SELECT z FROM systems WHERE name = ? LIMIT 1) AS uz) u \
                 WHERE u.ux IS NULL OR sys.x IS NULL OR ((sys.x - u.ux)*(sys.x - u.ux) + (sys.y - u.uy)*(sys.y - u.uy) + (sys.z - u.uz)*(sys.z - u.uz)) <= ? * ?",
            )
            .bind(&system)
            .bind(&system)
            .bind(&system)
            .bind(radius)
            .bind(radius)
            .fetch_all(p)
            .await?;
            rows.into_iter()
                .map(|r| Candidate {
                    station_id: r.station_id,
                    system_name: r.system_name,
                    station_name: r.station_name,
                    pad_size: r.pad_size,
                    is_fleet_carrier: r.is_fleet_carrier != 0,
                    coords: match (r.x, r.y, r.z) {
                        (Some(x), Some(y), Some(z)) => Some((x, y, z)),
                        _ => None,
                    },
                })
                .collect()
        }
        Db::Postgres(p) => {
            let rows: Vec<(
                i64,
                String,
                String,
                Option<String>,
                bool,
                Option<f64>,
                Option<f64>,
                Option<f64>,
            )> = sqlx::query_as(
                "SELECT s.station_id, s.system_name, s.station_name, s.pad_size, s.is_fleet_carrier, sys.x, sys.y, sys.z \
                 FROM stations s \
                 LEFT JOIN systems sys ON sys.name = s.system_name \
                 CROSS JOIN (SELECT (SELECT x FROM systems WHERE name = $1 LIMIT 1) AS ux, \
                                    (SELECT y FROM systems WHERE name = $1 LIMIT 1) AS uy, \
                                    (SELECT z FROM systems WHERE name = $1 LIMIT 1) AS uz) u \
                 WHERE u.ux IS NULL OR sys.x IS NULL OR ((sys.x - u.ux)*(sys.x - u.ux) + (sys.y - u.uy)*(sys.y - u.uy) + (sys.z - u.uz)*(sys.z - u.uz)) <= $2 * $2",
            )
            .bind(&system)
            .bind(radius)
            .fetch_all(p)
            .await?;
            rows.into_iter()
                .map(|r| Candidate {
                    station_id: r.0,
                    system_name: r.1,
                    station_name: r.2,
                    pad_size: r.3,
                    is_fleet_carrier: r.4,
                    coords: match (r.5, r.6, r.7) {
                        (Some(x), Some(y), Some(z)) => Some((x, y, z)),
                        _ => None,
                    },
                })
                .collect()
        }
    };
    Ok(candidates
        .into_iter()
        .filter(|c| match c.pad_size.as_deref() {
            Some(p) => pad_rank(p) >= min_rank,
            None => true,
        })
        .collect())
}
