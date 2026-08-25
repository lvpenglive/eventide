// B3 批次：Notify Logs
import { request } from './request'
import type { NotifyLogItem, NotifyQuery, NotifyQuerySuccess } from './types'

function normalizeSuccess(
  v: NotifyQuerySuccess | undefined,
): string | undefined {
  if (v === undefined || v === null) return undefined
  if (v === '') return ''
  if (v === true) return '1'
  if (v === false) return '0'
  // 字符串：归一 true/ok→'1'，false/fail→'0'，其他原样
  const s = String(v)
  const lower = s.toLowerCase()
  if (lower === 'true' || lower === 'ok') return '1'
  if (lower === 'false' || lower === 'fail') return '0'
  return s
}

/**
 * 通知发送日志：GET /api/notifies
 */
export function listNotifies(params?: NotifyQuery): Promise<NotifyLogItem[]> {
  const query: Record<string, string | number | boolean | undefined | null> = {}
  if (params) {
    if (params.channel_id !== undefined && params.channel_id !== null) {
      query.channel_id = params.channel_id
    }
    const sv = normalizeSuccess(params.success)
    if (sv !== undefined) query.success = sv
    if (params.q !== undefined && params.q !== null && params.q !== '') {
      query.q = params.q
    }
    if (typeof params.limit === 'number') {
      query.limit = params.limit
    }
  }
  return request<NotifyLogItem[]>('/api/notifies', {
    method: 'GET',
    query: Object.keys(query).length ? query : undefined,
  })
}
