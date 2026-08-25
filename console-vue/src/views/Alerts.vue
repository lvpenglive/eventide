<script setup lang="ts">
/* =========================================================
 * Alerts.vue —— 告警事件视图（Vue 3 + TS + Element Plus）
 * 严格复刻旧版 alerts.js 主要 UX。
 * 本文件内所有类型仅用于补齐 vue-tsc 严格校验；
 * 与 @/api/alerts、@/api/common、@/api/types 的模块契约按任务要求对齐。
 * ========================================================= */

import {
  computed,
  customRef,
  nextTick,
  onBeforeUnmount,
  onMounted,
  reactive,
  ref,
  watch,
  h,
} from 'vue'
import { useRoute, useRouter } from 'vue-router'

// --- Element Plus 按需 ---
import {
  ElAffix,
  ElBadge,
  ElButton,
  ElCard,
  ElCheckbox,
  ElCol,
  ElDescriptions,
  ElDescriptionsItem,
  ElDialog,
  ElDrawer,
  ElDropdown,
  ElDropdownItem,
  ElDropdownMenu,
  ElEmpty,
  ElForm,
  ElFormItem,
  ElIcon,
  ElInput,
  ElInputNumber,
  ElMessage,
  ElMessageBox,
  ElOption,
  ElPagination,
  ElPopover,
  ElProgress,
  ElRadioButton,
  ElRadioGroup,
  ElRow,
  ElSelect,
  ElStatistic,
  ElTable,
  ElTableColumn,
  ElTabPane,
  ElTabs,
  ElTag,
} from 'element-plus'
import type { FormInstance, FormRules, TableInstance } from 'element-plus'

import {
  Bell,
  Check,
  CircleClose,
  CopyDocument,
  DataAnalysis,
  Refresh,
  Search,
  Setting,
  Switch,
  Tickets,
  View,
  WarningFilled,
} from '@element-plus/icons-vue'

// --- 业务模块（任务要求存在，并行任务会创建具体实现） ---
import {
  listAlerts,
  getAlert,
  ackAlert,
  unackAlert,
  closeAlert,
  listAlertNotifies,
  batchAckAlerts,
  batchCloseAlerts,
} from '@/api/alerts'
import { listChannels, listMaintenanceWindows } from '@/api/common'
import {
  useAuthStore,
} from '@/stores/auth'

// --- 从 @/api/types 复用部分已有类型，其余在此文件补齐 ---
import type {
  AlertEvent as _TsAlertEvent,      // 可能不存在
  NotifyLog as _TsNotifyLog,          // 可能不存在
  AlertQuery as _TsAlertQuery,        // 可能不存在
  AlertStatusCounts as _TsAlertStatusCounts,
  BatchOpResp as _TsBatchOpResp,
  MaintenanceWindow as _TsMaintenanceWindow,
  NotifyChannel as _TsNotifyChannel,
} from '@/api/types'

// =================================================================
// 类型补齐（不修改 api/types.ts，全部在本地临时 interface 兜底）
// =================================================================
type Severity = 'critical' | 'error' | 'warning' | 'info' | 'ok' | string
type AlertStatus = 'firing' | 'pending' | 'resolved' | string
type AlertSource = 'ingress' | 'rule' | 'manual' | string

interface StatusCounts {
  firing: number
  pending: number
  resolved: number
  total?: number
}

// 告警事件（与 Rust 侧 AlertEvent 字段一一对应，按 alerts.js 使用范围补齐）
export interface LocalAlertEvent {
  id: string
  fingerprint: string
  status: AlertStatus
  severity: Severity
  /** 告警名：优先 alertname / bizchainName / rule */
  alertname?: string
  bizchainName?: string
  rule?: string
  rule_id?: string | null
  summary?: string
  description?: string
  /** label 集合 */
  labels?: Record<string, string> & {
    alertname?: string
    severity?: string
    instance?: string
    host?: string
    hostname?: string
    ip?: string
    alertIp?: string
    alert_group?: string
    alertkey?: string
    alert_key?: string
    alertGroup?: string
  }
  annotations?: Record<string, string>
  /** contacts 字段（来自 enrich 规则 / labels），别名规则见下文 extractContacts */
  contacts?: Array<{ role?: string; name?: string; phone?: string; email?: string }>
  source?: AlertSource
  ingress_name?: string
  ingress_kind?: string
  /** 起始时间 */
  starts_at?: string | null
  ends_at?: string | null
  created_at?: string | null
  updated_at?: string | null
  /** 最后一次评估时间 */
  evaluated_at?: string | null
  last_eval_at?: string | null
  /** 告警分组 key */
  alert_group?: string
  alertGroup?: string
  alert_key?: string
  alertKey?: string
  /** 接手信息 */
  acknowledged_at?: string | null
  acknowledged_by?: string | null
  ack_note?: string | null
  /** 升级时间 */
  escalated_at?: string | null
  /** 触发次数 */
  count?: number
  trigger_count?: number
  /** 备注 */
  note?: string | null
  comment?: string | null
  /** 原始 trap varbinds */
  varbinds?: Array<{ oid: string; val: string }>
  varbind_count?: number
}

export interface LocalNotifyLog {
  id?: string
  channel_name?: string
  channel_type?: string
  target?: string
  transition?: string
  status?: 'success' | 'failed' | 'pending' | string
  body?: string
  error?: string
  sent_at?: string | null
  created_at?: string | null
}

export interface LocalMaintenanceWindow {
  id?: string
  name: string
  enabled: boolean
  starts_at: string
  ends_at: string
  rule_id?: string | null
  matchers?: Array<{ key: string; value: string; operator?: '=' | '!=' | '=~' | '!~' | string }> | Record<string, string>
  created_by?: string
}

export interface LocalNotifyChannel {
  id?: string
  name: string
  type: string
  enabled?: boolean
}

export interface LocalBatchOpResp {
  ok: number
  failed: number
  errors?: Array<{ id?: string; message?: string }>
}

export interface LocalAlertQuery {
  status?: AlertStatus | 'all'
  q?: string
  ip?: string
  severity?: Severity | 'all'
  source?: AlertSource | 'all'
  acked?: 'all' | 'acked' | 'unacked'
  store?: 'default' | 'recent' | 'history'
  page?: number
  page_size?: number
}

// 对齐 @/api/types 导入名（无论是否存在，以本地 interface 为准）
type AlertEvent = RequiredKeys<_TsAlertEvent, never> extends never
  ? LocalAlertEvent
  : _TsAlertEvent & LocalAlertEvent
type NotifyLog = _TsNotifyLog extends object ? _TsNotifyLog & LocalNotifyLog : LocalNotifyLog
type MaintenanceWindow = _TsMaintenanceWindow extends object
  ? _TsMaintenanceWindow & LocalMaintenanceWindow
  : LocalMaintenanceWindow
type NotifyChannel = _TsNotifyChannel extends object
  ? _TsNotifyChannel & LocalNotifyChannel
  : LocalNotifyChannel
type BatchOpResp = _TsBatchOpResp extends object ? _TsBatchOpResp & LocalBatchOpResp : LocalBatchOpResp
type AlertStatusCounts = _TsAlertStatusCounts extends object
  ? _TsAlertStatusCounts & StatusCounts
  : StatusCounts
type AlertQuery = _TsAlertQuery extends object ? _TsAlertQuery & LocalAlertQuery : LocalAlertQuery

type RequiredKeys<T, K extends keyof T> = T

// =================================================================
// 运行时状态
// =================================================================
const auth = useAuthStore()
const router = useRouter()
const route = useRoute()

const LS_VIEW = 'eventide_alert_view'
const LS_AUTO = 'eventide_alert_auto'

const loading = ref(false)
const alerts = ref<AlertEvent[]>([])
const total = ref(0)
const statusCounts = ref<AlertStatusCounts>({ firing: 0, pending: 0, resolved: 0, total: 0 })

const maintenanceWindows = ref<MaintenanceWindow[]>([])
const channels = ref<NotifyChannel[]>([])
const channelsLoaded = ref(false)
const mwLoaded = ref(false)

// --- 过滤 & 视图 ---
type StatusTab = 'all' | 'firing' | 'pending' | 'resolved'
const statusTab = ref<StatusTab>('all')
const qRaw = ref('')
const ipRaw = debouncedRef('', 280)
const qDeb = debouncedRef('', 280)
watch(qRaw, (v) => (qDeb.value = v))
const severity = ref<Severity | 'all'>('all')
const source = ref<AlertSource | 'all'>('all')
const acked = ref<'all' | 'acked' | 'unacked'>('all')
const store = ref<'default' | 'recent' | 'history'>('default')
const autoRefresh = ref<0 | 15 | 30 | 60>(0)
const viewMode = ref<'cards' | 'table'>('cards')

// --- 分页 ---
const pageNum = ref(1)
const pageSize = 50
const pagedAlerts = computed<AlertEvent[]>(() => {
  const start = (pageNum.value - 1) * pageSize
  return alerts.value.slice(start, start + pageSize)
})

// --- 选择 ---
const selection = ref<Set<string>>(new Set())
const batchLimit = 50
const batchIds = computed<string[]>(() => {
  const ids = Array.from(selection.value).slice(0, batchLimit)
  if (selection.value.size > batchLimit) {
    // 超出时 warning 只提示一次：在操作方法中再提示，此处不自动
  }
  return ids
})

// --- drawer 详情 ---
const drawerVisible = ref(false)
const drawerLoading = ref(false)
const detailAlert = ref<AlertEvent | null>(null)
const detailNotifies = ref<NotifyLog[]>([])
const detailTab = ref<'overview' | 'notifies' | 'history'>('overview')
const notifyViewVisible = ref(false)
const notifyViewLog = ref<NotifyLog | null>(null)

