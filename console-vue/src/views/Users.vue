<script setup lang="ts">
import { computed, onMounted, reactive, ref, markRaw, watch } from 'vue'
import { useRoute } from 'vue-router'
import {
  ElAlert,
  ElAvatar,
  ElButton,
  ElCard,
  ElCol,
  ElDescriptions,
  ElDescriptionsItem,
  ElDialog,
  ElDrawer,
  ElEmpty,
  ElForm,
  ElFormItem,
  ElInput,
  ElInputNumber,
  ElMessage,
  ElMessageBox,
  ElOption,
  ElPopover,
  ElRow,
  ElSelect,
  ElSwitch,
  ElTable,
  ElTableColumn,
  ElTag,
  ElTooltip,
} from 'element-plus'
import type { FormInstance, FormRules } from 'element-plus'
import {
  Delete,
  Edit,
  Key,
  Plus,
  Refresh,
  View,
} from '@element-plus/icons-vue'

// ============================================================
// 本地 IAM 类型定义（不触碰外部文件）
// ============================================================
interface Department {
  id: string
  name: string
  parent_id: string | null
  sort_order: number
  enabled: boolean
  updated_at: string
  created_at?: string
}
interface DepartmentInput {
  name: string
  parent_id: string | null
  sort_order: number
  enabled: boolean
}

interface Role {
  id: string
  name: string
  description?: string
  permissions: string[]
  is_system?: boolean
  updated_at: string
  created_at?: string
}
interface RoleInput {
  name: string
  description?: string
  permissions: string[]
}

interface UserRow {
  id: string
  username: string
  display_name?: string
  department_id: string | null
  role_ids: string[]
  enabled: boolean
  created_at: string
  password_changed_at?: string | null
}
interface UserInput {
  username: string
  display_name?: string
  password?: string
  department_id?: string | null
  role_ids?: string[]
  enabled?: boolean
}

interface ResetPasswordResp {
  ok: boolean
  temporary_password: string
}

interface AuditLog {
  id: string
  created_at: string
  actor_uid?: string
  actor_username?: string
  method?: string
  path?: string
  status_code?: number
  action?: string
  resource_type?: string
  resource_id?: string
  client_ip?: string
  detail_json?: unknown
}
interface AuditLogQuery {
  actor?: string
  action?: string
  resource_type?: string
  q?: string
  limit?: number
}

interface PermissionItem {
  key: string
  label?: string
  group?: string
}

interface IamApiShim {
  listDepartments(): Promise<Department[]>
  createDepartment(body: DepartmentInput): Promise<Department>
  updateDepartment(id: string, body: DepartmentInput): Promise<Department>
  deleteDepartment(id: string): Promise<{ ok: boolean }>

  listRoles(): Promise<Role[]>
  createRole(body: RoleInput): Promise<Role>
  updateRole(id: string, body: RoleInput): Promise<Role>
  deleteRole(id: string): Promise<{ ok: boolean }>
  listPermissions(): Promise<PermissionItem[]>

  listUsers(q?: {
    q?: string
    department_id?: string | null
    enabled?: boolean | null
  }): Promise<UserRow[]>
  createUser(body: UserInput): Promise<UserRow>
  updateUser(id: string, body: UserInput): Promise<UserRow>
  deleteUser(id: string): Promise<{ ok: boolean }>
  resetUserPassword(id: string): Promise<ResetPasswordResp>

  listAuditLogs(query?: AuditLogQuery): Promise<AuditLog[]>
}

// ============================================================
// Shim / 真实模块双模式绑定
// ============================================================
function _unimpl(name: string): never {
  throw new Error(`[API shim] 模块未实现：${name}，请等待并行任务创建对应 api/*.ts`)
}
const _shimIam: IamApiShim = {
  listDepartments: () => Promise.resolve([]),
  createDepartment: () => Promise.reject(_unimpl('createDepartment')),
  updateDepartment: () => Promise.reject(_unimpl('updateDepartment')),
  deleteDepartment: () => Promise.reject(_unimpl('deleteDepartment')),
  listRoles: () => Promise.resolve([]),
  createRole: () => Promise.reject(_unimpl('createRole')),
  updateRole: () => Promise.reject(_unimpl('updateRole')),
  deleteRole: () => Promise.reject(_unimpl('deleteRole')),
  listPermissions: () => Promise.resolve([]),
  listUsers: () => Promise.resolve([]),
  createUser: () => Promise.reject(_unimpl('createUser')),
  updateUser: () => Promise.reject(_unimpl('updateUser')),
  deleteUser: () => Promise.reject(_unimpl('deleteUser')),
  resetUserPassword: () => Promise.reject(_unimpl('resetUserPassword')),
  listAuditLogs: () => Promise.resolve([]),
}
// @ts-ignore 若 @/api/iam 模块尚未创建则忽略解析错误
import * as _rawIam from '@/api/iam'
const _iam = markRaw(_rawIam as unknown as IamApiShim | Record<string, unknown>)
function _bindApi<A extends object, S>(api: A | Record<string, unknown>, shim: S): S {
  const out = { ...(shim as unknown as object) } as S
  for (const key of Object.keys(shim as unknown as object)) {
    if (key in api && typeof (api as Record<string, unknown>)[key] === 'function') {
      ;(out as unknown as Record<string, unknown>)[key] = (api as Record<string, unknown>)[key]
    }
  }
  return out
}
const _api = _bindApi<Record<string, unknown>, IamApiShim>(
  _iam as unknown as Record<string, unknown>,
  _shimIam
)
const {
  listDepartments,
  createDepartment,
  updateDepartment,
  deleteDepartment,
  listRoles,
  createRole,
  updateRole,
  deleteRole,
  listPermissions,
  listUsers,
  createUser,
  updateUser,
  deleteUser,
  resetUserPassword,
  listAuditLogs,
} = _api

