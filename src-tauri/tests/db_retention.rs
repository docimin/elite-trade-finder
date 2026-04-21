use chrono::{Duration, Utc};
use elite_trade_finder_lib::db;
use tempfile::TempDir;

async fn seed_fixture(conn: &db::Db) {
    let pool = match conn {
        db::Db::Sqlite(p) => p.clone(),
        _ => panic!(),
    };
    sqlx::query("INSERT INTO stations (station_id, system_name, station_name, last_seen_at) VALUES (1, 'Sol', 'Abraham Lincoln', ?)")
        .bind(Utc::now().to_rfc3339())
        .execute(&pool)
        .await
        .unwrap();
    db::seed::commodities(conn).await.unwrap();

    let now = Utc::now();
    let fresh = now - Duration::days(2);
    let stale = now - Duration::days(10);
    for t in [fresh, stale] {
        sqlx::query("INSERT INTO market_snapshots (station_id, commodity_id, buy_price, sell_price, supply, demand, recorded_at, source) VALUES (1, 128049152, 100, 150, 10, 10, ?, 'test')")
            .bind(t.to_rfc3339())
            .execute(&pool)
            .await
            .unwrap();
    }
    sqlx::query("INSERT INTO alert_log (route_hash, profit_per_ton, fired_at, channel) VALUES ('fresh', 1, ?, 'toast')")
        .bind(fresh.to_rfc3339())
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO alert_log (route_hash, profit_per_ton, fired_at, channel) VALUES ('stale', 1, ?, 'toast')")
        .bind(stale.to_rfc3339())
        .execute(&pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn prune_removes_rows_older_than_7_days() {
    let tmp = TempDir::new().unwrap();
    let url = db::default_sqlite_url(tmp.path());
    let conn = db::connect(&url).await.unwrap();
    db::migrations::run(&conn).await.unwrap();
    seed_fixture(&conn).await;

    let deleted = db::retention::prune_once(&conn).await.unwrap();
    assert_eq!(deleted.snapshots, 1);
    assert_eq!(deleted.alerts, 1);

    let pool = match &conn {
        db::Db::Sqlite(p) => p.clone(),
        _ => panic!(),
    };
    let (snap_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM market_snapshots")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(snap_count, 1);
    let (alert_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM alert_log")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(alert_count, 1);
}
