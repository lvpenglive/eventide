// B3 批次：Enrich Rules + Lookup Tables + Enrich Preview
import { request } from './request'
import type {
  EnrichRule,
  EnrichInput,
  EnrichPreviewInput,
  EnrichPreviewResp,
  LookupTable,
  LookupInput,
  IngressRoute,
} from './types'

function _enrichInputFromRow(row: EnrichRule): EnrichInput {
  return {
    name: row.name,
    kind: typeof row.kind === 'string' ? row.kind : undefined,
    enabled: row.enabled,
    priority: typeof row.priority === 'number' ? row.priority : 100,
    matchers: row.matchers ? { ...row.matchers } : {},
    templates: row.templates ? { ...row.templates } : {},
    field_templates: row.field_templates ? { ...row.field_templates } : {},
    label_extracts: row.label_extracts ? { ...row.label_extracts } : {},
    write_labels: Boolean(row.write_labels),
    match_key: row.match_key ?? undefined,
    mappings: row.mappings ? Object.fromEntries(Object.entries(row.mappings).map(([k, v]) => [k, { ...v }])) : {},
    lookup_table_ids: row.lookup_table_ids ? [...row.lookup_table_ids] : [],
    lookup_match_keys: row.lookup_match_keys ? { ...row.lookup_match_keys } : {},
  }
}

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
// Ingress Routes (for enrich preview)
// ——————————————————————————————————————
export function listIngresses(): Promise<IngressRoute[]> {
  return request<IngressRoute[]>('/api/ingress', {
    method: 'GET',
  })
}

// ——————————————————————————————————————
// Enrich Preview
// ——————————————————————————————————————

export async function previewEnrich(
  body: EnrichPreviewInput,
): Promise<EnrichPreviewResp> {
  const merged: EnrichPreviewInput = { ...body }
  // 兼容：如果传了 rule_id 但没传 rule / use_saved，则自动把 rule_id 转为 rule 草稿
  if (merged.rule_id && !merged.rule && !merged.use_saved) {
    try {
      const rule = await getEnrich(merged.rule_id)
      merged.rule = _enrichInputFromRow(rule)
      if (!merged.rule_name) merged.rule_name = rule.name
    } catch (_e) {
      // 失败就回退：忽略 rule_id 直接按原样请求
    }
  }
  return request<EnrichPreviewResp>('/api/enrich/preview', {
    method: 'POST',
    body: JSON.stringify(merged),
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
