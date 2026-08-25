/**
 * 权限与页面目录。与旧版 app.js / PERMISSION_CATALOG 对齐（TR-4.6）
 */

export type PageKey =
  | 'overview'
  | 'alerts'
  | 'silences'
  | 'maintenance'
  | 'datasources'
  | 'rules'
  | 'ingress'
  | 'kafka'
  | 'trap'
  | 'mib'
  | 'policies'
  | 'channels'
  | 'notifies'
  | 'enrich'
  | 'lookups'
  | 'users'
  | 'roles'
  | 'departments'
  | 'audit'
  | 'settings'

// --- 页面显示顺序（MainLayout 侧栏使用） ---
export const PAGE_ORDER: PageKey[] = [
  'overview',
  'alerts',
  'silences',
  'maintenance',
  'datasources',
  'rules',
  'ingress',
  'kafka',
  'trap',
  'mib',
  'policies',
  'channels',
  'notifies',
  'enrich',
  'lookups',
  'users',
  'roles',
  'departments',
  'settings',
  'audit',
]

export const PAGE_LABEL: Record<PageKey, string> = {
  overview: '总览',
  alerts: '告警事件',
  silences: '静默策略',
  maintenance: '维护窗',
  datasources: '数据源',
  rules: '告警规则',
  ingress: '告警接入',
  kafka: 'Kafka 接入',
  trap: 'SNMP Trap',
  mib: 'MIB 库',
  policies: 'Trap 策略',
  channels: '通知渠道',
  notifies: '通知记录',
  enrich: '告警丰富',
  lookups: '查询字典',
  users: '用户管理',
  roles: '权限管理',
  departments: '部门管理',
  audit: '操作审计',
  settings: '系统设置',
}

export const PAGE_ICON: Record<PageKey, string> = {
  overview: '🏠',
  alerts: '🔔',
  silences: '🧘',
  maintenance: '🔧',
  datasources: '🗄️',
  rules: '📏',
  ingress: '📥',
  kafka: '🦄',
  trap: '🪤',
  mib: '📚',
  policies: '🎯',
  channels: '📣',
  notifies: '✉️',
  enrich: '🧬',
  lookups: '🔎',
  users: '👥',
  roles: '🔑',
  departments: '🏢',
  audit: '📓',
  settings: '⚙',
}

// --- 侧栏分组（对齐旧版 static/index.html 结构） ---
// null = 不属任何分组（如 overview 独立展示在分组之前）
export type NavGroupKey = 'ops' | 'config' | 'notify' | 'system' | 'tools'

export const PAGE_GROUP: Record<PageKey, NavGroupKey | null> = {
  overview: null,
  alerts: 'ops',
  silences: 'ops',
  maintenance: 'ops',
  datasources: 'config',
  rules: 'config',
  ingress: 'config',
  kafka: 'tools',
  trap: 'config',
  mib: 'config',
  policies: 'config',
  channels: 'notify',
  notifies: 'notify',
  enrich: 'notify',
  lookups: 'notify',
  users: 'system',
  roles: 'system',
  departments: 'system',
  settings: 'system',
  audit: 'system',
}

export interface NavGroupDef {
  key: NavGroupKey
  label: string
  icon: string
}

// 分组在侧栏中的展示顺序（overview 在最前独立展示）
export const NAV_GROUPS: NavGroupDef[] = [
  { key: 'ops', label: '告警运营', icon: '⚠' },
  { key: 'config', label: '接入配置', icon: '▣' },
  { key: 'notify', label: '通知丰富', icon: '✦' },
  { key: 'system', label: '系统管理', icon: '⚙' },
  { key: 'tools', label: '工具', icon: '⚒' },
]

// --- 每页所需读权限（未实现的页仍然定义权限，方便后续接入） ---
export const PAGE_PERM: Record<PageKey, string> = {
  overview: 'overview:read',
  alerts: 'alerts:read',
  silences: 'silences:read',
  maintenance: 'maintenance:read',
  datasources: 'datasources:read',
  rules: 'rules:read',
  ingress: 'ingress:read',
  kafka: 'ingress:read',
  trap: 'trap:read',
  mib: 'trap:read',
  policies: 'trap:read',
  channels: 'channels:read',
  notifies: 'notifies:read',
  enrich: 'enrich:read',
  lookups: 'lookups:read',
  users: 'iam:read',
  roles: 'iam:read',
  departments: 'iam:read',
  audit: 'audit:read',
  settings: 'settings:read',
}

