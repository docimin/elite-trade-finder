use super::Db;
use anyhow::Result;

/// Back-fill `is_fleet_carrier` on stations imported before the
/// "Drake-Class Carrier" detection was added. Cheap — one indexed UPDATE.
pub async fn fix_fleet_carrier_flags(db: &Db) -> Result<u64> {
    let updated = match db {
        Db::Sqlite(p) => sqlx::query(
            "UPDATE stations SET is_fleet_carrier = 1 \
             WHERE is_fleet_carrier = 0 AND station_type IS NOT NULL \
               AND LOWER(station_type) LIKE '%carrier%'",
        )
        .execute(p)
        .await?
        .rows_affected(),
        Db::Postgres(p) => sqlx::query(
            "UPDATE stations SET is_fleet_carrier = TRUE \
             WHERE is_fleet_carrier = FALSE AND station_type IS NOT NULL \
               AND LOWER(station_type) LIKE '%carrier%'",
        )
        .execute(p)
        .await?
        .rows_affected(),
    };
    if updated > 0 {
        tracing::info!(updated, "flagged pre-existing stations as fleet carriers");
    }
    Ok(updated)
}

/// Deduplicate the `systems` table on (name). Keeps the row with real coords
/// when available, otherwise the lowest id64. A prior version of the journal
/// watcher inserted synthetic-id rows for systems that Spansh had already
/// populated, which breaks scalar subqueries in the route engine.
pub async fn dedupe_systems(db: &Db) -> Result<u64> {
    let deleted = match db {
        Db::Sqlite(p) => sqlx::query(
            "DELETE FROM systems WHERE id64 NOT IN ( \
                 SELECT id64 FROM ( \
                     SELECT id64, ROW_NUMBER() OVER ( \
                         PARTITION BY name \
                         ORDER BY CASE WHEN x IS NULL THEN 1 ELSE 0 END, id64 \
                     ) AS rn \
                     FROM systems \
                 ) WHERE rn = 1 \
             )",
        )
        .execute(p)
        .await?
        .rows_affected(),
        Db::Postgres(p) => sqlx::query(
            "DELETE FROM systems WHERE id64 NOT IN ( \
                 SELECT id64 FROM ( \
                     SELECT id64, ROW_NUMBER() OVER ( \
                         PARTITION BY name \
                         ORDER BY CASE WHEN x IS NULL THEN 1 ELSE 0 END, id64 \
                     ) AS rn \
                     FROM systems \
                 ) t WHERE rn = 1 \
             )",
        )
        .execute(p)
        .await?
        .rows_affected(),
    };
    if deleted > 0 {
        tracing::info!(deleted, "deduped systems table");
    }
    Ok(deleted)
}
