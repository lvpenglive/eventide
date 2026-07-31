//! Kafka consumer-group admin helpers (list + lag) via samsa protocol.

use bytes::{Buf, BufMut, Bytes};
use eventide_sources::{latest_offset, KafkaClientPool};
use samsa::prelude::encode::ToByte;
use samsa::prelude::protocol::{
    FindCoordinatorRequest, HeaderRequest, OffsetFetchRequest, OffsetFetchResponse,
};
use samsa::prelude::{BrokerAddress, BrokerConnection, KafkaCode, TcpConnection};
use serde::Serialize;
use std::sync::Arc;

const CLIENT_ID: &str = "eventide";
const API_LIST_GROUPS: i16 = 16;
const API_DESCRIBE_GROUPS: i16 = 15;

#[derive(Debug, Clone, Serialize)]
pub struct ListedGroup {
    pub group_id: String,
    pub protocol_type: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct GroupMember {
    pub member_id: String,
    pub client_id: String,
    pub client_host: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PartitionLag {
    pub topic: String,
    pub partition: i32,
    pub committed: i64,
    pub latest: i64,
    pub lag: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct GroupDescribe {
    pub group_id: String,
    pub state: String,
    pub protocol_type: String,
    pub protocol: String,
    pub members: Vec<GroupMember>,
    pub partitions: Vec<PartitionLag>,
    pub total_lag: i64,
}

fn parse_bootstrap(brokers: &[String]) -> anyhow::Result<Vec<BrokerAddress>> {
    let mut out = Vec::new();
    for part in brokers {
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
        anyhow::bail!("brokers required");
    }
    Ok(out)
}

struct ListGroupsRequest<'a> {
    header: HeaderRequest<'a>,
}

impl ToByte for ListGroupsRequest<'_> {
    fn encode<T: BufMut>(&self, buffer: &mut T) -> samsa::prelude::Result<()> {
        self.header.encode(buffer)
    }
}

struct DescribeGroupsRequest<'a> {
    header: HeaderRequest<'a>,
    group_ids: &'a [&'a str],
}

impl ToByte for DescribeGroupsRequest<'_> {
    fn encode<T: BufMut>(&self, buffer: &mut T) -> samsa::prelude::Result<()> {
        self.header.encode(buffer)?;
        (self.group_ids.len() as i32).encode(buffer)?;
        for id in self.group_ids {
            (*id).encode(buffer)?;
        }
        Ok(())
    }
}

/// OffsetFetch with topics=null (all topics the group has committed).
struct OffsetFetchAll<'a> {
    header: HeaderRequest<'a>,
    group_id: &'a str,
}

impl ToByte for OffsetFetchAll<'_> {
    fn encode<T: BufMut>(&self, buffer: &mut T) -> samsa::prelude::Result<()> {
        self.header.encode(buffer)?;
        self.group_id.encode(buffer)?;
        (-1_i32).encode(buffer)?;
        Ok(())
    }
}

fn need(buf: &mut Bytes, n: usize) -> anyhow::Result<()> {
    if buf.remaining() < n {
        anyhow::bail!("short kafka response");
    }
    Ok(())
}

fn read_i16(buf: &mut Bytes) -> anyhow::Result<i16> {
    need(buf, 2)?;
    Ok(buf.get_i16())
}

fn read_i32(buf: &mut Bytes) -> anyhow::Result<i32> {
    need(buf, 4)?;
    Ok(buf.get_i32())
}

fn read_string(buf: &mut Bytes) -> anyhow::Result<String> {
    let len = read_i16(buf)?;
    if len < 0 {
        return Ok(String::new());
    }
    let n = len as usize;
    need(buf, n)?;
    let b = buf.copy_to_bytes(n);
    Ok(String::from_utf8_lossy(&b).into_owned())
}

fn read_bytes(buf: &mut Bytes) -> anyhow::Result<Bytes> {
    let len = read_i32(buf)?;
    if len < 0 {
        return Ok(Bytes::new());
    }
    let n = len as usize;
    need(buf, n)?;
    Ok(buf.copy_to_bytes(n))
}

fn skip_correlation(buf: &mut Bytes) -> anyhow::Result<()> {
    // Response header v0: correlation_id INT32
    let _ = read_i32(buf)?;
    Ok(())
}

