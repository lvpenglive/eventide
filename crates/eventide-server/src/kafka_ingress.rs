//! Kafka ingress: consume alert JSON from topics.

use crate::notify_pipeline::{persist_and_notify, synthetic_rule_for_ingress};
use crate::state::AppState;
use chrono::Utc;
use eventide_core::{
    apply_ingress, IngressKind, IngressRoute,
};
use eventide_sources::{earliest_offset, fetch_records_from, latest_offset};
use std::sync::Arc;
use std::time::Duration;

pub fn spawn_kafka_ingress(state: Arc<AppState>) {
    let tick = state.config.scheduler_tick_seconds.max(2);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(tick));
        loop {
            interval.tick().await;
            if let Err(e) = poll_once(state.clone()).await {
                tracing::warn!("kafka ingress poll failed: {e:#}");
            }
        }
    });
}

async fn poll_once(state: Arc<AppState>) -> anyhow::Result<()> {
    let routes = state.db.list_ingress_routes()?;
    for route in routes {
        if !route.enabled || route.kind != IngressKind::Kafka {
            continue;
        }
        if let Err(e) = poll_route(state.clone(), &route).await {
            tracing::warn!(route = %route.name, "kafka ingress route failed: {e:#}");
        }
    }
    Ok(())
}

async fn poll_route(state: Arc<AppState>, route: &IngressRoute) -> anyhow::Result<()> {
    let brokers: Vec<String> = route
        .endpoint
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if brokers.is_empty() {
        anyhow::bail!("kafka ingress missing endpoint (brokers)");
    }
    let topic = route
        .options
        .get("topic")
        .cloned()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| anyhow::anyhow!("kafka ingress missing options.topic"))?;

    let max_partitions: i32 = route
        .options
        .get("partitions")
        .and_then(|s| s.parse().ok())
        .unwrap_or(8);

    for partition in 0..max_partitions {
        let start = match state.db.get_kafka_offset(route.id, partition)? {
            Some(o) => o,
            None => {
                // Default: start at latest (only new alerts). Use earliest if options.start=earliest
                let start_at = route
                    .options
                    .get("start")
                    .map(|s| s.as_str())
                    .unwrap_or("latest");
                if start_at == "earliest" {
                    match earliest_offset(&brokers, &topic, partition).await {
                        Ok(o) => o,
                        Err(_) if partition > 0 => break,
                        Err(e) => return Err(e.into()),
                    }
                } else {
                    match latest_offset(&brokers, &topic, partition).await {
                        Ok(o) => o,
                        Err(_) if partition > 0 => break,
                        Err(e) => return Err(e.into()),
                    }
                }
            }
        };

        let (payloads, high) =
            match fetch_records_from(&brokers, &topic, partition, start).await {
                Ok(v) => v,
                Err(_) if partition > 0 && state.db.get_kafka_offset(route.id, partition)?.is_none() => {
                    break;
                }
                Err(e) => return Err(e.into()),
            };

        for raw in payloads {
            if let Err(e) = ingest_payload(state.clone(), route, &raw).await {
                tracing::warn!(route = %route.name, "bad kafka alert payload: {e:#}");
            }
        }

        // Advance to high watermark (or keep start if nothing new).
        let next = high.max(start);
        state.db.set_kafka_offset(route.id, partition, next)?;
    }
    Ok(())
}

async fn ingest_payload(
    state: Arc<AppState>,
    route: &IngressRoute,
    raw: &[u8],
) -> anyhow::Result<()> {
    let alerts = eventide_core::parse_ingress_payload_with_options(raw, &route.options)
        .map_err(anyhow::Error::msg)?;

    let now = Utc::now();
    for incoming in alerts {
        let fp = incoming
            .fingerprint
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| eventide_core::alert_fingerprint(route.id, &incoming.labels));
        let full_fp = format!("{}:{}", route.id, fp);
        let existing = state.db.get_alert_by_fingerprint(&full_fp)?;
        let had = existing.is_some();
        let (event, transition) = apply_ingress(route, &incoming, existing, now);
        let rule = synthetic_rule_for_ingress(route, &event);
        persist_and_notify(
            &state,
            &rule,
            &route.channel_ids,
            event,
            transition,
            had,
            now,
        )
        .await?;
    }
    Ok(())
}
