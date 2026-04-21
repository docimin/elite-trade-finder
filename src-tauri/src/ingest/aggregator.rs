use crate::db::Db;
use anyhow::Result;

pub async fn fill_gap_if_stale(_db: &Db, _station_id: i64) -> Result<()> {
    Ok(())
}
