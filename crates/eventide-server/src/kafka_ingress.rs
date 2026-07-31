//! Kafka ingress via Kafka consumer groups (samsa, pure Rust).
//!
//! Multi-instance scale-out uses Kafka JoinGroup / SyncGroup / Heartbeat /
//! OffsetCommit — no Redis partition leases.

use crate::notify_pipeline::{persist_and_queue_notify, synthetic_rule_for_ingress};
use crate::state::AppState;
use bytes::Bytes;
use chrono::Utc;
use eventide_core::{apply_ingress, IngressKind, IngressRoute};
use eventide_sources::{earliest_offset, latest_offset, topic_partition_count};
use samsa::prelude::{
    commit_offset, fetch_offset, find_coordinator, BrokerAddress, BrokerConnection,
    ConsumeMessage, ConsumerGroupBuilder, KafkaCode, PartitionOffsets, TcpConnection,
    TopicPartitionsBuilder,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinHandle;
use tokio_stream::StreamExt;
use uuid::Uuid;

pub fn spawn_kafka_ingress(state: Arc<AppState>) {
    tokio::spawn(async move {
        let mut running: HashMap<Uuid, (String, JoinHandle<()>)> = HashMap::new();
        let mut interval = tokio::time::interval(Duration::from_secs(8));
        loop {
            interval.tick().await;
            let routes = match state.db.list_ingress_routes() {
                Ok(r) => r,
                Err(e) => {
                    tracing::warn!("kafka ingress list routes failed: {e:#}");
                    continue;
                }
            };

            let mut desired: HashMap<Uuid, (String, IngressRoute)> = HashMap::new();
            for route in routes {
                if !route.enabled || route.kind != IngressKind::Kafka {
                    continue;
                }
                let fp = route_fingerprint(&route);
                desired.insert(route.id, (fp, route));
            }

            let active: Vec<Uuid> = running.keys().copied().collect();
            for id in active {
                let still_ok = desired
                    .get(&id)
                    .and_then(|(fp, _)| running.get(&id).map(|(old, _)| old == fp))
                    .unwrap_or(false);
                if !still_ok {
                    if let Some((_, handle)) = running.remove(&id) {
                        handle.abort();
                        tracing::info!(route_id = %id, "stopped kafka consumer group task");
                    }
                }
            }

            for (id, (fp, route)) in desired {
                if running.contains_key(&id) {
                    continue;
                }
                let state_c = state.clone();
                let fp_c = fp.clone();
                let handle = tokio::spawn(async move {
                    run_route_supervised(state_c, route).await;
                });
                running.insert(id, (fp_c, handle));
                tracing::info!(route_id = %id, "started kafka consumer group task");
            }
        }
    });
}

fn route_fingerprint(route: &IngressRoute) -> String {
    format!(
        "{}|{}|{}|{}|{}",
        route.endpoint.trim(),
        route.options.get("topic").map(|s| s.as_str()).unwrap_or(""),
        route
            .options
            .get("group_id")
            .map(|s| s.as_str())
            .unwrap_or(""),
        route
            .options
            .get("partitions")
            .map(|s| s.as_str())
            .unwrap_or(""),
        route
            .options
            .get("start")
            .map(|s| s.as_str())
            .unwrap_or("latest"),
    )
}

async fn run_route_supervised(state: Arc<AppState>, route: IngressRoute) {
    loop {
        match run_route_consumer(state.clone(), &route).await {
            Ok(()) => {
                tracing::info!(
                    route = %route.name,
                    "kafka consumer group stream ended; restarting"
                );
            }
            Err(e) => {
                tracing::warn!(route = %route.name, "kafka consumer group failed: {e:#}");
            }
        }
        tokio::time::sleep(Duration::from_secs(3)).await;
    }
}

async fn run_route_consumer(state: Arc<AppState>, route: &IngressRoute) -> anyhow::Result<()> {
    let bootstrap = parse_bootstrap(&route.endpoint)?;
    let topic = route
        .options
        .get("topic")
        .cloned()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| anyhow::anyhow!("kafka ingress missing options.topic"))?;
    let group_id = route
        .options
        .get("group_id")
        .cloned()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("eventide-ingress-{}", route.id));
    let start_at = route
        .options
        .get("start")
        .map(|s| s.as_str())
        .unwrap_or("latest");

    let max_partitions = resolve_partition_count(state.clone(), route, &topic).await?;
    let partitions: Vec<i32> = (0..max_partitions).collect();
    let assignment = TopicPartitionsBuilder::new()
        .assign(topic.clone(), partitions.clone())
        .build();

    tracing::info!(
        route = %route.name,
        topic = %topic,
        group_id = %group_id,
        max_partitions,
        start_at,
        "kafka ingress joining consumer group"
    );

    // When the group has no committed offsets yet, seed start position so
    // `latest` does not replay the whole topic (samsa defaults missing offsets to 0).
    if let Err(e) = seed_initial_offsets(
        state.clone(),
        route,
        bootstrap.clone(),
        &group_id,
        &topic,
        &partitions,
        start_at,
    )
    .await
    {
        tracing::warn!(
            route = %route.name,
            error = %e,
            "kafka initial offset seed skipped; consumer will use group commits / default"
        );
    }

    let member = ConsumerGroupBuilder::<TcpConnection>::new(bootstrap, group_id, assignment)
        .await
        .map_err(|e| anyhow::anyhow!("consumer group builder: {e:?}"))?
        .session_timeout_ms(45_000)
        .rebalance_timeout_ms(45_000)
        .max_wait_ms(2_000)
        .min_bytes(1)
        .max_bytes(1_048_576)
        .max_partition_bytes(524_288)
        .build()
        .await
        .map_err(|e| anyhow::anyhow!("consumer group connect: {e:?}"))?;

    let stream = member.into_stream();
    tokio::pin!(stream);

    while let Some(batch) = stream.next().await {
        let messages: Vec<ConsumeMessage> = match batch {
            Ok(iter) => iter.collect(),
            Err(e) => {
                return Err(anyhow::anyhow!("kafka consumer group stream: {e:?}"));
            }
        };
        if messages.is_empty() {
            continue;
        }
        tracing::debug!(
            route = %route.name,
            count = messages.len(),
            "kafka consumer group batch"
        );
        for msg in messages {
            if let Err(e) = ingest_payload(state.clone(), route, msg.value.as_ref()).await {
                tracing::warn!(
                    route = %route.name,
                    partition = msg.partition_index,
                    offset = msg.offset,
                    "bad kafka alert payload: {e:#}"
                );
            }
        }
    }
    Ok(())
}

