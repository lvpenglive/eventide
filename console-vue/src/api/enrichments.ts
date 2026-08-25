// B3 批次：Enrich Rules + Lookup Tables + Enrich Preview
import { request } from './request'
import type {
  EnrichRule,
  EnrichInput,
  EnrichPreviewInput,
  EnrichPreviewResp,
  LookupTable,
  LookupInput,
} from './types'

// ——————————————————————————————————————
// Enrich Rules
// ——————————————————————————————————————

export function listEnrich(): Promise<EnrichRule[]> {
  return request<EnrichRule[]>('/api/enrich', {
    method: 'GET',
  })
}

export function getEnrich(id: string): Promise<EnrichRule> {
  return request<EnrichRule>(`/api/enrich/${encodeURIComponent(id)}`, {
    method: 'GET',
  })
}

export function createEnrich(body: EnrichInput): Promise<EnrichRule> {
  return request<EnrichRule>('/api/enrich', {
    method: 'POST',
    body: JSON.stringify(body),
  })
}

export function updateEnrich(
  id: string,
  body: EnrichInput,
): Promise<EnrichRule> {
  return request<EnrichRule>(`/api/enrich/${encodeURIComponent(id)}`, {
    method: 'PUT',
    body: JSON.stringify(body),
  })
}

export async function deleteEnrich(id: string): Promise<{ ok: true }> {
  try {
    const resp = await request<{ ok: true } | null | undefined>(
      `/api/enrich/${encodeURIComponent(id)}`,
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

// ——————————————————————————————————————
// Enrich Preview
// ——————————————————————————————————————

export function previewEnrich(
  body: EnrichPreviewInput,
): Promise<EnrichPreviewResp> {
  return request<EnrichPreviewResp>('/api/enrich/preview', {
    method: 'POST',
    body: JSON.stringify(body),
  })
}

// ——————————————————————————————————————
// Lookup Tables
// ——————————————————————————————————————

export function listLookups(): Promise<LookupTable[]> {
  return request<LookupTable[]>('/api/lookups', {
    method: 'GET',
  })
}

export function getLookup(id: string): Promise<LookupTable> {
  return request<LookupTable>(`/api/lookups/${encodeURIComponent(id)}`, {
    method: 'GET',
  })
}

export function createLookup(body: LookupInput): Promise<LookupTable> {
  return request<LookupTable>('/api/lookups', {
    method: 'POST',
    body: JSON.stringify(body),
  })
}

export function updateLookup(
  id: string,
  body: LookupInput,
): Promise<LookupTable> {
  return request<LookupTable>(`/api/lookups/${encodeURIComponent(id)}`, {
    method: 'PUT',
    body: JSON.stringify(body),
  })
}

export async function deleteLookup(id: string): Promise<{ ok: true }> {
  try {
    const resp = await request<{ ok: true } | null | undefined>(
      `/api/lookups/${encodeURIComponent(id)}`,
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