fn parse_list_groups(mut buf: Bytes) -> anyhow::Result<Vec<ListedGroup>> {
    skip_correlation(&mut buf)?;
    let error_code = read_i16(&mut buf)?;
    if error_code != 0 {
        anyhow::bail!("list_groups error_code={error_code}");
    }
    let count = read_i32(&mut buf)?;
    let n = if count < 0 { 0 } else { count as usize };
    let mut groups = Vec::with_capacity(n);
    for _ in 0..n {
        groups.push(ListedGroup {
            group_id: read_string(&mut buf)?,
            protocol_type: read_string(&mut buf)?,
        });
    }
    Ok(groups)
}

struct DescribedMeta {
    state: String,
    protocol_type: String,
    protocol: String,
    members: Vec<GroupMember>,
}

fn parse_describe_groups(mut buf: Bytes, want: &str) -> anyhow::Result<DescribedMeta> {
    skip_correlation(&mut buf)?;
    let count = read_i32(&mut buf)?;
    let n = if count < 0 { 0 } else { count as usize };
    let mut found = None;
    for _ in 0..n {
        let error_code = read_i16(&mut buf)?;
        let group_id = read_string(&mut buf)?;
        let state = read_string(&mut buf)?;
        let protocol_type = read_string(&mut buf)?;
        let protocol = read_string(&mut buf)?;
        let mcount = read_i32(&mut buf)?;
        let mn = if mcount < 0 { 0 } else { mcount as usize };
        let mut members = Vec::with_capacity(mn);
        for _ in 0..mn {
            let member_id = read_string(&mut buf)?;
            let client_id = read_string(&mut buf)?;
            let client_host = read_string(&mut buf)?;
            let _ = read_bytes(&mut buf)?;
            let _ = read_bytes(&mut buf)?;
            members.push(GroupMember {
                member_id,
                client_id,
                client_host,
            });
        }
        if group_id == want {
            if error_code != 0 {
                tracing::warn!(error_code, group_id = %want, "describe_groups error");
            }
            found = Some(DescribedMeta {
                state,
                protocol_type,
                protocol,
                members,
            });
        }
    }
    Ok(found.unwrap_or(DescribedMeta {
        state: "Unknown".into(),
        protocol_type: String::new(),
        protocol: String::new(),
        members: Vec::new(),
    }))
}

pub async fn list_consumer_groups(brokers: &[String]) -> anyhow::Result<Vec<ListedGroup>> {
    let bootstrap = parse_bootstrap(brokers)?;
    let mut conn = TcpConnection::new(bootstrap)
        .await
        .map_err(|e| anyhow::anyhow!("connect: {e:?}"))?;
    let req = ListGroupsRequest {
        header: HeaderRequest::new(API_LIST_GROUPS, 0, 1, CLIENT_ID),
    };
    conn.send_request(&req)
        .await
        .map_err(|e| anyhow::anyhow!("list_groups send: {e:?}"))?;
    let raw = conn
        .receive_response()
        .await
        .map_err(|e| anyhow::anyhow!("list_groups recv: {e:?}"))?;
    parse_list_groups(raw.freeze())
}

async fn connect_coordinator(
    bootstrap: Vec<BrokerAddress>,
    group_id: &str,
) -> anyhow::Result<TcpConnection> {
    let mut conn = TcpConnection::new(bootstrap.clone())
        .await
        .map_err(|e| anyhow::anyhow!("connect: {e:?}"))?;
    let req = FindCoordinatorRequest::new(1, CLIENT_ID, group_id);
    conn.send_request(&req)
        .await
        .map_err(|e| anyhow::anyhow!("find_coordinator send: {e:?}"))?;
    let raw = conn
        .receive_response()
        .await
        .map_err(|e| anyhow::anyhow!("find_coordinator recv: {e:?}"))?;
    let coord = samsa::prelude::protocol::FindCoordinatorResponse::try_from(raw.freeze())
        .map_err(|e| anyhow::anyhow!("find_coordinator parse: {e:?}"))?;
    if coord.error_code != KafkaCode::None {
        anyhow::bail!("find_coordinator: {:?}", coord.error_code);
    }
    let host = std::str::from_utf8(coord.host.as_ref())
        .map_err(|_| anyhow::anyhow!("coordinator host utf8"))?
        .to_string();
    let port = u16::try_from(coord.port).map_err(|_| anyhow::anyhow!("coordinator port"))?;
    TcpConnection::from_addr(bootstrap, BrokerAddress { host, port })
        .await
        .map_err(|e| anyhow::anyhow!("coordinator connect: {e:?}"))
}

