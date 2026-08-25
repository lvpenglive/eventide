// B3 批次：Settings（AlertHistory / TrapToken / Storm / UiBetaToggle）
import { request } from './request'
import type {
  AlertHistorySettingsView,
  AlertHistorySettingsUpdate,
  TrapTokenSettingsView,
  StormSettingsView,
  StormSettingsUpdate,
  UiBetaSettingsView,
} from './types'

// —— 1. Alert History ——
export function getAlertHistory(): Promise<AlertHistorySettingsView> {
  return request<AlertHistorySettingsView>('/api/settings/alert-history', {
    method: 'GET',
  })
}

export function putAlertHistory(
  body: AlertHistorySettingsUpdate,
): Promise<AlertHistorySettingsView> {
  return request<AlertHistorySettingsView>('/api/settings/alert-history', {
    method: 'PUT',
    body: JSON.stringify(body),
  })
}

// —— 2. Trap Token ——
export function getTrapToken(): Promise<TrapTokenSettingsView> {
  return request<TrapTokenSettingsView>('/api/settings/trap-token', {
    method: 'GET',
  })
}

export function putTrapToken(
  body: { regenerate?: boolean },
): Promise<TrapTokenSettingsView> {
  return request<TrapTokenSettingsView>('/api/settings/trap-token', {
    method: 'PUT',
    body: JSON.stringify(body),
  })
}

// —— 3. Storm ——
export function getStorm(): Promise<StormSettingsView> {
  return request<StormSettingsView>('/api/settings/storm', {
    method: 'GET',
  })
}

export function putStorm(
  body: StormSettingsUpdate,
): Promise<StormSettingsView> {
  return request<StormSettingsView>('/api/settings/storm', {
    method: 'PUT',
    body: JSON.stringify(body),
  })
}

// —— 4. UI Beta Toggle ——
export function getUiBetaToggle(): Promise<UiBetaSettingsView> {
  return request<UiBetaSettingsView>('/api/settings/ui-betatoggle', {
    method: 'GET',
  })
}

export function putUiBetaToggle(
  body: { enabled: boolean },
): Promise<UiBetaSettingsView> {
  return request<UiBetaSettingsView>('/api/settings/ui-betatoggle', {
    method: 'PUT',
    body: JSON.stringify(body),
  })
}
