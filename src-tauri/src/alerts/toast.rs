use crate::types::RankedRoute;
use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;

pub fn fire(app: &AppHandle, r: &RankedRoute) {
    let (from, to) = match r.legs.first() {
        Some(l) => (l.from_station.clone(), l.to_station.clone()),
        None => return,
    };
    let mode_label = match r.mode {
        crate::types::RouteMode::Single => "SINGLE",
        crate::types::RouteMode::Loop2 => "LOOP-2",
        crate::types::RouteMode::Loop3 => "LOOP-3",
        crate::types::RouteMode::Loop4 => "LOOP-4",
        crate::types::RouteMode::RareChain => "RARE",
    };
    let title = format!(
        "{:.1}M cr/hr {mode_label}",
        r.cr_per_hour as f64 / 1_000_000.0
    );
    let body = format!(
        "{} → {} · {} ly · {}s ago",
        from,
        to,
        r.legs.iter().map(|l| l.distance_ly).sum::<f64>().round() as i32,
        r.freshest_age_seconds
    );
    let _ = app.notification().builder().title(title).body(body).show();
}
