// B4 批次：Kafka 独立 API 模块
// 注意：ingress.ts 中已存在 B1 版 Kafka 工具（probeKafkaCluster / createKafkaTopic 等），
// 本文件是按 B4 合约全新封装的独立 API，返回类型、函数签名均为 B4 Kafka* 类型，
// 与 ingress.ts 旧实现并存互不影响。
import { request } from './request'
import type {
  B4KafkaClusterProbe,
  B4KafkaTopicDescribe,
  B4KafkaBrowseResult,
  B4KafkaGroupListed,
  B4KafkaGroupState,
  B4KafkaProduceResult,
} from './types'

/**
 * 探测 Kafka 分区数：POST /api/ingress/kafka/partitions
 * 返回 { topic, partitions }
 */
export function probeKafkaPartitions(
  brokers: string,
  topic: string,
): Promise<{ topic: string; partitions: number }> {
  const body = { brokers: brokers ?? '', topic: topic ?? '' }
  return request<{ topic: string; partitions: number }>('/api/ingress/kafka/partitions', {
    method: 'POST',
    body: JSON.stringify(body),
  })
}

/**
 * 探测 Kafka 集群连通性+列 Topic：POST /api/ingress/kafka/probe
 */
export function probeKafkaCluster(body: {
  brokers: string
  topic?: string
}): Promise<B4KafkaClusterProbe> {
  const payload: { brokers: string; topic?: string } = { brokers: body?.brokers ?? '' }
  if (body?.topic !== undefined && body.topic !== '') payload.topic = body.topic
  return request<B4KafkaClusterProbe>('/api/ingress/kafka/probe', {
    method: 'POST',
    body: JSON.stringify(payload),
  })
}

/**
 * 创建 Kafka Topic：POST /api/ingress/kafka/topics
 */
export function createKafkaTopic(body: {
  brokers: string
  topic: string
  partitions?: number
  replication_factor?: number
  configs?: Record<string, string>
}): Promise<{ ok: true; topic: string }> {
  const payload: {
    brokers: string
    topic: string
    partitions?: number
    replication_factor?: number
    configs?: Record<string, string>
  } = {
    brokers: body?.brokers ?? '',
    topic: body?.topic ?? '',
  }
  if (body?.partitions !== undefined) payload.partitions = body.partitions
  if (body?.replication_factor !== undefined) payload.replication_factor = body.replication_factor
  if (body?.configs !== undefined && Object.keys(body.configs).length > 0) payload.configs = body.configs
  return request<{ ok: true; topic: string }>('/api/ingress/kafka/topics', {
    method: 'POST',
    body: JSON.stringify(payload),
  })
}

/**
 * 删除 Kafka Topic：POST /api/ingress/kafka/topics/delete
 */
export function deleteKafkaTopic(body: {
  brokers: string
  topic: string
}): Promise<{ ok: true }> {
  const payload = { brokers: body?.brokers ?? '', topic: body?.topic ?? '' }
  return request<{ ok: true }>('/api/ingress/kafka/topics/delete', {
    method: 'POST',
    body: JSON.stringify(payload),
  })
}

/**
 * 描述 Kafka Topic 分区详情：POST /api/ingress/kafka/describe
 */
export function describeKafkaTopic(body: {
  brokers: string
  topic: string
}): Promise<B4KafkaTopicDescribe> {
  const payload = { brokers: body?.brokers ?? '', topic: body?.topic ?? '' }
  return request<B4KafkaTopicDescribe>('/api/ingress/kafka/describe', {
    method: 'POST',
    body: JSON.stringify(payload),
  })
}

/**
 * 浏览 Kafka Topic 消息：POST /api/ingress/kafka/messages
 */
export function browseKafkaMessages(body: {
  brokers: string
  topic: string
  partition: number
  from: 'latest' | 'earliest' | 'offset'
  max?: number
  offset?: number
}): Promise<B4KafkaBrowseResult> {
  const payload: {
    brokers: string
    topic: string
    partition: number
    from: 'latest' | 'earliest' | 'offset'
    max?: number
    offset?: number
  } = {
    brokers: body?.brokers ?? '',
    topic: body?.topic ?? '',
    partition: body?.partition ?? 0,
    from: body?.from ?? 'latest',
  }
  if (body?.max !== undefined && body.max > 0) payload.max = body.max
  if (body?.from === 'offset' && body?.offset !== undefined) payload.offset = body.offset
  return request<B4KafkaBrowseResult>('/api/ingress/kafka/messages', {
    method: 'POST',
    body: JSON.stringify(payload),
  })
}

/**
 * 生产 Kafka 消息：POST /api/ingress/kafka/produce
 */
export function produceKafkaMessage(body: {
  brokers: string
  topic: string
  partition?: number
  key?: string
  value: string
}): Promise<B4KafkaProduceResult> {
  const payload: {
    brokers: string
    topic: string
    partition?: number
    key?: string
    value: string
  } = {
    brokers: body?.brokers ?? '',
    topic: body?.topic ?? '',
    value: body?.value ?? '',
  }
  if (body?.partition !== undefined) payload.partition = body.partition
  if (body?.key !== undefined && body.key !== '') payload.key = body.key
  return request<B4KafkaProduceResult>('/api/ingress/kafka/produce', {
    method: 'POST',
    body: JSON.stringify(payload),
  })
}

/**
 * 列出 Kafka Consumer Groups：POST /api/ingress/kafka/groups
 */
export function listKafkaGroups(body: {
  brokers: string
}): Promise<{ groups: B4KafkaGroupListed[] }> {
  const payload = { brokers: body?.brokers ?? '' }
  return request<{ groups: B4KafkaGroupListed[] }>('/api/ingress/kafka/groups', {
    method: 'POST',
    body: JSON.stringify(payload),
  })
}

/**
 * 描述 Kafka Consumer Group 详情：POST /api/ingress/kafka/groups/describe
 */
export function describeKafkaGroup(body: {
  brokers: string
  group_id: string
  topic?: string
}): Promise<B4KafkaGroupState> {
  const payload: { brokers: string; group_id: string; topic?: string } = {
    brokers: body?.brokers ?? '',
    group_id: body?.group_id ?? '',
  }
  if (body?.topic !== undefined && body.topic !== '') payload.topic = body.topic
  return request<B4KafkaGroupState>('/api/ingress/kafka/groups/describe', {
    method: 'POST',
    body: JSON.stringify(payload),
  })
}
