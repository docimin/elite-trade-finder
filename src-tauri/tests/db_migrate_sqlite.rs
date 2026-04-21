use elite_trade_finder_lib::db;
use tempfile::TempDir;

#[tokio::test]
async fn sqlite_migrations_create_all_tables() {
    let tmp = TempDir::new().unwrap();
    let url = db::default_sqlite_url(tmp.path());
    let conn = db::connect(&url).await.unwrap();
    db::migrations::run(&conn).await.unwrap();

    let pool = match &conn {
        db::Db::Sqlite(p) => p.clone(),
        _ => panic!("expected sqlite"),
    };

    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' AND name NOT LIKE '_sqlx_%' ORDER BY name",
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    let names: Vec<&str> = rows.iter().map(|r| r.0.as_str()).collect();
    for expected in [
        "alert_log",
        "commodities",
        "market_snapshots",
        "settings",
        "stations",
        "systems",
        "user_state",
    ] {
        assert!(names.contains(&expected), "missing table: {expected}");
    }
}
