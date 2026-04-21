use crate::db::Db;
use crate::types::*;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct AppState {
    pub db: Db,
    pub user_id: Arc<String>,
    pub top_routes: Arc<RwLock<Vec<RankedRoute>>>,
    pub user_state: Arc<RwLock<UserState>>,
    pub settings: Arc<RwLock<Settings>>,
    pub override_ship: Arc<RwLock<Option<ShipSpec>>>,
    pub eddn_status: Arc<RwLock<EddnStatus>>,
    pub journal_status: Arc<RwLock<String>>,
}

#[derive(Debug, Clone, Default)]
pub struct EddnStatus {
    pub connected: bool,
    pub msgs_per_sec: f64,
    pub last_msg_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl AppState {
    pub fn new(db: Db, user_id: String) -> Self {
        Self {
            db,
            user_id: Arc::new(user_id),
            top_routes: Arc::new(RwLock::new(Vec::new())),
            user_state: Arc::new(RwLock::new(UserState {
                current_system: None,
                current_station: None,
                ship_type: None,
                cargo_capacity: None,
                jump_range_ly: None,
                credits: None,
                pad_size_max: None,
                updated_at: chrono::Utc::now(),
            })),
            settings: Arc::new(RwLock::new(Settings {
                score_weights: Default::default(),
                alerts: Default::default(),
                data_sources: Default::default(),
            })),
            override_ship: Arc::new(RwLock::new(None)),
            eddn_status: Arc::new(RwLock::new(EddnStatus::default())),
            journal_status: Arc::new(RwLock::new("disconnected".into())),
        }
    }
}
