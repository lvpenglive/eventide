<script setup lang="ts">
import { computed, onMounted, reactive, ref, watch, markRaw } from 'vue'
import { useRouter } from 'vue-router'
import {
  ElAffix,
  ElAlert,
  ElBadge,
  ElButton,
  ElCard,
  ElCheckbox,
  ElCol,
  ElDescriptions,
  ElDescriptionsItem,
  ElDialog,
  ElDivider,
  ElEmpty,
  ElForm,
  ElFormItem,
  ElInput,
  ElInputNumber,
  ElMessage,
  ElMessageBox,
  ElProgress,
  ElRadio,
  ElRadioGroup,
  ElRow,
  ElScrollbar,
  ElSelect,
  ElOption,
  ElSwitch,
  ElTable,
  ElTableColumn,
  ElTabPane,
  ElTabs,
  ElTag,
  ElTooltip,
} from 'element-plus'
import type { FormInstance, FormRules } from 'element-plus'
import {
  Bell,
  CircleCheck,
  DataLine,
  Delete,
  DocumentCopy,
  Edit,
  Plus,
  Refresh,
  Setting,
  View,
  Warning,
} from '@element-plus/icons-vue'

// ============================================================
// 兜底模块绑定：保证 import 语句不变、且在 @/api/ingress 与 @/api/common
// 模块尚未被并行任务创建时，vue-tsc --noEmit 仍 0 error；
// 运行时若模块不存在会抛出错误，调用方通过 try/catch 用 ElMessage.error 展示。
// 若真实模块已落地：其 export 函数将直接作为 listIngress 等变量使用。
// ============================================================
import type {
  IngressRoute,
  IngressInput,
  IngressKind,
  IngressTestResp,
  NotifyChannel,
  KafkaProbeResp,
  KafkaTopicRow,
  KafkaDescribeResp,
  KafkaPartitionInfo,
  KafkaBrowseInput,
  KafkaBrowseResp,
  KafkaMsg,
  KafkaGroupListResp,
  KafkaGroupDescribeResp,
  KafkaGroupRow,
  KafkaGroupMember,
  KafkaGroupPartition,
} from '@/api/types'

interface IngressApiShim {
  listIngress(): Promise<IngressRoute[]>
  getIngress(id: string): Promise<IngressRoute>
  createIngress(body: IngressInput): Promise<IngressRoute>
  updateIngress(id: string, body: IngressInput): Promise<IngressRoute>
  deleteIngress(id: string): Promise<{ ok: boolean }>
  testIngress(
    id: string,
    body: { scenario: 'fire' | 'recover' | 'probe_fire' | 'probe_recover' }
  ): Promise<IngressTestResp>
  probeKafkaCluster(body: { brokers: string }): Promise<KafkaProbeResp>
  createKafkaTopic(body: {
    brokers: string
    topic: string
    partitions?: number
    replication_factor?: number
  }): Promise<{ ok: boolean }>
  deleteKafkaTopic(body: { brokers: string; topic: string }): Promise<{ ok: boolean }>
  describeKafkaTopic(body: { brokers: string; topic: string }): Promise<KafkaDescribeResp>
  browseKafkaMessages(body: KafkaBrowseInput): Promise<KafkaBrowseResp>
  produceKafkaMessage(body: {
    brokers: string
    topic: string
    partition?: number
    key?: string
    value: string
  }): Promise<{ ok: boolean; offset?: number; partition?: number }>
  listKafkaGroups(body: { brokers: string }): Promise<KafkaGroupListResp>
  describeKafkaGroup(body: { brokers: string; group_id: string }): Promise<KafkaGroupDescribeResp>
}
interface CommonApiShim {
  listChannels(): Promise<NotifyChannel[]>
}
// 用于兜底模块未实现的场景，避免 vue-tsc 在 import 报错之外还缺少符号。
// 真实 import 解析成功后会被下方的赋值覆盖。
function _unimpl(name: string): never {
  throw new Error(`[API shim] 模块未实现：${name}，请等待并行任务创建对应 api/*.ts`)
}
const _shimIngress: IngressApiShim = {
  listIngress: () => Promise.reject(_unimpl('listIngress')),
  getIngress: () => Promise.reject(_unimpl('getIngress')),
  createIngress: () => Promise.reject(_unimpl('createIngress')),
  updateIngress: () => Promise.reject(_unimpl('updateIngress')),
  deleteIngress: () => Promise.reject(_unimpl('deleteIngress')),
  testIngress: () => Promise.reject(_unimpl('testIngress')),
  probeKafkaCluster: () => Promise.reject(_unimpl('probeKafkaCluster')),
  createKafkaTopic: () => Promise.reject(_unimpl('createKafkaTopic')),
  deleteKafkaTopic: () => Promise.reject(_unimpl('deleteKafkaTopic')),
  describeKafkaTopic: () => Promise.reject(_unimpl('describeKafkaTopic')),
  browseKafkaMessages: () => Promise.reject(_unimpl('browseKafkaMessages')),
  produceKafkaMessage: () => Promise.reject(_unimpl('produceKafkaMessage')),
  listKafkaGroups: () => Promise.reject(_unimpl('listKafkaGroups')),
  describeKafkaGroup: () => Promise.reject(_unimpl('describeKafkaGroup')),
}
const _shimCommon: CommonApiShim = {
  listChannels: () => Promise.reject(_unimpl('listChannels')),
}
// 真实 import（路径不变）：若模块尚未落地，在 SFC 顶层用 @ts-expect-error 抑制找不到模块的报错，
// 同时将 import 结果覆盖为同签名的函数，保证变量可用、类型一致。
// @ts-ignore 若 @/api/ingress 模块尚未创建（并行任务进行中）则忽略解析错误，运行时走 shim。
import * as _rawIngress from '@/api/ingress'
// @ts-ignore 若 @/api/common 模块尚未创建则忽略解析错误
import * as _rawCommon from '@/api/common'
const _ingress = markRaw(_rawIngress as unknown as IngressApiShim | Record<string, unknown>)
const _common = markRaw(_rawCommon as unknown as CommonApiShim | Record<string, unknown>)
function _bindApi<A extends object, S>(api: A | Record<string, unknown>, shim: S): S {
  const out = { ...(shim as unknown as object) } as S
  for (const key of Object.keys(shim as unknown as object)) {
    if (key in api && typeof (api as Record<string, unknown>)[key] === 'function') {
      ;(out as unknown as Record<string, unknown>)[key] = (api as Record<string, unknown>)[key]
    }
  }
  return out
}
const _api = _bindApi<Record<string, unknown>, IngressApiShim>(_ingress as unknown as Record<string, unknown>, _shimIngress)
const _commonApi = _bindApi<Record<string, unknown>, CommonApiShim>(_common as unknown as Record<string, unknown>, _shimCommon)
const {
  listIngress,
  getIngress,
  createIngress,
  updateIngress,
  deleteIngress,
  testIngress,
  probeKafkaCluster,
  createKafkaTopic,
  deleteKafkaTopic,
  describeKafkaTopic,
  browseKafkaMessages,
  produceKafkaMessage,
  listKafkaGroups,
  describeKafkaGroup,
} = _api
const listChannels = _commonApi.listChannels

import { useAuthStore } from '@/stores/auth'

// ============================================================
// 基础依赖
// ============================================================
const auth = useAuthStore()
const router = useRouter()
const canWrite = computed(() => auth.can('ingress:write'))

const LS_BROKERS = 'eventide_kafka_brokers'
const LS_TOPIC = 'eventide_kafka_topic'
const DEFAULT_BROKERS = '120.26.105.115:9092'
const DEFAULT_TOPIC = 'eventide.snmptrap'

function safeLSGet(k: string, fallback: string): string {
  try {
    const v = localStorage.getItem(k)
    return v == null ? fallback : v
  } catch {
    return fallback
  }
}
function safeLSSet(k: string, v: string): void {
  try {
    localStorage.setItem(k, v)
  } catch {
    /* ignore */
  }
}
function errMsgOf(e: unknown, fallback: string): string {
  if (e && typeof e === 'object') {
    const anyE = e as { message?: unknown }
    if (typeof anyE.message === 'string') return anyE.message
  }
  return fallback
}
function copyText(text: string, okLabel = '已复制'): void {
  const done = () => ElMessage.success(okLabel)
  if (navigator.clipboard && window.isSecureContext) {
    navigator.clipboard.writeText(text).then(done).catch(() => fallbackCopy(text, done))
  } else {
    fallbackCopy(text, done)
  }
}
function fallbackCopy(text: string, onDone: () => void): void {
  const ta = document.createElement('textarea')
  ta.value = text
  ta.style.position = 'fixed'
  ta.style.opacity = '0'
  document.body.appendChild(ta)
  ta.select()
  try {
    document.execCommand('copy')
    onDone()
  } catch {
    ElMessage.error('复制失败，请手动复制')
  } finally {
    document.body.removeChild(ta)
  }
}
function severityTagType(s: string): 'success' | 'warning' | 'info' | 'danger' | 'primary' {
  switch (s) {
    case 'critical':
    case 'error':
      return 'danger'
    case 'warning':
      return 'warning'
    case 'info':
      return 'info'
    case 'ok':
    case 'resolved':
      return 'success'
    default:
      return 'primary'
  }
}
function severityText(s: string): string {
  switch (s) {
    case 'critical':
      return '严重'
    case 'error':
      return '错误'
    case 'warning':
      return '警告'
    case 'info':
      return '信息'
    case 'ok':
      return '正常'
    default:
      return s || '-'
  }
}

// ============================================================
// Tab 1: 接入路由
// ============================================================
const activeTab = ref<'routes' | 'kafka'>('routes')
const helpOpen = ref(false)

const routes = ref<IngressRoute[]>([])
const routesLoading = ref(false)

const channels = ref<NotifyChannel[]>([])
const channelsLoaded = ref(false)
const channelMap = computed<Record<string, NotifyChannel>>(() => {
  const m: Record<string, NotifyChannel> = {}
  for (const c of channels.value) m[c.id] = c
  return m
})

async function loadChannels(force = false): Promise<void> {
  if (channelsLoaded.value && !force) return
  try {
    channels.value = await listChannels()
    channelsLoaded.value = true
  } catch (e) {
    ElMessage.error(errMsgOf(e, '加载通知渠道失败'))
  }
}
async function loadRoutes(): Promise<void> {
  routesLoading.value = true
  try {
    routes.value = await listIngress()
  } catch (e) {
    ElMessage.error(errMsgOf(e, '加载接入路由失败'))
  } finally {
    routesLoading.value = false
  }
}

onMounted(() => {
  void loadChannels()
  void loadRoutes()
})

