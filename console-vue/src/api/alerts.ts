import { request } from './request'
import type {
  AckInput,
  AlertEvent,
  AlertListResp,
  AlertQuery,
  BatchOpResp,
  CloseInput,
  NotifyLog,
} from './types'

function cleanQuery(q: AlertQuery): Record<string, string | number | boolean> {
  const out: Record<string, string | number | boolean> = {}
  for (const [k, v] of Object.entries(q)) {
    if (v === undefined || v === null || v === '') continue
    out[k] = v as string | number | boolean
  }
  return out
}

/**
 * 告警列表：GET /api/alerts
 */
export function listAlerts(q: AlertQuery = {}): Promise<AlertListResp> {
  return request<AlertListResp>('/api/alerts', {
    method: 'GET',
    query: cleanQuery(q),
  })
}

/**
 * 告警详情：GET /api/alerts/{id}
 */
export function getAlert(id: string): Promise<AlertEvent> {
  return request<AlertEvent>(`/api/alerts/${encodeURIComponent(id)}`, {
    method: 'GET',
  })
}

/**
 * 确认告警：POST /api/alerts/{id}/ack
 */
export function ackAlert(id: string, body: AckInput): Promise<AlertEvent> {
  return request<AlertEvent>(`/api/alerts/${encodeURIComponent(id)}/ack`, {
    method: 'POST',
    body: JSON.stringify(body),
  })
}

/**
 * 取消确认告警：POST /api/alerts/{id}/unack
 */
export function unackAlert(id: string): Promise<AlertEvent> {
  return request<AlertEvent>(`/api/alerts/${encodeURIComponent(id)}/unack`, {
    method: 'POST',
  })
}

/**
 * 关闭告警：POST /api/alerts/{id}/close
 */
export function closeAlert(id: string, body: CloseInput): Promise<AlertEvent> {
  return request<AlertEvent>(`/api/alerts/${encodeURIComponent(id)}/close`, {
    method: 'POST',
    body: JSON.stringify({ comment: body.comment, notify: body.notify ?? true }),
  })
}

/**
 * 告警对应的通知日志：GET /api/alerts/{id}/notifies
 */
export function listAlertNotifies(id: string): Promise<NotifyLog[]> {
  return request<NotifyLog[]>(`/api/alerts/${encodeURIComponent(id)}/notifies`, {
    method: 'GET',
  })
}

/**
 * 批量确认告警：POST /api/alerts/batch/ack
 */
export function batchAckAlerts(ids: string[], body: AckInput): Promise<BatchOpResp> {
  return request<BatchOpResp>('/api/alerts/batch/ack', {
    method: 'POST',
    body: JSON.stringify({
      ids,
      assignee: body.assignee,
      comment: body.comment,
    }),
  })
}

/**
 * 批量关闭告警：POST /api/alerts/batch/close
 */
export function batchCloseAlerts(ids: string[], body: CloseInput): Promise<BatchOpResp> {
  return request<BatchOpResp>('/api/alerts/batch/close', {
    method: 'POST',
    body: JSON.stringify({
      ids,
      comment: body.comment,
      notify: body.notify ?? true,
    }),
  })
}