// ============================================================
// 通用工具
// ============================================================
function errMsgOf(e: unknown, fallback: string): string {
  if (e && typeof e === 'object') {
    const anyE = e as { message?: unknown }
    if (typeof anyE.message === 'string') return anyE.message
  }
  return fallback
}
function initial(name: string): string {
  const s = (name || '?').trim()
  if (!s) return '?'
  return s[0]!.toUpperCase()
}
function prettyJson(v: unknown): string {
  try {
    if (v == null) return ''
    if (typeof v === 'string') {
      try {
        return JSON.stringify(JSON.parse(v), null, 2)
      } catch {
        return v
      }
    }
    return JSON.stringify(v, null, 2)
  } catch {
    return String(v)
  }
}

// ============================================================
// Tab 1: 部门
// ============================================================
const route = useRoute()

// 根据路由名映射到对应 Tab：Users→user / Roles→role / Departments→department / Audit→自动打开审计抽屉
function tabFromRoute(name: string | symbol | undefined): 'department' | 'role' | 'user' {
  const n = String(name || '')
  if (n === 'Users') return 'user'
  if (n === 'Roles') return 'role'
  return 'department'
}
const activeTab = ref<'department' | 'role' | 'user'>(tabFromRoute(route.name))
const pageTitle = computed(() => {
  if (activeTab.value === 'department') return '部门管理'
  if (activeTab.value === 'role') return '权限管理'
  return '用户管理'
})
const pageSubtitle = computed(() => {
  if (activeTab.value === 'department') return '组织架构'
  if (activeTab.value === 'role') return '角色与权限码'
  return '账号 · 部门 · 角色'
})
const departments = ref<Department[]>([])
const departmentsLoading = ref(false)
const departmentMap = computed<Record<string, Department>>(() => {
  const m: Record<string, Department> = {}
  for (const d of departments.value) m[d.id] = d
  return m
})
const departmentOptions = computed<{ label: string; value: string | null }[]>(() => {
  const opts: { label: string; value: string | null }[] = [
    { label: '顶层', value: null },
  ]
  for (const d of departments.value) {
    opts.push({ label: d.name, value: d.id })
  }
  return opts
})
async function loadDepartments(force = false): Promise<void> {
  if (departments.value.length && !force) return
  departmentsLoading.value = true
  try {
    departments.value = await listDepartments()
  } catch (e) {
    ElMessage.error(errMsgOf(e, '加载部门列表失败'))
  } finally {
    departmentsLoading.value = false
  }
}

const deptDialogVisible = ref(false)
const deptEditingId = ref<string | null>(null)
const deptForm = reactive<DepartmentInput>({
  name: '',
  parent_id: null,
  sort_order: 0,
  enabled: true,
})
const deptFormRef = ref<FormInstance>()
const deptRules: FormRules<DepartmentInput> = {
  name: [{ required: true, message: '请输入部门名称', trigger: 'blur' }],
}
const isEditingSystemDept = computed(() => {
  return isSystemDept({ name: deptForm.name } as Department) && !!deptEditingId.value
})
function openCreateDepartment(): void {
  deptEditingId.value = null
  deptForm.name = ''
  deptForm.parent_id = null
  deptForm.sort_order = 0
  deptForm.enabled = true
  deptDialogVisible.value = true
}
function openEditDepartment(row: Department): void {
  deptEditingId.value = row.id
  deptForm.name = row.name
  deptForm.parent_id = row.parent_id
  deptForm.sort_order = row.sort_order
  deptForm.enabled = row.enabled
  deptDialogVisible.value = true
}
async function submitDepartment(): Promise<void> {
  const valid = await deptFormRef.value?.validate().catch(() => false)
  if (!valid) return
  try {
    if (deptEditingId.value) {
      await updateDepartment(deptEditingId.value, { ...deptForm })
      ElMessage.success('部门已更新')
    } else {
      await createDepartment({ ...deptForm })
      ElMessage.success('部门已创建')
    }
    deptDialogVisible.value = false
    await loadDepartments(true)
  } catch (e) {
    ElMessage.error(errMsgOf(e, '保存部门失败'))
  }
}
async function onToggleDepartmentEnabled(row: Department): Promise<void> {
  if (isSystemDept(row)) {
    ElMessage.warning('系统部门不可禁用')
    row.enabled = true
    return
  }
  try {
    await updateDepartment(row.id, {
      name: row.name,
      parent_id: row.parent_id,
      sort_order: row.sort_order,
      enabled: row.enabled,
    })
    ElMessage.success('已更新')
  } catch (e) {
    row.enabled = !row.enabled
    ElMessage.error(errMsgOf(e, '更新失败'))
  }
}
function isSystemDept(row: Department): boolean {
  return row.name.includes('默认') || row.name.includes('系统')
}
async function handleDeleteDepartment(row: Department): Promise<void> {
  if (isSystemDept(row)) {
    ElMessage.warning('系统部门不可删除')
    return
  }
  try {
    await ElMessageBox.confirm(`确认删除部门「${row.name}」？`, '删除部门', {
      type: 'warning',
    })
  } catch {
    return
  }
  try {
    await deleteDepartment(row.id)
    ElMessage.success('部门已删除')
    await loadDepartments(true)
  } catch (e) {
    ElMessage.error(errMsgOf(e, '删除部门失败'))
  }
}
function parentNameOf(id: string | null): string {
  if (id == null) return '顶层'
  return departmentMap.value[id]?.name ?? id
}