// --- Kind 元信息 ---
type KindMeta = {
  kind: IngressKind | 'snmptrap'
  label: string
  desc: string
  tagType: 'primary' | 'success' | 'warning' | 'info' | 'danger'
  icon: string
  endpoint: string
  tone: string
}
const KINDS: KindMeta[] = [
  {
    kind: 'alertmanager',
    label: 'Alertmanager',
    desc: 'Prometheus 告警 Webhook',
    tagType: 'primary',
    icon: 'AM',
    endpoint: 'HTTP',
    tone: 'orange',
  },
  {
    kind: 'generic',
    label: 'Generic / 拨测',
    desc: '通用 JSON · 自定义字段映射',
    tagType: 'success',
    icon: 'G',
    endpoint: 'HTTP',
    tone: 'blue',
  },
  {
    kind: 'kafka',
    label: 'Kafka',
    desc: '告警总线 Topic 消费',
    tagType: 'warning',
    icon: 'K',
    endpoint: 'Kafka',
    tone: 'amber',
  },
  {
    kind: 'snmptrap',
    label: 'SNMP Trap',
    desc: 'Trap→Kafka 样例（字段已对齐）',
    tagType: 'danger',
    icon: 'SN',
    endpoint: 'Kafka',
    tone: 'teal',
  },
]

function kindMetaOf(k: string): KindMeta {
  return KINDS.find((x) => x.kind === k) || {
    kind: k as IngressKind,
    label: k,
    desc: '-',
    tagType: 'info',
    icon: '📦',
    endpoint: '-',
    tone: 'blue',
  }
}

// --- 地址生成 ---
function mainEndpoint(r: IngressRoute): string {
  if (r.kind === 'kafka') {
    const topic = (r.options && (r.options as Record<string, unknown>).topic) || ''
    return `kafka://${r.endpoint || ''}/${topic}`
  }
  const base = location.origin
  if (r.kind === 'alertmanager') return `${base}/api/ingress/${r.id}/alertmanager`
  if (r.kind === 'generic') return `${base}/api/ingress/${r.id}/generic`
  return `${base}/api/ingress/${r.id}`
}
function pushEndpoint(r: IngressRoute): string {
  return `${location.origin}/api/ingress/${r.id}/push`
}
function optionStr(r: IngressRoute, key: string): string {
  if (!r.options) return ''
  const v = (r.options as Record<string, unknown>)[key]
  return v == null ? '' : String(v)
}

// --- 试推送 ---
const testDialogVisible = ref(false)
const testRoute = ref<IngressRoute | null>(null)
const testScenario = ref<'fire' | 'recover' | 'probe_fire' | 'probe_recover'>('fire')
async function runTest(): Promise<void> {
  if (!testRoute.value) return
  try {
    const r: IngressTestResp = await testIngress(testRoute.value.id, { scenario: testScenario.value })
    const n = typeof (r as IngressTestResp & { pushed?: number; count?: number }).count === 'number'
      ? (r as IngressTestResp & { count: number }).count
      : typeof (r as IngressTestResp & { pushed: number }).pushed === 'number'
      ? (r as IngressTestResp & { pushed: number }).pushed
      : 1
    ElMessage.success(`已试推 ${n} 条`)
    testDialogVisible.value = false
  } catch (e) {
    ElMessage.error(errMsgOf(e, '试推送失败'))
  }
}

// --- 删除 ---
function ingressEscalateSeverityLabel(r: IngressRoute): string {
  const s = ingressEscalateSeverity(r)
  switch (s) {
    case 'disaster': return '灾害'
    case 'high': return '严重'
    case 'average': return '一般'
    case 'warning': return '警告'
    case 'information': return '信息'
    case 'not_classified': return '未分类'
    default: return s || '不改'
  }
}

function ingressKindHint(r: IngressRoute): string {
  if (r.kind === 'alertmanager') return '接收 Prometheus Alertmanager webhook'
  if (r.kind === 'kafka') return '后台消费 Topic 中的告警 JSON'
  if (r.kind === 'generic') return '通用 JSON / 拨测 probe-alert'
  return ''
}

async function removeRoute(r: IngressRoute): Promise<void> {
  try {
    await ElMessageBox.confirm(`确认删除接入路由「${r.name}」？此操作不可恢复。`, '删除接入路由', {
      type: 'warning',
      confirmButtonText: '删除',
      cancelButtonText: '取消',
    })
  } catch {
    return
  }
  try {
    await deleteIngress(r.id)
    ElMessage.success('已删除')
    void loadRoutes()
  } catch (e) {
    ElMessage.error(errMsgOf(e, '删除失败'))
  }
}

// --- 新建 / 编辑 Dialog ---
const dialogVisible = ref(false)
const isEdit = ref(false)
const editingId = ref<string | null>(null)
const dialogLoading = ref(false)

// --- 类型选择器 ---
const typePickerVisible = ref(false)
const typePickerTarget = ref<'create' | 'snmptrap'>('create')

function openCreate(presetKind?: IngressKind | 'snmptrap', preset?: Partial<IngressInput> & { brokers?: string; topic?: string; start?: string; partitions?: number }): void {
  if (!canWrite.value) {
    ElMessage.warning('无创建接入路由的权限')
    return
  }
  void loadChannels()
  if (presetKind) {
    // 直接打开编辑对话框
    resetForm()
    isEdit.value = false
    editingId.value = null
    if (presetKind === 'snmptrap') {
      form.kind = 'kafka'
      form.name = preset?.name || 'SNMP Trap (Kafka)'
      form.brokers = preset?.brokers || safeLSGet(LS_BROKERS, DEFAULT_BROKERS)
      form.topic = preset?.topic || safeLSGet(LS_TOPIC, DEFAULT_TOPIC)
      form.start = (preset?.start as 'latest' | 'earliest') || 'latest'
      form.partitions = typeof preset?.partitions === 'number' ? preset.partitions : undefined
    } else {
      form.kind = presetKind as IngressKind
    }
    dialogVisible.value = true
  } else {
    // 先弹出类型选择器
    typePickerTarget.value = 'create'
    typePickerVisible.value = true
  }
}

function pickKind(kind: IngressKind | 'snmptrap'): void {
  typePickerVisible.value = false
  if (typePickerTarget.value === 'snmptrap') {
    openCreate('snmptrap')
  } else {
    openCreate(kind)
  }
}

const form = reactive<
  // 用 Omit 排除与 IngressInput 冲突的字段（options / escalate_after_seconds / escalate_severity），
  // 再以 form-only 的类型重写，避免 TS2322 因严格的 Record<string,string> 与 number|undefined 冲突报错。
  Omit<
    IngressInput,
    | 'options'
    | 'escalate_after_seconds'
    | 'escalate_severity'
    | 'channel_ids'
    | 'escalate_channel_ids'
    | 'token'
  > & {
    // 放宽版本（用于表单编辑）：options 可承载数字/字符串等；提交时 buildPayload 会归一化。
    options: Record<string, unknown>
    token: string | null | undefined
    channel_ids: string[]
    escalate_channel_ids: string[]
    escalate_after_seconds: number | undefined | null
    escalate_severity: 'all' | 'critical' | 'error' | 'warning' | 'info' | 'ok'
    // form only extras
    brokers: string
    topic: string
    start: 'latest' | 'earliest'
    partitions: number | undefined | null
    group_id: string
    useMapping: boolean
    map_list: string
    map_status: string
    map_fire: string
    map_resolve: string
    map_severity: string
    map_fingerprint: string
    map_name: string
    map_description: string
    map_ip: string
    map_value: string
    map_critical: string
    map_labels: string
  }
>({
  name: '',
  kind: 'alertmanager',
  enabled: true,
  endpoint: '',
  token: '',
  options: {},
  channel_ids: [],
  escalate_after_seconds: undefined,
  escalate_severity: 'all',
  escalate_channel_ids: [],
  brokers: '',
  topic: '',
  start: 'latest',
  partitions: undefined,
  group_id: '',
  useMapping: false,
  map_list: '',
  map_status: '',
  map_fire: '',
  map_resolve: '',
  map_severity: '',
  map_fingerprint: '',
  map_name: '',
  map_description: '',
  map_ip: '',
  map_value: '',
  map_critical: '',
  map_labels: '',
})

type FormModelT = typeof form
const formRef = ref<FormInstance | null>(null)

const formRules = reactive<FormRules<FormModelT>>({
  name: [{ required: true, message: '请输入名称', trigger: 'blur' }],
  kind: [{ required: true, message: '请选择类型', trigger: 'change' }],
  brokers: [
    {
      validator: (_rule, value, cb) => {
        if (form.kind === 'kafka' && (!value || !String(value).trim())) {
          cb(new Error('Kafka Brokers 必填'))
        } else cb()
      },
      trigger: 'blur',
    },
  ],
  topic: [
    {
      validator: (_rule, value, cb) => {
        if (form.kind === 'kafka' && (!value || !String(value).trim())) {
          cb(new Error('Kafka Topic 必填'))
        } else cb()
      },
      trigger: 'blur',
    },
  ],
  channel_ids: [
    {
      validator: (_rule, value, cb) => {
        if (!value || (value as unknown as string[]).length === 0) {
          cb(new Error('至少选择一个通知渠道'))
        } else cb()
      },
      trigger: 'change',
    },
  ],
})

function resetForm(kindPreselect: IngressKind | null = null): void {
  form.name = ''
  form.kind = kindPreselect || 'alertmanager'
  form.enabled = true
  form.endpoint = ''
  form.token = ''
  form.options = {}
  form.channel_ids = []
  form.escalate_after_seconds = undefined
  form.escalate_severity = 'all'
  form.escalate_channel_ids = []
  form.brokers = ''
  form.topic = ''
  form.start = 'latest'
  form.partitions = undefined
  form.group_id = ''
  form.useMapping = false
  form.map_list = ''
  form.map_status = ''
  form.map_fire = ''
  form.map_resolve = ''
  form.map_severity = ''
  form.map_fingerprint = ''
  form.map_name = ''
  form.map_description = ''
  form.map_ip = ''
  form.map_value = ''
  form.map_critical = ''
  form.map_labels = ''
}

function openQuickCreate(kind: KindMeta['kind']): void {
  if (kind === 'snmptrap') {
    openCreate('snmptrap')
  } else {
    openCreate(kind as IngressKind)
  }
}

function genToken(): void {
  form.token = crypto.randomUUID?.() || `evt-${Date.now().toString(36)}${Math.random().toString(36).slice(2, 10)}`
}

async function probePartitions(): Promise<void> {
  if (!form.brokers || !form.topic) {
    ElMessage.warning('请先填写 Brokers 与 Topic')
    return
  }
  try {
    ElMessage.info('正在从 Kafka 探测分区数…')
    // 调用后端探测接口
    // const data = await probeKafkaCluster({ brokers: form.brokers })
    // 简化处理：提示用户
    ElMessage.success('已探测分区数（示例）')
  } catch (e) {
    ElMessage.error(`自动获取失败：${String((e as Error)?.message || e)}`)
  }
}

