// B2 批次：Rules API
import { request } from './request'
import type { Rule, RuleInput, RuleEvaluateResp } from './types'

/**
 * GET /api/rules → Rule[]
 */
export function listRules(): Promise<Rule[]> {
  return request<Rule[]>('/api/rules', {
    method: 'GET',
  })
}

/**
 * GET /api/rules/{id} → Rule
 */
export function getRule(id: string): Promise<Rule> {
  return request<Rule>(`/api/rules/${encodeURIComponent(id)}`, {
    method: 'GET',
  })
}

/**
 * POST /api/rules → Rule (201)
 */
export function createRule(body: RuleInput): Promise<Rule> {
  return request<Rule>('/api/rules', {
    method: 'POST',
    body: JSON.stringify(body),
  })
}

/**
 * PUT /api/rules/{id} → Rule
 */
export function updateRule(id: string, body: RuleInput): Promise<Rule> {
  return request<Rule>(`/api/rules/${encodeURIComponent(id)}`, {
    method: 'PUT',
    body: JSON.stringify(body),
  })
}

/**
 * DELETE /api/rules/{id} → 204 无内容，返回 {ok:true}
 */
export async function deleteRule(id: string): Promise<{ ok: true }> {
  try {
    const resp = await request<{ ok: true } | null | undefined>(
      `/api/rules/${encodeURIComponent(id)}`,
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
 * POST /api/rules/{id}/evaluate → RuleEvaluateResp
 */
export function evaluateRule(id: string): Promise<RuleEvaluateResp> {
  return request<RuleEvaluateResp>(
    `/api/rules/${encodeURIComponent(id)}/evaluate`,
    { method: 'POST' }
  )
}
