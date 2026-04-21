use crate::types::*;
use tauri::{AppHandle, Emitter};

pub const ROUTES_UPDATED: &str = "routes_updated";
pub const ROUTE_ALERT: &str = "route_alert";
pub const FIREHOSE_TICK: &str = "firehose_tick";
pub const USER_STATE_CHANGED: &str = "user_state_changed";
pub const SPANSH_PROGRESS: &str = "spansh_progress";

pub fn emit_routes(app: &AppHandle, routes: &[RankedRoute]) {
    let _ = app.emit(ROUTES_UPDATED, routes);
}

pub fn emit_alert(app: &AppHandle, route: &RankedRoute) {
    let _ = app.emit(ROUTE_ALERT, route);
}

pub fn emit_firehose(app: &AppHandle, tick: &FirehoseTick) {
    let _ = app.emit(FIREHOSE_TICK, tick);
}

pub fn emit_user_state(app: &AppHandle, us: &UserState) {
    let _ = app.emit(USER_STATE_CHANGED, us);
}

pub fn emit_spansh(app: &AppHandle, p: &SpanshProgress) {
    let _ = app.emit(SPANSH_PROGRESS, p);
}