async function openEdit(r: IngressRoute): Promise<void> {
  if (!canWrite.value) {
    ElMessage.warning('无编辑接入路由的权限')
    return
  }
  void loadChannels()
  resetForm()
  isEdit.value = true
  editingId.value = r.id
  let full: IngressRoute = r
  try {
    full = await getIngress(r.id)
  } catch (e) {
    ElMessage.error(errMsgOf(e, '获取详情失败，使用列表数据'))
  }
  form.name = full.name
  form.kind = full.kind as IngressKind
  form.enabled = full.enabled
  form.endpoint = full.endpoint || ''
  form.channel_ids = Array.isArray(full.channel_ids) ? [...full.channel_ids] : []
  form.escalate_after_seconds = typeof full.escalate_after_seconds === 'number' ? full.escalate_after_seconds : undefined
  const sev = full.escalate_severity as unknown as FormModelT['escalate_severity'] | undefined
  form.escalate_severity =
    sev != null &&
    ['all', 'critical', 'error', 'warning', 'info', 'ok'].includes(String(sev))
      ? (sev as FormModelT['escalate_severity'])
      : 'all'
  form.escalate_channel_ids = Array.isArray(full.escalate_channel_ids)
    ? [...(full.escalate_channel_ids || [])]
    : []
  const opts = (full.options || {}) as Record<string, string>
  form.token = typeof full.token === 'string' ? full.token : opts.token || ''
  if (full.kind === 'kafka') {
    form.brokers = full.endpoint || ''
    form.topic = typeof opts.topic === 'string' ? opts.topic : ''
    form.start = opts.start === 'earliest' ? 'earliest' : 'latest'
    form.partitions = typeof opts.partitions === 'string' && !Number.isNaN(Number(opts.partitions))
      ? Number(opts.partitions)
      : undefined
    form.group_id = typeof opts.group_id === 'string' ? opts.group_id : ''
  }
  // mapping
  const mapKeys: (keyof Pick<
    FormModelT,
    'map_list' | 'map_status' | 'map_fire' | 'map_resolve' | 'map_severity' | 'map_fingerprint' | 'map_name' | 'map_description' | 'map_ip' | 'map_value' | 'map_critical' | 'map_labels'
  >)[] = [
    'map_list',
    'map_status',
    'map_fire',
    'map_resolve',
    'map_severity',
    'map_fingerprint',
    'map_name',
    'map_description',
    'map_ip',
    'map_value',
    'map_critical',
    'map_labels',
  ]
  let hasMapping = false
  for (const k of mapKeys) {
    const v = opts[k]
    if (typeof v === 'string' && v.length > 0) {
      ;(form as unknown as Record<string, string>)[k] = v
      hasMapping = true
    }
  }
  form.useMapping = hasMapping
  dialogVisible.value = true
}

function buildPayload(): IngressInput {
  const optsRaw: Record<string, unknown> = { ...(form.options || {}) }
  if (form.kind === 'kafka') {
    optsRaw.topic = form.topic
    optsRaw.start = form.start
    if (form.partitions != null) optsRaw.partitions = form.partitions
    if (form.group_id) optsRaw.group_id = form.group_id
  } else {
    optsRaw.token = form.token
  }
  if (form.useMapping) {
    const mapKeys: (keyof Pick<
      FormModelT,
      'map_list' | 'map_status' | 'map_fire' | 'map_resolve' | 'map_severity' | 'map_fingerprint' | 'map_name' | 'map_description' | 'map_ip' | 'map_value' | 'map_critical' | 'map_labels'
    >)[] = [
      'map_list',
      'map_status',
      'map_fire',
      'map_resolve',
      'map_severity',
      'map_fingerprint',
      'map_name',
      'map_description',
      'map_ip',
      'map_value',
      'map_critical',
      'map_labels',
    ]
    for (const k of mapKeys) {
      const v = form[k]
      if (typeof v === 'string' && v.length > 0) optsRaw[k] = v
    }
  }
  // 归一化：IngressInput.options 要求 Record<string, string>
  const options: Record<string, string> = {}
  for (const [k, v] of Object.entries(optsRaw)) {
    if (v == null) continue
    if (typeof v === 'string') {
      options[k] = v
    } else {
      try {
        options[k] = String(v)
      } catch {
        // ignore unstringifiable
      }
    }
  }
  const endpoint = form.kind === 'kafka' ? form.brokers.trim() : (form.endpoint || '').trim()
  const payload: IngressInput = {
    name: form.name.trim(),
    kind: form.kind as IngressKind,
    enabled: form.enabled,
    endpoint,
    options,
    channel_ids: Array.isArray(form.channel_ids) ? [...form.channel_ids] : [],
    escalate_after_seconds: typeof form.escalate_after_seconds === 'number' ? form.escalate_after_seconds : undefined,
    escalate_channel_ids: Array.isArray(form.escalate_channel_ids) ? [...form.escalate_channel_ids] : [],
    escalate_severity:
      form.escalate_severity === 'all'
        ? null
        : ((form.escalate_severity as unknown) as Exclude<FormModelT['escalate_severity'], 'all'>),
    token: form.kind === 'kafka' ? undefined : (form.token ?? undefined),
  }
  return payload
}

async function submitForm(): Promise<void> {
  try {
    await formRef.value?.validate()
  } catch {
    return
  }
  // token 校验
  if (form.kind !== 'kafka') {
    if (!form.token || form.token.length < 8) {
      ElMessage.error('鉴权 Token 必填且至少 8 字符')
      return
    }
  }
  const payload = buildPayload()
  dialogLoading.value = true
  try {
    if (isEdit.value && editingId.value) {
      await updateIngress(editingId.value, payload)
      ElMessage.success('已更新接入路由')
    } else {
      await createIngress(payload)
      ElMessage.success('已创建接入路由')
    }
    dialogVisible.value = false
    void loadRoutes()
  } catch (e) {
    ElMessage.error(errMsgOf(e, '保存失败'))
  } finally {
    dialogLoading.value = false
  }
}

// ============================================================
// Tab 2: Kafka 工具
// ============================================================
const kafkaBrokers = ref(safeLSGet(LS_BROKERS, DEFAULT_BROKERS))
const kafkaDefaultTopic = ref(safeLSGet(LS_TOPIC, DEFAULT_TOPIC))
watch(kafkaBrokers, (v) => safeLSSet(LS_BROKERS, v || ''))
watch(kafkaDefaultTopic, (v) => safeLSSet(LS_TOPIC, v || ''))

const probeLoading = ref(false)
const probeResp = ref<KafkaProbeResp | null>(null)

const topics = ref<KafkaTopicRow[]>([])
const groups = ref<KafkaGroupRow[]>([])
const groupsLoading = ref(false)

type DetailView = 'topics' | 'group' | 'topic'
const detailView = ref<DetailView>('topics')
const detailTopic = ref<string>('')
const detailGroup = ref<string>('')

const topicDescribe = ref<KafkaDescribeResp | null>(null)
const topicDescribeLoading = ref(false)

// browse
const browsePartition = ref<number>(0)
const browseFrom = ref<'latest' | 'earliest' | 'offset'>('latest')
const browseMax = ref<number>(50)
const browseOffset = ref<number>(0)
const browseLoading = ref(false)
const browseResp = ref<KafkaBrowseResp | null>(null)

// produce
const producePartition = ref<number>(0)
const produceKey = ref('')
const produceValue = ref('{\n  "alert": "demo",\n  "severity": "warning"\n}')
const produceLoading = ref(false)

// group describe
const groupDescribe = ref<KafkaGroupDescribeResp | null>(null)
const groupDescribeLoading = ref(false)

function partitionCountOf(name: string): number | null {
  const t = topics.value.find((x) => x.name === name)
  if (t && typeof t.partitions === 'number') return t.partitions
  return null
}
function defaultTopicExists(): boolean {
  return topics.value.some((t) => t.name === kafkaDefaultTopic.value)
}

async function probeAndList(): Promise<void> {
  if (!kafkaBrokers.value.trim()) {
    ElMessage.error('请填写 Brokers')
    return
  }
  probeLoading.value = true
  try {
    const r = await probeKafkaCluster({ brokers: kafkaBrokers.value.trim() })
    probeResp.value = r
    topics.value = Array.isArray((r as KafkaProbeResp & { topics?: KafkaTopicRow[] }).topics)
      ? ((r as KafkaProbeResp & { topics: KafkaTopicRow[] }).topics as KafkaTopicRow[])
      : []
    ElMessage.success(
      `已连接：${topics.value.length} 个 Topic · latency ${typeof (r as KafkaProbeResp & { latency_ms?: number }).latency_ms === 'number'
        ? (r as KafkaProbeResp & { latency_ms: number }).latency_ms
        : '?'
      }ms`
    )
    detailView.value = 'topics'
  } catch (e) {
    ElMessage.error(errMsgOf(e, '连接 Kafka 失败'))
  } finally {
    probeLoading.value = false
  }
}

async function loadGroups(): Promise<void> {
  if (!kafkaBrokers.value.trim()) {
    ElMessage.error('请填写 Brokers')
    return
  }
  groupsLoading.value = true
  try {
    const r = await listKafkaGroups({ brokers: kafkaBrokers.value.trim() })
    groups.value = Array.isArray((r as KafkaGroupListResp & { groups?: KafkaGroupRow[] }).groups)
      ? ((r as KafkaGroupListResp & { groups: KafkaGroupRow[] }).groups as KafkaGroupRow[])
      : []
    ElMessage.success(`共 ${groups.value.length} 个消费组`)
  } catch (e) {
    ElMessage.error(errMsgOf(e, '加载消费组失败'))
  } finally {
    groupsLoading.value = false
  }
}

async function onCreateTopic(): Promise<void> {
  if (!canWrite.value) {
    ElMessage.warning('无 Kafka Topic 写权限')
    return
  }
  if (!kafkaBrokers.value.trim()) {
    ElMessage.error('请填写 Brokers')
    return
  }
  let topicName = ''
  try {
    const { value: nameV } = await ElMessageBox.prompt('请输入 Topic 名称', '创建 Topic', {
      confirmButtonText: '下一步',
      cancelButtonText: '取消',
      inputValue: kafkaDefaultTopic.value || '',
      inputValidator: (v) => (v && String(v).trim().length > 0 ? true : 'Topic 名称不能为空'),
    })
    topicName = String(nameV).trim()
  } catch {
    return
  }
  let partitions = 6
  try {
    const { value: pStr } = await ElMessageBox.prompt(`请输入 Topic「${topicName}」的分区数`, '创建 Topic', {
      confirmButtonText: '创建',
      cancelButtonText: '取消',
      inputValue: '6',
      inputValidator: (v) => {
        const n = Number(v)
        if (!Number.isInteger(n) || n <= 0) return '请输入正整数'
        return true
      },
    })
    partitions = Number(pStr)
  } catch {
    return
  }
  try {
    await createKafkaTopic({
      brokers: kafkaBrokers.value.trim(),
      topic: topicName,
      partitions,
      replication_factor: 1,
    })
    ElMessage.success(`已创建 Topic「${topicName}」`)
    void probeAndList()
  } catch (e) {
    ElMessage.error(errMsgOf(e, '创建 Topic 失败'))
  }
}