// --- 批量操作弹框 ---
const batchAckVisible = ref(false)
const batchCloseVisible = ref(false)
const batchFormRef = ref<FormInstance | null>(null)
const batchForm = reactive<{ note: string; reason: string }>({ note: '', reason: '' })
const batchFormRules: FormRules = {
  reason: [
    { required: true, message: '请填写关闭原因', trigger: 'blur' },
    { min: 2, max: 120, message: '长度 2 ~ 120', trigger: 'blur' },
  ],
}

// --- 行级右键菜单（dropdown 模拟） ---
interface CtxState {
  visible: boolean
  x: number
  y: number
  alert: AlertEvent | null
}
const ctx = reactive<CtxState>({ visible: false, x: 0, y: 0, alert: null })

// --- 自动刷新 timer ---
let autoTimer: number | null = null

// =================================================================
// 工具函数
// =================================================================
function readLS<T>(key: string, fallback: T): T {
  try {
    const raw = localStorage.getItem(key)
    if (raw == null) return fallback
    return JSON.parse(raw) as T
  } catch {
    return fallback
  }
}
function writeLS(key: string, v: unknown): void {
  try {
    localStorage.setItem(key, JSON.stringify(v))
  } catch {
    /* noop */
  }
}

// 简易 debounced ref
function debouncedRef<T>(initial: T, wait = 280) {
  const r = ref(initial) as { value: T }
  let timer: number | null = null
  const set = (v: T) => {
    if (timer) window.clearTimeout(timer)
    timer = window.setTimeout(() => (r.value = v), wait)
  }
  return customRef<T>((track, trigger) => ({
    get() {
      track()
      return r.value
    },
    set(v) {
      trigger()
      set(v as T)
    },
  }))
}

