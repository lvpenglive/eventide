//! Background rule evaluation scheduler.

use crate::notify_pipeline::persist_and_notify;
use crate::state::AppState;
use chrono::Utc;
use eventide_core::evaluate_rule;
use eventide_sources::fetch_samples;
use std::sync::Arc;
use std::time::Duration;

pub fn spawn_scheduler(state: Arc<AppState>) {
    let tick = state.config.scheduler_tick_seconds.max(1);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(tick));
        loop {
            interval.tick().await;
            if let Err(e) = run_once(state.clone()).await {
                tracing::error!("scheduler tick failed: {e:#}");
            }
        }
    });
}

pub async fn run_once(state: Arc<AppState>) -> anyhow::Result<()> {
    let now = Utc::now();
    let due = state.db.list_enabled_rules_due(now)?;
    for rule in due {
        if let Err(e) = evaluate_one(state.clone(), &rule, now).await {
            tracing::error!(rule = %rule.name, "evaluate failed: {e:#}");
        }
        let _ = state.db.touch_rule_run(rule.id, now);
    }
    Ok(())
}

pub async fn evaluate_one(
    state: Arc<AppState>,
    rule: &eventide_core::Rule,
    now: chrono::DateTime<Utc>,
) -> anyhow::Result<()> {
    let ds = state
        .db
        .get_datasource(rule.datasource_id)?
        .ok_or_else(|| anyhow::anyhow!("datasource {} not found", rule.datasource_id))?;

    if !ds.enabled {
        return Ok(());
    }

    let samples = fetch_samples(&ds, &rule.expr).await?;
    let existing = state.db.alerts_for_rule(rule.id)?;
    let results = evaluate_rule(rule, &samples, &existing, now);

    for (event, transition) in results {
        let had = existing.contains_key(&event.fingerprint);
        persist_and_notify(
            &state,
            rule,
            &rule.channel_ids,
            event,
            transition,
            had,
            now,
        )
        .await?;
    }

    Ok(())
}
