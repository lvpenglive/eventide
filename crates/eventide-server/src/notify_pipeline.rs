//! Shared notify + persist helpers for scheduler and ingress.

use crate::state::AppState;
use chrono::{DateTime, Utc};
use eventide_core::{
    build_group_key, build_throttle_key, enrich_alert, format_aggregate_sample,
    format_aggregate_text, format_notify_log_body, AlertEvent, AlertStatus, AlertTransition,
    AggregatePushResult, Comparator, IngressRoute, NotifyLog, Rule, ThrottleDecision,
};
use std::collections::BTreeMap;
use std::sync::Arc;
use tokio::sync::{mpsc, Semaphore};
use uuid::Uuid;

/// Max queued notify jobs (Kafka backpressure when full).
const NOTIFY_QUEUE_CAPACITY: usize = 1024;
/// Concurrent outbound notify HTTP calls.
const NOTIFY_WORKERS: usize = 8;

pub struct NotifyJob {
    pub rule: Rule,
    pub channel_ids: Vec<Uuid>,
    pub event: AlertEvent,
    pub transition: AlertTransition,
    pub now: DateTime<Utc>,
}

/// Outbound notify queue — persist first, send in background workers.
#[derive(Clone)]
pub struct NotifyQueue {
    tx: mpsc::Sender<NotifyJob>,
}

impl NotifyQueue {
    pub fn new() -> (Self, mpsc::Receiver<NotifyJob>) {
        let (tx, rx) = mpsc::channel(NOTIFY_QUEUE_CAPACITY);
        (Self { tx }, rx)
    }

    pub async fn enqueue(&self, job: NotifyJob) -> Result<(), mpsc::error::SendError<NotifyJob>> {
        self.tx.send(job).await
    }
}

pub fn spawn_notify_workers(state: Arc<AppState>, mut rx: mpsc::Receiver<NotifyJob>) {
    tokio::spawn(async move {
        let sem = Arc::new(Semaphore::new(NOTIFY_WORKERS));
        while let Some(job) = rx.recv().await {
            let permit = match sem.clone().acquire_owned().await {
                Ok(p) => p,
                Err(_) => break,
            };
            let state = state.clone();
            tokio::spawn(async move {
                let _permit = permit;
                if let Err(e) = dispatch_queued_notify(&state, job).await {
                    tracing::warn!("queued notify failed: {e:#}");
                }
            });
        }
    });
}

fn write_log(
    state: &AppState,
    alert_id: Uuid,
    channel_id: Uuid,
    transition: AlertTransition,
    success: bool,
    error: Option<String>,
    body: String,
    now: DateTime<Utc>,
) {
    let _ = state.db.insert_notify_log(&NotifyLog {
        id: Uuid::new_v4(),
        alert_id,
        channel_id,
        transition,
        success,
        error,
        body,
        created_at: now,
    });
}

/// Persist + notify inline (scheduler / HTTP ingress).
pub async fn persist_and_notify(
    state: &Arc<AppState>,
    rule: &Rule,
    channel_ids: &[Uuid],
    event: AlertEvent,
    transition: AlertTransition,
    existing_had_fp: bool,
    now: DateTime<Utc>,
) -> anyhow::Result<()> {
    persist_and_notify_inner(
        state,
        rule,
        channel_ids,
        event,
        transition,
        existing_had_fp,
        now,
        false,
    )
    .await
}

/// Persist immediately, enqueue notify (Kafka ingress — digests topic faster).
pub async fn persist_and_queue_notify(
    state: &Arc<AppState>,
    rule: &Rule,
    channel_ids: &[Uuid],
    event: AlertEvent,
    transition: AlertTransition,
    existing_had_fp: bool,
    now: DateTime<Utc>,
) -> anyhow::Result<()> {
    persist_and_notify_inner(
        state,
        rule,
        channel_ids,
        event,
        transition,
        existing_had_fp,
        now,
        true,
    )
    .await
}