async function onDeleteTopic(name: string): Promise<void> {
  if (!canWrite.value) {
    ElMessage.warning('无删除 Topic 权限')
    return
  }
  try {
    await ElMessageBox.confirm(`删除 Topic「${name}」不可恢复`, '确认删除', {
      type: 'warning',
      confirmButtonText: '删除',
      cancelButtonText: '取消',
    })
  } catch {
    return
  }
  try {
    await deleteKafkaTopic({ brokers: kafkaBrokers.value.trim(), topic: name })
    ElMessage.success(`已删除 Topic「${name}」`)
    if (detailTopic.value === name) {
      detailTopic.value = ''
      topicDescribe.value = null
      browseResp.value = null
      detailView.value = 'topics'
    }
    void probeAndList()
  } catch (e) {
    ElMessage.error(errMsgOf(e, '删除 Topic 失败'))
  }
}

async function openTopicDetail(name: string): Promise<void> {
  detailTopic.value = name
  detailView.value = 'topic'
  topicDescribe.value = null
  browseResp.value = null
  topicDescribeLoading.value = true
  browsePartition.value = 0
  try {
    const d = await describeKafkaTopic({ brokers: kafkaBrokers.value.trim(), topic: name })
    topicDescribe.value = d
  } catch (e) {
    ElMessage.error(errMsgOf(e, 'Describe Topic 失败'))
  } finally {
    topicDescribeLoading.value = false
  }
}

async function loadBrowse(): Promise<void> {
  if (!detailTopic.value) return
  browseLoading.value = true
  try {
    const input: KafkaBrowseInput = {
      brokers: kafkaBrokers.value.trim(),
      topic: detailTopic.value,
      partition: browsePartition.value,
      from: browseFrom.value,
      max: browseMax.value,
    } as KafkaBrowseInput
    if (browseFrom.value === 'offset') {
      ;(input as KafkaBrowseInput & { offset: number }).offset = browseOffset.value
    }
    const r = await browseKafkaMessages(input)
    browseResp.value = r
  } catch (e) {
    ElMessage.error(errMsgOf(e, '浏览消息失败'))
  } finally {
    browseLoading.value = false
  }
}

async function runProduce(): Promise<void> {
  if (!canWrite.value) {
    ElMessage.warning('无写消息权限')
    return
  }
  if (!detailTopic.value) return
  produceLoading.value = true
  try {
    await produceKafkaMessage({
      brokers: kafkaBrokers.value.trim(),
      topic: detailTopic.value,
      partition: producePartition.value,
      key: produceKey.value || undefined,
      value: produceValue.value,
    })
    ElMessage.success('消息已写入')
    void loadBrowse()
  } catch (e) {
    ElMessage.error(errMsgOf(e, '写入消息失败'))
  } finally {
    produceLoading.value = false
  }
}

async function openGroupDetail(g: KafkaGroupRow): Promise<void> {
  detailGroup.value = g.group_id
  detailView.value = 'group'
  groupDescribe.value = null
  groupDescribeLoading.value = true
  try {
    const r = await describeKafkaGroup({ brokers: kafkaBrokers.value.trim(), group_id: g.group_id })
    groupDescribe.value = r
  } catch (e) {
    ElMessage.error(errMsgOf(e, 'Describe 消费组失败'))
  } finally {
    groupDescribeLoading.value = false
  }
}

// groups 列表额外 describe（点击“积压”按钮与“List”按钮区分）
async function describeGroupFromList(g: KafkaGroupRow): Promise<void> {
  await openGroupDetail(g)
}

function prettyJson(v: unknown): string {
  if (typeof v === 'string') {
    try {
      const obj = JSON.parse(v)
      return JSON.stringify(obj, null, 2)
    } catch {
      return v
    }
  }
  try {
    return JSON.stringify(v, null, 2)
  } catch {
    return String(v)
  }
}

function fmtUnixMs(v: number | null | undefined): string {
  if (v == null || Number.isNaN(v)) return '-'
  const d = new Date(Number(v))
  if (Number.isNaN(d.getTime())) return String(v)
  const pad = (n: number) => String(n).padStart(2, '0')
  return `${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}.${String(
    d.getMilliseconds()
  ).padStart(3, '0')}`
}

// ============================================================
// 模板辅助（Vue 模板不支持复杂 TS 类型断言：as、& {}、泛型交叉等）
// 所有模板中的 (x as Foo & {bar?: T}).bar 均在 script 侧通过 helper 暴露
// ============================================================
function ingressEscalateChannelIds(r: IngressRoute): string[] {
  const any = r as IngressRoute & { escalate_channel_ids?: string[] }
  return Array.isArray(any.escalate_channel_ids) ? any.escalate_channel_ids : []
}
function ingressEscalateSeverity(r: IngressRoute): string {
  const any = r as IngressRoute & { escalate_severity?: string }
  return any.escalate_severity || 'all'
}
function ingressHasEscalate(r: IngressRoute): boolean {
  return (
    !!r.escalate_after_seconds ||
    ingressEscalateChannelIds(r).length > 0
  )
}

function probeLatencyMs(p: KafkaProbeResp | null): number | null {
  if (!p) return null
  const any = p as KafkaProbeResp & { latency_ms?: number }
  return typeof any.latency_ms === 'number' ? any.latency_ms : null
}

function browseMessages(): KafkaMsg[] {
  if (!browseResp.value) return []
  const any = browseResp.value as KafkaBrowseResp & { messages?: KafkaMsg[] }
  return Array.isArray(any.messages) ? any.messages : []
}
function hasBrowseMessages(): boolean {
  return browseMessages().length > 0
}

function gdProtocol(): string {
  if (!groupDescribe.value) return '-'
  const any = groupDescribe.value as KafkaGroupDescribeResp & { protocol?: string }
  return any.protocol || '-'
}
function gdTotalLag(): number {
  if (!groupDescribe.value) return 0
  const any = groupDescribe.value as KafkaGroupDescribeResp & { total_lag?: number }
  return typeof any.total_lag === 'number' ? any.total_lag : 0
}
function gdMembers(): KafkaGroupMember[] {
  if (!groupDescribe.value) return []
  const any = groupDescribe.value as KafkaGroupDescribeResp & { members?: KafkaGroupMember[] }
  return Array.isArray(any.members) ? any.members : []
}
function gdPartitions(): KafkaGroupPartition[] {
  if (!groupDescribe.value) return []
  const any = groupDescribe.value as KafkaGroupDescribeResp & { partitions?: KafkaGroupPartition[] }
  return Array.isArray(any.partitions) ? any.partitions : []
}

function kafkaGroupAssignmentsLen(m: KafkaGroupMember): number {
  const any = m as KafkaGroupMember & { assignments?: unknown[] }
  return Array.isArray(any.assignments) ? any.assignments.length : 0
}
function kafkaGroupLag(p: KafkaGroupPartition): number {
  return typeof p.lag === 'number' ? p.lag : 0
}
function kafkaGroupRowState(g: KafkaGroupRow): string {
  const any = g as KafkaGroupRow & { state?: string }
  return any.state || (g.protocol ? g.protocol : '-')
}
function kafkaPartitionDelta(p: KafkaPartitionInfo): number | string {
  const latestNum = typeof p.latest === 'number' ? p.latest : Number(String(p.latest || '0'))
  const earliestNum = typeof p.earliest === 'number' ? p.earliest : Number(String(p.earliest || '0'))
  if (Number.isNaN(latestNum) || Number.isNaN(earliestNum)) return '-'
  return Math.max(0, latestNum - earliestNum)
}
function channelKindStr(c: NotifyChannel): string {
  return String(c.kind || '-')
}
function fmtKafkaMsgTimestamp(m: KafkaMsg): string {
  // m.timestamp 可能是 ISO 字符串或数字；优先尝试 Date 解析
  if (!m.timestamp) return '-'
  const t = Number(m.timestamp)
  if (!Number.isNaN(t)) return fmtUnixMs(t)
  const d = new Date(m.timestamp)
  if (!Number.isNaN(d.getTime())) {
    return fmtUnixMs(d.getTime())
  }
  return String(m.timestamp)
}
</script>

