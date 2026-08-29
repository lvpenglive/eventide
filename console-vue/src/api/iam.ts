// IAM (Users / Roles / Departments / Permissions / Audit)
import { request } from './request'
import type {
  Department, DepartmentInput,
  Role, RoleInput,
  UserAccount, UserInput, UserUpdateInput, ResetPasswordResp, AuditLog,
} from './types'

// ============== Departments ==============

export function listDepartments(): Promise<Department[]> {
  return request<Department[]>('/api/departments', { method: 'GET' })
}

export function createDepartment(body: DepartmentInput): Promise<Department> {
  return request<Department>('/api/departments', {
    method: 'POST',
    body: JSON.stringify(body),
  })
}

export function updateDepartment(id: string, body: DepartmentInput): Promise<Department> {
  return request<Department>(`/api/departments/${encodeURIComponent(id)}`, {
    method: 'PUT',
    body: JSON.stringify(body),
  })
}

export function deleteDepartment(id: string): Promise<{ ok: true }> {
  return request<{ ok: true }>(`/api/departments/${encodeURIComponent(id)}`, { method: 'DELETE' })
}

// ============== Roles ==============

export function listRoles(): Promise<Role[]> {
  return request<Role[]>('/api/roles', { method: 'GET' })
}

export function createRole(body: RoleInput): Promise<Role> {
  return request<Role>('/api/roles', {
    method: 'POST',
    body: JSON.stringify(body),
  })
}

export function updateRole(id: string, body: RoleInput): Promise<Role> {
  return request<Role>(`/api/roles/${encodeURIComponent(id)}`, {
    method: 'PUT',
    body: JSON.stringify(body),
  })
}

export function deleteRole(id: string): Promise<{ ok: true }> {
  return request<{ ok: true }>(`/api/roles/${encodeURIComponent(id)}`, { method: 'DELETE' })
}

// ============== Permissions ==============

export function listPermissions(): Promise<Array<{ key: string; label?: string; group?: string }>> {
  return request<Array<{ key: string; label?: string; group?: string }>>('/api/permissions', { method: 'GET' })
}

// ============== Users ==============

export interface UserListQuery {
  q?: string
  department_id?: string | null
  enabled?: boolean | null
}

export function listUsers(q?: UserListQuery): Promise<UserAccount[]> {
  const query: Record<string, string | number | boolean | undefined | null> = {}
  if (q?.q) query.q = q.q
  if (q?.department_id !== undefined && q?.department_id !== null) query.department_id = q.department_id
  if (q?.enabled !== undefined && q?.enabled !== null) query.enabled = q.enabled
  return request<UserAccount[]>('/api/users', { method: 'GET', query })
}

export function createUser(body: UserInput): Promise<UserAccount> {
  return request<UserAccount>('/api/users', {
    method: 'POST',
    body: JSON.stringify(body),
  })
}

export function updateUser(id: string, body: UserUpdateInput): Promise<UserAccount> {
  return request<UserAccount>(`/api/users/${encodeURIComponent(id)}`, {
    method: 'PUT',
    body: JSON.stringify(body),
  })
}

export function deleteUser(id: string): Promise<{ ok: true }> {
  return request<{ ok: true }>(`/api/users/${encodeURIComponent(id)}`, { method: 'DELETE' })
}

export function resetUserPassword(id: string): Promise<ResetPasswordResp> {
  return request<ResetPasswordResp>(`/api/users/${encodeURIComponent(id)}/reset-password`, {
    method: 'POST',
  })
}

// ============== Audit Logs (backend /api/audit-logs not yet wired; safe stub) ==============

export function listAuditLogs(_query?: Record<string, unknown>): Promise<AuditLog[]> {
  return Promise.resolve([])
}
