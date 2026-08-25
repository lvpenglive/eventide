// B2 批次：Datasources API
import { request } from './request'
import type { Datasource, DatasourceInput } from './types'

/**
 * GET /api/datasources → Datasource[]
 */
export function listDatasources(): Promise<Datasource[]> {
  return request<Datasource[]>('/api/datasources', {
    method: 'GET',
  })
}

/**
 * GET /api/datasources/{id} → Datasource
 */
export function getDatasource(id: string): Promise<Datasource> {
  return request<Datasource>(`/api/datasources/${encodeURIComponent(id)}`, {
    method: 'GET',
  })
}

/**
 * POST /api/datasources → Datasource (201)
 */
export function createDatasource(body: DatasourceInput): Promise<Datasource> {
  return request<Datasource>('/api/datasources', {
    method: 'POST',
    body: JSON.stringify(body),
  })
}

/**
 * PUT /api/datasources/{id} → Datasource
 */
export function updateDatasource(
  id: string,
  body: DatasourceInput
): Promise<Datasource> {
  return request<Datasource>(`/api/datasources/${encodeURIComponent(id)}`, {
    method: 'PUT',
    body: JSON.stringify(body),
  })
}

/**
 * DELETE /api/datasources/{id} → 204 无内容，返回 {ok:true}
 */
export async function deleteDatasource(id: string): Promise<{ ok: true }> {
  try {
    const resp = await request<{ ok: true } | null | undefined>(
      `/api/datasources/${encodeURIComponent(id)}`,
      { method: 'DELETE' }
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
 * 轻量「读即探测」：后端没有统一 probe 端点，成功 GET 即视为可达。
 */
export async function probeDatasource(
  id: string
): Promise<{ ok: true; datasource: Datasource }> {
  const ds = await getDatasource(id)
  return { ok: true, datasource: ds }
}
