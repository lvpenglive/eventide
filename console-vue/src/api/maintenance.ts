// B2 批次：Maintenance Windows API（避免与 common.ts 的 listMaintenanceWindows 冲突）
import { request } from './request'
import type { MaintenanceWindow, MaintenanceInput } from './types'

/**
 * GET /api/maintenance-windows → MaintenanceWindow[]
 */
export function listMaintenance(): Promise<MaintenanceWindow[]> {
  return request<MaintenanceWindow[]>('/api/maintenance-windows', {
    method: 'GET',
  })
}

/**
 * GET /api/maintenance-windows/{id} → MaintenanceWindow
 */
export function getMaintenance(id: string): Promise<MaintenanceWindow> {
  return request<MaintenanceWindow>(
    `/api/maintenance-windows/${encodeURIComponent(id)}`,
    { method: 'GET' }
  )
}

/**
 * POST /api/maintenance-windows → MaintenanceWindow (201)
 */
export function createMaintenance(body: MaintenanceInput): Promise<MaintenanceWindow> {
  return request<MaintenanceWindow>('/api/maintenance-windows', {
    method: 'POST',
    body: JSON.stringify(body),
  })
}

/**
 * PUT /api/maintenance-windows/{id} → MaintenanceWindow
 */
export function updateMaintenance(
  id: string,
  body: MaintenanceInput
): Promise<MaintenanceWindow> {
  return request<MaintenanceWindow>(
    `/api/maintenance-windows/${encodeURIComponent(id)}`,
    {
      method: 'PUT',
      body: JSON.stringify(body),
    }
  )
}

/**
 * DELETE /api/maintenance-windows/{id} → 204 无内容，返回 {ok:true}
 */
export async function deleteMaintenance(id: string): Promise<{ ok: true }> {
  try {
    const resp = await request<{ ok: true } | null | undefined>(
      `/api/maintenance-windows/${encodeURIComponent(id)}`,
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
