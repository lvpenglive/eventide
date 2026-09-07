//! Background rule evaluation scheduler.

use crate::notify_pipeline::persist_and_notify;
use crate::state::AppState;
use chrono::Utc;
use eventide_core::evaluate_rule;
use eventide_sources::fetch_samples;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;

pub fn spawn_scheduler(state: Arc<AppState>) {
    let tick = state.config.scheduler_tick_seconds.max(1);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(tick));
        loop {
            interval.tick().await;
            if !state.leader.is_leader() {
                continue;
            }
            if let Err(e) = run_once(state.clone()).await {
                tracing::error!("scheduler tick failed: {e:#}");
            }
        }
    });
}

pub async fn run_once(state: Arc<AppState>) -> anyhow::Result<()> {
    let now = Utc::now();
    let due = state.db.list_enabled_rules_due(now)?;

    if due.is_empty() {
        return Ok(());
    }

    // 控制并发评估数量，避免过度并行导致系统过载
    const MAX_CONCURRENT_EVALS: usize = 8;
    let sem = Arc::new(Semaphore::new(MAX_CONCURRENT_EVALS));

    let mut tasks = Vec::with_capacity(due.len());
    for rule in &due {
        let state = state.clone();
        let sem = sem.clone();
        let rule_ref = rule.clone();
        let task = async move {
            let _permit = sem.acquire_owned().await;
            evaluate_one(state, &rule_ref, now).await
        };
        tasks.push(task);
    }

    // 并行执行评估任务
    let results = futures::future::join_all(tasks).await;

    // 触达规则运行时间（保持与原串行逻辑相同顺序）
    for rule in &due {
        let _ = state.db.touch_rule_run(rule.id, now);
    }

    for (_i, result) in results.into_iter().enumerate() {
        if let Err(e) = result {
            // 记录评估失败（use rule name for logging if needed)
            tracing::error!("evaluate failed: {e:#}");
        }
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
