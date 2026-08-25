// B3 批次：IAM（Department / Role / User / ResetPassword / Audit / Permissions）
import { request } from './request'
import type {
  Department,
  DepartmentInput,
  Role,
  RoleInput,
  UserAccount,
  UserInput,
  UserUpdateInput,
  ResetPasswordResp,
  AuditLog,
  AuditQuery,
} from './types'

// ——————————————————————————————————————
// Department
// ——————————————————————————————————————

export function listDepartments(): Promise<Department[]> {
  return request<Department[]>('/api/departments', {
    method: 'GET',
  })
}

export function createDepartment(body: DepartmentInput): Promise<Department> {
  return request<Department>('/api/departments', {
    method: 'POST',
    body: JSON.stringify(body),
  })
}

export function updateDepartment(
  id: string,
  body: DepartmentInput,
): Promise<Department> {
  return request<Department>(`/api/departments/${encodeURIComponent(id)}`, {
    method: 'PUT',
    body: JSON.stringify(body),
  })
}

export async function deleteDepartment(id: string): Promise<{ ok: true }> {
  try {
    const resp = await request<{ ok: true } | null | undefined>(
      `/api/departments/${encodeURIComponent(id)}`,
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
// Role
// ——————————————————————————————————————

export function listRoles(): Promise<Role[]> {
  return request<Role[]>('/api/roles', {
    method: 'GET',
  })
}

export function createRole(body: RoleInput): Promise<Role> {
  return request<Role>('/api/roles', {
    method: 'POST',
    body: JSON.stringify(body),
  })
}

export function updateRole(
  id: string,
  body: RoleInput,
): Promise<Role> {
  return request<Role>(`/api/roles/${encodeURIComponent(id)}`, {
    method: 'PUT',
    body: JSON.stringify(body),
  })
}

export async function deleteRole(id: string): Promise<{ ok: true }> {
  try {
    const resp = await request<{ ok: true } | null | undefined>(
      `/api/roles/${encodeURIComponent(id)}`,
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
// User
// ——————————————————————————————————————

export interface UsersQuery {
  q?: string
  department_id?: string
  role_id?: string
  enabled?: boolean | null
}

export function listUsers(query?: UsersQuery): Promise<UserAccount[]> {
  const q: Record<string, string | number | boolean | undefined | null> = {}
  if (query) {
    if (query.q) q.q = query.q
    if (query.department_id) q.department_id = query.department_id
    if (query.role_id) q.role_id = query.role_id
    if (query.enabled === true || query.enabled === false) {
      q.enabled = query.enabled ? '1' : '0'
    }
  }
  return request<UserAccount[]>('/api/users', {
    method: 'GET',
    query: Object.keys(q).length ? q : undefined,
  })
}

export function createUser(body: UserInput): Promise<UserAccount> {
  return request<UserAccount>('/api/users', {
    method: 'POST',
    body: JSON.stringify(body),
  })
}

export function updateUser(
  id: string,
  body: UserUpdateInput,
): Promise<UserAccount> {
  return request<UserAccount>(`/api/users/${encodeURIComponent(id)}`, {
    method: 'PUT',
    body: JSON.stringify(body),
  })
}

export async function deleteUser(id: string): Promise<{ ok: true }> {
  try {
    const resp = await request<{ ok: true } | null | undefined>(
      `/api/users/${encodeURIComponent(id)}`,
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

export function resetUserPassword(id: string): Promise<ResetPasswordResp> {
  return request<ResetPasswordResp>(
    `/api/users/${encodeURIComponent(id)}/reset-password`,
    { method: 'POST' },
  )
}

// ——————————————————————————————————————
// Audit Logs
// ——————————————————————————————————————

export function listAuditLogs(params?: AuditQuery): Promise<AuditLog[]> {
  const query: Record<string, string | number | boolean | undefined | null> = {}
  if (params) {
    if (params.actor) query.actor = params.actor
    if (params.action) query.action = params.action
    if (params.resource_type) query.resource_type = params.resource_type
    if (params.q) query.q = params.q
    if (typeof params.limit === 'number') query.limit = params.limit
  }
  return request<AuditLog[]>('/api/audit-logs', {
    method: 'GET',
    query: Object.keys(query).length ? query : undefined,
  })
}

// ——————————————————————————————————————
// Permissions 目录
// ——————————————————————————————————————

export function listPermissions(): Promise<unknown> {
  return request<unknown>('/api/permissions', {
    method: 'GET',
  })
}