// ============================================================
// Tab 2: 角色
// ============================================================
const roles = ref<Role[]>([])
const rolesLoading = ref(false)
const permissions = ref<PermissionItem[]>([])
const permissionsLoaded = ref(false)
const permissionOptions = computed<{ label: string; value: string }[]>(() => {
  if (permissions.value.length === 0) return []
  return permissions.value.map((p) => ({
    label: p.label ?? p.key,
    value: p.key,
  }))
})
async function loadPermissions(force = false): Promise<void> {
  if (permissionsLoaded.value && !force) return
  try {
    permissions.value = await listPermissions()
    permissionsLoaded.value = true
  } catch (e) {
    ElMessage.error(errMsgOf(e, '加载权限列表失败'))
  }
}
async function loadRoles(force = false): Promise<void> {
  if (roles.value.length && !force) return
  rolesLoading.value = true
  try {
    roles.value = await listRoles()
  } catch (e) {
    ElMessage.error(errMsgOf(e, '加载角色列表失败'))
  } finally {
    rolesLoading.value = false
  }
}

const roleDialogVisible = ref(false)
const roleEditingId = ref<string | null>(null)
const roleEditingIsSystem = ref(false)
const roleForm = reactive<RoleInput>({
  name: '',
  description: '',
  permissions: [],
})
const roleFormRef = ref<FormInstance>()
const roleRules: FormRules<RoleInput> = {
  name: [{ required: true, message: '请输入角色名称', trigger: 'blur' }],
}
function openCreateRole(): void {
  roleEditingId.value = null
  roleEditingIsSystem.value = false
  roleForm.name = ''
  roleForm.description = ''
  roleForm.permissions = []
  roleDialogVisible.value = true
}
function openEditRole(row: Role): void {
  roleEditingId.value = row.id
  roleEditingIsSystem.value = !!row.is_system
  roleForm.name = row.name
  roleForm.description = row.description ?? ''
  roleForm.permissions = [...(row.permissions ?? [])]
  roleDialogVisible.value = true
}
async function submitRole(): Promise<void> {
  const valid = await roleFormRef.value?.validate().catch(() => false)
  if (!valid) return
  // 保护: 系统角色提交时保留原有权限
  const payload: RoleInput = roleEditingIsSystem.value
    ? { name: roleForm.name, description: roleForm.description } as RoleInput
    : { ...roleForm }
  try {
    if (roleEditingId.value) {
      await updateRole(roleEditingId.value, payload)
      ElMessage.success('角色已更新')
    } else {
      await createRole(payload)
      ElMessage.success('角色已创建')
    }
    roleDialogVisible.value = false
    await loadRoles(true)
  } catch (e) {
    ElMessage.error(errMsgOf(e, '保存角色失败'))
  }
}
async function handleDeleteRole(row: Role): Promise<void> {
  try {
    await ElMessageBox.confirm(`确认删除角色「${row.name}」？`, '删除角色', {
      type: 'warning',
    })
  } catch {
    return
  }
  try {
    await deleteRole(row.id)
    ElMessage.success('角色已删除')
    await loadRoles(true)
  } catch (e) {
    ElMessage.error(errMsgOf(e, '删除角色失败'))
  }
}

// ============================================================
// Tab 3: 用户
// ============================================================
const users = ref<UserRow[]>([])
const usersLoading = ref(false)
const userFilter = reactive<{
  q: string
  department_id: string | null
  enabled: 'all' | 'true' | 'false'
}>({
  q: '',
  department_id: null,
  enabled: 'all',
})
const roleMap = computed<Record<string, Role>>(() => {
  const m: Record<string, Role> = {}
  for (const r of roles.value) m[r.id] = r
  return m
})
const userDepartmentOptions = computed<{ label: string; value: string | null }[]>(() => {
  return [
    { label: '全部部门', value: null },
    ...departments.value.map((d) => ({ label: d.name, value: d.id })),
  ]
})
async function loadUsers(force = false): Promise<void> {
  usersLoading.value = true
  try {
    const q: {
      q?: string
      department_id?: string | null
      enabled?: boolean | null
    } = {}
    if (userFilter.q) q.q = userFilter.q
    if (userFilter.department_id) q.department_id = userFilter.department_id
    if (userFilter.enabled !== 'all') q.enabled = userFilter.enabled === 'true'
    users.value = await listUsers(q)
  } catch (e) {
    ElMessage.error(errMsgOf(e, '加载用户列表失败'))
  } finally {
    usersLoading.value = false
  }
}

