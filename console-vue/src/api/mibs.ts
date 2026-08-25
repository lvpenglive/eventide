// B4 批次：MIB 管理 API 模块
// 包含 MIB 列表/上传/删除、树节点浏览、Notification 列表、
// 单模块/全量 Policies 导出 + 应用，以及 SNMP Get 工具。
// 注意：
//   - uploadMib 使用 application/octet-stream 原始字节，且需支持进度回调 → 走 XHR。
//   - exportModulePolicies / exportAllPolicies 返回 xlsx blob → 直接 fetch。
//   - 其余 CRUD 使用 ./request 的 request() 封装。
import { request } from './request'
import type {
  B4MibModuleItem,
  B4MibModuleList,
  B4MibTreeNodeList,
  B4MibNodeDetail,
  B4MibNotificationList,
  B4ApplyPolicyResult,
  B4SnmpGetResult,
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

/**
 * MIB 模块列表：GET /api/mibs
 */
export function listMibs(): Promise<B4MibModuleList> {
  return request<B4MibModuleList>('/api/mibs', { method: 'GET' })
}

/**
 * 上传单个 MIB 文件：POST /api/mibs?filename=xxx
 * body = 原始字节；Content-Type: application/octet-stream；
 * 可选 onProgress 回调 (0–100)。
 */
export function uploadMib(
  filename: string,
  bytes: ArrayBuffer | Uint8Array | Blob,
  onProgress?: (pct: number) => void,
): Promise<{ ok: boolean; item?: B4MibModuleItem }> {
  return new Promise((resolve, reject) => {
    const xhr = new XMLHttpRequest()
    const url = `/api/mibs?filename=${encodeURIComponent(filename ?? 'mib')}`
    xhr.open('POST', url, true)
    xhr.setRequestHeader('Content-Type', 'application/octet-stream')
    const token = getAuthToken()
    if (token) xhr.setRequestHeader('Authorization', `Bearer ${token}`)

    if (onProgress && xhr.upload) {
      xhr.upload.onprogress = (ev: ProgressEvent) => {
        if (ev.lengthComputable && ev.total > 0) {
          const pct = Math.max(0, Math.min(100, Math.round((ev.loaded / ev.total) * 100)))
          try {
            onProgress(pct)
          } catch {
            /* ignore user cb error */
          }
        }
      }
    }

    xhr.onload = () => {
      if (xhr.status === 401) {
        localStorage.removeItem('eventide_token')
        localStorage.removeItem('eventide_user')
        window.dispatchEvent(new CustomEvent('auth:unauthorized'))
        reject(new Error('未登录或登录已过期'))
        return
      }
      if (xhr.status >= 200 && xhr.status < 300) {
        try {
          const data = xhr.responseText ? JSON.parse(xhr.responseText) : { ok: true }
          resolve({
            ok: data?.ok ?? true,
            item: data?.item ?? undefined,
          })
        } catch {
          resolve({ ok: true })
        }
      } else {
        let msg = `上传失败 (${xhr.status})`
        try {
          const d = xhr.responseText ? JSON.parse(xhr.responseText) : null
          if (d?.message && typeof d.message === 'string') msg = d.message
        } catch {
          /* ignore parse err */
        }
        reject(new Error(msg))
      }
    }

    xhr.onerror = () => reject(new Error('网络错误：MIB 上传请求失败'))
    xhr.onabort = () => reject(new Error('上传已取消'))

    const body: ArrayBuffer | Uint8Array | Blob = bytes ?? new Uint8Array()
    xhr.send(body)
  })
}

/**
 * MIB 模块详情：GET /api/mibs/{id}
 */
export function getMib(id: string): Promise<B4MibModuleItem> {
  return request<B4MibModuleItem>(`/api/mibs/${encodeURIComponent(id ?? '')}`, { method: 'GET' })
}

/**
 * 删除 MIB 模块：DELETE /api/mibs/{id}
 */
export async function deleteMib(id: string): Promise<{ ok: boolean }> {
  try {
    const resp = await request<{ ok: boolean } | null | undefined>(
      `/api/mibs/${encodeURIComponent(id ?? '')}`,
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
 * 列出 MIB 树某节点的子节点：GET /api/mibs/{id}/children?oid=xxx
 * oid 缺省或空串表示取根节点子节点
 */
export function listChildren(
  id: string,
  oid?: string,
): Promise<B4MibTreeNodeList> {
  const query: Record<string, string> = {}
  if (oid !== undefined && oid !== '') query.oid = oid
  return request<B4MibTreeNodeList>(`/api/mibs/${encodeURIComponent(id ?? '')}/children`, {
    method: 'GET',
    query,
  })
}

/**
 * 获取 MIB 节点详情：GET /api/mibs/{id}/node?oid=xxx
 */
export function getNodeDetail(
  id: string,
  oid: string,
): Promise<B4MibNodeDetail> {
  return request<B4MibNodeDetail>(`/api/mibs/${encodeURIComponent(id ?? '')}/node`, {
    method: 'GET',
    query: { oid: oid ?? '' },
  })
}

/**
 * 列出 MIB 模块中所有 Notification/Trap 定义：
 *   GET /api/mibs/{id}/notifications
 */
export function listNotifications(id: string): Promise<B4MibNotificationList> {
  return request<B4MibNotificationList>(
    `/api/mibs/${encodeURIComponent(id ?? '')}/notifications`,
    { method: 'GET' },
  )
}

/**
 * 导出单个模块的 Policies 为 xlsx blob：
 *   GET /api/mibs/{id}/export-policies
 */
export function exportModulePolicies(id: string): Promise<Blob> {
  return fetchBlob(`/api/mibs/${encodeURIComponent(id ?? '')}/export-policies`)
}

/**
 * 导出全部模块合并后的 Policies 为 xlsx blob：
 *   GET /api/mibs/export-policies
 */
export function exportAllPolicies(): Promise<Blob> {
  return fetchBlob('/api/mibs/export-policies')
}

/**
 * 应用单个模块的 Policies：POST /api/mibs/{id}/apply-policies?mode=xxx
 */
export function applyModulePolicies(
  id: string,
  mode: 'merge' | 'replace' | 'keep',
): Promise<{ ok: boolean; result: B4ApplyPolicyResult }> {
  const url = `/api/mibs/${encodeURIComponent(id ?? '')}/apply-policies`
  return request<{ ok: boolean; result: B4ApplyPolicyResult }>(url, {
    method: 'POST',
    query: { mode: mode ?? 'merge' },
  })
}

/**
 * 应用全部模块的 Policies：POST /api/mibs/apply-policies?mode=xxx
 */
export function applyAllPolicies(
  mode: 'merge' | 'replace' | 'keep',
): Promise<{ ok: boolean; result: B4ApplyPolicyResult }> {
  return request<{ ok: boolean; result: B4ApplyPolicyResult }>(
    '/api/mibs/apply-policies',
    {
      method: 'POST',
      query: { mode: mode ?? 'merge' },
    },
  )
}

/**
 * 让后端重新加载 MIB 目录：POST /api/mibs/reload
 */
export function reloadMibs(): Promise<B4MibModuleList> {
  return request<B4MibModuleList>('/api/mibs/reload', { method: 'POST' })
}

/**
 * SNMP Get 工具：POST /api/snmp/get
 */
export function snmpGet(body: {
  host: string
  port?: number
  community?: string
  oid: string
}): Promise<B4SnmpGetResult> {
  const payload: { host: string; port?: number; community?: string; oid: string } = {
    host: body?.host ?? '',
    oid: body?.oid ?? '',
  }
  if (body?.port !== undefined && body.port > 0) payload.port = body.port
  if (body?.community !== undefined && body.community !== '') payload.community = body.community
  return request<B4SnmpGetResult>('/api/snmp/get', {
    method: 'POST',
    body: JSON.stringify(payload),
  })
}
