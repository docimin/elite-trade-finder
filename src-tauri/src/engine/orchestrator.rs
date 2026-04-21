use super::{loops, rare_chains, single_hop};
use crate::events;
use crate::state::AppState;
use crate::types::RankedRoute;
use tauri::AppHandle;

pub async fn recompute_all(app: &AppHandle, state: &AppState) {
    let weights = state.settings.read().await.score_weights.clone();
    let user_id = state.user_id.as_str();

    let singles_r = single_hop::find(&state.db, user_id, &weights, 200).await;
    let loops2_r = loops::find_two_leg(&state.db, user_id, &weights, 200).await;
    let loops_multi_r = loops::find_multi_leg(&state.db, user_id, &weights, 4, 200).await;
    let rares_r = rare_chains::find(&state.db, user_id, &weights, 50).await;

    // If every query errored (e.g. DB connection blip), keep the previous
    // top_routes visible rather than wiping the UI to "No routes yet".
    let all_errored = singles_r.is_err()
        && loops2_r.is_err()
        && loops_multi_r.is_err()
        && rares_r.is_err();
    if all_errored {
        tracing::warn!(
            singles_err = ?singles_r.as_ref().err().map(|e| format!("{:#}", e)),
            "all route queries failed — keeping previous top_routes"
        );
        return;
    }

    let singles = singles_r.unwrap_or_else(|e| {
        tracing::warn!(error = %format!("{:#}", e), "single_hop::find failed");
        Vec::new()
    });
    let loops2 = loops2_r.unwrap_or_else(|e| {
        tracing::warn!(error = %format!("{:#}", e), "loops::find_two_leg failed");
        Vec::new()
    });
    let loops_multi = loops_multi_r.unwrap_or_else(|e| {
        tracing::warn!(error = %format!("{:#}", e), "loops::find_multi_leg failed");
        Vec::new()
    });
    let rares = rares_r.unwrap_or_else(|e| {
        tracing::warn!(error = %format!("{:#}", e), "rare_chains::find failed");
        Vec::new()
    });

    tracing::info!(
        singles = singles.len(),
        loops2 = loops2.len(),
        loops_multi = loops_multi.len(),
        rares = rares.len(),
        "recompute_all finished"
    );

    // Also guard against "no errors but all empty" — this happens when
    // user_state hasn't been written yet on a fresh boot. Keep prior routes
    // visible until we have a real answer.
    let all_empty = singles.is_empty() && loops2.is_empty() && loops_multi.is_empty() && rares.is_empty();
    let prior_non_empty = !state.top_routes.read().await.is_empty();
    if all_empty && prior_non_empty {
        tracing::debug!("recompute returned 0 routes; keeping prior top_routes to avoid UI flicker");
        return;
    }

    // Cap each mode's contribution BEFORE merging so no single mode (usually
    // loop3/4, which compound profit across legs) can crowd out the others.
    // Otherwise the global sort-then-truncate-to-200 can drop every single-hop
    // and 2-leg result if loops happen to score higher.
    const PER_MODE_CAP: usize = 75;
    let mut all: Vec<RankedRoute> = Vec::new();
    all.extend(singles.into_iter().take(PER_MODE_CAP));
    all.extend(loops2.into_iter().take(PER_MODE_CAP));
    all.extend(loops_multi.into_iter().take(PER_MODE_CAP));
    all.extend(rares.into_iter().take(PER_MODE_CAP));
    all.sort_by(|a, b| b.cr_per_hour.cmp(&a.cr_per_hour));

    *state.top_routes.write().await = all.clone();
    events::emit_routes(app, &all);

    let settings = state.settings.read().await.clone();
    let min = settings.alerts.min_cr_per_hour;
    for route in all.iter().take(20) {
        if route.cr_per_hour < min {
            break;
        }
        crate::alerts::dispatcher::dispatch(app, state, route).await;
    }
}

pub fn spawn_periodic(app: AppHandle, state: AppState) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(std::time::Duration::from_secs(10));
        loop {
            ticker.tick().await;
            recompute_all(&app, &state).await;
        }
    })
}