async fn seed_initial_offsets(
    state: Arc<AppState>,
    route: &IngressRoute,
    bootstrap: Vec<BrokerAddress>,
    group_id: &str,
    topic: &str,
    partitions: &[i32],
    start_at: &str,
) -> anyhow::Result<()> {
    let assignment = TopicPartitionsBuilder::new()
        .assign(topic.to_string(), partitions.to_vec())
        .build();

    let conn = TcpConnection::new(bootstrap.clone())
        .await
        .map_err(|e| anyhow::anyhow!("seed connect: {e:?}"))?;
    let coord = find_coordinator(conn, 1, "eventide", group_id)
        .await
        .map_err(|e| anyhow::anyhow!("find_coordinator: {e:?}"))?;
    if coord.error_code != KafkaCode::None {
        anyhow::bail!("find_coordinator: {:?}", coord.error_code);
    }
    let host = std::str::from_utf8(coord.host.as_ref())
        .map_err(|_| anyhow::anyhow!("coordinator host utf8"))?
        .to_string();
    let port = u16::try_from(coord.port).map_err(|_| anyhow::anyhow!("coordinator port"))?;
    let coordinator = TcpConnection::from_addr(
        bootstrap,
        BrokerAddress { host, port },
    )
    .await
    .map_err(|e| anyhow::anyhow!("coordinator connect: {e:?}"))?;

    let fetched = fetch_offset(1, "eventide", group_id, coordinator.clone(), &assignment)
        .await
        .map_err(|e| anyhow::anyhow!("fetch_offset: {e:?}"))?;
    if fetched.error_code != KafkaCode::None {
        anyhow::bail!("fetch_offset: {:?}", fetched.error_code);
    }

    let mut needs_seed = true;
    for (_topic_name, partition) in fetched.into_box_iter() {
        if partition.error_code == KafkaCode::None && partition.committed_offset >= 0 {
            needs_seed = false;
            break;
        }
    }
    if !needs_seed {
        return Ok(());
    }

    let brokers: Vec<String> = route
        .endpoint
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    let client = state.kafka_pool.get(&brokers).await?;

    let mut offsets: PartitionOffsets = HashMap::new();
    for &partition in partitions {
        let offset = if start_at == "earliest" {
            earliest_offset(client.as_ref(), topic, partition).await?
        } else {
            latest_offset(client.as_ref(), topic, partition).await?
        };
        offsets.insert((topic.to_string(), partition), offset);
    }

    tracing::info!(
        route = %route.name,
        group_id,
        start_at,
        partitions = offsets.len(),
        "seeding kafka consumer group offsets"
    );

    commit_offset(
        1,
        "eventide",
        group_id,
        coordinator,
        -1,
        Bytes::new(),
        offsets,
        100_000,
    )
    .await
    .map_err(|e| anyhow::anyhow!("commit_offset seed: {e:?}"))?;
    Ok(())
}

