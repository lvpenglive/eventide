// B2 批次：Silences API
import { request } from './request'
import type { Silence, SilenceInput } from './types'

/**
 * GET /api/silences → Silence[]
 */
export function listSilences(): Promise<Silence[]> {
  return request<Silence[]>('/api/silences', {
    method: 'GET',
  })
}

/**
 * POST /api/silences → Silence (Status 201)
 */
export function createSilence(body: SilenceInput): Promise<Silence> {
  return request<Silence>('/api/silences', {
    method: 'POST',
    body: JSON.stringify(body),
  })
}

/**
 * DELETE /api/silences/{id} → 204 无内容，返回 {ok:true}
 */
export async function deleteSilence(id: string): Promise<{ ok: true }> {
  try {
    const resp = await request<{ ok: true } | null | undefined>(
      `/api/silences/${encodeURIComponent(id)}`,
      { method: 'DELETE' }
    )
    // 正常空响应经过 request.ts 会是 null；这里统一 ok=true。
    if (resp === null || resp === undefined) return { ok: true }
    return resp && 'ok' in resp && resp.ok ? { ok: true } : { ok: true }
  } catch (e: any) {
    // 有些 request 封装在 204 + 空 body 被 JSON.parse 抛错时会走到这里
    if (e && typeof e === 'object' && 'status' in e && e.status === 204) {
      return { ok: true }
    }
    throw e
  }
}
