import { request } from './request'
import type { MaintenanceWindow, NotifyChannel } from './types'

/**
 * 通知渠道列表：GET /api/channels
 */
export function listChannels(): Promise<NotifyChannel[]> {
  return request<NotifyChannel[]>('/api/channels', {
    method: 'GET',
  })
}

/**
 * 维护窗口列表：GET /api/maintenance-windows
 */
export function listMaintenanceWindows(): Promise<MaintenanceWindow[]> {
  return request<MaintenanceWindow[]>('/api/maintenance-windows', {
    method: 'GET',
  })
}
