// B4 批次：Trap Policies（告警策略）API 模块
// 包含策略 CRUD、批量导入 (xlsx/zip/json 原始字节)、批量导出 (xlsx blob)。
import { request } from './request'
import type {
  B4TrapPolicy,
  B4TrapPolicyList,
  B4TrapPolicyImportResult,
  B4PolicyImportMode,
} from './types'

// —— 内部：取当前 Authorization token（用于直连 fetch/XHR）——
function getAuthToken(): string | null {
  try {
    return localStorage.getItem('eventide_token')
  } catch {
    return null
  }
}

// —— 内部：直连 GET Blob（绕过 request 的 JSON 解析假设）——
async function fetchBlob(url: string): Promise<Blob> {
  const headers: Record<string, string> = {}
  const token = getAuthToken()
  if (token) headers['Authorization'] = `Bearer ${token}`

  const res = await fetch(url, { method: 'GET', headers })
  if (res.status === 401) {
    localStorage.removeItem('eventide_token')
    localStorage.removeItem('eventide_user')
    window.dispatchEvent(new CustomEvent('auth:unauthorized'))
    throw new Error('未登录或登录已过期')
  }
  if (!res.ok) {
    let msg = `下载失败 (${res.status})`
    try {
      const txt = await res.text()
      if (txt) msg = txt
    } catch {
      /* ignore */
    }
    throw new Error(msg)
  }
  return res.blob()
}

// —— 内部：POST 原始 bytes，响应 JSON——
async function postRawBytes<T>(
  url: string,
  bytes: ArrayBuffer | Uint8Array | Blob,
): Promise<T> {
  const headers: Record<string, string> = {}
  const token = getAuthToken()
  if (token) headers['Authorization'] = `Bearer ${token}`

  const res = await fetch(url, {
    method: 'POST',
    headers,
    body: bytes ?? new Uint8Array(),
  })
  if (res.status === 401) {
    localStorage.removeItem('eventide_token')
    localStorage.removeItem('eventide_user')
    window.dispatchEvent(new CustomEvent('auth:unauthorized'))
    throw new Error('未登录或登录已过期')
  }
  let data: unknown
  const text = await res.text()
  try {
    data = text ? JSON.parse(text) : null
  } catch {
    data = text
  }
  if (!res.ok) {
    const message =
      data &&
      typeof data === 'object' &&
      'message' in data &&
      typeof (data as { message: unknown }).message === 'string'
        ? (data as { message: string }).message
        : `请求失败 (${res.status})`
    throw new Error(message)
  }
  return data as T
}

/**
 * 策略列表：GET /api/policies
 */
export function listPolicies(): Promise<B4TrapPolicyList> {
  return request<B4TrapPolicyList>('/api/policies', { method: 'GET' })
}

/**
 * 单条策略详情：GET /api/policies/{id}
 */
export function getPolicy(id: string): Promise<B4TrapPolicy> {
  return request<B4TrapPolicy>(`/api/policies/${encodeURIComponent(id ?? '')}`, {
    method: 'GET',
  })
}

/**
 * 创建策略：POST /api/policies
 */
export function createPolicy(
  body: Partial<B4TrapPolicy>,
): Promise<{ ok: boolean; item: B4TrapPolicy }> {
  // 构造 payload：保证必填项至少有合理空值兜底
  const payload: Partial<B4TrapPolicy> = { ...(body ?? {}) }
  if (payload.name === undefined) payload.name = ''
  if (payload.trap_oid === undefined) payload.trap_oid = ''
  if (payload.match_mode === undefined) payload.match_mode = 'exact'
  if (payload.severity === undefined) payload.severity = 'warning'
  if (payload.enabled === undefined) payload.enabled = true
  return request<{ ok: boolean; item: B4TrapPolicy }>('/api/policies', {
    method: 'POST',
    body: JSON.stringify(payload),
  })
}

/**
 * 更新策略：PUT /api/policies/{id}
 */
export function updatePolicy(
  id: string,
  body: B4TrapPolicy,
): Promise<{ ok: boolean; item: B4TrapPolicy }> {
  return request<{ ok: boolean; item: B4TrapPolicy }>(
    `/api/policies/${encodeURIComponent(id ?? '')}`,
    {
      method: 'PUT',
      body: JSON.stringify(body ?? {}),
    },
  )
}

/**
 * 删除策略：DELETE /api/policies/{id}
 */
export async function deletePolicy(id: string): Promise<{ ok: boolean }> {
  try {
    const resp = await request<{ ok: boolean } | null | undefined>(
      `/api/policies/${encodeURIComponent(id ?? '')}`,
      { method: 'DELETE' },
    )
    if (resp === null || resp === undefined) return { ok: true }
    return { ok: resp?.ok ?? true }
  } catch (e: unknown) {
    // 204 无内容兜底 { ok: true }
    if (e && typeof e === 'object' && 'status' in e && (e as { status: unknown }).status === 204) {
      return { ok: true }
    }
    throw e
  }
}

/**
 * 批量导入策略：POST /api/policies/import?mode=xxx
 * body = 原始 bytes（后端识别 xlsx / zip / json）。
 * 若提供 filename 则附在查询串（部分后端实现可能用于 extension 嗅探）。
 */
export function importPolicies(
  mode: B4PolicyImportMode,
  bytes: ArrayBuffer | Uint8Array | Blob,
  filename?: string,
): Promise<B4TrapPolicyImportResult> {
  const qs: Record<string, string> = { mode: mode ?? 'merge' }
  if (filename !== undefined && filename !== '') {
    qs.filename = filename
  }
  const params = new URLSearchParams(qs)
  const url = `/api/policies/import?${params.toString()}`
  return postRawBytes<B4TrapPolicyImportResult>(url, bytes)
}

/**
 * 批量导出策略为 xlsx blob：GET /api/policies/export
 */
export function exportPolicies(): Promise<Blob> {
  return fetchBlob('/api/policies/export')
}