pub async fn describe_consumer_group(
    brokers: &[String],
    group_id: &str,
    topic_filter: Option<&str>,
    pool: &Arc<KafkaClientPool>,
) -> anyhow::Result<GroupDescribe> {
    let bootstrap = parse_bootstrap(brokers)?;
    let mut coordinator = connect_coordinator(bootstrap, group_id).await?;

    let ids = [group_id];
    let desc_req = DescribeGroupsRequest {
        header: HeaderRequest::new(API_DESCRIBE_GROUPS, 0, 2, CLIENT_ID),
        group_ids: &ids,
    };
    coordinator
        .send_request(&desc_req)
        .await
        .map_err(|e| anyhow::anyhow!("describe_groups send: {e:?}"))?;
    let desc_raw = coordinator
        .receive_response()
        .await
        .map_err(|e| anyhow::anyhow!("describe_groups recv: {e:?}"))?;
    let meta = parse_describe_groups(desc_raw.freeze(), group_id)?;

    let offset_resp = if let Some(topic) = topic_filter.filter(|t| !t.is_empty()) {
        let client = pool.get(brokers).await?;
        let n = eventide_sources::topic_partition_count(client.as_ref(), topic).await?;
        let mut req = OffsetFetchRequest::new(3, CLIENT_ID, group_id);
        for p in 0..n {
            req.add(topic, p);
        }
        coordinator
            .send_request(&req)
            .await
            .map_err(|e| anyhow::anyhow!("offset_fetch send: {e:?}"))?;
        let raw = coordinator
            .receive_response()
            .await
            .map_err(|e| anyhow::anyhow!("offset_fetch recv: {e:?}"))?;
        OffsetFetchResponse::try_from(raw.freeze())
            .map_err(|e| anyhow::anyhow!("offset_fetch parse: {e:?}"))?
    } else {
        let req = OffsetFetchAll {
            header: HeaderRequest::new(9, 2, 3, CLIENT_ID),
            group_id,
        };
        coordinator
            .send_request(&req)
            .await
            .map_err(|e| anyhow::anyhow!("offset_fetch_all send: {e:?}"))?;
        let raw = coordinator
            .receive_response()
            .await
            .map_err(|e| anyhow::anyhow!("offset_fetch_all recv: {e:?}"))?;
        OffsetFetchResponse::try_from(raw.freeze())
            .map_err(|e| anyhow::anyhow!("offset_fetch_all parse: {e:?}"))?
    };

    if offset_resp.error_code != KafkaCode::None {
        anyhow::bail!("offset_fetch: {:?}", offset_resp.error_code);
    }

    let client = pool.get(brokers).await?;
    let mut partitions = Vec::new();
    let mut total_lag: i64 = 0;
    for (topic_name, part) in offset_resp.into_box_iter() {
        if part.error_code != KafkaCode::None {
            continue;
        }
        let topic = String::from_utf8_lossy(topic_name.as_ref()).into_owned();
        if let Some(filter) = topic_filter.filter(|t| !t.is_empty()) {
            if topic != filter {
                continue;
            }
        }
        let committed = part.committed_offset;
        let latest = match latest_offset(client.as_ref(), &topic, part.partition_index).await {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(
                    topic = %topic,
                    partition = part.partition_index,
                    error = %e,
                    "latest_offset failed for lag"
                );
                continue;
            }
        };
        let lag = if committed < 0 {
            latest.max(0)
        } else {
            (latest - committed).max(0)
        };
        total_lag = total_lag.saturating_add(lag);
        partitions.push(PartitionLag {
            topic,
            partition: part.partition_index,
            committed,
            latest,
            lag,
        });
    }
    partitions.sort_by(|a, b| (&a.topic, a.partition).cmp(&(&b.topic, b.partition)));

    Ok(GroupDescribe {
        group_id: group_id.to_string(),
        state: meta.state,
        protocol_type: meta.protocol_type,
        protocol: meta.protocol,
        members: meta.members,
        partitions,
        total_lag,
    })
}
