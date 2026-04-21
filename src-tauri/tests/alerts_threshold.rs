use chrono::Utc;
use elite_trade_finder_lib::alerts::dispatcher;
use elite_trade_finder_lib::db;
use elite_trade_finder_lib::types::*;
use tempfile::TempDir;

fn route(cr_hr: i64, ppt: i32, distance: f64, hash: &str) -> RankedRoute {
    RankedRoute {
        mode: RouteMode::Single,
        legs: vec![RouteLeg {
            from_system: "A".into(),
            from_station: "SA".into(),
            to_system: "B".into(),
            to_station: "SB".into(),
            commodity: "Gold".into(),
            buy_price: 1,
            sell_price: 1 + ppt,
            profit_per_ton: ppt,
            supply: 1000,
            demand: 1000,
            jumps: 1,
            distance_ly: distance,
            recorded_at: Utc::now(),
        }],
        cr_per_hour: cr_hr,
        profit_per_cycle: 1_000_000,
        cycle_seconds: 600,
        total_jumps: 1,
        sustainability: Sustainability::Sustainable,
        score: 1.0,
        freshest_age_seconds: 0,
        touches_fleet_carrier: false,
        route_hash: hash.into(),
    }
}

#[tokio::test]
async fn passes_all_thresholds() {
    let tmp = TempDir::new().unwrap();
    let url = db::default_sqlite_url(tmp.path());
    let conn = db::connect(&url).await.unwrap();
    db::migrations::run(&conn).await.unwrap();

    let settings = AlertSettings::default();
    let r = route(20_000_000, 100_000, 10.0, "A->B:Gold");
    assert!(dispatcher::should_fire(&conn, "test", &settings, &r).await.unwrap());
}

#[tokio::test]
async fn rejects_below_min_cr_per_hour() {
    let tmp = TempDir::new().unwrap();
    let url = db::default_sqlite_url(tmp.path());
    let conn = db::connect(&url).await.unwrap();
    db::migrations::run(&conn).await.unwrap();

    let settings = AlertSettings::default();
    let r = route(5_000_000, 100_000, 10.0, "A->B:Gold");
    assert!(!dispatcher::should_fire(&conn, "test", &settings, &r).await.unwrap());
}

#[tokio::test]
async fn cooldown_prevents_refire() {
    let tmp = TempDir::new().unwrap();
    let url = db::default_sqlite_url(tmp.path());
    let conn = db::connect(&url).await.unwrap();
    db::migrations::run(&conn).await.unwrap();

    let settings = AlertSettings::default();
    let r = route(20_000_000, 100_000, 10.0, "A->B:Gold");
    assert!(dispatcher::should_fire(&conn, "test", &settings, &r).await.unwrap());
    dispatcher::record_fire(&conn, "test", &r, "toast").await.unwrap();
    assert!(!dispatcher::should_fire(&conn, "test", &settings, &r).await.unwrap());
}
