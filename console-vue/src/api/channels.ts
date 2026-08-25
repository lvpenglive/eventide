// B3 批次：Channels CRUD + Test
// 注意：common.ts 已导出 listChannels() / listMaintenanceWindows()，
// 本文件**不使用** listChannels 命名，改用 listAllChannels() 以 100% 避免命名冲突。
import { request } from './request'
import type {
  NotifyChannel,
  ChannelInput,
  ChannelTestResp,
} from './types'

/**
 * 全部通知渠道：GET /api/channels
 * （有意命名为 listAllChannels，避免与 common.listChannels 冲突）
 */
export function listAllChannels(): Promise<NotifyChannel[]> {
  return request<NotifyChannel[]>('/api/channels', {
    method: 'GET',
  })
}

/**
 * 获取单个渠道详情：GET /api/channels/{id}
 */
export function getChannel(id: string): Promise<NotifyChannel> {
  return request<NotifyChannel>(`/api/channels/${encodeURIComponent(id)}`, {
    method: 'GET',
  })
}

/**
 * 创建通知渠道：POST /api/channels (201)
 */
export function createChannel(body: ChannelInput): Promise<NotifyChannel> {
  return request<NotifyChannel>('/api/channels', {
    method: 'POST',
    body: JSON.stringify(body),
  })
}

/**
 * 更新通知渠道：PUT /api/channels/{id}
 */
export function updateChannel(
  id: string,
  body: ChannelInput,
): Promise<NotifyChannel> {
  return request<NotifyChannel>(`/api/channels/${encodeURIComponent(id)}`, {
    method: 'PUT',
    body: JSON.stringify(body),
  })
}

/**
 * 删除通知渠道：DELETE /api/channels/{id} → 204 兜底 {ok:true}
 */
export async function deleteChannel(id: string): Promise<{ ok: true }> {
  try {
    const resp = await request<{ ok: true } | null | undefined>(
      `/api/channels/${encodeURIComponent(id)}`,
      { method: 'DELETE' },
    )
    if (resp === null || resp === undefined) return { ok: true }
    return resp && 'ok' in resp && resp.ok ? { ok: true } : { ok: true }
  } catch (e: any) {
    if (e && typeof e === 'object' && 'status' in e && e.status === 204) {
      return { ok: true }
    }
    throw e
  }
}

/**
 * 测试渠道连通性：POST /api/channels/{id}/test
 */
export function testChannel(id: string): Promise<ChannelTestResp> {
  return request<ChannelTestResp>(
    `/api/channels/${encodeURIComponent(id)}/test`,
    { method: 'POST' },
  )
}