async fn persist_and_notify_inner(
    state: &Arc<AppState>,
    rule: &Rule,
    channel_ids: &[Uuid],
    mut event: AlertEvent,
    transition: AlertTransition,
    existing_had_fp: bool,
    now: DateTime<Utc>,
    queue_notify: bool,
) -> anyhow::Result<()> {
    // Skip brand-new resolved placeholders with no prior state.
    if event.status == AlertStatus::Resolved
        && transition == AlertTransition::Unchanged
        && !existing_had_fp
    {
        return Ok(());
    }

    // Enrich after fingerprint is fixed, before silence / notify.
    let enrich_rules = state.db.list_enrich_rules().unwrap_or_default();
    let lookups = state.db.lookup_tables_map().unwrap_or_default();
    enrich_alert(&mut event, Some(&rule.name), &enrich_rules, &lookups);

    let silences = state.db.active_silences(now)?;
    let silenced = silences
        .iter()
        .any(|s| s.matches(rule.id, &event.labels, now));

    let should_notify = match transition {
        AlertTransition::BecameFiring if !event.notified_firing && !silenced => true,
        AlertTransition::BecameResolved if !event.notified_resolved && !silenced => true,
        _ => false,
    };

    if !should_notify {
        state.db.upsert_alert(&event)?;
        maybe_index_alert(state, &event);
        return Ok(());
    }

    state.pressure.note_notify_attempt(now);
    if state.pressure.should_skip_notify(now) {
        tracing::warn!(
            inflight = state.pressure.inflight(),
            rate = state.pressure.notify_rate(now),
            "notify skipped by storm degrade"
        );
        for ch_id in channel_ids {
            if let Some(ch) = state.db.get_channel(*ch_id)? {
                let body = format_notify_log_body(&ch, rule, &event, transition);
                write_log(
                    state,
                    event.id,
                    ch.id,
                    transition,
                    false,
                    Some("degraded".into()),
                    body,
                    now,
                );
            }
        }
        match transition {
            AlertTransition::BecameFiring => event.notified_firing = true,
            AlertTransition::BecameResolved => event.notified_resolved = true,
            AlertTransition::Unchanged => {}
        }
        state.db.upsert_alert(&event)?;
        maybe_index_alert(state, &event);
        return Ok(());
    }

    if queue_notify {
        // Land alert in DB first so Kafka offset can advance without waiting on HTTP.
        state.db.upsert_alert(&event)?;
        maybe_index_alert(state, &event);
        let job = NotifyJob {
            rule: rule.clone(),
            channel_ids: channel_ids.to_vec(),
            event,
            transition,
            now,
        };
        if let Err(e) = state.notify_queue.enqueue(job).await {
            // Channel closed — fall back to inline dispatch.
            tracing::warn!("notify queue closed, falling back inline");
            let mut job = e.0;
            run_channel_notifies(
                state,
                &job.rule,
                &job.channel_ids,
                &mut job.event,
                job.transition,
                job.now,
            )
            .await?;
            state.db.upsert_alert(&job.event)?;
            maybe_index_alert(state, &job.event);
        }
        return Ok(());
    }

    run_channel_notifies(state, rule, channel_ids, &mut event, transition, now).await?;
    state.db.upsert_alert(&event)?;
    maybe_index_alert(state, &event);
    Ok(())
}

fn maybe_index_alert(state: &Arc<AppState>, event: &AlertEvent) {
    if !state.alert_history_prefs().write_to_es {
        return;
    }
    let Some(es) = state.es_client() else {
        return;
    };
    let event = event.clone();
    tokio::spawn(async move {
        if let Err(e) = es.index_alert(&event).await {
            tracing::warn!(alert = %event.id, error = %e, "elasticsearch index failed");
        }
    });
}

async fn dispatch_queued_notify(state: &Arc<AppState>, mut job: NotifyJob) -> anyhow::Result<()> {
    run_channel_notifies(
        state,
        &job.rule,
        &job.channel_ids,
        &mut job.event,
        job.transition,
        job.now,
    )
    .await?;
    state.db.upsert_alert(&job.event)?;
    maybe_index_alert(state, &job.event);
    Ok(())
}

async fn run_channel_notifies(
    state: &Arc<AppState>,
    rule: &Rule,
    channel_ids: &[Uuid],
    event: &mut AlertEvent,
    transition: AlertTransition,
    now: DateTime<Utc>,
) -> anyhow::Result<()> {
    let agg_cfg = state.aggregate.config();
    let sample_labels = &agg_cfg.sample_labels;
    let group_by = &agg_cfg.group_by;
    for ch_id in channel_ids {
        if let Some(ch) = state.db.get_channel(*ch_id)? {
            let body = format_notify_log_body(&ch, rule, event, transition);
            // P1: aggregate BecameFiring only.
            let agg_action = if transition == AlertTransition::BecameFiring
                && state.aggregate.enabled()
            {
                let gkey = build_group_key(group_by, &event.labels);
                let sample = format_aggregate_sample(sample_labels, &event.labels);
                state.aggregate.push(
                    &ch.id.to_string(),
                    &gkey,
                    &event.fingerprint,
                    sample,
                    now,
                )
            } else {
                AggregatePushResult::Bypass
            };

            match agg_action {
                AggregatePushResult::Buffered => {
                    write_log(
                        state,
                        event.id,
                        ch.id,
                        transition,
                        false,
                        Some("aggregated".into()),
                        body,
                        now,
                    );
                    continue;
                }
                AggregatePushResult::SendHead | AggregatePushResult::Bypass => {}
            }

            let tkey = if matches!(agg_action, AggregatePushResult::SendHead) {
                build_group_key(group_by, &event.labels)
            } else {
                build_throttle_key(
                    &state.storm_prefs().throttle_key,
                    &event.fingerprint,
                    &event.labels,
                )
            };

            if !send_with_throttle(state, &ch, rule, event, transition, &tkey, &body, now).await?
            {
                continue;
            }
        }
    }
    match transition {
        AlertTransition::BecameFiring => event.notified_firing = true,
        AlertTransition::BecameResolved => event.notified_resolved = true,
        AlertTransition::Unchanged => {}
    }
    Ok(())
}

