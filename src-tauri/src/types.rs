use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/")]
pub struct UserState {
    pub current_system: Option<String>,
    pub current_station: Option<String>,
    pub ship_type: Option<String>,
    pub cargo_capacity: Option<i32>,
    pub jump_range_ly: Option<f64>,
    pub credits: Option<i64>,
    pub pad_size_max: Option<String>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, TS)]
#[ts(export, export_to = "../../src/types/")]
#[serde(rename_all = "snake_case")]
pub enum RouteMode {
    Single,
    Loop2,
    Loop3,
    Loop4,
    RareChain,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/")]
pub struct RouteLeg {
    pub from_system: String,
    pub from_station: String,
    pub to_system: String,
    pub to_station: String,
    pub commodity: String,
    pub buy_price: i32,
    pub sell_price: i32,
    pub profit_per_ton: i32,
    pub supply: i32,
    pub demand: i32,
    pub jumps: i32,
    pub distance_ly: f64,
    pub recorded_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/")]
pub struct RankedRoute {
    pub mode: RouteMode,
    pub legs: Vec<RouteLeg>,
    pub cr_per_hour: i64,
    pub profit_per_cycle: i64,
    pub cycle_seconds: i32,
    pub total_jumps: i32,
    pub sustainability: Sustainability,
    pub score: f64,
    pub freshest_age_seconds: i32,
    pub touches_fleet_carrier: bool,
    pub route_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/")]
#[serde(rename_all = "snake_case")]
pub enum Sustainability {
    Sustainable,
    Decaying { estimated_cycles: i32 },
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/")]
pub struct RouteFilter {
    pub modes: Option<Vec<RouteMode>>,
    pub max_jumps: Option<i32>,
    pub min_cr_per_hour: Option<i64>,
    pub max_profit_per_ton: Option<i32>,
    pub pad_size_min: Option<String>,
    pub allow_anarchy: bool,
    pub require_fleet_carrier: bool,
    pub exclude_fleet_carrier: bool,
    pub limit: i32,
}

impl Default for RouteFilter {
    fn default() -> Self {
        Self {
            modes: None,
            max_jumps: Some(20),
            min_cr_per_hour: None,
            max_profit_per_ton: Some(300_000),
            pad_size_min: None,
            allow_anarchy: true,
            require_fleet_carrier: false,
            exclude_fleet_carrier: false,
            limit: 50,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/")]
pub struct Settings {
    pub score_weights: ScoreWeights,
    pub alerts: AlertSettings,
    pub data_sources: DataSourceSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/")]
pub struct ScoreWeights {
    pub freshness: f64,
    pub niche: f64,
    pub fleet_carrier: f64,
    pub reachability: f64,
}

impl Default for ScoreWeights {
    fn default() -> Self {
        Self {
            freshness: 1.0,
            niche: 1.0,
            fleet_carrier: 1.0,
            reachability: 1.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/")]
pub struct AlertSettings {
    pub desktop_enabled: bool,
    pub min_profit_per_ton: i32,
    pub min_cr_per_hour: i64,
    pub max_distance_ly: f64,
    pub cooldown_minutes: i32,
    pub webhook_url: Option<String>,
}

impl Default for AlertSettings {
    fn default() -> Self {
        Self {
            desktop_enabled: true,
            min_profit_per_ton: 50_000,
            min_cr_per_hour: 10_000_000,
            max_distance_ly: 30.0,
            cooldown_minutes: 30,
            webhook_url: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/")]
pub struct DataSourceSettings {
    pub eddn_relay_url: String,
    pub journal_dir: Option<String>,
    pub inara_api_key: Option<String>,
    pub spansh_galaxy_downloaded: bool,
    pub database_url: Option<String>,
}

impl Default for DataSourceSettings {
    fn default() -> Self {
        Self {
            eddn_relay_url: "tcp://eddn.edcd.io:9500".into(),
            journal_dir: None,
            inara_api_key: None,
            spansh_galaxy_downloaded: false,
            database_url: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/")]
pub struct Diagnostics {
    pub db_dialect: String,
    pub db_bytes: i64,
    pub snapshot_count: i64,
    pub oldest_snapshot: Option<DateTime<Utc>>,
    pub newest_snapshot: Option<DateTime<Utc>>,
    pub eddn_connected: bool,
    pub eddn_msgs_per_sec: f64,
    pub eddn_last_msg_at: Option<DateTime<Utc>>,
    pub journal_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/")]
pub struct ShipSpec {
    pub ship_type: String,
    pub cargo_capacity: i32,
    pub jump_range_ly: f64,
    pub pad_size_max: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/")]
pub struct FirehoseTick {
    pub at: DateTime<Utc>,
    pub system: String,
    pub station: String,
    pub commodities_updated: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/")]
#[serde(rename_all = "snake_case")]
pub enum SpanshPhase {
    Downloading,
    Importing,
    Done,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../src/types/")]
pub struct SpanshProgress {
    pub phase: SpanshPhase,
    pub bytes_done: i64,
    pub bytes_total: Option<i64>,
    pub systems_imported: i64,
    pub message: Option<String>,
}