const userDialogVisible = ref(false)
const userEditingId = ref<string | null>(null)
const userForm = reactive<{
  username: string
  display_name: string
  password: string
  department_id: string | null
  role_ids: string[]
  enabled: boolean
}>({
  username: '',
  display_name: '',
  password: '',
  department_id: null,
  role_ids: [],
  enabled: true,
})
const userFormRef = ref<FormInstance>()
const userRules: FormRules = {
  username: [
    { required: true, message: '请输入用户名', trigger: 'blur' },
    {
      min: 2,
      max: 32,
      message: '用户名长度 2-32 个字符',
      trigger: 'blur',
    },
  ],
  password: [
    {
      validator: (_r, value: string, cb: (err?: Error) => void) => {
        if (!userEditingId.value && (!value || value.length === 0)) {
          cb(new Error('新建用户时密码必填'))
        } else {
          cb()
        }
      },
      trigger: 'blur',
    },
  ],
}
function openCreateUser(): void {
  userEditingId.value = null
  userForm.username = ''
  userForm.display_name = ''
  userForm.password = ''
  userForm.department_id = null
  userForm.role_ids = []
  userForm.enabled = true
  userDialogVisible.value = true
}
function openEditUser(row: UserRow): void {
  userEditingId.value = row.id
  userForm.username = row.username
  userForm.display_name = row.display_name ?? ''
  userForm.password = '' // 空字符串 = 不改密码
  userForm.department_id = row.department_id
  userForm.role_ids = [...(row.role_ids ?? [])]
  userForm.enabled = row.enabled
  userDialogVisible.value = true
}
async function submitUser(): Promise<void> {
  const valid = await userFormRef.value?.validate().catch(() => false)
  if (!valid) return
  // 保护: admin 用户不能移除所有角色
  if (userForm.username === 'admin' && userForm.role_ids.length === 0) {
    ElMessage.error('admin 用户至少需要保留一个角色')
    return
  }
  const body: UserInput = {
    username: userForm.username,
    display_name: userForm.display_name || undefined,
    department_id: userForm.department_id,
    role_ids: userForm.role_ids,
    enabled: userForm.enabled,
  }
  if (userForm.password) body.password = userForm.password
  try {
    if (userEditingId.value) {
      await updateUser(userEditingId.value, body)
      ElMessage.success('用户已更新')
    } else {
      await createUser(body)
      ElMessage.success('用户已创建')
    }
    userDialogVisible.value = false
    await loadUsers(true)
  } catch (e) {
    ElMessage.error(errMsgOf(e, '保存用户失败'))
  }
}
async function onToggleUserEnabled(row: UserRow): Promise<void> {
  try {
    await updateUser(row.id, {
      username: row.username,
      enabled: row.enabled,
      role_ids: row.role_ids ?? [],
    })
    ElMessage.success('已更新')
  } catch (e) {
    row.enabled = !row.enabled
    ElMessage.error(errMsgOf(e, '更新失败'))
  }
}
async function handleResetPassword(row: UserRow): Promise<void> {
  try {
    await ElMessageBox.confirm(`确认重置用户「${row.username}」的密码？系统将生成临时密码。`, '重置密码', {
      type: 'warning',
    })
  } catch {
    return
  }
  try {
    const resp = await resetUserPassword(row.id)
    await ElMessageBox.alert(
      `用户 ${row.username} 的临时密码为：\n\n${resp.temporary_password}\n\n请尽快登录并修改密码。`,
      '密码已重置',
      { type: 'info', confirmButtonText: '我知道了' }
    )
  } catch (e) {
    ElMessage.error(errMsgOf(e, '重置密码失败'))
  }
}
async function handleDeleteUser(row: UserRow): Promise<void> {
  try {
    await ElMessageBox.confirm(`确认删除用户「${row.username}」？`, '删除用户', {
      type: 'warning',
    })
  } catch {
    return
  }
  try {
    await deleteUser(row.id)
    ElMessage.success('用户已删除')
    await loadUsers(true)
  } catch (e) {
    ElMessage.error(errMsgOf(e, '删除用户失败'))
  }
}
function userDepartmentName(row: UserRow): string {
  if (!row.department_id) return '-'
  return departmentMap.value[row.department_id]?.name ?? row.department_id
}
function userRoleTags(ids: string[]): { id: string; name: string }[] {
  return (ids ?? []).map((id) => ({
    id,
    name: roleMap.value[id]?.name ?? id,
  }))
}

// ============================================================
// 审计日志 Drawer
// ============================================================
const auditDrawerVisible = ref(false)
const auditLogs = ref<AuditLog[]>([])
const auditLoading = ref(false)
const auditFilter = reactive<AuditLogQuery & { limit: number }>({
  actor: '',
  action: '',
  resource_type: '',
  q: '',
  limit: 200,
})
const auditDetailRow = ref<AuditLog | null>(null)
const auditDetailDrawerVisible = ref(false)
async function loadAuditLogs(): Promise<void> {
  auditLoading.value = true
  try {
    auditLogs.value = await listAuditLogs({
      actor: auditFilter.actor || undefined,
      action: auditFilter.action || undefined,
      resource_type: auditFilter.resource_type || undefined,
      q: auditFilter.q || undefined,
      limit: auditFilter.limit,
    })
  } catch (e) {
    ElMessage.error(errMsgOf(e, '加载审计日志失败'))
  } finally {
    auditLoading.value = false
  }
}
function openAuditDrawer(): void {
  auditDrawerVisible.value = true
  void loadAuditLogs()
}
function openAuditDetail(row: AuditLog): void {
  auditDetailRow.value = row
  auditDetailDrawerVisible.value = true
}
function statusTagType(code?: number): 'success' | 'danger' | 'info' {
  if (code == null) return 'info'
  return code >= 200 && code < 300 ? 'success' : 'danger'
}

// 监听路由切换（同组件复用）时同步 Tab
watch(
  () => route.name,
  (n) => {
    activeTab.value = tabFromRoute(n)
  },
)

onMounted(async () => {
  await Promise.all([loadDepartments(), loadRoles(), loadUsers(), loadPermissions()])
})
</script>