async fn send_with_throttle(
    state: &Arc<AppState>,
    ch: &eventide_core::NotifyChannel,
    rule: &Rule,
    event: &AlertEvent,
    transition: AlertTransition,
    tkey: &str,
    body: &str,
    now: DateTime<Utc>,
) -> anyhow::Result<bool> {
    let decision = state
        .throttle
        .allow(&ch.id.to_string(), tkey, transition, now);
    if matches!(
        decision,
        ThrottleDecision::DenyMinInterval | ThrottleDecision::DenyWindowLimit
    ) {
        tracing::debug!(
            channel = %ch.name,
            key = %tkey,
            ?decision,
            "notify skipped by storm throttle"
        );
        write_log(
            state,
            event.id,
            ch.id,
            transition,
            false,
            Some("throttled".into()),
            body.to_string(),
            now,
        );
        return Ok(false);
    }

    let result = state.notifier.send(ch, rule, event, transition).await;
    let (success, error) = match result {
        Ok(()) => (true, None),
        Err(e) => (false, Some(e.to_string())),
    };
    write_log(
        state,
        event.id,
        ch.id,
        transition,
        success,
        error,
        body.to_string(),
        now,
    );
    Ok(true)
}

/// Flush due aggregate summaries (call from background tick).
pub async fn flush_aggregates(state: &Arc<AppState>) {
    if !state.aggregate.enabled() {
        return;
    }
    let now = Utc::now();
    let due = state.aggregate.take_due(now);
    for summary in due {
        let Ok(ch_id) = Uuid::parse_str(&summary.channel_id) else {
            continue;
        };
        let Ok(Some(ch)) = state.db.get_channel(ch_id) else {
            continue;
        };
        let text = format_aggregate_text(&summary);
        let decision = state.throttle.allow(
            &summary.channel_id,
            &summary.group_key,
            AlertTransition::BecameFiring,
            now,
        );
        if matches!(
            decision,
            ThrottleDecision::DenyMinInterval | ThrottleDecision::DenyWindowLimit
        ) {
            tracing::debug!(
                group = %summary.group_key,
                count = summary.count,
                "aggregate summary throttled"
            );
            write_log(
                state,
                Uuid::nil(),
                ch.id,
                AlertTransition::BecameFiring,
                false,
                Some("throttled".into()),
                text,
                now,
            );
            continue;
        }
        match state.notifier.send_text(&ch, &text).await {
            Ok(()) => {
                tracing::info!(
                    group = %summary.group_key,
                    count = summary.count,
                    channel = %ch.name,
                    "aggregate summary sent"
                );
                write_log(
                    state,
                    Uuid::nil(),
                    ch.id,
                    AlertTransition::BecameFiring,
                    true,
                    Some(format!("aggregate:{}:{}", summary.group_key, summary.count)),
                    text,
                    now,
                );
            }
            Err(e) => {
                tracing::warn!(error = %e, "aggregate summary send failed");
                write_log(
                    state,
                    Uuid::nil(),
                    ch.id,
                    AlertTransition::BecameFiring,
                    false,
                    Some(e.to_string()),
                    text,
                    now,
                );
            }
        }
    }
}

pub fn spawn_aggregate_flusher(state: Arc<AppState>) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
        loop {
            interval.tick().await;
            if !state.leader.is_leader() {
                continue;
            }
            flush_aggregates(&state).await;
        }
    });
}

/// Synthetic rule used only for notification text when alerts come from ingress.
pub fn synthetic_rule_for_ingress(route: &IngressRoute, event: &AlertEvent) -> Rule {
    let name = eventide_core::ingress_alert_name(route, &event.labels);
    Rule {
        id: route.id,
        name,
        datasource_id: Uuid::nil(),
        expr: format!("ingress:{}", route.kind.as_str()),
        comparator: Comparator::Gt,
        threshold: 0.0,
        for_seconds: 0,
        interval_seconds: 0,
        severity: event.severity,
        labels: BTreeMap::new(),
        annotations: BTreeMap::new(),
        channel_ids: route.channel_ids.clone(),
        enabled: true,
        created_at: route.created_at,
        updated_at: route.updated_at,
    }
}