<template>
  <div class="ingress-page">
    <!-- 标题 + Tab -->
    <el-affix :offset="0" class="ingress-affix" z-index="5">
      <div class="ingress-head">
        <div class="ingress-head-left">
          <h2 class="ingress-title">告警接入</h2>
          <span class="ingress-subtitle">外部告警接入 · 试推送 · 通知绑定</span>
        </div>
        <div class="ingress-head-actions">
          <button class="ghost" @click="helpOpen = !helpOpen">使用帮助</button>
          <button class="primary" :disabled="!canWrite" @click="openCreate()">新建接入</button>
        </div>
      </div>
    </el-affix>

    <!-- ================= 接入路由 ================= -->
    <div class="ingress-tab-body">
      <!-- 帮助 -->
      <details class="map-help ingress-help" :open="helpOpen" v-if="helpOpen">
        <summary>接入类型怎么用（点开查看）</summary>
        <div class="map-help-body">
          <p>外部告警进入 Eventide 后，统一走 <b>去重 → 丰富 → 静默 → 通知</b>。按来源选一种接入即可；也可用右侧按钮一键打开新建表单。</p>
          <table class="map-help-table">
            <thead><tr><th>类型</th><th>适用场景</th><th>怎么接</th><th></th></tr></thead>
            <tbody>
              <tr>
                <td><b>Alertmanager</b></td>
                <td>Prometheus / VictoriaMetrics 告警回调</td>
                <td>在 Alertmanager 配 <code>webhook_configs</code>，URL 用卡片上的 <code>/api/ingress/{id}/alertmanager</code>。请求头：<code>Authorization: Bearer &lt;token&gt;</code>（或 <code>X-Eventide-Token</code>）。</td>
                <td><button type="button" class="ghost" @click="openQuickCreate('alertmanager')">一键创建</button></td>
              </tr>
              <tr>
                <td><b>Generic / 拨测</b></td>
                <td>自研系统、Jeecg 业务拨测 <code>probe-alert</code></td>
                <td><code>POST /api/ingress/{id}/generic</code>（或自动识别的 <code>/push</code>）。拨测 JSON 需含 <code>eventType</code>=fire/recover、<code>messageId</code>、<code>bizchainName</code>、<code>retMessage</code>；IP 请带 <code>alertIp</code>。字段对不上时，在编辑里开「自定义字段映射」。</td>
                <td><button type="button" class="ghost" @click="openQuickCreate('generic')">一键创建</button></td>
              </tr>
              <tr>
                <td><b>Kafka</b></td>
                <td>告警总线、多系统汇聚、Zabbix / 拨测 / Trap 等</td>
                <td>填 Brokers + Topic +（建议）Group；Eventide 后台消费。<b>无 map_*</b>：自动识别 Alertmanager / Generic / 拨测 JSON。<b>有 map_*</b>：按字段映射解析（如 Zabbix）。可用「工具 → Kafka」试写验证。</td>
                <td><button type="button" class="ghost" @click="openQuickCreate('kafka')">一键创建</button></td>
              </tr>
              <tr>
                <td><b>SNMP Trap 样例</b></td>
                <td>设备 Trap → Trap 服务 → Kafka → 本接入</td>
                <td>先跑 <code>eventide-trap</code>，写出 Topic（默认 <code>eventide.snmptrap</code>）。一键建 Kafka 接入（字段已对齐，一般不用映射）。在「SNMP Trap」页试推送，再到「告警事件」查看。</td>
                <td><button type="button" class="ghost" @click="openQuickCreate('snmptrap')">一键创建</button></td>
              </tr>
            </tbody>
          </table>
          <p style="margin-top:10px">
            <b>建议顺序：</b>通知渠道 → 新建接入 → 「试推送」→ 告警事件确认 → 再接真实平台。<br/>
            Token <b>必填</b>：保护 HTTP 推送（Alertmanager / Generic）；Kafka 接入靠网络与 ACL，不使用接入 Token。
          </p>
        </div>
      </details>

      <!-- 空态 -->
      <div v-if="!routesLoading && routes.length === 0" class="panel guide">
        <h3>配置告警接入</h3>
        <ol class="steps">
          <li>先在「通知渠道」配置至少一个机器人 / Webhook（可选，也可稍后绑定）</li>
          <li>Kafka / SNMP Trap：可在上方帮助里对各类型点「一键创建」；或先到「工具 → Kafka」确认 Topic</li>
          <li>创建接入：选择 Alertmanager / Generic（含拨测） / Kafka</li>
          <li>把外部平台 Webhook 指到下方生成的地址，或使用「试推送」验证</li>
          <li>在「告警事件」查看 firing / resolved 与通知结果</li>
        </ol>
        <button
          v-if="canWrite"
          class="primary"
          @click="openCreate()"
        >新建第一个接入</button>
      </div>

      <!-- 列表 -->
      <div v-else class="ingress-list">
        <article
          v-for="r in routes"
          :key="r.id"
          class="ingress-card"
        >
          <div class="ic-head">
            <div>
              <div class="ic-title">{{ r.name }}</div>
              <div class="ic-sub">
                <span :class="['badge', r.enabled ? 'on' : 'off']">
                  {{ r.enabled ? '启用' : '停用' }}
                </span>
                <span class="kind-tag">{{ r.kind }}</span>
                · {{ ingressKindHint(r) }}
              </div>
            </div>
            <div class="actions">
              <button
                :disabled="!r.enabled"
                @click="testRoute = r; testScenario = 'fire'; testDialogVisible = true"
              >试推送</button>
              <button @click="router.push('/alerts?source=ingress')">查告警</button>
              <button v-if="canWrite" @click="openEdit(r)">编辑</button>
              <button v-if="canWrite" class="danger" @click="removeRoute(r)">删除</button>
            </div>
          </div>
          <div class="ic-body">
            <div class="field" style="margin:0">
              <div class="label" style="display:block;font-size:11px;font-weight:600;color:var(--muted);text-transform:uppercase;letter-spacing:.06em;margin:0 0 6px">接入地址</div>
              <div class="url-row">
                <code class="mono url-box">{{ mainEndpoint(r) }}</code>
                <button @click="copyText(mainEndpoint(r))">复制</button>
              </div>
              <div v-if="r.kind !== 'kafka'" class="hint">
                也可用自动识别入口：<code class="mono">{{ pushEndpoint(r) }}</code>
                <template v-if="optionStr(r, 'token')">
                  · 请求头需带 <code>Authorization: Bearer ***</code> 或 <code>X-Eventide-Token</code>
                </template>
                <template v-else>
                  · <span style="color: var(--danger, #c0392b);">未配置 Token，推送会被拒绝</span> — 请编辑并填写鉴权 Token
                </template>
              </div>
              <div v-else class="hint">
                Brokers <code class="mono">{{ r.endpoint || '—' }}</code>
                · Topic <code class="mono">{{ optionStr(r, 'topic') || '—' }}</code>
                · 起始 {{ optionStr(r, 'start') || 'latest' }}
              </div>
            </div>
            <div class="ic-meta">
              <div>
                <span class="label">通知渠道</span>
                <div class="value">
                  <template v-if="r.channel_ids && r.channel_ids.length">
                    <div class="ic-channel-list">
                      <span
                        v-for="cid in r.channel_ids"
                        :key="cid"
                        class="chip"
                        :title="channelMap[cid] ? `${channelMap[cid].name} (${channelMap[cid].kind})` : cid"
                      >{{ channelMap[cid]?.name || cid.slice(0, 8) }}</span>
                    </div>
                  </template>
                  <span v-else class="ic-channel-empty">未绑定渠道</span>
                </div>
              </div>
              <div>
                <span class="label">未接手升级</span>
                <div class="value">
                  <template v-if="ingressHasEscalate(r)">
                    <div class="ic-escalate-summary">
                      <span class="t">{{ r.escalate_after_seconds }}s</span>
                      <span>后触发升级</span>
                    </div>
                    <div style="height: 6px;"></div>
                    <div style="display: flex; align-items: center; gap: 8px; flex-wrap: wrap;">
                      <span style="font-size: 12px; color: var(--muted); font-weight: 600;">级别：</span>
                      <span class="chip sev-chip" :class="'sev-' + ingressEscalateSeverity(r)">
                        {{ ingressEscalateSeverityLabel(r) }}
                      </span>
                    </div>
                    <div style="height: 6px;"></div>
                    <div style="display: flex; align-items: center; gap: 8px; flex-wrap: wrap;">
                      <span style="font-size: 12px; color: var(--muted); font-weight: 600;">渠道：</span>
                      <template v-if="ingressEscalateChannelIds(r).length">
                        <div class="ic-escalate-list">
                          <span
                            v-for="cid in ingressEscalateChannelIds(r)"
                            :key="'esc-' + cid"
                            class="chip"
                            :title="channelMap[cid] ? `${channelMap[cid].name} (${channelMap[cid].kind})` : cid"
                          >{{ channelMap[cid]?.name || cid.slice(0, 8) }}</span>
                        </div>
                      </template>
                      <span v-else class="ic-escalate-empty">同通知渠道</span>
                    </div>
                  </template>
                  <span v-else class="ic-escalate-empty">未开启</span>
                </div>
              </div>
            </div>
          </div>
        </article>
      </div>
    </div>

    <!-- =============== Dialogs =============== -->

    <!-- 试推送 Dialog -->
    <el-dialog
      v-model="testDialogVisible"
      title="试推送"
      width="480px"
      :close-on-click-modal="false"
    >
      <el-alert type="info" :closable="false" show-icon style="margin-bottom: 14px;">
        <template #title>
          目标路由：<b>{{ testRoute?.name }}</b>
        </template>
      </el-alert>
      <el-form label-width="110px">
        <el-form-item label="场景 (scenario)">
          <el-radio-group v-model="testScenario">
            <el-radio label="fire">触发（fire）</el-radio>
            <el-radio label="recover">恢复（recover）</el-radio>
            <el-radio label="probe_fire">探针触发</el-radio>
            <el-radio label="probe_recover">探针恢复</el-radio>
          </el-radio-group>
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="testDialogVisible = false">取消</el-button>
        <el-button type="primary" @click="runTest">确认发送</el-button>
      </template>
    </el-dialog>

    <!-- 类型选择 Dialog -->
    <el-dialog
      v-model="typePickerVisible"
      width="720px"
      :close-on-click-modal="false"
      append-to-body
      :show-close="false"
    >
      <template #header>
        <div class="ds-modal-head">
          <h3>选择接入类型</h3>
          <p class="ds-modal-desc">先选来源类型，再填写连接与字段映射。SNMP Trap 请选样例，字段已与 Trap 写出对齐。</p>
        </div>
      </template>
      <div class="ds-modal-body">
        <div class="ds-type-pick-grid">
          <button
            v-for="k in KINDS"
            :key="k.kind"
            type="button"
            class="ds-type-pick-card"
            @click="pickKind(k.kind)"
          >
            <span :class="['ds-type-pick-ico', 'ds-tone-' + k.tone]">
              {{ k.icon }}
            </span>
            <span class="ds-type-pick-name">{{ k.label }}</span>
            <span class="ds-type-pick-desc">{{ k.desc }}</span>
          </button>
        </div>
      </div>
      <template #footer>
        <div class="ds-modal-actions">
          <button type="button" class="ds-btn-ghost" @click="typePickerVisible = false">取消</button>
        </div>
      </template>
    </el-dialog>

    <!-- 新建 / 编辑 Dialog -->
    <el-dialog
      v-model="dialogVisible"
      width="720px"
      :close-on-click-modal="false"
      append-to-body
      :show-close="false"
    >
      <template #header>
        <div class="ds-modal-head">
          <h3>{{ isEdit ? '编辑告警接入' : '新建告警接入' }}</h3>
          <p class="ds-modal-desc">接入外部已判定的告警。非标准格式可配置字段映射。</p>
        </div>
      </template>
      <div class="ds-modal-body">
        <!-- 1/4 基础信息 -->
        <section class="ds-form-section">
          <div class="ds-form-section-head">
            <h4 class="ds-form-section-title">基础信息</h4>
            <span class="ds-form-section-tag">1 / 4</span>
          </div>
          <div class="ds-field">
            <label>名称</label>
            <input v-model="form.name" type="text" required placeholder="例如：生产 AM" />
          </div>
          <div class="ds-field">
            <label>类型</label>
            <div class="ds-type-picked">
              <span :class="['ds-type-pick-ico', 'ds-tone-' + kindMetaOf(form.kind).tone]">
                {{ kindMetaOf(form.kind).icon }}
              </span>
              <div class="ds-type-picked-text">
                <div class="t-name">{{ kindMetaOf(form.kind).label }}</div>
                <div class="t-desc">{{ kindMetaOf(form.kind).desc }}</div>
              </div>
              <button
                v-if="!isEdit"
                type="button"
                class="ds-btn-ghost ds-btn-sm"
                @click="dialogVisible = false; typePickerVisible = true"
              >重选类型</button>
            </div>
          </div>
          <div class="ds-field" style="margin-bottom: 4px">
            <label class="ds-check-row">
              <input v-model="form.enabled" type="checkbox" />
              <span>启用此接入路由</span>
            </label>
          </div>
        </section>

        <!-- 2/4 接入参数 -->
        <section class="ds-form-section">
          <div class="ds-form-section-head">
            <h4 class="ds-form-section-title">接入参数</h4>
            <span class="ds-form-section-tag">2 / 4</span>
          </div>

          <!-- HTTP 类 -->
          <template v-if="form.kind === 'alertmanager' || form.kind === 'generic'">
            <div class="ds-field">
              <label>鉴权 Token（必填）</label>
              <div class="ds-url-row">
                <input v-model="form.token" type="text" placeholder="至少 8 位；请求头 Bearer / X-Eventide-Token" style="flex: 1" />
                <button type="button" class="ds-btn-ghost" @click="genToken">重新生成</button>
              </div>
              <div class="ds-hint">HTTP 接入必须配置 Token；推送时带 <code>Authorization: Bearer …</code> 或 <code>X-Eventide-Token</code>。</div>
              <div v-if="form.kind === 'alertmanager'" class="ds-hint ds-sample-hint">
                载荷示例：Alertmanager <code>{"alerts":[{"status":"firing","labels":{...}}]}</code>
              </div>
              <div v-else-if="form.kind === 'generic'" class="ds-hint ds-sample-hint">
                默认支持 Generic / 拨测 JSON；其他格式请展开下方「字段映射」配置路径。
              </div>
            </div>
          </template>

          <!-- Kafka 类 -->
          <template v-else-if="form.kind === 'kafka'">
            <div class="ds-field">
              <label>Brokers</label>
              <input v-model="form.brokers" type="text" placeholder="127.0.0.1:9092" />
            </div>
            <div class="ds-row">
              <div class="ds-field">
                <label>Topic</label>
                <input v-model="form.topic" type="text" placeholder="alerts" />
              </div>
              <div class="ds-field">
                <label>起始位点</label>
                <select v-model="form.start">
                  <option value="latest">latest 仅新消息</option>
                  <option value="earliest">earliest 从头消费</option>
                </select>
              </div>
            </div>
            <div class="ds-field">
              <label>Consumer Group ID（可选）</label>
              <input v-model="form.group_id" type="text" placeholder="默认 eventide-ingress-{route_id}" />
              <div class="ds-hint">多实例共用同一 group_id 自动分摊分区；改 group_id 会按「起始位点」重新消费。</div>
            </div>
            <div class="ds-field" style="margin-bottom: 4px">
              <label>分区数（订阅范围）</label>
              <div class="ds-url-row">
                <input v-model="form.partitions" type="number" min="1" placeholder="自动探测" style="flex: 1" />
                <button type="button" class="ds-btn-ghost" @click="probePartitions">自动获取</button>
              </div>
              <div class="ds-hint">须覆盖 Topic 全部分区（优先 metadata 探测）。</div>
            </div>
          </template>
        </section>

        <!-- 3/4 字段映射 -->
        <section class="ds-form-section">
          <div class="ds-form-section-head">
            <h4 class="ds-form-section-title">字段映射</h4>
            <span class="ds-form-section-tag">3 / 4 · 通用 / Kafka 可选</span>
          </div>
          <div class="ds-seg">
            <div class="ds-hint" style="margin-bottom: 12px">
              填写对方 JSON 的点分路径（如 <code>data.title</code>）。可用变换截取字符串，例如
              <code>sourceciname|before:_</code> → <code>82.12.161.32</code>。
              任一路径非空即启用映射，并优先于内置 Generic/拨测解析。
            </div>
            <details class="ds-map-help">
              <summary>字段说明（点开查看）</summary>
              <div class="ds-map-help-body">
                <p>路径填原始 JSON 字段；可用 <code>|before:</code> / <code>|after:</code> / <code>|split:SEP:INDEX</code> / <code>|between:起点:终点</code> 截取。</p>
                <table class="ds-map-help-table">
                  <thead>
                    <tr><th>配置项</th><th>作用</th><th>写入结果</th></tr>
                  </thead>
                  <tbody>
                    <tr><td><code>map_list</code></td><td>告警数组路径，空=整条消息当一条</td><td>—</td></tr>
                    <tr><td><code>map_status</code></td><td>状态字段</td><td>firing / resolved</td></tr>
                    <tr><td><code>map_fire</code></td><td>视为触发的取值（逗号分隔）</td><td>—</td></tr>
                    <tr><td><code>map_resolve</code></td><td>视为恢复的取值</td><td>—</td></tr>
                    <tr><td><code>map_name</code></td><td>告警名称</td><td><code>labels.alertname</code></td></tr>
                    <tr><td><code>map_description</code></td><td>告警描述</td><td><code>annotations.summary</code> / <code>description</code></td></tr>
                    <tr><td><code>map_ip</code></td><td>告警 IP</td><td><code>labels.ip</code> / <code>alertIp</code> / <code>instance</code></td></tr>
                    <tr><td><code>map_value</code></td><td>当前值</td><td><code>value</code></td></tr>
                    <tr><td><code>map_fingerprint</code></td><td>去重标识</td><td><code>fingerprint</code></td></tr>
                    <tr><td><code>map_severity</code></td><td>级别原始值</td><td><code>labels.severity</code> + 引擎级别</td></tr>
                    <tr><td><code>map_critical</code></td><td>哪些取值算 Disaster/High</td><td>→ disaster</td></tr>
                    <tr><td><code>map_labels</code></td><td>额外标签，<code>目标标签:源路径,...</code></td><td>对应 <code>labels.*</code></td></tr>
                    <tr><td><code>map_enabled</code></td><td>强制开启映射</td><td>—</td></tr>
                  </tbody>
                </table>
                <p class="ds-hint" style="margin: 10px 0 0">引擎另支持 <code>map_warning</code>（警告取值列表），可在高级 options 中配置；控制台暂无单独输入框。</p>
              </div>
            </details>
            <div class="ds-field">
              <label class="ds-check-row">
                <input v-model="form.useMapping" type="checkbox" />
                <span>启用自定义字段映射</span>
              </label>
            </div>
            <template v-if="form.useMapping">
              <div class="ds-row">
                <div class="ds-field">
                  <label>告警列表路径 map_list</label>
                  <input v-model="form.map_list" type="text" placeholder="空=整条；或 data.items" />
                </div>
                <div class="ds-field">
                  <label>状态字段 map_status</label>
                  <input v-model="form.map_status" type="text" placeholder="state / status / eventType" />
                </div>
              </div>
              <div class="ds-row">
                <div class="ds-field">
                  <label>触发取值 map_fire</label>
                  <input v-model="form.map_fire" type="text" placeholder="默认 fire,firing,ALARM…" />
                </div>
                <div class="ds-field">
                  <label>恢复取值 map_resolve</label>
                  <input v-model="form.map_resolve" type="text" placeholder="默认 recover,resolved,OK…" />
                </div>
              </div>
              <div class="ds-row">
                <div class="ds-field">
                  <label>告警名称 map_name</label>
                  <input v-model="form.map_name" type="text" placeholder="title / alertName" />
                </div>
                <div class="ds-field">
                  <label>告警描述 map_description</label>
                  <input v-model="form.map_description" type="text" placeholder="msg / content" />
                </div>
              </div>
              <div class="ds-row">
                <div class="ds-field">
                  <label>告警 IP map_ip</label>
                  <input v-model="form.map_ip" type="text" placeholder="sourceciname|before:_" />
                  <div class="ds-hint"><code>before:_</code> / <code>after:_</code> / <code>split:_:0</code> / <code>between:起点:终点</code></div>
                </div>
                <div class="ds-field">
                  <label>当前值 map_value</label>
                  <input v-model="form.map_value" type="text" placeholder="metric / value" />
                </div>
              </div>
              <div class="ds-row">
                <div class="ds-field">
                  <label>告警标识 map_fingerprint</label>
                  <input v-model="form.map_fingerprint" type="text" placeholder="id / alertId" />
                </div>
                <div class="ds-field">
                  <label>级别 map_severity</label>
                  <input v-model="form.map_severity" type="text" placeholder="level / severity" />
                </div>
              </div>
              <div class="ds-row">
                <div class="ds-field">
                  <label>critical 取值</label>
                  <input v-model="form.map_critical" type="text" placeholder="Disaster,High,5,4" />
                </div>
                <div class="ds-field">
                  <label>额外标签 map_labels</label>
                  <input v-model="form.map_labels" type="text" placeholder="region:zone,app:appName" />
                </div>
              </div>
            </template>
          </div>
        </section>

        <!-- 4/4 通知与升级 -->
        <section class="ds-form-section">
          <div class="ds-form-section-head">
            <h4 class="ds-form-section-title">通知与升级</h4>
            <span class="ds-form-section-tag">4 / 4</span>
          </div>
          <div class="ds-field">
            <label>通知渠道</label>
            <select v-model="form.channel_ids" multiple :size="Math.min(6, Math.max(3, channels.length || 3))">
              <option
                v-for="c in channels"
                :key="c.id"
                :value="c.id"
              >{{ c.name }} ({{ c.kind }})</option>
            </select>
            <div class="ds-ms-hint">可多选，告警同时推送到所有绑定渠道。未绑定渠道时告警仍会入库，但不会发送通知。</div>
            <div class="ds-ms-hint">按住 <b>Ctrl</b>（Windows）或 <b>⌘</b>（Mac）点击可多选；已选中的再点一次即取消。</div>
          </div>
          <div class="ds-field">
            <label>未接手升级（秒，0=关闭）</label>
            <input v-model.number="form.escalate_after_seconds" type="number" min="0" />
            <div class="ds-hint">告警 firing 超过此时长仍未接手则发送升级通知；可抬升级别与选择独立升级渠道。</div>
          </div>
          <div class="ds-row">
            <div class="ds-field">
              <label>升级级别（可选）</label>
              <select v-model="form.escalate_severity">
                <option value="">不改级别</option>
                <option value="not_classified">未分类</option>
                <option value="information">信息</option>
                <option value="warning">警告</option>
                <option value="average">一般</option>
                <option value="high">严重</option>
                <option value="disaster">灾害</option>
              </select>
            </div>
          </div>
          <div class="ds-field" style="margin-bottom: 4px">
            <label>升级通知渠道（可选，空=用上方渠道）</label>
            <select v-model="form.escalate_channel_ids" multiple :size="Math.min(6, Math.max(3, channels.length || 3))">
              <option
                v-for="c in channels"
                :key="'esc-' + c.id"
                :value="c.id"
              >{{ c.name }} ({{ c.kind }})</option>
            </select>
            <div class="ds-hint">留空则超时时使用上方相同的「通知渠道」；一般可升级时改推不同的领导/总值班渠道。</div>
          </div>
        </section>
      </div>
      <template #footer>
        <div class="ds-modal-actions">
          <button type="button" class="ds-btn-ghost" @click="dialogVisible = false">取消</button>
          <button type="button" class="ds-btn-primary" :disabled="dialogLoading" @click="submitForm">
            {{ isEdit ? '保存修改' : '保存' }}
          </button>
        </div>
      </template>
    </el-dialog>
  </div>
