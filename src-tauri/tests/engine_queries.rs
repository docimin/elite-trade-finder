use chrono::Utc;
use elite_trade_finder_lib::db;
use elite_trade_finder_lib::engine::queries;
use tempfile::TempDir;

async fn setup() -> db::Db {
    let tmp = TempDir::new().unwrap();
    let url = db::default_sqlite_url(tmp.path());
    let conn = db::connect(&url).await.unwrap();
    db::migrations::run(&conn).await.unwrap();
    db::seed::commodities(&conn).await.unwrap();

    let pool = match &conn {
        db::Db::Sqlite(p) => p.clone(),
        _ => panic!(),
    };
    for (id, name, x, y, z) in [
        (1i64, "Sol", 0.0, 0.0, 0.0),
        (2, "LHS 3006", 5.0, 2.0, 1.0),
        (3, "Far System", 200.0, 200.0, 200.0),
    ] {
        sqlx::query("INSERT INTO systems (id64, name, x, y, z) VALUES (?, ?, ?, ?, ?)")
            .bind(id)
            .bind(name)
            .bind(x)
            .bind(y)
            .bind(z)
            .execute(&pool)
            .await
            .unwrap();
    }
    for (sid, sys, name, pad, fc) in [
        (100i64, "Sol", "Abraham Lincoln", "L", 0),
        (200, "LHS 3006", "Some Station", "L", 0),
        (300, "Far System", "Far Station", "L", 0),
        (400, "Sol", "Tiny Outpost", "S", 0),
    ] {
        sqlx::query("INSERT INTO stations (station_id, system_name, station_name, pad_size, is_fleet_carrier, last_seen_at) VALUES (?, ?, ?, ?, ?, ?)")
            .bind(sid).bind(sys).bind(name).bind(pad).bind(fc).bind(Utc::now().to_rfc3339())
            .execute(&pool).await.unwrap();
    }
    sqlx::query("INSERT OR REPLACE INTO user_state (user_id, current_system, cargo_capacity, jump_range_ly, pad_size_max, updated_at) VALUES ('test', 'Sol', 500, 25.0, 'L', ?)")
        .bind(Utc::now().to_rfc3339()).execute(&pool).await.unwrap();
    conn
}

#[tokio::test]
async fn finds_reachable_stations_within_jump_range() {
    let conn = setup().await;
    let reachable = queries::reachable_stations(&conn, "test", 3).await.unwrap();
    let names: Vec<_> = reachable.iter().map(|s| s.station_name.as_str()).collect();
    assert!(names.contains(&"Abraham Lincoln"));
    assert!(names.contains(&"Some Station"));
    assert!(!names.contains(&"Far Station"), "Far System is beyond jump range");
    assert!(!names.contains(&"Tiny Outpost"), "pad size mismatch");
}
