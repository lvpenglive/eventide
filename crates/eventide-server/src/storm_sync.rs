//! Multi-instance storm prefs: Redis SET + PUBLISH → peer hot-reload (no restart).

use crate::config::StormConfig;
use crate::state::AppState;
use eventide_trap_data::{run_resilient_pubsub, STORM_CONFIG_CHANNEL};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Notify;

/// Subscribe to storm config changes and hot-apply on every eventide-server peer.
pub fn spawn_storm_config_pubsub(state: Arc<AppState>) {
    let Some(redis) = state.policy_redis.clone() else {
        return;
    };

    let notify = Arc::new(Notify::new());
    let notify_sub = notify.clone();
    let client = redis.client().clone();
    std::thread::Builder::new()
        .name("storm-config-pubsub".into())
        .spawn(move || {
            run_resilient_pubsub(
                &client,
                &[STORM_CONFIG_CHANNEL],
                || notify_sub.notify_one(),
                |_ch| notify_sub.notify_one(),
            );
        })
        .expect("spawn storm config pubsub thread");

    let redis_async = redis;
    tokio::spawn(async move {
        loop {
            notify.notified().await;
            // Coalesce bursts (publisher also receives its own message).
            loop {
                tokio::select! {
                    biased;
                    _ = notify.notified() => {}
                    _ = tokio::time::sleep(Duration::from_millis(80)) => break,
                }
            }
            match redis_async.load_storm_config() {
                Ok(Some(raw)) => match serde_json::from_str::<StormConfig>(&raw) {
                    Ok(c) => {
                        let c = c.normalize();
                        if let Err(e) = c.validate() {
                            tracing::warn!(error = %e, "storm config from redis invalid; ignored");
                            continue;
                        }
                        state.set_storm_prefs(c);
                        tracing::info!("storm config hot-reloaded via redis pubsub");
                    }
                    Err(e) => tracing::warn!(error = %e, "parse storm config from redis failed"),
                },
                Ok(None) => {
                    state.set_storm_prefs(state.config.storm.clone());
                    tracing::info!("storm config reset to toml via redis pubsub");
                }
                Err(e) => tracing::warn!(error = %e, "storm config reload failed"),
            }
        }
    });
}