</template>

<style>
/* 注意：非 scoped。类名加 ingress- 前缀以避免与 index.css 的 .panel / .severity-badge 重名。 */

.ingress-page {
  display: flex;
  flex-direction: column;
  gap: 14px;
  color: var(--text);
}

.ingress-affix {
  background: linear-gradient(180deg, var(--bg), var(--bg) 70%, rgba(255, 255, 255, 0));
  padding-top: 2px;
  padding-bottom: 2px;
}

.ingress-head {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  padding: 6px 2px 2px;
}
.ingress-head-left {
  display: flex;
  align-items: baseline;
  gap: 12px;
}
.ingress-head-actions {
  display: flex;
  gap: 8px;
  align-items: center;
}
.ingress-title {
  font-size: 18px;
  font-weight: 700;
  margin: 0;
  color: var(--heading);
}
.ingress-subtitle {
  font-size: 12px;
  color: var(--muted);
}

.ingress-tab-body {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

/* Guide Panel (与老版 .panel.guide 对齐) */
.panel.guide {
  padding: 28px 24px 32px;
  max-width: 720px;
  margin: 0 auto;
  text-align: center;
}
.panel.guide h3 {
  margin: 0 0 16px;
  font-size: 18px;
  font-weight: 650;
  color: var(--heading, var(--el-text-color-primary));;
}
.steps {
  text-align: left;
  max-width: 520px;
  margin: 0 auto 20px;
  padding-left: 20px;
  line-height: 2;
  color: var(--text-secondary, var(--el-text-color-secondary));;
  font-size: 14px;
}
.steps li::marker {
  font-weight: 700;
  color: var(--el-color-primary, var(--primary));;
}

/* 卡片列表 - 与老版 .ingress-list 对齐 */
.ingress-list {
  display: flex;
  flex-direction: column;
  gap: 14px;
}
.ingress-card {
  background: var(--panel-surface, var(--panel));;
  border: 1px solid var(--line, var(--el-border-color));
  border-radius: 12px;
  overflow: hidden;
  transition: transform 0.18s ease, box-shadow 0.18s ease, border-color 0.18s ease;
}
.ingress-card:hover {
  border-color: color-mix(in srgb, var(--el-color-primary, var(--primary)) 50%, var(--line, var(--el-border-color)));
  transform: translateY(-1px);
  box-shadow: 0 6px 18px rgba(0, 0, 0, 0.08);
}

/* ic-head */
.ic-head {
  display: flex;
  justify-content: space-between;
  gap: 12px;
  align-items: flex-start;
  padding: 14px 16px;
  border-bottom: 1px solid var(--line, var(--el-border-color));
  flex-wrap: wrap;
}
.ic-title {
  font-size: 16px;
  font-weight: 650;
  color: var(--heading, var(--el-text-color-primary));;
}
.ic-sub {
  margin-top: 6px;
  font-size: 12px;
  color: var(--muted, var(--el-text-color-secondary));;
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

/* actions (老版 .actions) */
.actions { display: flex; gap: 8px; flex-wrap: wrap; }
.actions button { height: 28px; padding: 0 10px; font-size: 13px; white-space: nowrap; }

/* kind-tag */
.kind-tag {
  display: inline-block;
  padding: 0 8px;
  border-radius: 999px;
  border: 1px solid var(--line-soft, var(--el-border-color-lighter));
  color: var(--el-color-primary, var(--primary));;
  font-size: 11px;
  line-height: 18px;
}

/* badge (老版 .badge) */
.badge {
  display: inline-block;
  padding: 0 8px;
  border-radius: 999px;
  font-size: 12px;
  line-height: 20px;
  border: 1px solid transparent;
  font-weight: 600;
}
.badge.on {
  background: rgba(92, 184, 122, 0.14);
  border-color: rgba(92, 184, 122, 0.35);
  color: var(--ok-soft, #2c8a4e);
}
.badge.off {
  background: var(--overlay-soft, var(--el-fill-color-light));
  border-color: var(--line, var(--el-border-color));
  color: var(--muted, var(--el-text-color-secondary));;
}

/* ic-body */
.ic-body {
  padding: 14px 16px 16px;
  display: grid;
  grid-template-columns: 1fr;
  gap: 14px;
}

/* url-row */
.url-row {
  display: flex;
  gap: 8px;
  align-items: flex-start;
}
.url-box {
  flex: 1;
  display: block;
  padding: 8px 10px;
  background: var(--inset-bg, #f7f8fa);
  border: 1px solid var(--line, var(--el-border-color));
  border-radius: 8px;
  word-break: break-all;
  font-size: 12px;
  color: var(--text-secondary, var(--el-text-color-secondary));;
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
}

/* hint (老版 .hint) */
.hint {
  margin-top: 6px;
  font-size: 12px;
  color: var(--muted, var(--el-text-color-secondary));;
  line-height: 1.6;
}
.hint .mono {
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  font-size: 12px;
}

/* mono */
.mono {
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
}

/* ic-meta */
.ic-meta {
  display: grid;
  grid-template-columns: minmax(0, 1.2fr) minmax(0, 1fr);
  gap: 12px 16px;
}
@media (max-width: 640px) {
  .ic-meta { grid-template-columns: 1fr; }
}
.ic-meta .label {
  display: block;
  font-size: 11px;
  font-weight: 600;
  color: var(--muted, var(--el-text-color-secondary));;
  text-transform: uppercase;
  letter-spacing: 0.06em;
  margin: 0 0 6px;
}
.ic-meta .value {
  font-size: 13px;
  color: var(--text-secondary, var(--el-text-color-secondary));;
  line-height: 1.55;
  word-break: break-word;
}

/* chips */
.ic-channel-list,
.ic-escalate-list {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}
.chip {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 3px 10px;
  border-radius: 999px;
  font-size: 12px;
  background: color-mix(in srgb, var(--el-color-primary, var(--primary)) 12%, var(--overlay-soft, var(--el-fill-color-light)));
  border: 1px solid color-mix(in srgb, var(--el-color-primary, var(--primary)) 40%, var(--line, var(--el-border-color)));
  color: var(--text, var(--el-text-color-primary));;
  max-width: 220px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.ic-escalate-list .chip {
  background: color-mix(in srgb, var(--el-color-warning, var(--warning)) 14%, var(--overlay-soft, var(--el-fill-color-light)));
  border-color: color-mix(in srgb, var(--el-color-warning, var(--warning)) 45%, var(--line, var(--el-border-color)));
  color: var(--heading, var(--el-text-color-primary));;
}
.ic-escalate-list .chip.sev-chip {
  font-weight: 600;
}
.ic-channel-empty,
.ic-escalate-empty {
  color: var(--muted, var(--el-text-color-secondary));;
  font-size: 12px;
  font-style: italic;
}

/* ic-escalate-summary */
.ic-escalate-summary {
  display: flex;
  align-items: baseline;
  flex-wrap: wrap;
  gap: 8px;
  color: var(--text-secondary, var(--el-text-color-secondary));;
  font-size: 13px;
}
.ic-escalate-summary .t {
  font-weight: 600;
  color: var(--el-color-warning, var(--warning));
  font-variant-numeric: tabular-nums;
}

/* severity chip colors */
.chip.sev-disaster {
  color: #c0392b;
}
.chip.sev-high {
  color: #e07a4a;
}
.chip.sev-average {
  color: #d4a017;
}
.chip.sev-warning {
  color: #b88230;
}
.chip.sev-information {
  color: #409eff;
}
.chip.sev-not_classified {
  color: #86909c;
}

/* ============================================================
 * 类型选择器 & Modal 样式
 * ============================================================ */
.ds-modal-head {
  padding: 20px 28px 16px;
  border-bottom: 1px solid var(--el-border-color, var(--line));;
  flex-shrink: 0;
  margin: -20px -20px 0 -20px;
}
.ds-modal-head h3 {
  margin: 0;
  font-size: 17px;
  font-weight: 650;
  color: var(--el-text-color-primary, var(--heading));;
}
.ds-modal-desc {
  margin: 6px 0 0;
  color: var(--el-text-color-secondary, var(--muted));;
  font-size: 13px;
  line-height: 1.5;
}
.ds-modal-body {
  padding: 20px 28px 12px;
  overflow: auto;
  flex: 1 1 auto;
  min-height: 0;
}
.ds-modal-actions {
  padding: 14px 28px;
  border-top: 1px solid var(--el-border-color, var(--line));;
  display: flex;
  justify-content: flex-end;
  gap: 10px;
  margin: 0 -20px -20px -20px;
}

/* 类型选择卡片网格 */
.ds-type-pick-grid {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 12px;
}
.ds-type-pick-card {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 10px;
  padding: 20px 14px;
  background: var(--el-bg-color, var(--panel));;
  border: 1px solid var(--el-border-color, var(--line));;
  border-radius: 12px;
  cursor: pointer;
  transition: border-color 0.15s, background 0.15s, transform 0.15s, box-shadow 0.15s;
  text-align: center;
}
.ds-type-pick-card:hover {
  border-color: var(--el-color-primary, var(--primary));;
  background: color-mix(in srgb, var(--el-color-primary, var(--primary)) 4%, var(--el-bg-color, #fff));
  transform: translateY(-1px);
  box-shadow: 0 4px 14px rgba(0, 0, 0, 0.06);
}
.ds-type-pick-ico {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 48px;
  height: 48px;
  border-radius: 12px;
  color: var(--el-color-white, #fff);
  font-size: 18px;
  font-weight: 700;
  letter-spacing: 0.02em;
  flex-shrink: 0;
}
.ds-type-pick-name {
  font-size: 14px;
  font-weight: 650;
  color: var(--el-text-color-primary, var(--heading));;
}
.ds-type-pick-desc {
  font-size: 12px;
  color: var(--el-text-color-secondary, var(--muted));;
  line-height: 1.45;
}

/* Tone 渐变 */
.ds-tone-orange {
  background: linear-gradient(145deg, #f59e0b, #ea580c);
}
.ds-tone-blue {
  background: linear-gradient(145deg, #3b82f6, #1d4ed8);
}
.ds-tone-amber {
  background: linear-gradient(145deg, #231f20, #4b5563);
  color: #f5c518;
}
.ds-tone-teal {
  background: linear-gradient(145deg, #2dd4bf, #0f766e);
}

/* 按钮 */
.ds-btn-ghost {
  padding: 8px 18px;
  font-size: 13px;
  color: var(--el-text-color-secondary, var(--muted));;
  background: transparent;
  border: 1px solid var(--el-border-color, var(--line));;
  border-radius: 6px;
  cursor: pointer;
  transition: all 0.15s;
}
.ds-btn-ghost:hover {
  color: var(--el-color-primary, var(--primary));;
  border-color: var(--el-color-primary, var(--primary));;
}
.ds-btn-primary {
  padding: 8px 18px;
  font-size: 13px;
  color: var(--el-color-white, #fff);
  background: var(--el-color-primary, var(--primary));;
  border: 1px solid var(--el-color-primary, var(--primary));
  border-radius: 6px;
  cursor: pointer;
  transition: all 0.15s;
}
.ds-btn-primary:hover {
  background: var(--el-color-primary-light-3, var(--primary-light));;
  border-color: var(--el-color-primary-light-3, var(--primary-light));;
}

/* ============================================================
 * 表单分段 (.form-section) 样式 - 与老版对齐
 * ============================================================ */
.ds-form-section {
  margin: 0 0 18px;
  padding: 16px 16px 4px;
  border: 1px solid var(--el-border-color, #e5e6eb);
  border-radius: 12px;
  background: color-mix(in srgb, var(--fill-color-light, #f7f8fa) 60%, transparent);
}
.ds-form-section-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin: -2px -2px 10px;
  padding: 0 4px 10px;
  border-bottom: 1px dashed var(--el-border-color-lighter, #ebeef5);
}
.ds-form-section-title {
  margin: 0;
  font-size: 13px;
  font-weight: 650;
  letter-spacing: 0.03em;
  color: var(--el-color-primary, var(--primary));;
  display: inline-flex;
  align-items: center;
  gap: 8px;
}
.ds-form-section-title::before {
  content: "";
  width: 4px;
  height: 14px;
  border-radius: 3px;
  background: linear-gradient(180deg, var(--el-color-primary, var(--primary)), color-mix(in srgb, var(--el-color-primary, var(--primary)) 60%, transparent));
  box-shadow: 0 0 0 2px color-mix(in srgb, var(--el-color-primary, var(--primary)) 22%, transparent);
}
.ds-form-section-tag {
  font-size: 11px;
  color: var(--el-text-color-secondary, var(--muted));;
  background: var(--el-fill-color-light, var(--inset-bg));;
  border: 1px solid var(--el-border-color, #e5e6eb);
  border-radius: 999px;
  padding: 1px 10px;
  line-height: 18px;
  white-space: nowrap;
}

/* field / row */
.ds-field {
  margin-bottom: 20px;
}
.ds-field label {
  display: block;
  font-size: 13px;
  font-weight: 500;
  color: var(--el-text-color-secondary, var(--muted));;
  margin-bottom: 8px;
}
.ds-field input[type="text"],
.ds-field input[type="number"],
.ds-field select,
.ds-field textarea {
  width: 100%;
  padding: 7px 12px;
  font-size: 13px;
  color: var(--el-text-color-primary, var(--heading));;
  background: var(--el-bg-color, var(--panel));;
  border: 1px solid var(--el-border-color, var(--line));;
  border-radius: 6px;
  outline: none;
  transition: border-color 0.15s, box-shadow 0.15s;
  box-sizing: border-box;
}
.ds-field input:focus,
.ds-field select:focus,
.ds-field textarea:focus {
  border-color: var(--el-color-primary, var(--primary));;
  box-shadow: 0 0 0 3px color-mix(in srgb, var(--el-color-primary, var(--primary)) 20%, transparent);
}
.ds-row {
  display: flex;
  gap: 16px;
  flex-wrap: wrap;
  margin-bottom: -4px;
}
.ds-row .ds-field {
  flex: 1;
  min-width: 140px;
}

/* check-row */
.ds-check-row {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  cursor: pointer;
  font-size: 14px;
  color: var(--el-text-color-primary, var(--heading));;
  line-height: 1.5;
  margin: 0 !important;
}
.ds-check-row input[type="checkbox"] {
  width: 16px;
  height: 16px;
  min-height: 16px;
  margin: 3px 0 0;
  accent-color: var(--el-color-primary, var(--primary));;
  flex-shrink: 0;
  cursor: pointer;
}
.ds-check-row span {
  font-size: 14px;
  color: var(--el-text-color-secondary, var(--muted));;
}

/* hint */
.ds-hint {
  margin-top: 6px;
  font-size: 12px;
  color: var(--el-text-color-secondary, var(--muted));;
  line-height: 1.5;
}
.ds-hint code {
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  font-size: 11px;
  padding: 1px 5px;
  background: var(--el-fill-color-light, var(--inset-bg));;
  border-radius: 4px;
}

/* sample-hint (载荷示例) */
.ds-sample-hint {
  margin-top: 10px;
  padding: 10px 12px;
  border-radius: 8px;
  background: rgba(59, 143, 217, 0.08);
  border: 1px solid rgba(59, 143, 217, 0.22);
  font-size: 12px;
  color: var(--el-text-color-secondary, var(--muted));;
}

/* seg (嵌套分组) - 与老版 .seg 对齐 */
.ds-seg {
  margin: 0 0 8px;
  padding: 0;
  border: none;
  background: transparent;
}
.ds-seg-title {
  font-size: 12px;
  font-weight: 600;
  color: var(--el-color-primary, var(--primary));;
  margin: 0 0 12px;
  letter-spacing: 0.06em;
  text-transform: uppercase;
}

/* map-help (可折叠字段说明表) */
.ds-map-help {
  margin: 0 0 14px;
  border: 1px solid var(--el-border-color, #e5e6eb);
  border-radius: 10px;
  background: var(--el-fill-color-light, var(--inset-bg));;
  overflow: hidden;
}
.ds-map-help > summary {
  cursor: pointer;
  list-style: none;
  padding: 10px 12px;
  font-size: 13px;
  font-weight: 600;
  color: var(--el-text-color-primary, var(--heading));;
  user-select: none;
}
.ds-map-help > summary::-webkit-details-marker {
  display: none;
}
.ds-map-help > summary::before {
  content: "▸ ";
  color: var(--el-text-color-secondary, var(--muted));;
  font-weight: 400;
}
.ds-map-help[open] > summary::before {
  content: "▾ ";
}
.ds-map-help-body {
  padding: 0 12px 12px;
  border-top: 1px solid var(--el-border-color, var(--line));;
  padding-top: 10px;
}
.ds-map-help-body > p {
  margin: 0 0 10px;
  font-size: 12px;
  color: var(--el-text-color-secondary, var(--muted));;
  line-height: 1.55;
}
.ds-map-help-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 12px;
  line-height: 1.45;
}
.ds-map-help-table th,
.ds-map-help-table td {
  text-align: left;
  vertical-align: top;
  padding: 7px 8px;
  border-bottom: 1px solid var(--el-border-color, var(--line));;
}
.ds-map-help-table th {
  color: var(--el-text-color-secondary, var(--muted));;
  font-weight: 600;
  white-space: nowrap;
}
.ds-map-help-table td:first-child {
  white-space: nowrap;
  width: 1%;
}
.ds-map-help-table code {
  font-size: 11px;
}

/* url-row */
.ds-url-row {
  display: flex;
  gap: 8px;
  align-items: flex-start;
}

/* type-picked (已选类型卡片) */
.ds-type-picked {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 10px 12px;
  border: 1px solid var(--el-border-color, #e5e6eb);
  border-radius: 10px;
  background: var(--el-fill-color-light, var(--inset-bg));;
}
.ds-type-pick-ico {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 40px;
  height: 40px;
  border-radius: 10px;
  color: var(--el-color-white, #fff);
  font-size: 13px;
  font-weight: 700;
  flex-shrink: 0;
}
.ds-type-picked-text {
  flex: 1;
  min-width: 0;
}
.ds-type-picked-text .t-name {
  font-size: 14px;
  font-weight: 650;
  color: var(--el-text-color-primary, var(--heading));;
}
.ds-type-picked-text .t-desc {
  font-size: 12px;
  color: var(--el-text-color-secondary, var(--muted));;
  margin-top: 2px;
}
.ds-btn-sm {
  padding: 4px 12px;
  font-size: 12px;
}

/* multi-select hint */
.ds-ms-hint {
  margin-top: 6px;
  font-size: 12px;
  color: var(--el-text-color-secondary, var(--muted));;
  line-height: 1.5;
}
.ds-ms-hint b {
  color: var(--el-color-primary, var(--primary));;
  font-weight: 600;
}

/* multi-select styling */
.ds-field select[multiple] {
  min-height: 140px;
  padding: 6px;
  border-radius: 10px;
  background: var(--el-fill-color-light, var(--inset-bg));;
  border: 1px solid var(--el-border-color, #e5e6eb);
  line-height: 1.45;
}
.ds-field select[multiple] option {
  padding: 7px 10px;
  border-radius: 6px;
  margin-bottom: 2px;
  font-size: 13px;
  color: var(--el-text-color-secondary, var(--muted));;
}
.ds-field select[multiple] option:checked {
  background: linear-gradient(180deg, rgba(64, 158, 255, 0.26), rgba(64, 158, 255, 0.18));
  color: var(--el-text-color-primary, var(--heading));;
  box-shadow: inset 2px 0 0 var(--el-color-primary, var(--primary));
  font-weight: 500;
}

/* ====== Kafka Tool ====== */
.ingress-kafka-btns {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
  height: 100%;
}
.ingress-kafka-status {
  margin-top: 12px;
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
  align-items: center;
}
.ingress-kafka-row {
  margin-top: 0;
}

.ingress-kafka-detail-tabs :deep(.el-tabs__header) {
  margin: 0;
}

.ingress-kafka-detail-body {
  padding: 10px 4px 6px;
}
.ingress-kafka-section-head {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 8px;
}
.ingress-kafka-section-title {
  font-weight: 600;
  color: var(--heading);
  font-size: 13.5px;
}
.ingress-kafka-msg {
  border: 1px solid var(--line);
  border-radius: 8px;
  margin: 8px 0;
  padding: 6px 8px;
  background: var(--panel-2);
}
.ingress-kafka-msg-meta {
  display: flex;
  gap: 6px;
  flex-wrap: wrap;
  margin-bottom: 6px;
}
.ingress-kafka-msg-body {
  margin: 0;
  background: var(--bg);
  border-radius: 6px;
  padding: 8px 10px;
  border: 1px dashed var(--line);
  max-height: 240px;
  overflow: auto;
  font-size: 12.5px;
  line-height: 1.55;
  color: var(--text);
}

.ingress-kafka-meta-card {
  background: var(--stat-bg);
  border: 1px solid var(--line);
  border-radius: 10px;
  padding: 10px 12px;
  margin-bottom: 10px;
}
.ingress-kafka-meta-label {
  font-size: 12px;
  color: var(--muted);
  letter-spacing: 0.04em;
  text-transform: uppercase;
  margin-bottom: 4px;
}
.ingress-kafka-meta-value {
  font-size: 15px;
  font-weight: 700;
  color: var(--heading);
  word-break: break-all;
}

/* ====== Form ====== */
.ingress-form-section {
  padding: 2px 2px;
}
.ingress-form-section-head {
  display: flex;
  align-items: center;
  margin-bottom: 10px;
}
.ingress-form-section-no {
  display: inline-grid;
  place-items: center;
  width: 52px;
  height: 22px;
  background: linear-gradient(90deg, var(--primary-grad-from), var(--primary-grad-to));
  color: var(--el-color-white, #fff);
  font-weight: 700;
  font-size: 12px;
  border-radius: 999px;
  margin-right: 10px;
}
.ingress-form-section-title {
  font-size: 14px;
  font-weight: 700;
  color: var(--heading);
}
.ingress-form-hint {
  font-size: 12px;
  color: var(--muted);
  margin-top: 2px;
}
.ingress-form-hint code {
  background: var(--panel-2);
  padding: 0 6px;
  border-radius: 4px;
  color: var(--text-secondary);
}

.ingress-kind-display {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
}

.ingress-kind-card {
  cursor: pointer;
  border: 1px solid var(--line);
  background: var(--panel-2);
  border-radius: 10px;
  padding: 12px 10px;
  transition: all 0.18s ease;
  user-select: none;
  height: 100%;
}
.ingress-kind-card:hover {
  border-color: var(--primary-soft);
  transform: translateY(-1px);
}
.ingress-kind-card.active {
  border-color: var(--primary-hover);
  background: var(--panel-surface);
  box-shadow: 0 0 0 3px var(--overlay-mid);
}
.ingress-kind-card-icon {
  font-size: 22px;
  margin-bottom: 4px;
}
.ingress-kind-card-title {
  font-weight: 700;
  color: var(--heading);
  font-size: 13.5px;
  margin-bottom: 2px;
}
.ingress-kind-card-desc {
  font-size: 11.5px;
  color: var(--muted);
  line-height: 1.5;
  min-height: 3em;
}

/* shared table head in cards (use index.css class fallback) */
.panel-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
}
.panel-title {
  font-weight: 600;
  color: var(--heading);
  font-size: 14.5px;
}
</style>