<template>
  <div class="iam-users-page">
    <!-- 标题 + 副标题 -->
    <h2 class="page-title">{{ pageTitle }}</h2>
    <p class="page-subtitle">{{ pageSubtitle }}</p>

    <!-- 三大列表（按路由切换，无 tab 切换器，对齐旧版独立列表样式） -->
    <!-- ============ 部门列表 ============ -->
    <div v-show="activeTab === 'department'">
        <ElCard shadow="never">
          <template #header>
            <div class="card-header">
              <span>部门列表</span>
              <div>
                <ElButton type="primary" @click="openCreateDepartment" :icon="Plus">新建部门</ElButton>
                <ElButton style="margin-left: 8px" :icon="Refresh" @click="loadDepartments(true)">刷新</ElButton>
              </div>
            </div>
          </template>
          <ElTable :data="departments" v-loading="departmentsLoading" stripe>
            <ElTableColumn prop="name" label="名称" min-width="140" />
            <ElTableColumn label="父级" min-width="100">
              <template #default="{ row: raw }">
                {{ parentNameOf((raw as Department).parent_id) }}
              </template>
            </ElTableColumn>
            <ElTableColumn prop="sort_order" label="排序" width="70" align="right" />
            <ElTableColumn label="启用" width="70" align="center">
              <template #default="{ row: raw }">
                <ElSwitch
                  v-model="(raw as Department).enabled"
                  :disabled="isSystemDept(raw as Department)"
                  @change="onToggleDepartmentEnabled(raw as Department)"
                />
              </template>
            </ElTableColumn>
            <ElTableColumn prop="updated_at" label="更新时间" min-width="150" />
            <ElTableColumn label="操作" width="180">
              <template #default="{ row: raw }">
                <div class="row-actions">
                  <ElButton size="small" :icon="Edit" @click="openEditDepartment(raw as Department)">编辑</ElButton>
                  <ElButton
                    size="small"
                    type="danger"
                    plain
                    :icon="Delete"
                    :disabled="isSystemDept(raw as Department)"
                    @click="handleDeleteDepartment(raw as Department)"
                  >
                    删除
                  </ElButton>
                </div>
              </template>
            </ElTableColumn>
            <template #empty>
              <ElEmpty description="暂无部门数据" />
            </template>
          </ElTable>
        </ElCard>

        <!-- 部门 Dialog -->
        <ElDialog
          v-model="deptDialogVisible"
          :title="deptEditingId ? '编辑部门' : '新建部门'"
          width="520px"
          destroy-on-close
        >
          <ElForm ref="deptFormRef" :model="deptForm" :rules="deptRules" label-position="top">
            <div class="form-two-col">
              <ElFormItem label="名称" prop="name">
                <ElInput v-model="deptForm.name" placeholder="请输入部门名称" />
              </ElFormItem>
              <ElFormItem label="父级" prop="parent_id">
                <ElSelect v-model="deptForm.parent_id as unknown as string" placeholder="选择父级部门" clearable style="width: 100%">
                  <ElOption
                    v-for="opt in departmentOptions"
                    :key="String(opt.value)"
                    :label="opt.label"
                    :value="opt.value as unknown as string"
                  />
                </ElSelect>
              </ElFormItem>
            </div>
            <div class="form-two-col">
              <ElFormItem label="排序" prop="sort_order">
                <ElInputNumber v-model="deptForm.sort_order" :min="0" :step="1" controls-position="right" />
              </ElFormItem>
              <ElFormItem label="启用" prop="enabled">
                <ElSwitch v-model="deptForm.enabled" :disabled="isEditingSystemDept" />
                <span v-if="isEditingSystemDept" class="form-hint">系统部门不可禁用</span>
              </ElFormItem>
            </div>
          </ElForm>
          <template #footer>
            <ElButton @click="deptDialogVisible = false">取消</ElButton>
            <ElButton type="primary" @click="submitDepartment">保存</ElButton>
          </template>
        </ElDialog>
    </div>

    <!-- ============ 角色列表 ============ -->
    <div v-show="activeTab === 'role'">
        <ElCard shadow="never">
          <template #header>
            <div class="card-header">
              <span>角色列表</span>
              <div>
                <ElButton type="primary" @click="openCreateRole" :icon="Plus">新建角色</ElButton>
                <ElButton style="margin-left: 8px" :icon="Refresh" @click="loadRoles(true)">刷新</ElButton>
              </div>
            </div>
          </template>
          <ElTable :data="roles" v-loading="rolesLoading" stripe>
            <ElTableColumn prop="name" label="角色名称" min-width="140" />
            <ElTableColumn prop="description" label="描述" min-width="180" show-overflow-tooltip />
            <ElTableColumn label="权限" min-width="160">
              <template #default="{ row: raw }">
                <span class="perm-count">共 {{ ((raw as Role).permissions ?? []).length }} 项</span>
                <ElPopover
                  v-if="((raw as Role).permissions ?? []).length > 0"
                  placement="top-start"
                  :width="360"
                  trigger="hover"
                >
                  <template #reference>
                    <ElButton link type="primary" size="small">查看</ElButton>
                  </template>
                  <div class="perm-chip-wrap">
                    <ElTag
                      v-for="p in ((raw as Role).permissions ?? []).slice(0, 20)"
                      :key="p"
                      class="perm-chip"
                      size="small"
                      type="info"
                    >
                      {{ p }}
                    </ElTag>
                    <div v-if="((raw as Role).permissions ?? []).length > 20" class="perm-more">
                      其余 {{ ((raw as Role).permissions ?? []).length - 20 }} 项未展示
                    </div>
                  </div>
                </ElPopover>
              </template>
            </ElTableColumn>
            <ElTableColumn label="系统角色" width="80" align="center">
              <template #default="{ row: raw }">
                <ElTag v-if="(raw as Role).is_system" type="warning" effect="dark">系统</ElTag>
                <span v-else>-</span>
              </template>
            </ElTableColumn>
            <ElTableColumn prop="updated_at" label="更新时间" min-width="150" />
            <ElTableColumn label="操作" width="180">
              <template #default="{ row: raw }">
                <div class="row-actions">
                  <ElButton size="small" :icon="Edit" @click="openEditRole(raw as Role)">编辑</ElButton>
                  <ElButton v-if="!(raw as Role).is_system" size="small" type="danger" plain :icon="Delete" @click="handleDeleteRole(raw as Role)">删除</ElButton>
                  <span v-else class="admin-locked" title="系统角色不可删除">🔒</span>
                </div>
              </template>
            </ElTableColumn>
            <template #empty>
              <ElEmpty description="暂无角色数据" />
            </template>
          </ElTable>
        </ElCard>

        <!-- 角色 Dialog -->
        <ElDialog
          v-model="roleDialogVisible"
          :title="roleEditingId ? '编辑角色' : '新建角色'"
          width="600px"
          destroy-on-close
        >
          <ElForm ref="roleFormRef" :model="roleForm" :rules="roleRules" label-position="top">
            <div class="form-two-col">
              <ElFormItem label="名称" prop="name">
                <ElInput v-model="roleForm.name" placeholder="请输入角色名称" />
              </ElFormItem>
              <ElFormItem label="描述" prop="description">
                <ElInput
                  v-model="roleForm.description"
                  type="textarea"
                  :rows="3"
                  placeholder="选填，描述角色用途"
                />
              </ElFormItem>
            </div>
            <div class="form-two-col">
              <ElFormItem label="权限" prop="permissions" style="flex: 2">
                <ElSelect
                  v-if="!roleEditingIsSystem"
                  v-model="roleForm.permissions"
                  multiple
                  filterable
                  collapse-tags
                  collapse-tags-tooltip
                  reserve-keyword
                  placeholder="选择权限"
                  style="width: 100%"
                >
                  <ElOption
                    v-for="opt in permissionOptions"
                    :key="opt.value"
                    :label="opt.label"
                    :value="opt.value"
                  />
                </ElSelect>
                <ElTag v-else type="info" effect="dark" style="font-size: 14px; padding: 6px 12px;">
                  🔒 系统角色权限不可编辑 ({{ roleForm.permissions.length }} 项)
                </ElTag>
              </ElFormItem>
            </div>
          </ElForm>
          <template #footer>
            <ElButton @click="roleDialogVisible = false">取消</ElButton>
            <ElButton type="primary" @click="submitRole">保存</ElButton>
          </template>
        </ElDialog>
    </div>

    <!-- ============ 用户列表 ============ -->
    <div v-show="activeTab === 'user'">
        <ElCard shadow="never">
          <template #header>
            <div class="card-header">
              <span>用户列表</span>
              <div>
                <ElButton type="primary" @click="openCreateUser" :icon="Plus">新建用户</ElButton>
                <ElButton style="margin-left: 8px" :icon="Refresh" @click="loadUsers(true)">刷新</ElButton>
              </div>
            </div>
          </template>
          <!-- 筛选 -->
          <ElRow :gutter="12" class="filter-row">
            <ElCol :xs="24" :sm="12" :md="8" :lg="6">
              <ElInput
                v-model="userFilter.q"
                placeholder="搜索用户名 / 显示名"
                clearable
                :prefix-icon="View"
                @keyup.enter="loadUsers(true)"
              />
            </ElCol>
            <ElCol :xs="24" :sm="12" :md="8" :lg="6">
              <ElSelect v-model="userFilter.department_id as unknown as string" placeholder="所属部门" clearable style="width: 100%" @change="loadUsers(true)">
                <ElOption
                  v-for="opt in userDepartmentOptions"
                  :key="String(opt.value)"
                  :label="opt.label"
                  :value="opt.value as unknown as string"
                />
              </ElSelect>
            </ElCol>
            <ElCol :xs="24" :sm="12" :md="8" :lg="6">
              <ElSelect
                v-model="userFilter.enabled"
                placeholder="启用状态"
                style="width: 100%"
                @change="loadUsers(true)"
              >
                <ElOption label="全部" value="all" />
                <ElOption label="已启用" value="true" />
                <ElOption label="已禁用" value="false" />
              </ElSelect>
            </ElCol>
          </ElRow>
          <ElDivider style="margin: 12px 0" />
          <ElTable :data="users" v-loading="usersLoading" stripe>
            <ElTableColumn label="用户名" min-width="130">
              <template #default="{ row: raw }">
                <div class="user-cell">
                  <ElAvatar size="small" class="user-avatar">{{ initial((raw as UserRow).username) }}</ElAvatar>
                  <span class="user-name">{{ (raw as UserRow).username }}</span>
                </div>
              </template>
            </ElTableColumn>
            <ElTableColumn prop="display_name" label="显示名" min-width="140">
              <template #default="{ row: raw }">
                <span class="cell-ellipsis">{{ (raw as UserRow).display_name || '-' }}</span>
              </template>
            </ElTableColumn>
            <ElTableColumn label="部门" min-width="90">
              <template #default="{ row: raw }">
                {{ userDepartmentName(raw as UserRow) }}
              </template>
            </ElTableColumn>
            <ElTableColumn label="角色" min-width="140">
              <template #default="{ row: raw }">
                <div class="role-chip-wrap">
                  <ElTag
                    v-for="r in userRoleTags((raw as UserRow).role_ids)"
                    :key="r.id"
                    size="small"
                    type="primary"
                    class="role-chip"
                  >
                    {{ r.name }}
                  </ElTag>
                  <span v-if="!((raw as UserRow).role_ids ?? []).length" style="color: var(--el-text-color-secondary)">-</span>
                </div>
              </template>
            </ElTableColumn>
            <ElTableColumn label="启用" width="70" align="center">
              <template #default="{ row: raw }">
                <ElSwitch v-if="(raw as UserRow).username !== 'admin'" v-model="(raw as UserRow).enabled" @change="onToggleUserEnabled(raw as UserRow)" />
                <span v-else class="admin-locked">🔒</span>
              </template>
            </ElTableColumn>
            <ElTableColumn prop="created_at" label="创建时间" min-width="150" />
            <ElTableColumn prop="password_changed_at" label="密码修改时间" min-width="150">
              <template #default="{ row: raw }">
                {{ (raw as UserRow).password_changed_at || '未修改' }}
              </template>
            </ElTableColumn>
            <ElTableColumn label="操作" width="300">
              <template #default="{ row: raw }">
                <div class="row-actions">
                  <ElButton size="small" :icon="Key" @click="handleResetPassword(raw as UserRow)">重置密码</ElButton>
                  <ElButton size="small" :icon="Edit" @click="openEditUser(raw as UserRow)">编辑</ElButton>
                  <ElButton v-if="(raw as UserRow).username !== 'admin'" size="small" type="danger" plain :icon="Delete" @click="handleDeleteUser(raw as UserRow)">删除</ElButton>
                </div>
              </template>
            </ElTableColumn>
            <template #empty>
              <ElEmpty description="暂无用户数据" />
            </template>
          </ElTable>
        </ElCard>

        <!-- 用户 Dialog -->
        <ElDialog
          v-model="userDialogVisible"
          :title="userEditingId ? '编辑用户' : '新建用户'"
          width="720px"
          destroy-on-close
        >
          <ElForm ref="userFormRef" :model="userForm" :rules="userRules" label-position="top">
            <div class="form-two-col">
              <ElFormItem label="用户名" prop="username">
                <ElInput
                  v-model="userForm.username"
                  :disabled="!!userEditingId"
                  placeholder="2-32 字符，唯一"
                />
              </ElFormItem>
              <ElFormItem label="显示名" prop="display_name">
                <ElInput v-model="userForm.display_name" placeholder="选填" />
              </ElFormItem>
            </div>
            <div class="form-two-col">
              <ElFormItem label="密码" prop="password">
                <ElInput
                  v-model="userForm.password"
                  type="password"
                  show-password
                  :placeholder="userEditingId ? '留空 = 保持现有密码' : '请输入初始密码'"
                />
              </ElFormItem>
              <ElFormItem label="部门" prop="department_id">
                <ElSelect v-model="userForm.department_id as unknown as string" placeholder="选择部门" clearable style="width: 100%">
                  <ElOption
                    v-for="opt in departmentOptions.filter((o) => o.value !== null)"
                    :key="String(opt.value)"
                    :label="opt.label"
                    :value="opt.value as unknown as string"
                  />
                </ElSelect>
              </ElFormItem>
            </div>
            <div class="form-two-col">
              <ElFormItem label="角色" prop="role_ids">
                <ElSelect
                  v-model="userForm.role_ids"
                  multiple
                  collapse-tags
                  collapse-tags-tooltip
                  placeholder="选择角色"
                  style="width: 100%"
                >
                  <ElOption
                    v-for="r in roles"
                    :key="r.id"
                    :label="r.name"
                    :value="r.id"
                  />
                </ElSelect>
              </ElFormItem>
              <ElFormItem v-if="userForm.username !== 'admin'" label="启用" prop="enabled">
                <ElSwitch v-model="userForm.enabled" />
              </ElFormItem>
              <ElFormItem v-else label="启用">
                <span class="admin-locked">🔒 admin 用户不可禁用</span>
              </ElFormItem>
            </div>
          </ElForm>
          <template #footer>
            <ElButton @click="userDialogVisible = false">取消</ElButton>
            <ElButton type="primary" @click="submitUser">保存</ElButton>
          </template>
        </ElDialog>
    </div>

    <!-- 审计日志 Drawer -->
    <ElDrawer
      v-model="auditDrawerVisible"
      title="审计日志"
      direction="rtl"
      size="60%"
      destroy-on-close
    >
      <ElRow :gutter="12" class="filter-row">
        <ElCol :xs="24" :sm="12" :md="6">
          <ElInput
            v-model="auditFilter.actor"
            placeholder="执行者 actor"
            clearable
          />
        </ElCol>
        <ElCol :xs="24" :sm="12" :md="6">
          <ElInput
            v-model="auditFilter.action"
            placeholder="action"
            clearable
          />
        </ElCol>
        <ElCol :xs="24" :sm="12" :md="6">
          <ElInput
            v-model="auditFilter.resource_type"
            placeholder="resource_type"
            clearable
          />
        </ElCol>
        <ElCol :xs="24" :sm="12" :md="6">
          <ElInput
            v-model="auditFilter.q"
            placeholder="关键词 q"
            clearable
          />
        </ElCol>
        <ElCol :xs="24" :sm="12" :md="6">
          <div class="limit-wrap">
            <span class="limit-label">limit</span>
            <ElInputNumber v-model="auditFilter.limit" :min="1" :max="5000" controls-position="right" />
          </div>
        </ElCol>
        <ElCol :xs="24" :sm="12" :md="6">
          <ElButton type="primary" :icon="Refresh" @click="loadAuditLogs">刷新</ElButton>
        </ElCol>
      </ElRow>
      <ElDivider style="margin: 12px 0" />
      <ElTable :data="auditLogs" v-loading="auditLoading" stripe size="small">
        <ElTableColumn prop="created_at" label="时间" min-width="180" width="190" />
        <ElTableColumn prop="actor_username" label="执行者" min-width="140">
          <template #default="{ row: raw }">
            {{ (raw as AuditLog).actor_username || (raw as AuditLog).actor_uid || '-' }}
          </template>
        </ElTableColumn>
        <ElTableColumn label="方法 + 路径" min-width="220">
          <template #default="{ row: raw }">
            <span>
              <ElTag size="small" type="info">{{ (raw as AuditLog).method || '-' }}</ElTag>
              <span style="margin-left: 6px; word-break: break-all">{{ (raw as AuditLog).path || '' }}</span>
            </span>
          </template>
        </ElTableColumn>
        <ElTableColumn label="状态码" width="110" align="center">
          <template #default="{ row: raw }">
            <ElTag v-if="(raw as AuditLog).status_code != null" :type="statusTagType((raw as AuditLog).status_code)">
              {{ (raw as AuditLog).status_code }}
            </ElTag>
            <span v-else>-</span>
          </template>
        </ElTableColumn>
        <ElTableColumn label="动作 / 资源" min-width="260">
          <template #default="{ row: raw }">
            <div class="audit-act">
              <ElTag size="small" type="primary">{{ (raw as AuditLog).action || '-' }}</ElTag>
              <span style="margin-left: 6px">{{ (raw as AuditLog).resource_type || '' }}</span>
              <ElTooltip v-if="(raw as AuditLog).resource_id" :content="(raw as AuditLog).resource_id as string" placement="top">
                <span style="margin-left: 6px; color: var(--el-text-color-secondary)">
                  #{{
                    String((raw as AuditLog).resource_id).length > 10
                      ? String((raw as AuditLog).resource_id).slice(0, 10) + '…'
                      : (raw as AuditLog).resource_id
                  }}
                </span>
              </ElTooltip>
            </div>
          </template>
        </ElTableColumn>
        <ElTableColumn label="操作" width="110" align="center" fixed="right">
          <template #default="{ row: raw }">
            <ElButton size="small" :icon="View" @click="openAuditDetail(raw as AuditLog)">查看详情</ElButton>
          </template>
        </ElTableColumn>
        <template #empty>
          <ElEmpty description="暂无审计日志" />
        </template>
      </ElTable>
    </ElDrawer>

    <!-- 审计日志详情 Drawer -->
    <ElDrawer
      v-if="auditDetailRow"
      v-model="auditDetailDrawerVisible"
      title="审计日志详情"
      direction="rtl"
      size="40%"
      destroy-on-close
    >
      <ElDescriptions :column="1" border>
        <ElDescriptionsItem label="时间">{{ auditDetailRow.created_at }}</ElDescriptionsItem>
        <ElDescriptionsItem label="执行者 UID">
          {{ auditDetailRow.actor_uid || '-' }}
        </ElDescriptionsItem>
        <ElDescriptionsItem label="执行者用户名">
          {{ auditDetailRow.actor_username || '-' }}
        </ElDescriptionsItem>
        <ElDescriptionsItem label="客户端 IP">
          {{ auditDetailRow.client_ip || '-' }}
        </ElDescriptionsItem>
        <ElDescriptionsItem label="方法 / 路径">
          <ElTag size="small" type="info">{{ auditDetailRow.method || '-' }}</ElTag>
          <span style="margin-left: 6px">{{ auditDetailRow.path || '' }}</span>
        </ElDescriptionsItem>
        <ElDescriptionsItem label="状态码">
          <ElTag v-if="auditDetailRow.status_code != null" :type="statusTagType(auditDetailRow.status_code)">
            {{ auditDetailRow.status_code }}
          </ElTag>
          <span v-else>-</span>
        </ElDescriptionsItem>
        <ElDescriptionsItem label="动作">
          {{ auditDetailRow.action || '-' }}
        </ElDescriptionsItem>
        <ElDescriptionsItem label="资源类型 / ID">
          {{ auditDetailRow.resource_type || '-' }}
          <span v-if="auditDetailRow.resource_id" style="margin-left: 6px">
            #{{ auditDetailRow.resource_id }}
          </span>
        </ElDescriptionsItem>
      </ElDescriptions>
      <ElDivider />
      <div class="detail-json">
        <div class="detail-json-title">detail_json</div>
        <pre class="detail-json-body">{{ prettyJson(auditDetailRow.detail_json) || '(空)' }}</pre>
      </div>
    </ElDrawer>
  </div>
