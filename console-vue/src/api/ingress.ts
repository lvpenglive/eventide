import { request } from './request'
import type {
  IngressInput,
  IngressRoute,
  IngressTestResp,
  KafkaBrowseInput,
  KafkaBrowseResp,
  KafkaDescribeResp,
  KafkaGroupDescribeResp,
  KafkaGroupListResp,
  KafkaProbeResp,
} from './types'

// —— Ingress 路由 CRUD ——

/**
 * Ingress 路由列表：GET /api/ingress
 */
export function listIngress(): Promise<IngressRoute[]> {
  return request<IngressRoute[]>('/api/ingress', {
    method: 'GET',
  })
}

/**
 * Ingress 路由详情：GET /api/ingress/{id}
 */
export function getIngress(id: string): Promise<IngressRoute> {
  return request<IngressRoute>(`/api/ingress/${encodeURIComponent(id)}`, {
    method: 'GET',
  })
}

/**
 * 创建 Ingress 路由：POST /api/ingress
 */
export function createIngress(body: IngressInput): Promise<IngressRoute> {
  return request<IngressRoute>('/api/ingress', {
    method: 'POST',
    body: JSON.stringify(body),
  })
}

/**
 * 更新 Ingress 路由：PUT /api/ingress/{id}
 */
export function updateIngress(id: string, body: IngressInput): Promise<IngressRoute> {
  return request<IngressRoute>(`/api/ingress/${encodeURIComponent(id)}`, {
    method: 'PUT',
    body: JSON.stringify(body),
  })
}

/**
 * 删除 Ingress 路由：DELETE /api/ingress/{id}
 */
export function deleteIngress(id: string): Promise<unknown> {
  return request<unknown>(`/api/ingress/${encodeURIComponent(id)}`, {
    method: 'DELETE',
  })
}

/**
 * 测试 Ingress 路由：POST /api/ingress/{id}/test
 */
export function testIngress(
  id: string,
  scenario: 'fire' | 'recover' | 'probe_fire' | 'probe_recover' = 'fire',
): Promise<IngressTestResp> {
  return request<IngressTestResp>(`/api/ingress/${encodeURIComponent(id)}/test`, {
    method: 'POST',
    body: JSON.stringify({ scenario }),
  })
}

// —— Kafka 工具 ——

/**
 * 探测 Kafka 集群连通性：POST /api/ingress/kafka/probe
 */
export function probeKafkaCluster(brokers: string, topic?: string): Promise<KafkaProbeResp> {
  const body: { brokers: string; topic?: string } = { brokers }
  if (topic !== undefined && topic !== '') body.topic = topic
  return request<KafkaProbeResp>('/api/ingress/kafka/probe', {
    method: 'POST',
    body: JSON.stringify(body),
  })
}

/**
 * 创建 Kafka Topic：POST /api/ingress/kafka/topics
 */
export function createKafkaTopic(body: {
  brokers: string
  topic: string
  partitions: number
  replication_factor?: number
}): Promise<{ created: boolean; [k: string]: unknown }> {
  return request<{ created: boolean; [k: string]: unknown }>('/api/ingress/kafka/topics', {
    method: 'POST',
    body: JSON.stringify(body),
  })
}

/**
 * 删除 Kafka Topic：POST /api/ingress/kafka/topics/delete
 */
export function deleteKafkaTopic(brokers: string, topic: string): Promise<unknown> {
  return request<unknown>('/api/ingress/kafka/topics/delete', {
    method: 'POST',
    body: JSON.stringify({ brokers, topic }),
  })
}

/**
 * 描述 Kafka Topic 分区信息：POST /api/ingress/kafka/describe
 */
export function describeKafkaTopic(brokers: string, topic: string): Promise<KafkaDescribeResp> {
  return request<KafkaDescribeResp>('/api/ingress/kafka/describe', {
    method: 'POST',
    body: JSON.stringify({ brokers, topic }),
  })
}

/**
 * 浏览 Kafka 消息：POST /api/ingress/kafka/messages
 */
export function browseKafkaMessages(body: KafkaBrowseInput): Promise<KafkaBrowseResp> {
  return request<KafkaBrowseResp>('/api/ingress/kafka/messages', {
    method: 'POST',
    body: JSON.stringify(body),
  })
}

/**
 * 生产 Kafka 消息：POST /api/ingress/kafka/produce
 */
export function produceKafkaMessage(body: {
  brokers: string
  topic: string
  partition: number
  key?: string | null
  value: string
}): Promise<unknown> {
  return request<unknown>('/api/ingress/kafka/produce', {
    method: 'POST',
    body: JSON.stringify(body),
  })
}

/**
 * 列出 Kafka Consumer Group：POST /api/ingress/kafka/groups
 */
export function listKafkaGroups(brokers: string): Promise<KafkaGroupListResp> {
  return request<KafkaGroupListResp>('/api/ingress/kafka/groups', {
    method: 'POST',
    body: JSON.stringify({ brokers }),
  })
}

/**
 * 描述 Kafka Consumer Group 详情：POST /api/ingress/kafka/groups/describe
 */
export function describeKafkaGroup(
  group_id: string,
  brokers: string,
  topic?: string,
): Promise<KafkaGroupDescribeResp> {
  const body: { group_id: string; brokers: string; topic?: string } = { group_id, brokers }
  if (topic !== undefined && topic !== '') body.topic = topic
  return request<KafkaGroupDescribeResp>('/api/ingress/kafka/groups/describe', {
    method: 'POST',
    body: JSON.stringify(body),
  })
}
