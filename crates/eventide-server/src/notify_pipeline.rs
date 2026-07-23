//! Shared notify + persist helpers for scheduler and ingress.

use crate::state::AppState;
use chrono::{DateTime, Utc};
use eventide_core::{
    enrich_alert, AlertEvent, AlertStatus, AlertTransition, Comparator, IngressRoute, NotifyLog,
    Rule,
};
use std::collections::BTreeMap;
use std::sync::Arc;
use uuid::Uuid;

pub async fn persist_and_notify(
    state: &Arc<AppState>,
    rule: &Rule,
    channel_ids: &[Uuid],
    mut event: AlertEvent,
    transition: AlertTransition,
    existing_had_fp: bool,
    now: DateTime<Utc>,
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

    if should_notify {
        for ch_id in channel_ids {
            if let Some(ch) = state.db.get_channel(*ch_id)? {
                let result = state.notifier.send(&ch, rule, &event, transition).await;
                let (success, error) = match result {
                    Ok(()) => (true, None),
                    Err(e) => (false, Some(e.to_string())),
                };
                let _ = state.db.insert_notify_log(&NotifyLog {
                    id: Uuid::new_v4(),
                    alert_id: event.id,
                    channel_id: ch.id,
                    transition,
                    success,
                    error,
                    created_at: now,
                });
            }
        }
        match transition {
            AlertTransition::BecameFiring => event.notified_firing = true,
            AlertTransition::BecameResolved => event.notified_resolved = true,
            AlertTransition::Unchanged => {}
        }
    }

    state.db.upsert_alert(&event)?;
    Ok(())
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