</template>

<style scoped>
.iam-users-page {
  padding: 8px 4px 24px;
}
.page-title {
  margin: 4px 0 4px;
}
.page-subtitle {
  color: var(--el-text-color-secondary);
  margin: 0 0 16px;
}
.toolbar-affix {
  z-index: 10;
}
.toolbar-bar {
  display: flex;
  gap: 8px;
  padding: 10px 12px;
  background: var(--el-bg-color-page);
  border-radius: 6px;
  margin-bottom: 16px;
}
.card-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}
.filter-row {
  margin-bottom: 4px;
}
.row-actions {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-wrap: nowrap;
}
.cell-ellipsis {
  display: block;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.iam-tabs :deep(.el-tabs__content) {
  padding-top: 8px;
}
.perm-count {
  margin-right: 8px;
  color: var(--el-text-color-regular);
}
.perm-chip-wrap {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}
.perm-chip {
  max-width: 300px;
}
.perm-more {
  width: 100%;
  margin-top: 6px;
  color: var(--el-text-color-secondary);
  font-size: 12px;
}
.user-cell {
  display: flex;
  align-items: center;
  gap: 8px;
}
.user-avatar {
  background: var(--el-color-primary);
  color: #fff;
}
.user-name {
  font-weight: 500;
}
.admin-locked {
  color: var(--el-text-color-secondary);
  font-size: 13px;
  cursor: not-allowed;
}
.form-two-col {
  display: flex;
  gap: 16px;
  margin-bottom: 4px;
}
.form-two-col .el-form-item {
  flex: 1;
  margin-bottom: 16px;
}
.form-hint {
  margin-left: 8px;
  font-size: 12px;
  color: var(--el-text-color-secondary);
}
.role-chip-wrap {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}
.role-chip {
  max-width: 180px;
}
.limit-wrap {
  display: flex;
  align-items: center;
  gap: 8px;
}
.limit-label {
  color: var(--el-text-color-secondary);
  font-size: 13px;
}
.audit-act {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
}
.detail-json {
  margin-top: 8px;
}
.detail-json-title {
  font-weight: 600;
  margin-bottom: 8px;
  color: var(--el-text-color-regular);
}
.detail-json-body {
  background: var(--el-fill-color-light);
  padding: 12px;
  border-radius: 6px;
  max-height: 50vh;
  overflow: auto;
  font-size: 12px;
  white-space: pre-wrap;
  word-break: break-all;
}
</style>
