//! Leader-only scan: unacked firing alerts past rule/ingress escalate timeout.

use crate::notify_pipeline::{escalate_alert_notify, rule_context_for_alert};
use crate::state::AppState;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;

pub fn spawn_escalation_loop(state: Arc<AppState>) {
    tokio::spawn(async move {
        let mut tick = interval(Duration::from_secs(10));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tick.tick().await;
            if !state.leader.is_leader() {
                continue;
            }
            if let Err(e) = run_escalation_once(&state).await {
                tracing::warn!(error = %e, "escalation scan failed");
            }
        }
    });
}

async fn run_escalation_once(state: &Arc<AppState>) -> anyhow::Result<()> {
    let now = chrono::Utc::now();
    let candidates = state.db.list_escalation_candidates()?;
    for event in candidates {
        let (rule, _) = match rule_context_for_alert(state, &event) {
            Ok(ctx) => ctx,
            Err(e) => {
                tracing::debug!(alert = %event.id, error = %e, "escalation skip: no rule context");
                continue;
            }
        };
        if rule.escalate_after_seconds == 0 {
            continue;
        }
        if let Err(e) = escalate_alert_notify(state, event, &rule, now).await {
            tracing::warn!(error = %e, "escalate notify failed");
        }
    }
    Ok(())
}