function fmtTime(ts: string | null | undefined): string {
  if (!ts) return '-'
  const d = new Date(ts)
  if (Number.isNaN(d.getTime())) return String(ts)
  const pad = (n: number) => String(n).padStart(2, '0')
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`
}

// 持续时长：<60s → s；<3600 → m s；<48h → h m；否则 d h
function fmtDuration(start: string | null | undefined, end?: string | null | undefined): string {
  const s = start ? new Date(start).getTime() : NaN
  if (Number.isNaN(s)) return '-'
  const e = end ? new Date(end).getTime() : Date.now()
  let diff = Math.floor((e - s) / 1000)
  if (diff < 0) diff = 0
  if (diff < 60) return `${diff}s`
  if (diff < 3600) {
    const m = Math.floor(diff / 60)
    const ss = diff % 60
    return `${m}m ${ss}s`
  }
  if (diff < 48 * 3600) {
    const h = Math.floor(diff / 3600)
    const m = Math.floor((diff % 3600) / 60)
    return `${h}h ${m}m`
  }
  const d = Math.floor(diff / (3600 * 24))
  const h = Math.floor((diff % (3600 * 24)) / 3600)
  return `${d}d ${h}h`
}

// 告警名：优先 alertname / bizchainName / rule / labels.alertname
function alertName(a: AlertEvent): string {
  const aa = a as AlertEvent & LocalAlertEvent
  return (
    aa.alertname ||
    aa.bizchainName ||
    aa.rule ||
    aa.labels?.alertname ||
    aa.labels?.alert_name ||
    aa.fingerprint?.slice(0, 10) ||
    '(无标题告警)'
  )
}

function alertSummary(a: AlertEvent, max = 160): string {
  const aa = a as AlertEvent & LocalAlertEvent
  const s = aa.summary || aa.annotations?.summary || aa.description || aa.annotations?.description || ''
  if (!s) return '(无摘要)'
  return s.length > max ? s.slice(0, max) + '…' : s
}

// 抽取 IP / 主机 / 实例
function alertIp(a: AlertEvent): string {
  const aa = a as AlertEvent & LocalAlertEvent
  return (
    aa.labels?.alertIp ||
    aa.labels?.ip ||
    aa.labels?.instance?.split(':')[0] ||
    aa.labels?.host ||
    aa.labels?.hostname ||
    ''
  )
}
function alertHost(a: AlertEvent): string {
  const aa = a as AlertEvent & LocalAlertEvent
  return (
    aa.labels?.hostname ||
    aa.labels?.host ||
    aa.labels?.instance ||
    ''
  )
}

function sourceText(a: AlertEvent): string {
  const aa = a as AlertEvent & LocalAlertEvent
  if (aa.source === 'ingress' || aa.ingress_name) return `接入·${aa.ingress_name || 'ingress'}`
  if (aa.source === 'rule' || aa.rule) return `规则·${aa.rule || 'rule'}`
  if (aa.source) return String(aa.source)
  return '未知'
}

// severity / status 映射
type TagType = 'success' | 'warning' | 'info' | 'danger' | 'primary'
function severityMeta(s: Severity): { text: string; type: TagType; color: string } {
  switch (s) {
    case 'critical':
      return { text: '严重', type: 'danger', color: 'var(--crit)' }
    case 'error':
      return { text: '错误', type: 'danger', color: 'var(--crit-soft)' }
    case 'warning':
      return { text: '警告', type: 'warning', color: 'var(--warn)' }
    case 'info':
      return { text: '信息', type: 'info', color: 'var(--info)' }
    case 'ok':
      return { text: '正常', type: 'success', color: 'var(--ok)' }
    default:
      return { text: String(s || '-'), type: 'primary', color: 'var(--primary)' }
  }
}
function statusMeta(s: AlertStatus): { text: string; type: TagType; color: string } {
  switch (s) {
    case 'firing':
      return { text: '触发', type: 'danger', color: 'var(--crit)' }
    case 'pending':
      return { text: '等待', type: 'warning', color: 'var(--warn)' }
    case 'resolved':
      return { text: '已恢复', type: 'success', color: 'var(--ok)' }
    default:
      return { text: String(s || '-'), type: 'info', color: 'var(--info)' }
  }
}

// 状态色条
function railColor(a: AlertEvent): string {
  const aa = a as AlertEvent & LocalAlertEvent
  if (aa.status === 'resolved') return 'var(--ok)'
  const s = severityMeta(aa.severity)
  return s.color
}

// 抽取联系人 chips：三段角色（硬件/应用负责人）
interface ContactChip {
  role: string
  name: string
  phone: string
  email: string
}
function extractContacts(a: AlertEvent): ContactChip[] {
  const aa = a as AlertEvent & LocalAlertEvent
  const out: ContactChip[] = []
  const seen = new Set<string>()

  // 1) 直接来自 contacts 字段
  if (Array.isArray(aa.contacts)) {
    for (const c of aa.contacts) {
      if (!c || !c.name) continue
      const key = `${c.role || ''}::${c.name}::${c.phone || ''}`
      if (seen.has(key)) continue
      seen.add(key)
      out.push({
        role: c.role || '联系人',
        name: c.name,
        phone: c.phone || '',
        email: c.email || '',
      })
    }
  }
  // 2) labels + annotations 的三段角色：
  //    hw_owner / hw_phone / hw_email
  //    app_owner / app_phone / app_email
  //    owner / phone / email
  const buckets: Array<{ role: string; keys: [string, string, string] }> = [
    { role: '硬件负责人', keys: ['hw_owner', 'hw_phone', 'hw_email'] },
    { role: '应用负责人', keys: ['app_owner', 'app_phone', 'app_email'] },
    { role: '负责人', keys: ['owner', 'phone', 'email'] },
  ]
  const allKV: Record<string, string> = { ...(aa.labels || {}), ...(aa.annotations || {}) }
  for (const b of buckets) {
    const [kn, kp, ke] = b.keys
    const name = allKV[kn] || allKV[kn + '_name']
    if (!name) continue
    const phone = allKV[kp] || ''
    const email = allKV[ke] || ''
    const key = `${b.role}::${name}::${phone}`
    if (seen.has(key)) continue
    seen.add(key)
    out.push({ role: b.role, name, phone, email })
  }
  return out
}

// 维护窗命中：(enabled, now 在区间内, rule_id 匹配或为 null, matchers 全部 AND 匹配)
function matchMW(a: AlertEvent, mw: MaintenanceWindow): boolean {
  const mwa = mw as MaintenanceWindow & LocalMaintenanceWindow
  if (!mwa.enabled) return false
  const now = Date.now()
  const s = new Date(mwa.starts_at).getTime()
  const e = new Date(mwa.ends_at).getTime()
  if (!(now >= s && now < e)) return false

  const aa = a as AlertEvent & LocalAlertEvent
  if (mwa.rule_id) {
    if (aa.rule_id && String(aa.rule_id) !== String(mwa.rule_id)) return false
  }
  const labels = { ...(aa.labels || {}) }
  const matchers = Array.isArray(mwa.matchers)
    ? mwa.matchers.map((m) => ({ key: (m as { key: string }).key, value: (m as { value: string }).value, operator: (m as { operator?: string }).operator || '=' }))
    : Object.entries(mwa.matchers || {}).map(([k, v]) => ({ key: k, value: String(v), operator: '=' as const }))
  for (const m of matchers) {
    const lv = labels[m.key] || ''
    switch (m.operator) {
      case '!=':
        if (lv === m.value) return false
        break
      case '=~':
        try {
          if (!new RegExp(m.value).test(lv)) return false
        } catch {
          if (lv !== m.value) return false
        }
        break
      case '!~':
        try {
          if (new RegExp(m.value).test(lv)) return false
        } catch {
          if (lv === m.value) return false
        }
        break
      case '=':
      default:
        if (lv !== m.value) return false
    }
  }
  return true
}
function matchedMWs(a: AlertEvent): MaintenanceWindow[] {
  return maintenanceWindows.value.filter((mw) => matchMW(a, mw))
}
function mwBadgeText(a: AlertEvent): string {
  const list = matchedMWs(a)
  if (list.length === 0) return ''
  return list.map((m) => (m as MaintenanceWindow & LocalMaintenanceWindow).name).join('、')
}

// 复制
async function copyText(t: string, label?: string): Promise<void> {
  try {
    if (navigator.clipboard && navigator.clipboard.writeText) {
      await navigator.clipboard.writeText(t)
    } else {
      // 兜底
      const ta = document.createElement('textarea')
      ta.value = t
      ta.style.position = 'fixed'
      ta.style.left = '-9999px'
      document.body.appendChild(ta)
      ta.select()
      document.execCommand('copy')
      document.body.removeChild(ta)
    }
    ElMessage.success(`${label || '已复制'}：${t.length > 24 ? t.slice(0, 24) + '…' : t}`)
  } catch (e) {
    ElMessage.error('复制失败，请手动复制')
  }
}

// =================================================================
// 数据加载
// =================================================================
async function safeCall<T>(p: Promise<T>, onErr?: (msg: string) => void): Promise<T | null> {
  try {
    return await p
  } catch (e) {
    const msg = e && typeof e === 'object' && 'message' in e
      ? String((e as { message: unknown }).message)
      : '加载失败'
    ElMessage.error(msg)
    onErr?.(msg)
    return null
  }
}

interface ListAlertsRespShape {
  items: AlertEvent[]
  total: number
  status_counts: AlertStatusCounts
}

function buildQuery(): AlertQuery {
  const q: AlertQuery = {
    page: undefined,
    page_size: undefined,
  } as AlertQuery
  if (statusTab.value !== 'all') (q as LocalAlertQuery).status = statusTab.value
  const kv: LocalAlertQuery = q as LocalAlertQuery
  if (qDeb.value) kv.q = qDeb.value
  if (ipRaw.value) kv.ip = ipRaw.value
  if (severity.value !== 'all') kv.severity = severity.value
  if (source.value !== 'all') kv.source = source.value
  kv.acked = acked.value
  kv.store = store.value
  return q
}

async function loadAlerts(): Promise<void> {
  loading.value = true
  try {
    const q = buildQuery()
    const resp = (await safeCall(
      (listAlerts as unknown as (query: AlertQuery) => Promise<ListAlertsRespShape | AlertEvent[]>)(q),
    )) as ListAlertsRespShape | AlertEvent[] | null
    if (Array.isArray(resp)) {
      alerts.value = resp as AlertEvent[]
      total.value = resp.length
    } else if (resp && 'items' in resp) {
      alerts.value = resp.items || []
      total.value = Number(resp.total ?? alerts.value.length)
      if (resp.status_counts) statusCounts.value = resp.status_counts
    } else {
      alerts.value = []
      total.value = 0
    }
    // 分页归位（切换过滤条件时归 1，此处由 watcher 处理）
  } finally {
    loading.value = false
  }
}

async function loadMaintenanceWindows(force = false): Promise<void> {
  if (mwLoaded.value && !force) return
  const list = await safeCall<unknown>(
    (listMaintenanceWindows as unknown as () => Promise<unknown>)(),
  )
  if (Array.isArray(list)) {
    maintenanceWindows.value = list as MaintenanceWindow[]
  } else if (list && typeof list === 'object' && 'items' in list) {
    maintenanceWindows.value = ((list as { items?: MaintenanceWindow[] }).items || []) as MaintenanceWindow[]
  }
  mwLoaded.value = true
}

async function loadChannels(force = false): Promise<void> {
  if (channelsLoaded.value && !force) return
  const list = await safeCall<unknown>(
    (listChannels as unknown as () => Promise<unknown>)(),
  )
  if (Array.isArray(list)) {
    channels.value = list as NotifyChannel[]
  } else if (list && typeof list === 'object' && 'items' in list) {
    channels.value = ((list as { items?: NotifyChannel[] }).items || []) as NotifyChannel[]
  }
  channelsLoaded.value = true
}

// =================================================================
// 详情抽屉
// =================================================================
async function openDetail(idOrAlert: string | AlertEvent): Promise<void> {
  drawerVisible.value = true
  detailTab.value = 'overview'
  detailNotifies.value = []
  notifyViewLog.value = null
  notifyViewVisible.value = false

  const aa: AlertEvent | null =
    typeof idOrAlert === 'string'
      ? alerts.value.find((x) => (x as AlertEvent & LocalAlertEvent).id === idOrAlert) || null
      : idOrAlert
  if (aa) detailAlert.value = aa

  drawerLoading.value = true
  try {
    const id =
      typeof idOrAlert === 'string'
        ? idOrAlert
        : (idOrAlert as AlertEvent & LocalAlertEvent).id
    const fresh = await safeCall(
      (getAlert as unknown as (id: string) => Promise<AlertEvent>)(id),
    )
    if (fresh) detailAlert.value = fresh
  } finally {
    drawerLoading.value = false
  }
}

async function loadNotifies(id: string): Promise<void> {
  if (detailNotifies.value.length) return
  const list = await safeCall<unknown>(
    (listAlertNotifies as unknown as (id: string) => Promise<unknown>)(id),
  )
  if (Array.isArray(list)) detailNotifies.value = list as NotifyLog[]
  else if (list && typeof list === 'object' && 'items' in list) detailNotifies.value = ((list as { items?: NotifyLog[] }).items || []) as NotifyLog[]
}

watch(
  () => [drawerVisible.value, detailTab.value],
  () => {
    if (drawerVisible.value && detailTab.value === 'notifies' && detailAlert.value) {
      loadNotifies((detailAlert.value as AlertEvent & LocalAlertEvent).id)
    }
  },
)

// =================================================================
// 单条操作
// =================================================================
function isAcked(a: AlertEvent): boolean {
  return !!(a as AlertEvent & LocalAlertEvent).acknowledged_at
}
function isEscalated(a: AlertEvent): boolean {
  return !!(a as AlertEvent & LocalAlertEvent).escalated_at
}
function isResolved(a: AlertEvent): boolean {
  return (a as AlertEvent & LocalAlertEvent).status === 'resolved'
}

async function doAck(id: string, note?: string): Promise<boolean> {
  const r = await safeCall((ackAlert as unknown as (id: string, note?: string) => Promise<unknown>)(id, note))
  if (r !== null) {
    ElMessage.success('已接手告警')
    return true
  }
  return false
}
async function doUnack(id: string): Promise<boolean> {
  const r = await safeCall((unackAlert as unknown as (id: string) => Promise<unknown>)(id))
  if (r !== null) {
    ElMessage.success('已取消接手')
    return true
  }
  return false
}
async function doClose(id: string, reason: string): Promise<boolean> {
  const r = await safeCall((closeAlert as unknown as (id: string, reason: string) => Promise<unknown>)(id, reason))
  if (r !== null) {
    ElMessage.success('告警已关闭')
    return true
  }
  return false
}

// =================================================================
// 批量操作
// =================================================================
const batchAckFormRef = ref<FormInstance | null>(null)
const batchCloseFormRef = ref<FormInstance | null>(null)

function openBatchAck(): void {
  if (!auth.can('alerts:write')) return
  guardBatchSelection()
  batchForm.note = ''
  batchAckVisible.value = true
}
function openBatchClose(): void {
  if (!auth.can('alerts:write')) return
  guardBatchSelection()
  batchForm.reason = ''
  batchCloseVisible.value = true
}
function guardBatchSelection(): void {
  if (selection.value.size === 0) {
    ElMessage.warning('请先选择告警')
    return
  }
  if (selection.value.size > batchLimit) {
    ElMessage.warning(`已超过批量上限 ${batchLimit} 条，将自动取前 ${batchLimit} 条`)
  }
}
function cancelSelection(): void {
  selection.value = new Set()
  // 清除 table selection（table ref 可能为空）
  try {
    ;(tableRef.value as TableInstance | null)?.clearSelection?.()
  } catch {
    /* noop */
  }
}

async function confirmBatchAck(): Promise<void> {
  try {
    await batchAckFormRef.value?.validate()
  } catch {
    return
  }
  const ids = batchIds.value
  if (ids.length === 0) return
  const resp = await safeCall(
    (batchAckAlerts as unknown as (ids: string[], note?: string) => Promise<BatchOpResp>)(ids, batchForm.note || undefined),
  )
  batchAckVisible.value = false
  if (!resp) return
  ElMessage.success(`批量接手：成功 ${resp.ok} 条 / 失败 ${resp.failed} 条`)
  cancelSelection()
  await loadAlerts()
}

async function confirmBatchClose(): Promise<void> {
  try {
    await batchCloseFormRef.value?.validate()
  } catch {
    return
  }
  const ids = batchIds.value
  if (ids.length === 0) return
  const resp = await safeCall(
    (batchCloseAlerts as unknown as (ids: string[], reason: string) => Promise<BatchOpResp>)(ids, batchForm.reason),
  )
  batchCloseVisible.value = false
  if (!resp) return
  ElMessage.success(`批量关闭：成功 ${resp.ok} 条 / 失败 ${resp.failed} 条`)
  cancelSelection()
  await loadAlerts()
}

// =================================================================
// 选择联动
// =================================================================
const tableRef = ref<TableInstance | null>(null)
function onSelectionChange(rows: AlertEvent[]): void {
  const set = new Set<string>()
  for (const r of rows) {
    const id = (r as AlertEvent & LocalAlertEvent).id
    if (id) set.add(id)
  }
  // 保留其它页已选
  // 但本组件仅支持本页选择（简化），所以直接覆盖本页
  const pageIds = new Set(pagedAlerts.value.map((a) => (a as AlertEvent & LocalAlertEvent).id))
  for (const old of Array.from(selection.value)) {
    if (!pageIds.has(old)) set.add(old)
  }
  selection.value = set
}

function isSelected(a: AlertEvent): boolean {
  return selection.value.has((a as AlertEvent & LocalAlertEvent).id)
}
function toggleSelect(a: AlertEvent, e?: Event): void {
  e?.stopPropagation?.()
  const id = (a as AlertEvent & LocalAlertEvent).id
  const next = new Set(selection.value)
  if (next.has(id)) next.delete(id)
  else next.add(id)
  selection.value = next
  // 同步 table
  nextTick(() => {
    try {
      const t = tableRef.value as TableInstance | null
      if (!t) return
      // 简化：clear + toggleRowSelection
      const page = pagedAlerts.value
      for (const r of page) {
        t.toggleRowSelection(r, next.has((r as AlertEvent & LocalAlertEvent).id))
      }
    } catch {
      /* noop */
    }
  })
}

function togglePageAll(): void {
  const page = pagedAlerts.value
  const next = new Set(selection.value)
  const allSelected = page.every((a) => next.has((a as AlertEvent & LocalAlertEvent).id))
  if (allSelected) {
    for (const a of page) next.delete((a as AlertEvent & LocalAlertEvent).id)
  } else {
    for (const a of page) next.add((a as AlertEvent & LocalAlertEvent).id)
  }
  selection.value = next
  nextTick(() => {
    const t = tableRef.value as TableInstance | null
    if (!t) return
    for (const r of page) {
      t.toggleRowSelection(r, !allSelected)
    }
  })
}

// =================================================================
// 右键菜单（用 dropdown 模拟）
// =================================================================
function openCtx(e: MouseEvent, a: AlertEvent): void {
  e.preventDefault()
  e.stopPropagation()
  const maxX = Math.min(e.clientX, window.innerWidth - 240)
  const maxY = Math.min(e.clientY, window.innerHeight - 320)
  ctx.x = maxX
  ctx.y = maxY
  ctx.alert = a
  ctx.visible = true
}
function closeCtx(): void {
  ctx.visible = false
  ctx.alert = null
}
function onCtxClickOutside(): void {
  closeCtx()
}
onMounted(() => {
  window.addEventListener('click', onCtxClickOutside, true)
})
onBeforeUnmount(() => {
  window.removeEventListener('click', onCtxClickOutside, true)
})

// ctx 菜单动作
async function ctxAction(act: string): Promise<void> {
  const a = ctx.alert
  closeCtx()
  if (!a) return
  const id = (a as AlertEvent & LocalAlertEvent).id
  switch (act) {
    case 'view':
      await openDetail(a)
      break
    case 'ack':
      if (isAcked(a)) {
        if (await doUnack(id)) await loadAlerts()
      } else {
        if (await doAck(id)) await loadAlerts()
      }
      break
    case 'close':
      if (isResolved(a)) return
      try {
        const { value } = await ElMessageBox.prompt('请输入关闭原因', '关闭告警', {
          confirmButtonText: '确认',
          cancelButtonText: '取消',
          inputValidator: (v) => (v && v.length >= 2 ? true : '长度至少 2 字符'),
        })
        if (await doClose(id, value)) await loadAlerts()
      } catch {
        /* noop */
      }
      break
    case 'silence':
    case 'maintenance':
      ElMessage.info('批次 2 实现')
      break
    case 'copy_desc':
      await copyText(alertSummary(a, 10000), '描述')
      break
    case 'copy_ip':
      await copyText(alertIp(a) || '-', 'IP')
      break
    case 'copy_fp':
      await copyText((a as AlertEvent & LocalAlertEvent).fingerprint || '-', '指纹')
      break
    case 'filter_name':
      qRaw.value = alertName(a)
      break
    case 'filter_ip':
      ipRaw.value = alertIp(a)
      break
  }
}

// =================================================================
// 自动刷新
// =================================================================
function applyAutoRefresh(): void {
  if (autoTimer) {
    window.clearInterval(autoTimer)
    autoTimer = null
  }
  if (autoRefresh.value > 0) {
    autoTimer = window.setInterval(() => {
      loadAlerts().catch(() => void 0)
    }, autoRefresh.value * 1000)
  }
}
watch(autoRefresh, (v) => {
  writeLS(LS_AUTO, v)
  applyAutoRefresh()
})
watch(viewMode, (v) => {
  writeLS(LS_VIEW, v)
})

// 过滤条件变化时回到 page 1
watch(
  [statusTab, qDeb, ipRaw, severity, source, acked, store],
  () => {
    pageNum.value = 1
    loadAlerts().catch(() => void 0)
  },
)

// =================================================================
// 初始化
// =================================================================
onMounted(async () => {
  viewMode.value = readLS<'cards' | 'table'>(LS_VIEW, 'cards')
  autoRefresh.value = readLS<0 | 15 | 30 | 60>(LS_AUTO, 0) as 0 | 15 | 30 | 60

  // route.query 初始化 status / severity
  const s = route.query.status
  if (typeof s === 'string' && ['all', 'firing', 'pending', 'resolved'].includes(s)) {
    statusTab.value = s as StatusTab
  }
  const sev = route.query.severity
  if (typeof sev === 'string' && ['all', 'critical', 'error', 'warning', 'info', 'ok'].includes(sev)) {
    severity.value = sev
  }

  applyAutoRefresh()
  // 并行加载
  await Promise.all([loadAlerts(), loadMaintenanceWindows(), loadChannels()])
})

onBeforeUnmount(() => {
  if (autoTimer) window.clearInterval(autoTimer)
})

// =================================================================
// 描述详情：剔除 vb$i_oid/val 与 varbinds/varbind_count
// =================================================================
function annotationsFiltered(a: AlertEvent): Array<{ k: string; v: string }> {
  const ann = (a as AlertEvent & LocalAlertEvent).annotations || {}
  return Object.entries(ann)
    .filter(([k]) => !/^vb\d+_(oid|val)$/i.test(k) && !/^varbind(s|_count)$/i.test(k))
    .map(([k, v]) => ({ k, v: String(v) }))
}
function snmpVarbinds(a: AlertEvent): Array<{ idx: number; oid: string; val: string }> {
  const aa = a as AlertEvent & LocalAlertEvent
  if (Array.isArray(aa.varbinds) && aa.varbinds.length) {
    return aa.varbinds.map((v, i) => ({ idx: i + 1, oid: v.oid, val: v.val }))
  }
  const ann = aa.annotations || {}
  const out: Array<{ idx: number; oid: string; val: string }> = []
  let i = 1
  while (true) {
    const oid = ann[`vb${i}_oid`]
    const val = ann[`vb${i}_val`]
    if (oid == null && val == null) break
    out.push({ idx: i, oid: oid == null ? '' : String(oid), val: val == null ? '' : String(val) })
    i++
    if (i > 64) break
  }
  return out
}
</script>

<template>
  <div class="alerts-page">
    <!-- ===== 顶部 toolbar ===== -->
    <el-card class="alert-toolbar panel" shadow="never">
      <template #header>
        <div style="display:flex;align-items:center;justify-content:space-between;flex-wrap:wrap;gap:10px;">
          <div style="display:flex;align-items:center;gap:8px;flex-wrap:wrap;">
            <el-icon :size="18" color="var(--primary)"><Bell /></el-icon>
            <strong style="color:var(--heading);">告警事件</strong>
            <el-tag size="small" type="info" effect="plain">共 {{ total }} 条</el-tag>
          </div>
          <div style="display:flex;align-items:center;gap:6px;flex-wrap:wrap;">
            <el-radio-group v-model="autoRefresh" size="small">
              <el-radio-button :value="0">自动刷新·关</el-radio-button>
              <el-radio-button :value="15">15s</el-radio-button>
              <el-radio-button :value="30">30s</el-radio-button>
              <el-radio-button :value="60">60s</el-radio-button>
            </el-radio-group>
            <el-radio-group v-model="viewMode" size="small">
              <el-radio-button value="cards">卡片</el-radio-button>
              <el-radio-button value="table">表格</el-radio-button>
            </el-radio-group>
            <el-button size="small" @click="loadAlerts" :loading="loading">
              <el-icon style="margin-right:4px;"><Refresh /></el-icon>刷新
            </el-button>
          </div>
        </div>

        <div class="alert-tabs">
          <el-tabs v-model="statusTab" class="no-gap">
            <el-tab-pane label="全部" name="all" />
            <el-tab-pane name="firing">
              <template #label>
                <span>告警中 <el-tag size="small" type="danger" effect="dark">{{ statusCounts.firing }}</el-tag></span>
              </template>
            </el-tab-pane>
            <el-tab-pane name="pending">
              <template #label>
                <span>等待 <el-tag size="small" type="warning" effect="dark">{{ statusCounts.pending }}</el-tag></span>
              </template>
            </el-tab-pane>
            <el-tab-pane name="resolved">
              <template #label>
                <span>已恢复 <el-tag size="small" type="success" effect="dark">{{ statusCounts.resolved }}</el-tag></span>
              </template>
            </el-tab-pane>
          </el-tabs>
        </div>
      </template>

      <el-row :gutter="10" class="alert-filters">
        <el-col :xs="24" :sm="12" :md="6">
          <el-input
            v-model="qRaw"
            size="small"
            placeholder="搜索：名称/描述/标签/指纹"
            clearable
          >
            <template #prefix><el-icon><Search /></el-icon></template>
          </el-input>
        </el-col>
        <el-col :xs="24" :sm="12" :md="5">
          <el-input
            v-model="ipRaw"
            size="small"
            placeholder="IP / 主机 / 实例（包含匹配）"
            clearable
          >
            <template #prefix><el-icon><Tickets /></el-icon></template>
          </el-input>
        </el-col>
        <el-col :xs="12" :sm="8" :md="3">
          <el-select v-model="severity" size="small" placeholder="级别" style="width:100%">
            <el-option label="全部级别" value="all" />
            <el-option label="严重 critical" value="critical" />
            <el-option label="错误 error" value="error" />
            <el-option label="警告 warning" value="warning" />
            <el-option label="信息 info" value="info" />
            <el-option label="正常 ok" value="ok" />
          </el-select>
        </el-col>
        <el-col :xs="12" :sm="8" :md="3">
          <el-select v-model="source" size="small" placeholder="来源" style="width:100%">
            <el-option label="全部来源" value="all" />
            <el-option label="接入" value="ingress" />
            <el-option label="规则" value="rule" />
          </el-select>
        </el-col>
        <el-col :xs="12" :sm="8" :md="3">
          <el-select v-model="acked" size="small" placeholder="接手" style="width:100%">
            <el-option label="全部" value="all" />
            <el-option label="未接手" value="unacked" />
            <el-option label="已接手" value="acked" />
          </el-select>
        </el-col>
        <el-col :xs="24" :sm="24" :md="4">
          <el-select v-model="store" size="small" placeholder="存储" style="width:100%">
            <el-option label="列表（默认）" value="default" />
            <el-option label="最近事件 MySQL" value="recent" />
            <el-option label="历史事件 ES" value="history" />
          </el-select>
        </el-col>
      </el-row>
    </el-card>

    <!-- ===== 批量操作条 ===== -->
    <el-affix v-if="auth.can('alerts:write') && selection.size > 0" :offset-top="0" class="batch-affix">
      <div class="batch-bar panel" style="padding:10px 16px;">
        <div style="display:flex;align-items:center;justify-content:space-between;flex-wrap:wrap;gap:8px;">
          <div style="display:flex;align-items:center;gap:10px;flex-wrap:wrap;">
            <el-tag type="primary" effect="plain">已选 {{ selection.size }} 条{{ selection.size > batchLimit ? `（仅前 ${batchLimit} 条参与批量）` : '' }}</el-tag>
            <el-button size="small" type="warning" @click="openBatchAck">
              <el-icon style="margin-right:4px;"><Check /></el-icon>批量接手
            </el-button>
            <el-button size="small" type="danger" @click="openBatchClose">
              <el-icon style="margin-right:4px;"><CircleClose /></el-icon>批量关闭
            </el-button>
            <el-button size="small" @click="cancelSelection">取消选择</el-button>
          </div>
        </div>
      </div>
    </el-affix>

    <!-- ===== 列表区 ===== -->
    <el-card class="alert-list-wrap panel" shadow="never" style="padding-top:8px;">
      <!-- 空态 -->
      <template v-if="!loading && alerts.length === 0">
        <el-empty description="暂无告警事件，先去接入告警数据源吧～">
          <template #image>
            <el-icon :size="56" color="var(--muted)"><WarningFilled /></el-icon>
          </template>
          <el-button type="primary" @click="router.push('/ingress')">去告警接入</el-button>
        </el-empty>
      </template>

      <!-- Cards 视图 -->
      <template v-else-if="viewMode === 'cards'">
        <div class="cards-topbar" style="display:flex;align-items:center;justify-content:space-between;padding:4px 2px 10px;">
          <el-checkbox :model-value="pagedAlerts.length > 0 && pagedAlerts.every(isSelected)" @change="togglePageAll">
            本页全选（{{ pagedAlerts.filter(isSelected).length }}/{{ pagedAlerts.length }}）
          </el-checkbox>
          <span style="color:var(--muted);font-size:12px;">右键卡片查看操作菜单</span>
        </div>
        <div class="alert-cards-grid">
          <div
            v-for="a in pagedAlerts"
            :key="(a as any).id || (a as any).fingerprint"
            class="alert-card"
            :class="{ selected: isSelected(a) }"
            @click="openDetail(a)"
            @contextmenu.prevent="openCtx($event, a)"
          >
            <div class="alert-card-rail" :style="{ background: railColor(a) }"></div>
            <div class="alert-card-body">
              <div class="alert-card-head">
                <div class="alert-card-head-left">
                  <el-checkbox :model-value="isSelected(a)" @click.stop="toggleSelect(a, $event)" style="margin-right:8px;" />
                  <el-tag size="small" :type="statusMeta((a as any).status).type" effect="light" style="margin-right:6px;">
                    {{ statusMeta((a as any).status).text }}
                  </el-tag>
                  <el-tag size="small" :type="severityMeta((a as any).severity).type" effect="dark">
                    {{ severityMeta((a as any).severity).text }}
                  </el-tag>
                </div>
                <div class="alert-card-head-right">
                  <el-badge
                    v-if="matchedMWs(a).length > 0"
                    :value="matchedMWs(a).length"
                    type="info"
                    class="mw-badge"
                  >
                    <el-popover placement="top" :width="260" trigger="hover">
                      <template #reference>
                        <el-tag size="small" type="info" effect="plain">维护中</el-tag>
                      </template>
                      <div style="font-size:12px;">
                        <div style="font-weight:600;margin-bottom:6px;">命中维护窗：</div>
                        <div v-for="m in matchedMWs(a)" :key="(m as any).id || (m as any).name" style="line-height:1.6;">
                          · {{ (m as any).name }}
                        </div>
                      </div>
                    </el-popover>
                  </el-badge>
                  <el-tag v-if="isEscalated(a)" size="small" type="danger" effect="plain" style="margin-left:4px;">升级</el-tag>
                  <el-tag v-if="isAcked(a)" size="small" effect="plain" style="margin-left:4px;">
                    接手·{{ (a as any).acknowledged_by || '-' }}
                  </el-tag>
                  <el-tag v-if="(a as any).count || (a as any).trigger_count" size="small" effect="plain" style="margin-left:4px;">
                    ×{{ (a as any).count || (a as any).trigger_count }}
                  </el-tag>
                </div>
              </div>

              <div class="alert-name">{{ alertName(a) }}</div>
              <div class="alert-summary">{{ alertSummary(a) }}</div>

              <div class="ac-row alert-meta-row">
                <span class="meta-cell"><el-icon :size="12"><Tickets /></el-icon> IP：{{ alertIp(a) || '-' }}</span>
                <span class="meta-cell">主机：{{ alertHost(a) || '-' }}</span>
                <span class="meta-cell">实例：{{ (a as any).labels?.instance || '-' }}</span>
                <span class="meta-cell">来源：{{ sourceText(a) }}</span>
              </div>
              <div class="ac-row alert-meta-row">
                <span class="meta-cell">开始：{{ fmtTime((a as any).starts_at || (a as any).created_at) }}</span>
                <span class="meta-cell">结束：{{ fmtTime((a as any).ends_at) }}</span>
                <span class="meta-cell">组：{{ (a as any).alert_group || (a as any).alertGroup || '-' }}</span>
                <span class="meta-cell">键：{{ (a as any).alert_key || (a as any).alertKey || '-' }}</span>
                <span class="meta-cell">末次评估：{{ fmtTime((a as any).evaluated_at || (a as any).last_eval_at || (a as any).updated_at) }}</span>
              </div>

              <div v-if="(a as any).note || (a as any).comment" class="ac-row" style="margin-top:6px;">
                <el-tag size="small" type="warning" effect="plain">备注：{{ ((a as any).note || (a as any).comment || '').slice(0, 80) }}</el-tag>
              </div>

              <div v-if="extractContacts(a).length > 0" class="ac-row contacts-row">
                <el-popover
                  v-for="(c, i) in extractContacts(a)"
                  :key="`${c.role}-${c.name}-${i}`"
                  placement="top"
                  trigger="hover"
                  :width="220"
                >
                  <template #reference>
                    <el-tag class="contact-chip" size="small" type="primary" effect="light">
                      {{ c.role }}：{{ c.name }}<template v-if="c.phone"> · {{ c.phone }}</template>
                    </el-tag>
                  </template>
                  <div style="font-size:12px;line-height:1.7;">
                    <div><b>角色：</b>{{ c.role }}</div>
                    <div><b>姓名：</b>{{ c.name }}</div>
                    <div v-if="c.phone"><b>电话：</b><a :href="`tel:${c.phone}`">{{ c.phone }}</a></div>
                    <div v-if="c.email"><b>邮箱：</b><a :href="`mailto:${c.email}`">{{ c.email }}</a></div>
                  </div>
                </el-popover>
              </div>
            </div>
          </div>
        </div>
      </template>

      <!-- Table 视图 -->
      <template v-else>
        <el-table
          ref="tableRef"
          :data="pagedAlerts"
          @selection-change="onSelectionChange"
          size="small"
          stripe
          @row-click="(_, __, e) => { if (!e) return; }"
          @row-contextmenu="(row, e) => openCtx(e as unknown as MouseEvent, row as any)"
          style="width:100%;"
          empty-text="暂无数据"
        >
          <el-table-column type="selection" width="44" />
          <el-table-column label="状态" width="90" fixed="left">
            <template #default="{ row }">
              <el-tag :type="statusMeta((row as any).status).type" effect="light" size="small">
                {{ statusMeta((row as any).status).text }}
              </el-tag>
            </template>
          </el-table-column>
          <el-table-column label="级别" width="90">
            <template #default="{ row }">
              <el-tag :type="severityMeta((row as any).severity).type" effect="dark" size="small">
                {{ severityMeta((row as any).severity).text }}
              </el-tag>
            </template>
          </el-table-column>
          <el-table-column label="名称 + 实例" min-width="280">
            <template #default="{ row }">
              <div style="font-weight:600;color:var(--heading);cursor:pointer;" @click.stop="openDetail(row as any)">
                {{ alertName(row as any) }}
              </div>
              <div style="color:var(--muted);font-size:12px;margin-top:2px;">
                {{ (row as any).labels?.instance || '-' }}
              </div>
              <div v-if="(row as any).labels && Object.keys((row as any).labels).length" style="margin-top:4px;display:flex;flex-wrap:wrap;gap:4px;">
                <el-tag
                  v-for="(v, k) in (row as any).labels"
                  :key="String(k)"
                  size="small"
                  effect="plain"
                  style="max-width:200px;overflow:hidden;text-overflow:ellipsis;"
                >
                  {{ k }}={{ String(v) }}
                </el-tag>
              </div>
            </template>
          </el-table-column>
          <el-table-column label="IP" min-width="130">
            <template #default="{ row }">{{ alertIp(row as any) || '-' }}</template>
          </el-table-column>
          <el-table-column label="主机" min-width="140">
            <template #default="{ row }">{{ alertHost(row as any) || '-' }}</template>
          </el-table-column>
          <el-table-column label="描述" min-width="260" show-overflow-tooltip>
            <template #default="{ row }">{{ alertSummary(row as any) }}</template>
          </el-table-column>
          <el-table-column label="联系人" min-width="180">
            <template #default="{ row }">
              <span v-if="extractContacts(row as any).length === 0" style="color:var(--muted);">-</span>
              <template v-else>
                <el-tag
                  v-for="(c, i) in extractContacts(row as any).slice(0, 2)"
                  :key="i"
                  size="small"
                  effect="light"
                  type="primary"
                  style="margin-right:4px;"
                >
                  {{ c.name }}
                </el-tag>
                <el-tag v-if="extractContacts(row as any).length > 2" size="small" effect="plain">
                  +{{ extractContacts(row as any).length - 2 }}
                </el-tag>
              </template>
            </template>
          </el-table-column>
          <el-table-column label="来源" min-width="130">
            <template #default="{ row }">{{ sourceText(row as any) }}</template>
          </el-table-column>
          <el-table-column label="持续" width="90">
            <template #default="{ row }">
              {{ fmtDuration((row as any).starts_at || (row as any).created_at, (row as any).status === 'resolved' ? ((row as any).ends_at || (row as any).updated_at) : undefined) }}
            </template>
          </el-table-column>
          <el-table-column label="开始" width="160">
            <template #default="{ row }">{{ fmtTime((row as any).starts_at || (row as any).created_at) }}</template>
          </el-table-column>
          <el-table-column label="末次" width="160">
            <template #default="{ row }">{{ fmtTime((row as any).evaluated_at || (row as any).last_eval_at || (row as any).updated_at) }}</template>
          </el-table-column>
          <el-table-column label="组" min-width="140" show-overflow-tooltip>
            <template #default="{ row }">{{ (row as any).alert_group || (row as any).alertGroup || '-' }}</template>
          </el-table-column>
          <el-table-column label="键" min-width="140" show-overflow-tooltip>
            <template #default="{ row }">{{ (row as any).alert_key || (row as any).alertKey || '-' }}</template>
          </el-table-column>
          <el-table-column label="评估" width="100">
            <template #default="{ row }">
              <el-tag size="small" effect="plain" :type="(row as any).last_eval_at || (row as any).evaluated_at ? 'success' : 'info'">
                {{ (row as any).last_eval_at || (row as any).evaluated_at ? '已评估' : '未知' }}
              </el-tag>
            </template>
          </el-table-column>
          <el-table-column label="接手" width="110">
            <template #default="{ row }">
              <el-tag v-if="isAcked(row as any)" size="small" effect="plain">
                {{ (row as any).acknowledged_by || '已接手' }}
              </el-tag>
              <span v-else style="color:var(--muted);">未接手</span>
            </template>
          </el-table-column>
          <el-table-column label="维护" width="90" align="center">
            <template #default="{ row }">
              <el-tag v-if="matchedMWs(row as any).length" size="small" type="info" effect="plain">
                维护·{{ matchedMWs(row as any).length }}
              </el-tag>
              <span v-else>-</span>
            </template>
          </el-table-column>
          <el-table-column label="备注" min-width="140" show-overflow-tooltip>
            <template #default="{ row }">{{ (row as any).note || (row as any).comment || '-' }}</template>
          </el-table-column>
          <el-table-column label="次数" width="80" align="center">
            <template #default="{ row }">
              <el-tag size="small" effect="plain">{{ (row as any).count || (row as any).trigger_count || 1 }}</el-tag>
            </template>
          </el-table-column>
          <el-table-column label="详情" width="80" align="center" fixed="right">
            <template #default="{ row }">
              <el-button size="small" type="primary" link @click.stop="openDetail(row as any)">
                <el-icon><View /></el-icon>详情
              </el-button>
            </template>
          </el-table-column>
        </el-table>
      </template>

      <!-- 分页 -->
      <div v-if="alerts.length > 0" class="alert-pagination">
        <el-pagination
          v-model:current-page="pageNum"
          background
          small
          :page-size="pageSize"
          :total="total"
          :show-size-changer="false"
          :show-jumper="false"
          layout="prev, pager, next, total"
        />
      </div>
    </el-card>

    <!-- ===== 模拟右键菜单：固定位置 dropdown ===== -->
    <teleport to="body">
      <div
        v-if="ctx.visible && ctx.alert"
        class="ctx-menu"
        :style="{ left: ctx.x + 'px', top: ctx.y + 'px' }"
        @click.stop
      >
        <el-dropdown v-model:visible="ctx.visible" trigger="click" @visible-change="(v) => { if (!v) closeCtx() }">
          <template #dropdown>
            <el-dropdown-menu>
              <el-dropdown-item @click.stop="ctxAction('view')">
                <el-icon style="margin-right:6px;"><View /></el-icon>查看详情
              </el-dropdown-item>
              <el-dropdown-item
                :disabled="!auth.can('alerts:write')"
                @click.stop="ctxAction('ack')"
              >
                <el-icon style="margin-right:6px;"><Check /></el-icon>
                {{ isAcked(ctx.alert!) ? '取消接手' : '接手告警' }}
              </el-dropdown-item>
              <el-dropdown-item
                :disabled="!auth.can('alerts:write') || isResolved(ctx.alert!)"
                @click.stop="ctxAction('close')"
              >
                <el-icon style="margin-right:6px;"><CircleClose /></el-icon>关闭告警
              </el-dropdown-item>
              <el-dropdown-item disabled>
                <el-popover placement="right" trigger="hover" :width="160">
                  <template #reference>
                    <span style="display:inline-flex;align-items:center;">
                      <el-icon style="margin-right:6px;"><Switch /></el-icon>据此静默
                    </span>
                  </template>
                  <span style="font-size:12px;">批次 2 实现</span>
                </el-popover>
              </el-dropdown-item>
              <el-dropdown-item disabled>
                <el-popover placement="right" trigger="hover" :width="160">
                  <template #reference>
                    <span style="display:inline-flex;align-items:center;">
                      <el-icon style="margin-right:6px;"><Setting /></el-icon>据此开维护
                    </span>
                  </template>
                  <span style="font-size:12px;">批次 2 实现</span>
                </el-popover>
              </el-dropdown-item>
              <el-dropdown-item divided @click.stop="ctxAction('copy_desc')">
                <el-icon style="margin-right:6px;"><CopyDocument /></el-icon>复制描述
              </el-dropdown-item>
              <el-dropdown-item @click.stop="ctxAction('copy_ip')">复制 IP</el-dropdown-item>
              <el-dropdown-item @click.stop="ctxAction('copy_fp')">复制指纹</el-dropdown-item>
              <el-dropdown-item divided @click.stop="ctxAction('filter_name')">按名称筛选</el-dropdown-item>
              <el-dropdown-item @click.stop="ctxAction('filter_ip')">按 IP 筛选</el-dropdown-item>
            </el-dropdown-menu>
          </template>
          <span style="display:none;" />
        </el-dropdown>
      </div>
    </teleport>

    <!-- ===== 详情 Drawer ===== -->
    <el-drawer
      v-model="drawerVisible"
      :with-header="false"
      size="780px"
      direction="rtl"
      destroy-on-close
      class="alert-drawer"
    >
      <template v-if="detailAlert">
        <div class="drawer-header">
          <div class="drawer-title">
            <el-tag :type="statusMeta((detailAlert as any).status).type" effect="dark" style="margin-right:6px;">
              {{ statusMeta((detailAlert as any).status).text }}
            </el-tag>
            <el-tag :type="severityMeta((detailAlert as any).severity).type" effect="dark" style="margin-right:6px;">
              {{ severityMeta((detailAlert as any).severity).text }}
            </el-tag>
            <span style="font-size:16px;font-weight:700;color:var(--heading);">{{ alertName(detailAlert) }}</span>
          </div>
          <div style="display:flex;align-items:center;gap:6px;flex-wrap:wrap;margin-top:6px;">
            <el-badge v-if="isAcked(detailAlert)" :is-dot="false" value="接手" type="warning">
              <el-tag size="small" effect="plain">{{ (detailAlert as any).acknowledged_by || '-' }}</el-tag>
            </el-badge>
            <el-tag v-if="matchedMWs(detailAlert).length > 0" size="small" type="info" effect="plain">
              维护中 · {{ mwBadgeText(detailAlert) }}
            </el-tag>
            <el-tag v-if="isEscalated(detailAlert)" size="small" type="danger" effect="plain">已升级</el-tag>
            <el-tag size="small" effect="plain">来源：{{ sourceText(detailAlert) }}</el-tag>
            <el-tag size="small" effect="plain">IP：{{ alertIp(detailAlert) || '-' }}</el-tag>
          </div>
        </div>

        <el-tabs v-model="detailTab" class="drawer-tabs">
          <!-- 概览 -->
          <el-tab-pane label="概览" name="overview">
            <div v-loading="drawerLoading">
              <el-descriptions :column="2" border size="small">
                <el-descriptions-item label="告警 ID">{{ (detailAlert as any).id || '-' }}</el-descriptions-item>
                <el-descriptions-item label="规则 ID">{{ (detailAlert as any).rule_id || '-' }}</el-descriptions-item>
                <el-descriptions-item label="指纹" :span="2">
                  <span style="font-family:monospace;">{{ (detailAlert as any).fingerprint || '-' }}</span>
                  <el-button size="small" link style="margin-left:8px;" @click="copyText((detailAlert as any).fingerprint || '-', '指纹')">
                    <el-icon><CopyDocument /></el-icon>复制
                  </el-button>
                </el-descriptions-item>
                <el-descriptions-item label="状态">
                  <el-tag :type="statusMeta((detailAlert as any).status).type">{{ statusMeta((detailAlert as any).status).text }}</el-tag>
                </el-descriptions-item>
                <el-descriptions-item label="严重度">
                  <el-tag :type="severityMeta((detailAlert as any).severity).type" effect="dark">{{ severityMeta((detailAlert as any).severity).text }}</el-tag>
                </el-descriptions-item>
                <el-descriptions-item label="开始时间">{{ fmtTime((detailAlert as any).starts_at || (detailAlert as any).created_at) }}</el-descriptions-item>
                <el-descriptions-item label="结束时间">{{ fmtTime((detailAlert as any).ends_at) }}</el-descriptions-item>
                <el-descriptions-item label="最后评估">{{ fmtTime((detailAlert as any).evaluated_at || (detailAlert as any).last_eval_at || (detailAlert as any).updated_at) }}</el-descriptions-item>
                <el-descriptions-item label="已创建">{{ fmtTime((detailAlert as any).created_at) }}</el-descriptions-item>
                <el-descriptions-item label="持续时长">
                  {{ fmtDuration((detailAlert as any).starts_at || (detailAlert as any).created_at, (detailAlert as any).status === 'resolved' ? ((detailAlert as any).ends_at || (detailAlert as any).updated_at) : undefined) }}
                </el-descriptions-item>
                <el-descriptions-item label="触发次数">{{ (detailAlert as any).count || (detailAlert as any).trigger_count || 1 }}</el-descriptions-item>
                <el-descriptions-item label="来源接入" :span="2">
                  {{ (detailAlert as any).ingress_name || '-' }}
                  <span v-if="(detailAlert as any).ingress_kind" style="color:var(--muted);margin-left:6px;">({{ (detailAlert as any).ingress_kind }})</span>
                </el-descriptions-item>
                <el-descriptions-item label="AlertGroup">{{ (detailAlert as any).alert_group || (detailAlert as any).alertGroup || '-' }}</el-descriptions-item>
                <el-descriptions-item label="AlertKey">{{ (detailAlert as any).alert_key || (detailAlert as any).alertKey || '-' }}</el-descriptions-item>
                <el-descriptions-item label="接手人">
                  <template v-if="isAcked(detailAlert)">
                    {{ (detailAlert as any).acknowledged_by || '-' }}
                    <span style="color:var(--muted);margin-left:6px;">{{ fmtTime((detailAlert as any).acknowledged_at) }}</span>
                  </template>
                  <span v-else style="color:var(--muted);">未接手</span>
                </el-descriptions-item>
                <el-descriptions-item label="接手备注">{{ (detailAlert as any).ack_note || '-' }}</el-descriptions-item>
                <el-descriptions-item label="升级时间">{{ fmtTime((detailAlert as any).escalated_at) }}</el-descriptions-item>
                <el-descriptions-item label="维护窗命中">
                  <template v-if="matchedMWs(detailAlert).length === 0">-</template>
                  <template v-else>
                    <el-tag
                      v-for="m in matchedMWs(detailAlert)"
                      :key="(m as any).id || (m as any).name"
                      size="small"
                      type="info"
                      effect="plain"
                      style="margin-right:4px;"
                    >
                      {{ (m as any).name }}
                    </el-tag>
                  </template>
                </el-descriptions-item>
                <el-descriptions-item label="备注">{{ (detailAlert as any).note || (detailAlert as any).comment || '-' }}</el-descriptions-item>
                <el-descriptions-item label="IP / 主机 / 实例" :span="2">
                  <div>IP：{{ alertIp(detailAlert) || '-' }}</div>
                  <div>主机：{{ alertHost(detailAlert) || '-' }}</div>
                  <div>实例：{{ (detailAlert as any).labels?.instance || '-' }}</div>
                </el-descriptions-item>
              </el-descriptions>

              <el-divider content-position="left">联系人</el-divider>
              <div v-if="extractContacts(detailAlert).length === 0" style="color:var(--muted);padding:6px 2px;">无联系人信息</div>
              <el-row v-else :gutter="10">
                <el-col v-for="(c, i) in extractContacts(detailAlert)" :key="i" :xs="24" :sm="12" :md="8">
                  <el-card shadow="never" size="small" style="margin-bottom:8px;">
                    <div style="font-weight:600;">{{ c.role }}：{{ c.name }}</div>
                    <div style="color:var(--muted);font-size:12px;margin-top:4px;">
                      <div v-if="c.phone">电话：<a :href="`tel:${c.phone}`">{{ c.phone }}</a></div>
                      <div v-if="c.email">邮箱：<a :href="`mailto:${c.email}`">{{ c.email }}</a></div>
                    </div>
                  </el-card>
                </el-col>
              </el-row>

              <el-divider content-position="left">告警描述</el-divider>
              <div style="white-space:pre-wrap;padding:10px 12px;background:var(--overlay-soft);border-radius:8px;border:1px solid var(--line);">
                {{ (detailAlert as any).summary || (detailAlert as any).description || (detailAlert as any).annotations?.summary || (detailAlert as any).annotations?.description || '(无描述)' }}
              </div>

              <el-divider content-position="left">Annotations（剔除 SNMP varbinds 字段）</el-divider>
              <el-table
                v-if="annotationsFiltered(detailAlert).length > 0"
                :data="annotationsFiltered(detailAlert)"
                size="small"
                stripe
              >
                <el-table-column prop="k" label="Key" width="220" />
                <el-table-column prop="v" label="Value" show-overflow-tooltip />
              </el-table>
              <div v-else style="color:var(--muted);">(无 annotations)</div>

              <el-divider content-position="left">SNMP Varbinds</el-divider>
              <el-table
                v-if="snmpVarbinds(detailAlert).length > 0"
                :data="snmpVarbinds(detailAlert)"
                size="small"
                stripe
              >
                <el-table-column prop="idx" label="#" width="60" />
                <el-table-column prop="oid" label="OID" min-width="260" show-overflow-tooltip />
                <el-table-column prop="val" label="Value" min-width="260" show-overflow-tooltip />
              </el-table>
              <div v-else style="color:var(--muted);">(非 trap 告警，无 varbinds)</div>

              <el-divider content-position="left">Labels</el-divider>
              <el-table
                v-if="(detailAlert as any).labels && Object.keys((detailAlert as any).labels).length"
                :data="Object.entries((detailAlert as any).labels).map(([k, v]) => ({ k, v: String(v) }))"
                size="small"
                stripe
              >
                <el-table-column prop="k" label="Key" width="220" />
                <el-table-column prop="v" label="Value" show-overflow-tooltip />
              </el-table>
              <div v-else style="color:var(--muted);">(无 labels)</div>
            </div>
          </el-tab-pane>

          <!-- 通知记录 -->
          <el-tab-pane label="通知记录" name="notifies">
            <el-table :data="detailNotifies" size="small" stripe empty-text="暂无通知记录">
              <el-table-column label="时间" width="170">
                <template #default="{ row }">{{ fmtTime((row as any).sent_at || (row as any).created_at) }}</template>
              </el-table-column>
              <el-table-column label="渠道" min-width="140">
                <template #default="{ row }">
                  <span style="font-weight:600;">{{ (row as any).channel_name || '-' }}</span>
                  <span v-if="(row as any).channel_type" style="color:var(--muted);margin-left:6px;">({{ (row as any).channel_type }})</span>
                </template>
              </el-table-column>
              <el-table-column label="目标" min-width="200" show-overflow-tooltip>
                <template #default="{ row }">{{ (row as any).target || '-' }}</template>
              </el-table-column>
              <el-table-column label="阶段" width="120">
                <template #default="{ row }">
                  <el-tag size="small" effect="plain">{{ (row as any).transition || '-' }}</el-tag>
                </template>
              </el-table-column>
              <el-table-column label="状态" width="100">
                <template #default="{ row }">
                  <el-tag
                    size="small"
                    :type="(row as any).status === 'success' ? 'success' : (row as any).status === 'failed' ? 'danger' : 'info'"
                  >
                    {{ (row as any).status || '-' }}
                  </el-tag>
                </template>
              </el-table-column>
              <el-table-column label="详情" width="80" align="center">
                <template #default="{ row }">
                  <el-button size="small" link @click="notifyViewLog = row as any; notifyViewVisible = true">查看</el-button>
                </template>
              </el-table-column>
            </el-table>
          </el-tab-pane>

          <!-- 历史占位 -->
          <el-tab-pane label="历史" name="history">
            <div style="padding:32px 8px;text-align:center;color:var(--muted);">
              <el-icon :size="32" color="var(--muted)" style="margin-bottom:8px;"><DataAnalysis /></el-icon>
              <div>告警历史独立 ES 查询：批次 3 接入 timeline。</div>
            </div>
          </el-tab-pane>
        </el-tabs>

        <!-- Drawer 底部按钮 -->
        <div class="drawer-footer">
          <div style="display:flex;align-items:center;gap:8px;flex-wrap:wrap;">
            <template v-if="!isResolved(detailAlert) && auth.can('alerts:write')">
              <el-button v-if="!isAcked(detailAlert)" type="warning" @click="async () => { const ok = await doAck((detailAlert as any).id); if (ok) { closeCtx(); await openDetail((detailAlert as any).id); await loadAlerts(); } }">
                <el-icon style="margin-right:4px;"><Check /></el-icon>接手告警
              </el-button>
              <el-button v-else type="info" @click="async () => { const ok = await doUnack((detailAlert as any).id); if (ok) { await openDetail((detailAlert as any).id); await loadAlerts(); } }">
                取消接手
              </el-button>
              <el-button
                type="danger"
                @click="async () => {
                  try {
                    const { value } = await ElMessageBox.prompt('请输入关闭原因', '关闭告警', {
                      confirmButtonText: '确认', cancelButtonText: '取消',
                      inputValidator: (v: string) => (v && v.length >= 2 ? true : '长度至少 2 字符'),
                    })
                    const ok = await doClose((detailAlert as any).id, value)
                    if (ok) { await openDetail((detailAlert as any).id); await loadAlerts() }
                  } catch { /* noop */ }
                }"
              >
                <el-icon style="margin-right:4px;"><CircleClose /></el-icon>关闭告警
              </el-button>
            </template>
            <el-tooltip content="批次 2 实现" placement="top" :show-after="200">
              <el-button disabled>
                <el-icon style="margin-right:4px;"><Switch /></el-icon>据此静默
              </el-button>
            </el-tooltip>
            <el-tooltip content="批次 2 实现" placement="top" :show-after="200">
              <el-button disabled>
                <el-icon style="margin-right:4px;"><Setting /></el-icon>据此开维护
              </el-button>
            </el-tooltip>
            <el-button @click="drawerVisible = false">关闭</el-button>
          </div>
        </div>
      </template>
    </el-drawer>

    <!-- ===== 通知正文 dialog ===== -->
    <el-dialog
      v-model="notifyViewVisible"
      title="通知内容"
      width="720px"
      destroy-on-close
    >
      <div v-if="notifyViewLog">
        <el-descriptions :column="1" size="small" border>
          <el-descriptions-item label="状态">
            <el-tag :type="(notifyViewLog as any).status === 'success' ? 'success' : (notifyViewLog as any).status === 'failed' ? 'danger' : 'info'">
              {{ (notifyViewLog as any).status || '-' }}
            </el-tag>
          </el-descriptions-item>
          <el-descriptions-item label="阶段">{{ (notifyViewLog as any).transition || '-' }}</el-descriptions-item>
          <el-descriptions-item label="渠道">{{ (notifyViewLog as any).channel_name || '-' }} ({{ (notifyViewLog as any).channel_type || '-' }})</el-descriptions-item>
          <el-descriptions-item label="目标">{{ (notifyViewLog as any).target || '-' }}</el-descriptions-item>
        </el-descriptions>
        <el-divider>Body</el-divider>
        <pre class="code-block">{{ (notifyViewLog as any).body || '(空)' }}</pre>
        <el-divider>Error</el-divider>
        <pre class="code-block error">{{ (notifyViewLog as any).error || '(无错误)' }}</pre>
      </div>
    </el-dialog>

    <!-- ===== 批量接手 dialog ===== -->
    <el-dialog v-model="batchAckVisible" title="批量接手告警" width="520px" destroy-on-close>
      <el-form
        ref="batchAckFormRef"
        :model="batchForm"
        label-width="80px"
        size="default"
      >
        <el-form-item label="已选">
          <el-tag type="primary" effect="plain">
            {{ batchIds.length }} 条{{ selection.size > batchLimit ? `（总数 ${selection.size}，超上限仅取前 ${batchLimit}）` : '' }}
          </el-tag>
        </el-form-item>
        <el-form-item label="接手备注" prop="note">
          <el-input v-model="batchForm.note" type="textarea" :rows="3" maxlength="120" show-word-limit placeholder="选填，最多 120 字" />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="batchAckVisible = false">取消</el-button>
        <el-button type="warning" @click="confirmBatchAck">确认接手</el-button>
      </template>
    </el-dialog>

    <!-- ===== 批量关闭 dialog ===== -->
    <el-dialog v-model="batchCloseVisible" title="批量关闭告警" width="520px" destroy-on-close>
      <el-form
        ref="batchCloseFormRef"
        :model="batchForm"
        :rules="batchFormRules"
        label-width="80px"
        size="default"
      >
        <el-form-item label="已选">
          <el-tag type="danger" effect="plain">
            {{ batchIds.length }} 条{{ selection.size > batchLimit ? `（总数 ${selection.size}，超上限仅取前 ${batchLimit}）` : '' }}
          </el-tag>
        </el-form-item>
        <el-form-item label="关闭原因" prop="reason">
          <el-input v-model="batchForm.reason" type="textarea" :rows="4" maxlength="120" show-word-limit placeholder="必填，2-120 字" />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="batchCloseVisible = false">取消</el-button>
        <el-button type="danger" @click="confirmBatchClose">确认关闭</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<!-- =================================================================
     仅必要的自定义工具类；避免与 index.css 中已存在的
     .panel / .severity-badge / .empty 等 class 重复。
     注意：未使用 scoped，因为任务禁止 scoped 自定义 .css。
     ================================================================= -->
<style>
.alerts-page {
  display: flex;
  flex-direction: column;
  gap: 14px;
  color: var(--text);
}

/* 顶部卡片头部样式（避免复用 scoped 写法，直接用简单 class） */
.alert-toolbar :deep(.el-card__header) {
  padding: 10px 16px 0;
  border-bottom: 1px solid var(--line);
  background: var(--panel-2);
}
.alert-toolbar :deep(.el-tabs__nav-wrap::after) { height: 1px; }
.alert-toolbar .alert-tabs .no-gap { margin-top: 6px; }

.alert-filters {
  padding: 4px 0 2px;
  row-gap: 8px;
}

/* 批量操作条 */
.batch-affix {
  z-index: 80;
  margin: -2px 0 12px;
}
.batch-bar {
  background: var(--panel-surface);
  border: 1px solid var(--line);
  border-radius: 10px;
}

/* 列表卡片通用 */
.alert-list-wrap {
  padding: 14px 16px;
}
.alert-list-wrap :deep(.el-card__body) { padding: 4px 0 0; }

/* cards 视图 */
.alert-cards-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(420px, 1fr));
  gap: 12px;
}
.alert-card {
  position: relative;
  display: flex;
  background: var(--panel-surface);
  border: 1px solid var(--line);
  border-radius: 10px;
  overflow: hidden;
  cursor: pointer;
  transition: border-color 0.15s ease, box-shadow 0.15s ease;
}
.alert-card:hover {
  border-color: var(--primary-soft);
  box-shadow: 0 6px 18px -8px var(--primary-grad-to);
}
.alert-card.selected {
  border-color: var(--primary);
  box-shadow: 0 0 0 2px var(--primary-soft) inset;
}
.alert-card-rail {
  width: 5px;
  flex: 0 0 5px;
  background: var(--muted);
}
.alert-card-body {
  flex: 1;
  padding: 10px 12px 12px;
  display: flex;
  flex-direction: column;
  gap: 6px;
  min-width: 0;
}
.alert-card-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 6px;
  flex-wrap: wrap;
}
.alert-card-head-left {
  display: flex;
  align-items: center;
  min-width: 0;
}
.alert-card-head-right {
  display: flex;
  align-items: center;
  gap: 4px;
  flex-wrap: wrap;
}
.alert-name {
  font-size: 15px;
  font-weight: 700;
  color: var(--heading);
  line-height: 1.35;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.alert-summary {
  color: var(--text-secondary);
  font-size: 13px;
  line-height: 1.55;
  overflow: hidden;
  display: -webkit-box;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 3;
}
.ac-row {
  display: flex;
  flex-wrap: wrap;
  gap: 12px;
  align-items: center;
}
.alert-meta-row {
  color: var(--muted);
  font-size: 12px;
  border-top: 1px dashed var(--line);
  padding-top: 6px;
}
.meta-cell {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.contacts-row { margin-top: 4px; }
.contact-chip { cursor: help; }
.mw-badge { display: inline-block; }

/* 分页位置 */
.alert-pagination {
  display: flex;
  justify-content: flex-end;
  align-items: center;
  padding: 10px 2px 2px;
}

/* 右键菜单 */
.ctx-menu {
  position: fixed;
  z-index: 3000;
  min-width: 200px;
  background: var(--panel-surface);
  border: 1px solid var(--line);
  border-radius: 8px;
  padding: 2px;
  box-shadow: 0 8px 24px -8px rgba(0,0,0,0.25);
}
.ctx-menu .el-dropdown-menu { border: 0; padding: 2px; background: transparent; box-shadow: none; }

/* Drawer */
.alert-drawer :deep(.el-drawer__body) {
  padding: 0;
  display: flex;
  flex-direction: column;
}
.drawer-header {
  padding: 16px 20px 12px;
  border-bottom: 1px solid var(--line);
  background: var(--panel-2);
}
.drawer-title {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 4px;
}
.drawer-tabs {
  flex: 1;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}
.drawer-tabs :deep(.el-tabs__content) {
  flex: 1;
  overflow: auto;
  padding: 14px 20px 20px;
}
.drawer-footer {
  padding: 10px 20px 14px;
  border-top: 1px solid var(--line);
  background: var(--panel-2);
}

.code-block {
  background: var(--overlay-soft);
  border: 1px solid var(--line);
  padding: 10px 12px;
  border-radius: 8px;
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  font-size: 12px;
  line-height: 1.5;
  white-space: pre-wrap;
  word-break: break-word;
  max-height: 320px;
  overflow: auto;
  color: var(--text);
}
.code-block.error {
  color: var(--crit);
}
</style>