// --- 当前批次已实现的页面；未实现页会在侧栏显示为禁用态 ---
export const IMPLEMENTED_PAGES: PageKey[] = [
  'overview',
  'alerts',
  'silences',
  'maintenance',
  'datasources',
  'rules',
  'ingress',
  'kafka',
  'trap',
  'mib',
  'channels',
  'notifies',
  'enrich',
  'lookups',
  'users',
  'roles',
  'departments',
  'audit',
  'settings',
  // 独立路由外的功能页（如策略 policies 作为 Tab 嵌入）不单独暴露：
  // - 'policies' 通过 Trap 策略菜单映射为 'policies' PageKey（见下）
  'policies',
]

// --- IAM 三页（users/roles/departments）共用 Users.vue，通过 ?tab= 定位 ---
export type IamTab = 'user' | 'role' | 'department'
export const PAGE_TO_IAM_TAB: Partial<Record<PageKey, IamTab>> = {
  users: 'user',
  roles: 'role',
  departments: 'department',
}

export function isPageImplemented(key: string): key is PageKey {
  return IMPLEMENTED_PAGES.includes(key as PageKey)
}

// --- 权限目录（对齐 eventide-server/src/iam.rs PERMISSION_CATALOG，至少 20 条） ---
export interface PermDef {
  code: string
  label: string
  group: string
}

export const PERMISSION_CATALOG: PermDef[] = [
  { code: 'overview:read', label: '查看总览', group: '运营' },
  { code: 'alerts:read', label: '查看告警事件', group: '运营' },
  { code: 'alerts:write', label: '管理告警事件（确认/关闭等）', group: '运营' },
  { code: 'silences:read', label: '查看静默策略', group: '运营' },
  { code: 'silences:write', label: '创建/更新/删除静默策略', group: '运营' },
  { code: 'maintenance:read', label: '查看维护窗', group: '运营' },
  { code: 'maintenance:write', label: '创建/更新/删除维护窗', group: '运营' },
  { code: 'datasources:read', label: '查看数据源', group: '接入' },
  { code: 'datasources:write', label: '添加/编辑/删除数据源', group: '接入' },
  { code: 'rules:read', label: '查看告警规则', group: '接入' },
  { code: 'rules:write', label: '管理告警规则', group: '接入' },
  { code: 'ingress:read', label: '查看告警接入路由与指标', group: '接入' },
  { code: 'ingress:write', label: '管理告警接入路由', group: '接入' },
  { code: 'trap:read', label: '查看 SNMP Trap 规则/MIB', group: '接入' },
  { code: 'trap:write', label: '管理 SNMP Trap / 试推送', group: '接入' },
  { code: 'channels:read', label: '查看通知渠道', group: '通知' },
  { code: 'channels:write', label: '管理通知渠道', group: '通知' },
  { code: 'notifies:read', label: '查看通知记录', group: '通知' },
  { code: 'enrich:read', label: '查看告警丰富', group: '通知' },
  { code: 'enrich:write', label: '管理告警丰富', group: '通知' },
  { code: 'lookups:read', label: '查看查询字典', group: '通知' },
  { code: 'lookups:write', label: '管理查询字典', group: '通知' },
  { code: 'iam:read', label: '查看用户/角色/部门', group: '系统' },
  { code: 'iam:write', label: '管理用户/角色/部门', group: '系统' },
  { code: 'audit:read', label: '查看操作审计', group: '系统' },
  { code: 'settings:read', label: '查看系统设置', group: '系统' },
  { code: 'settings:write', label: '修改系统设置', group: '系统' },
]

/**
 * 权限校验：支持通配 '*' 表示超级管理员
 */
export function can(perm: string, perms: string[]): boolean {
  if (!perms || perms.length === 0) return false
  return perms.includes('*') || perms.includes(perm)
}

/** 按页面 key 校验权限 */
export function canPage(page: string, perms: string[]): boolean {
  const need = PAGE_PERM[page as PageKey]
  if (!need) return true
  return can(need, perms)
}

/** 找到第一个有权限、且已实现的页面 */
export function firstAllowedPage(perms: string[]): PageKey {
  return (
    PAGE_ORDER.find((p) => isPageImplemented(p) && canPage(p, perms)) || 'overview'
  )
}

export default {
  PAGE_ORDER,
  PAGE_LABEL,
  PAGE_ICON,
  PAGE_PERM,
  IMPLEMENTED_PAGES,
  PERMISSION_CATALOG,
  can,
  canPage,
  firstAllowedPage,
}
