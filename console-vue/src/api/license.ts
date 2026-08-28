import { request } from './request'

export interface LicenseInfo {
  kind: 'trial' | 'licensed' | 'expired'
  customer?: string
  expires_at?: string
  days_left?: number | null
  writable?: boolean
  reason?: string
  has_license_token?: boolean
  install_id?: string
}

export async function getLicense(): Promise<LicenseInfo> {
  return request<LicenseInfo>('/api/license')
}

export async function importLicense(file: File): Promise<LicenseInfo> {
  const fd = new FormData()
  fd.append('file', file)
  return request<LicenseInfo>('/api/license', { method: 'POST', body: fd })
}

export async function clearLicense(): Promise<{ ok: boolean }> {
  return request<{ ok: boolean }>('/api/license', { method: 'DELETE' })
}

export async function requestLicenseRequest(params?: { customer?: string; email?: string }): Promise<Blob> {
  const qs = new URLSearchParams()
  if (params?.customer) qs.append('customer', params.customer)
  if (params?.email) qs.append('email', params.email)
  const url = `/api/license/request${qs.toString() ? `?${qs.toString()}` : ''}`
  const resp = await fetch(url, {
    method: 'GET',
    headers: {
      Accept: 'application/json',
      Authorization: `Bearer ${localStorage.getItem('eventide_token') || ''}`,
    },
  })
  if (!resp.ok) throw new Error(`请求失败 (${resp.status})`)
  return resp.blob()
}