fn parse_bootstrap(endpoint: &str) -> anyhow::Result<Vec<BrokerAddress>> {
    let mut out = Vec::new();
    for part in endpoint.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let (host, port) = match part.rsplit_once(':') {
            Some((h, p)) => {
                let port: u16 = p
                    .parse()
                    .map_err(|_| anyhow::anyhow!("invalid broker port in `{part}`"))?;
                (h.to_string(), port)
            }
            None => (part.to_string(), 9092),
        };
        out.push(BrokerAddress { host, port });
    }
    if out.is_empty() {
        anyhow::bail!("kafka ingress missing endpoint (brokers)");
    }
    Ok(out)
}

async fn resolve_partition_count(
    state: Arc<AppState>,
    route: &IngressRoute,
    topic: &str,
) -> anyhow::Result<i32> {
    let configured: Option<i32> = route
        .options
        .get("partitions")
        .map(|s| s.trim())
        .filter(|s| !s.is_empty() && !s.eq_ignore_ascii_case("auto"))
        .and_then(|s| s.parse().ok())
        .filter(|&n| n > 0);

    let brokers: Vec<String> = route
        .endpoint
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    match state.kafka_pool.get(&brokers).await {
        Ok(client) => match topic_partition_count(client.as_ref(), topic).await {
            Ok(n) => {
                if let Some(cfg) = configured {
                    if cfg != n {
                        tracing::info!(
                            route = %route.name,
                            topic = %topic,
                            configured = cfg,
                            actual = n,
                            "kafka ingress using metadata partition count"
                        );
                    }
                }
                Ok(n)
            }
            Err(e) => {
                let fallback = configured.unwrap_or(1);
                tracing::warn!(
                    route = %route.name,
                    topic = %topic,
                    fallback,
                    error = %e,
                    "kafka metadata partition probe failed; using configured/fallback"
                );
                Ok(fallback)
            }
        },
        Err(e) => {
            let fallback = configured.unwrap_or(1);
            tracing::warn!(
                route = %route.name,
                fallback,
                error = %e,
                "kafka client pool failed; using configured/fallback partitions"
            );
            Ok(fallback)
        }
    }
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
        persist_and_queue_notify(
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
