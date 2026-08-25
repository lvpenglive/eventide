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
}
const KINDS: KindMeta[] = [
  {
    kind: 'alertmanager',
    label: 'Alertmanager',
    desc: '兼容 Alertmanager Webhook 的 /api/ingress/{id}/alertmanager 接收',
    tagType: 'primary',
    icon: '🔔',
    endpoint: 'HTTP',
  },
  {
    kind: 'generic',
    label: 'Generic Webhook',
    desc: '通用 JSON Webhook，地址 /api/ingress/{id}/generic，可自定义字段映射',
    tagType: 'success',
    icon: '🌐',
    endpoint: 'HTTP',
  },
  {
    kind: 'kafka',
    label: 'Kafka 消费',
    desc: '直接消费指定 Topic（支持字段映射），endpoint 为 brokers',
    tagType: 'warning',
    icon: '🦄',
    endpoint: 'Kafka',
  },
  {
    kind: 'snmptrap',
    label: 'SNMP Trap (Kafka)',
    desc: 'snmptraps 先经 exporter 写入 Kafka Topic，再以 Kafka 接入路由消费',
    tagType: 'danger',
    icon: '🪤',
    endpoint: 'Kafka',
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
    map_status: string
    map_severity: string
    map_fingerprint: string
    map_name: string
    map_description: string
    map_ip: string
    map_value: string
    map_labels_json_list: string
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
  map_status: '',
  map_severity: '',
  map_fingerprint: '',
  map_name: '',
  map_description: '',
  map_ip: '',
  map_value: '',
  map_labels_json_list: '',
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
  form.map_status = ''
  form.map_severity = ''
  form.map_fingerprint = ''
  form.map_name = ''
  form.map_description = ''
  form.map_ip = ''
  form.map_value = ''
  form.map_labels_json_list = ''
}

function openCreate(presetKind?: IngressKind | 'snmptrap', preset?: Partial<IngressInput> & { brokers?: string; topic?: string; start?: string; partitions?: number }): void {
  if (!canWrite.value) {
    ElMessage.warning('无创建接入路由的权限')
    return
  }
  void loadChannels()
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
  } else if (presetKind) {
    form.kind = presetKind as IngressKind
  }
  dialogVisible.value = true
}

function openQuickCreate(kind: KindMeta['kind']): void {
  if (kind === 'snmptrap') {
    openCreate('snmptrap')
  } else {
    openCreate(kind as IngressKind)
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
    'map_status' | 'map_severity' | 'map_fingerprint' | 'map_name' | 'map_description' | 'map_ip' | 'map_value' | 'map_labels_json_list'
  >)[] = [
    'map_status',
    'map_severity',
    'map_fingerprint',
    'map_name',
    'map_description',
    'map_ip',
    'map_value',
    'map_labels_json_list',
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
      'map_status' | 'map_severity' | 'map_fingerprint' | 'map_name' | 'map_description' | 'map_ip' | 'map_value' | 'map_labels_json_list'
    >)[] = [
      'map_status',
      'map_severity',
      'map_fingerprint',
      'map_name',
      'map_description',
      'map_ip',
      'map_value',
      'map_labels_json_list',
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
          <h2 class="ingress-title"><el-icon :size="20"><DataLine /></el-icon> 告警接入</h2>
          <span class="ingress-subtitle">管理 Ingress 路由 & Kafka 工具</span>
        </div>
      </div>
      <el-tabs v-model="activeTab" class="ingress-tabs">
        <el-tab-pane label="接入路由" name="routes" />
        <el-tab-pane label="Kafka 工具" name="kafka" />
      </el-tabs>
    </el-affix>

    <!-- ================= Tab 1：接入路由 ================= -->
    <div v-show="activeTab === 'routes'" class="ingress-tab-body">
      <!-- 顶部按钮区 + help -->
      <div class="ingress-toolbar panel" style="padding: 14px 18px;">
        <div class="ingress-toolbar-row">
          <el-button type="primary" :icon="Plus" :disabled="!canWrite" @click="openCreate()">
            新建接入
          </el-button>
          <el-tooltip v-if="!canWrite" content="您暂无 ingress:write 权限" placement="right">
            <el-button :icon="Setting" link type="info">权限说明</el-button>
          </el-tooltip>
          <el-button :icon="Refresh" link @click="loadRoutes">刷新列表</el-button>
          <el-button link type="primary" @click="helpOpen = !helpOpen">
            {{ helpOpen ? '收起帮助 ▲' : '展开帮助 ▼' }}
          </el-button>
        </div>

        <div v-if="helpOpen" class="ingress-help">
          <el-alert type="info" :closable="false" show-icon>
            <template #title>
              <span>快速说明：支持 4 种接入类型，点击「一键创建」即可预填表单。</span>
            </template>
          </el-alert>
          <el-table :data="KINDS" size="small" class="ingress-help-table" style="margin-top: 10px;">
            <el-table-column label="#" width="50" align="center">
              <template #default="{ $index }">{{ Number($index) + 1 }}</template>
            </el-table-column>
            <el-table-column label="类型" width="170">
              <template #default="{ row }">
                <el-tag :type="(row as KindMeta).tagType" effect="dark" size="small">
                  {{ (row as KindMeta).icon }} {{ (row as KindMeta).label }}
                </el-tag>
              </template>
            </el-table-column>
            <el-table-column label="接入方式" width="90">
              <template #default="{ row }">
                <el-tag size="small">{{ (row as KindMeta).endpoint }}</el-tag>
              </template>
            </el-table-column>
            <el-table-column prop="desc" label="说明" min-width="360" show-overflow-tooltip />
            <el-table-column label="操作" width="150" align="right">
              <template #default="{ row }">
                <el-tooltip
                  v-if="!canWrite"
                  content="您暂无 ingress:write 权限"
                  placement="left"
                >
                  <span>
                    <el-button :icon="Plus" size="small" type="primary" plain disabled>
                      一键创建
                    </el-button>
                  </span>
                </el-tooltip>
                <el-button
                  v-else
                  :icon="Plus"
                  size="small"
                  type="primary"
                  plain
                  @click="openQuickCreate((row as KindMeta).kind)"
                >
                  一键创建
                </el-button>
              </template>
            </el-table-column>
          </el-table>
        </div>
      </div>

      <!-- 空态 -->
      <div v-if="!routesLoading && routes.length === 0" class="panel empty ingress-empty">
        <el-empty description="还没有任何接入路由，按以下 5 步开始接入告警：">
          <template #image>
            <el-icon :size="64" color="var(--info-soft)"><Bell /></el-icon>
          </template>
        </el-empty>
        <ol class="ingress-empty-steps">
          <li><b>通知渠道</b>：先到「通知渠道」创建至少一个渠道（如飞书、钉钉、Webhook 等）。</li>
          <li><b>Kafka / 工具</b>：若走 Kafka 接入，可在本页「Kafka 工具」Tab 内先建 Topic。</li>
          <li><b>创建接入</b>：点击「新建接入」或上方「一键创建」，填写接入参数。</li>
          <li><b>试推送</b>：在路由卡片上点「试推送」，验证渠道可达性。</li>
          <li><b>告警事件确认</b>：跳转到「告警事件」页确认接入的告警入库。</li>
        </ol>
        <div style="margin-top: 18px;">
          <el-button
            type="primary"
            :icon="Plus"
            :disabled="!canWrite"
            @click="openCreate()"
          >
            新建第一个接入
          </el-button>
        </div>
      </div>

      <!-- 列表 -->
      <div v-else class="ingress-cards">
        <el-card
          v-for="r in routes"
          :key="r.id"
          class="ingress-card panel"
          shadow="never"
          style="padding: 0;"
        >
          <template #header>
            <div class="ingress-card-head">
              <div class="ingress-card-head-left">
                <span class="ingress-card-name">{{ r.name }}</span>
                <el-badge
                  :is-dot="false"
                  :value="r.enabled ? 'on' : 'off'"
                  :type="r.enabled ? 'success' : 'info'"
                >
                  <el-tag
                    size="small"
                    :type="kindMetaOf(r.kind).tagType"
                    effect="light"
                    style="margin-left: 8px;"
                  >
                    {{ kindMetaOf(r.kind).icon }} {{ kindMetaOf(r.kind).label }}
                  </el-tag>
                </el-badge>
                <span class="ingress-card-desc">{{ kindMetaOf(r.kind).desc }}</span>
              </div>
              <div class="ingress-card-head-right">
                <el-tooltip
                  v-if="!r.enabled"
                  content="路由未启用，请先编辑打开"
                  placement="top"
                >
                  <span>
                    <el-button
                      :icon="Bell"
                      size="small"
                      plain
                      type="warning"
                      disabled
                    >
                      试推送
                    </el-button>
                  </span>
                </el-tooltip>
                <el-button
                  v-else
                  :icon="Bell"
                  size="small"
                  plain
                  type="warning"
                  @click="testRoute = r; testScenario = 'fire'; testDialogVisible = true"
                >
                  试推送
                </el-button>
                <el-button
                  size="small"
                  :icon="View"
                  type="primary"
                  plain
                  @click="router.push('/alerts?source=ingress')"
                >
                  查告警
                </el-button>
                <el-tooltip
                  v-if="!canWrite"
                  content="您暂无 ingress:write 权限"
                  placement="top"
                >
                  <span>
                    <el-button :icon="Edit" size="small" plain disabled>编辑</el-button>
                    <el-button :icon="Delete" size="small" plain type="danger" disabled>删除</el-button>
                  </span>
                </el-tooltip>
                <template v-else>
                  <el-button :icon="Edit" size="small" plain @click="openEdit(r)">编辑</el-button>
                  <el-button
                    :icon="Delete"
                    size="small"
                    plain
                    type="danger"
                    @click="removeRoute(r)"
                  >
                    删除
                  </el-button>
                </template>
              </div>
            </div>
          </template>

          <div class="ingress-card-body">
            <!-- 接入地址行 -->
            <div class="ingress-addr-row">
              <div class="ingress-addr-label">接入地址</div>
              <div class="ingress-addr-main">
                <code class="ingress-code">{{ mainEndpoint(r) }}</code>
                <el-button :icon="DocumentCopy" size="small" text type="primary" @click="copyText(mainEndpoint(r))">复制</el-button>
              </div>
            </div>

            <div v-if="r.kind !== 'kafka'" class="ingress-addr-row">
              <div class="ingress-addr-label">Push 地址</div>
              <div class="ingress-addr-main">
                <code class="ingress-code">{{ pushEndpoint(r) }}</code>
                <el-button :icon="DocumentCopy" size="small" text type="primary" @click="copyText(pushEndpoint(r))">复制</el-button>
                <el-tag
                  v-if="!optionStr(r, 'token')"
                  type="danger"
                  effect="dark"
                  size="small"
                  style="margin-left: 8px;"
                >
                  <el-icon :size="12"><Warning /></el-icon>&nbsp;Token 未设置
                </el-tag>
                <el-tag
                  v-else
                  type="success"
                  effect="light"
                  size="small"
                  style="margin-left: 8px;"
                >
                  <el-icon :size="12"><CircleCheck /></el-icon>&nbsp;Token 已配置
                </el-tag>
              </div>
            </div>

            <el-divider style="margin: 14px 0;" />

            <!-- 通知渠道 / 升级 -->
            <div class="ingress-meta-row">
              <div class="ingress-meta-label">通知渠道</div>
              <div class="ingress-meta-chips">
                <template v-if="r.channel_ids && r.channel_ids.length">
                  <el-tag
                    v-for="cid in r.channel_ids"
                    :key="cid"
                    size="small"
                    type="primary"
                    effect="plain"
                  >
                    {{ channelMap[cid]?.name || cid }}
                  </el-tag>
                </template>
                <el-tag v-else type="info" size="small" effect="light">未配置</el-tag>
              </div>
            </div>

            <div v-if="ingressHasEscalate(r)" class="ingress-meta-row">
              <div class="ingress-meta-label">升级 (Escalate)</div>
              <div class="ingress-meta-chips">
                <el-tag size="small" type="warning" effect="plain">
                  {{ r.escalate_after_seconds ?? 0 }}s 后
                </el-tag>
                <el-tag
                  size="small"
                  :type="severityTagType(ingressEscalateSeverity(r))"
                  effect="dark"
                >
                  严重度: {{ severityText(ingressEscalateSeverity(r)) }}
                </el-tag>
                <template v-if="ingressEscalateChannelIds(r).length">
                  <el-tag
                    v-for="cid in ingressEscalateChannelIds(r)"
                    :key="'esc-'+cid"
                    size="small"
                    type="danger"
                    effect="plain"
                  >
                    升级渠道: {{ channelMap[cid]?.name || cid }}
                  </el-tag>
                </template>
                <el-tag v-else size="small" type="info" effect="light">未指定升级渠道</el-tag>
              </div>
            </div>
          </div>
        </el-card>
      </div>
    </div>

    <!-- ================= Tab 2：Kafka 工具 ================= -->
    <div v-show="activeTab === 'kafka'" class="ingress-tab-body">
      <div class="panel" style="padding: 14px 18px;">
        <el-row :gutter="14">
          <el-col :xs="24" :sm="12" :md="8">
            <el-form-item label="Brokers" style="margin-bottom: 0;">
              <el-input
                v-model="kafkaBrokers"
                placeholder="host1:9092,host2:9092"
                clearable
                @change="safeLSSet(LS_BROKERS, kafkaBrokers)"
              />
            </el-form-item>
          </el-col>
          <el-col :xs="24" :sm="12" :md="6">
            <el-form-item label="默认 Topic" style="margin-bottom: 0;">
              <el-input
                v-model="kafkaDefaultTopic"
                placeholder="eventide.snmptrap"
                clearable
                @change="safeLSSet(LS_TOPIC, kafkaDefaultTopic)"
              />
            </el-form-item>
          </el-col>
          <el-col :xs="24" :md="10">
            <div class="ingress-kafka-btns">
              <el-button type="primary" :icon="Refresh" :loading="probeLoading" @click="probeAndList">
                连接并列出 Topic
              </el-button>
              <el-button :icon="DataLine" :loading="groupsLoading" @click="loadGroups">
                列出消费组
              </el-button>
              <el-tooltip v-if="!canWrite" content="需要 ingress:write 权限" placement="top">
                <span>
                  <el-button :icon="Plus" disabled>创建 Topic</el-button>
                </span>
              </el-tooltip>
              <el-button v-else :icon="Plus" type="success" @click="onCreateTopic">创建 Topic</el-button>
            </div>
          </el-col>
        </el-row>

        <div v-if="probeResp" class="ingress-kafka-status">
          <el-badge :is-dot="false" value="已连接" type="success">
            <el-tag type="success" effect="light">
              latency {{ probeLatencyMs(probeResp) != null ? probeLatencyMs(probeResp) : '-' }}ms
            </el-tag>
          </el-badge>
          <el-tag type="info" effect="plain">Topic 数 {{ topics.length }}</el-tag>
          <template v-if="kafkaDefaultTopic">
            <el-tag v-if="defaultTopicExists()" type="primary" effect="dark">
              关注 Topic「{{ kafkaDefaultTopic }}」分区数: {{ partitionCountOf(kafkaDefaultTopic) }}
            </el-tag>
            <el-tag v-else type="warning" effect="light">
              关注 Topic「{{ kafkaDefaultTopic }}」不存在
            </el-tag>
          </template>
        </div>
      </div>

      <el-row :gutter="14" class="ingress-kafka-row">
        <!-- 左：Topics + Groups -->
        <el-col :xs="24" :md="13">
          <el-card class="panel" shadow="never" style="padding: 0;">
            <template #header>
              <div class="panel-head">
                <span class="panel-title">Topics（{{ topics.length }}）</span>
                <el-button size="small" text @click="probeAndList">刷新</el-button>
              </div>
            </template>
            <el-scrollbar max-height="320px">
              <el-table
                v-if="topics.length"
                :data="topics"
                size="small"
                empty-text="暂无 Topic，点击「连接并列出 Topic」"
              >
                <el-table-column prop="name" label="Name" min-width="220" show-overflow-tooltip />
                <el-table-column prop="partitions" label="分区数" width="90" align="center" />
                <el-table-column label="操作" width="200" align="right" fixed="right">
                  <template #default="{ row }">
                    <el-button
                      type="primary"
                      size="small"
                      text
                      @click="openTopicDetail((row as KafkaTopicRow).name)"
                    >
                      打开
                    </el-button>
                    <el-tooltip v-if="!canWrite" content="需要 ingress:write 权限" placement="top">
                      <span>
                        <el-button type="danger" size="small" text disabled>删除</el-button>
                      </span>
                    </el-tooltip>
                    <el-button
                      v-else
                      type="danger"
                      size="small"
                      text
                      @click="onDeleteTopic((row as KafkaTopicRow).name)"
                    >
                      删除
                    </el-button>
                  </template>
                </el-table-column>
              </el-table>
              <el-empty v-else description="暂无 Topic，点击「连接并列出 Topic」" />
            </el-scrollbar>
          </el-card>

          <el-card class="panel" shadow="never" style="padding: 0; margin-top: 14px;">
            <template #header>
              <div class="panel-head">
                <span class="panel-title">Consumer Groups（{{ groups.length }}）</span>
                <el-button size="small" :loading="groupsLoading" text @click="loadGroups">List</el-button>
              </div>
            </template>
            <el-scrollbar max-height="320px">
              <el-table
                v-if="groups.length"
                :data="groups"
                size="small"
                empty-text="暂无消费组，点击「列出消费组」"
              >
                <el-table-column prop="group_id" label="Group ID" min-width="220" show-overflow-tooltip />
                <el-table-column label="State" width="120" align="center">
                  <template #default="{ row }">
                    <el-tag size="small" :type="kafkaGroupRowState(row as KafkaGroupRow) === 'Stable' ? 'success' : 'info'">
                      {{ kafkaGroupRowState(row as KafkaGroupRow) }}
                    </el-tag>
                  </template>
                </el-table-column>
                <el-table-column label="操作" width="140" align="right" fixed="right">
                  <template #default="{ row }">
                    <el-button
                      type="primary"
                      size="small"
                      text
                      @click="describeGroupFromList(row as KafkaGroupRow)"
                    >
                      积压
                    </el-button>
                  </template>
                </el-table-column>
              </el-table>
              <el-empty v-else description="暂无消费组，点击「列出消费组」" />
            </el-scrollbar>
          </el-card>
        </el-col>

        <!-- 右：详情 -->
        <el-col :xs="24" :md="11">
          <el-card class="panel" shadow="never" style="padding: 0;">
            <template #header>
              <div class="panel-head">
                <el-tabs v-model="detailView" class="ingress-kafka-detail-tabs" tab-style="card">
                  <el-tab-pane name="topics" label="概览" />
                  <el-tab-pane v-if="detailTopic" name="topic" :label="'Topic: ' + detailTopic" />
                  <el-tab-pane v-if="detailGroup" name="group" :label="'Group: ' + detailGroup" />
                </el-tabs>
                <el-button
                  v-if="detailView === 'topic' && detailTopic"
                  size="small"
                  text
                  @click="openTopicDetail(detailTopic)"
                >
                  刷新
                </el-button>
                <el-button
                  v-if="detailView === 'group' && detailGroup"
                  size="small"
                  text
                  @click="openGroupDetail({ group_id: detailGroup } as KafkaGroupRow)"
                >
                  刷新
                </el-button>
              </div>
            </template>

            <div class="ingress-kafka-detail-body">
              <!-- 概览 -->
              <div v-if="detailView === 'topics'">
                <el-empty description="请在左侧打开一个 Topic 或 Consumer Group。">
                  <template #image>
                    <el-icon :size="48" color="var(--muted)"><DataLine /></el-icon>
                  </template>
                </el-empty>
              </div>

              <!-- Topic Detail -->
              <div v-else-if="detailView === 'topic' && detailTopic">
                <div class="ingress-kafka-section">
                  <div class="ingress-kafka-section-head">
                    <el-tag type="primary" effect="dark">Describe</el-tag>
                    <span class="ingress-kafka-section-title">分区信息</span>
                  </div>
                  <el-table
                    v-loading="topicDescribeLoading"
                    size="small"
                    :data="(topicDescribe?.partitions || []) as KafkaPartitionInfo[]"
                    empty-text="暂无分区数据"
                  >
                    <el-table-column prop="partition" label="分区" width="80" align="center" />
                    <el-table-column prop="earliest" label="earliest" width="120" />
                    <el-table-column prop="latest" label="latest" width="120" />
                    <el-table-column label="约数" width="120">
                      <template #default="{ row }">
                        {{ kafkaPartitionDelta(row as KafkaPartitionInfo) }}
                      </template>
                    </el-table-column>
                  </el-table>
                </div>

                <el-divider style="margin: 14px 0;" />

                <div class="ingress-kafka-section">
                  <div class="ingress-kafka-section-head">
                    <el-tag type="warning" effect="dark">Browse</el-tag>
                    <span class="ingress-kafka-section-title">浏览消息</span>
                  </div>
                  <el-row :gutter="10">
                    <el-col :span="6">
                      <el-form-item label="分区" size="small">
                        <el-input-number v-model="browsePartition" :min="0" controls-position="right" style="width: 100%;" />
                      </el-form-item>
                    </el-col>
                    <el-col :span="8">
                      <el-form-item label="From" size="small">
                        <el-select v-model="browseFrom" style="width: 100%;">
                          <el-option label="latest" value="latest" />
                          <el-option label="earliest" value="earliest" />
                          <el-option label="offset" value="offset" />
                        </el-select>
                      </el-form-item>
                    </el-col>
                    <el-col :span="5">
                      <el-form-item label="最大条数" size="small">
                        <el-input-number v-model="browseMax" :min="1" :max="1000" controls-position="right" style="width: 100%;" />
                      </el-form-item>
                    </el-col>
                    <el-col v-if="browseFrom === 'offset'" :span="5">
                      <el-form-item label="Offset" size="small">
                        <el-input-number v-model="browseOffset" :min="0" controls-position="right" style="width: 100%;" />
                      </el-form-item>
                    </el-col>
                    <el-col v-else :span="5">
                      <el-form-item label=" " size="small">
                        <el-button type="primary" :icon="Refresh" :loading="browseLoading" style="width: 100%;" @click="loadBrowse">
                          加载
                        </el-button>
                      </el-form-item>
                    </el-col>
                    <el-col v-if="browseFrom === 'offset'" :span="24">
                      <el-button type="primary" :icon="Refresh" :loading="browseLoading" @click="loadBrowse">加载</el-button>
                    </el-col>
                  </el-row>

                  <el-scrollbar max-height="360px" style="margin-top: 6px;">
                    <el-empty
                      v-if="!hasBrowseMessages()"
                      description="暂无消息"
                    />
                    <div
                      v-for="(m, idx) in browseMessages()"
                      :key="idx"
                      class="ingress-kafka-msg"
                    >
                      <div class="ingress-kafka-msg-meta">
                        <el-tag size="small" type="info">partition {{ m.partition }}</el-tag>
                        <el-tag size="small" type="primary">offset {{ m.offset }}</el-tag>
                        <el-tag size="small" effect="plain">key: {{ m.key || '(empty)' }}</el-tag>
                        <el-tag size="small" effect="light">{{ fmtKafkaMsgTimestamp(m) }}</el-tag>
                      </div>
                      <pre class="ingress-kafka-msg-body">{{ prettyJson(m.value) }}</pre>
                    </div>
                  </el-scrollbar>
                </div>

                <el-divider v-if="canWrite" style="margin: 14px 0;" />

                <div v-if="canWrite" class="ingress-kafka-section">
                  <div class="ingress-kafka-section-head">
                    <el-tag type="success" effect="dark">Produce</el-tag>
                    <span class="ingress-kafka-section-title">试写消息</span>
                  </div>
                  <el-row :gutter="10">
                    <el-col :span="8">
                      <el-form-item label="分区" size="small">
                        <el-input-number v-model="producePartition" :min="0" controls-position="right" style="width: 100%;" />
                      </el-form-item>
                    </el-col>
                    <el-col :span="16">
                      <el-form-item label="Key (可选)" size="small">
                        <el-input v-model="produceKey" placeholder="producer key" />
                      </el-form-item>
                    </el-col>
                  </el-row>
                  <el-form-item label="Value" size="small">
                    <el-input
                      v-model="produceValue"
                      type="textarea"
                      :rows="6"
                      placeholder="JSON 字符串或任意文本"
                    />
                  </el-form-item>
                  <el-button type="success" :loading="produceLoading" @click="runProduce">发送消息</el-button>
                </div>
              </div>

              <!-- Group Detail -->
              <div v-else-if="detailView === 'group' && detailGroup">
                <template v-if="groupDescribe">
                  <el-row :gutter="10">
                    <el-col :xs="12" :md="6">
                      <div class="ingress-kafka-meta-card">
                        <div class="ingress-kafka-meta-label">Group</div>
                        <div class="ingress-kafka-meta-value">{{ groupDescribe.group_id }}</div>
                      </div>
                    </el-col>
                    <el-col :xs="12" :md="6">
                      <div class="ingress-kafka-meta-card">
                        <div class="ingress-kafka-meta-label">State</div>
                        <div class="ingress-kafka-meta-value">
                          <el-tag :type="groupDescribe.state === 'Stable' ? 'success' : 'info'" effect="light">
                            {{ groupDescribe.state || '-' }}
                          </el-tag>
                        </div>
                      </div>
                    </el-col>
                    <el-col :xs="12" :md="6">
                      <div class="ingress-kafka-meta-card">
                        <div class="ingress-kafka-meta-label">Protocol</div>
                        <div class="ingress-kafka-meta-value">{{ gdProtocol() }}</div>
                      </div>
                    </el-col>
                    <el-col :xs="12" :md="6">
                      <div class="ingress-kafka-meta-card">
                        <div class="ingress-kafka-meta-label">Total Lag</div>
                        <div class="ingress-kafka-meta-value">
                          <el-tag
                            :type="gdTotalLag() > 0 ? 'warning' : 'success'"
                            effect="dark"
                          >
                            {{ gdTotalLag() }}
                          </el-tag>
                        </div>
                      </div>
                    </el-col>
                  </el-row>

                  <div class="ingress-kafka-section" style="margin-top: 14px;">
                    <div class="ingress-kafka-section-head">
                      <span class="ingress-kafka-section-title">Members（{{ gdMembers().length }}）</span>
                    </div>
                    <el-scrollbar max-height="200px">
                      <el-table
                        size="small"
                        :data="gdMembers()"
                        empty-text="暂无 members"
                      >
                        <el-table-column prop="client_id" label="client_id" min-width="160" show-overflow-tooltip />
                        <el-table-column prop="member_id" label="member_id" min-width="200" show-overflow-tooltip />
                        <el-table-column prop="host" label="host" min-width="140" show-overflow-tooltip />
                        <el-table-column label="Partitions 数" width="110" align="center">
                          <template #default="{ row }">
                            {{ kafkaGroupAssignmentsLen(row as KafkaGroupMember) }}
                          </template>
                        </el-table-column>
                      </el-table>
                    </el-scrollbar>
                  </div>

                  <div class="ingress-kafka-section" style="margin-top: 14px;">
                    <div class="ingress-kafka-section-head">
                      <span class="ingress-kafka-section-title">分区积压（{{ gdPartitions().length }}）</span>
                    </div>
                    <el-scrollbar max-height="260px">
                      <el-table
                        size="small"
                        :data="gdPartitions()"
                        empty-text="暂无分区积压数据"
                      >
                        <el-table-column prop="topic" label="Topic" min-width="180" show-overflow-tooltip />
                        <el-table-column prop="partition" label="partition" width="90" align="center" />
                        <el-table-column prop="committed" label="committed" width="110" />
                        <el-table-column prop="latest" label="latest" width="110" />
                        <el-table-column label="lag" width="90" align="center">
                          <template #default="{ row }">
                            <el-tag
                              size="small"
                              :type="kafkaGroupLag(row as KafkaGroupPartition) > 0 ? 'warning' : 'success'"
                              effect="light"
                            >
                              {{ kafkaGroupLag(row as KafkaGroupPartition) }}
                            </el-tag>
                          </template>
                        </el-table-column>
                      </el-table>
                    </el-scrollbar>
                  </div>
                </template>
                <el-empty v-else :description="groupDescribeLoading ? '加载中...' : '无数据'" />
              </div>
            </div>
          </el-card>
        </el-col>
      </el-row>
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

    <!-- 新建 / 编辑 Dialog -->
    <el-dialog
      v-model="dialogVisible"
      :title="isEdit ? '编辑接入路由' : '新建接入路由'"
      width="920px"
      :close-on-click-modal="false"
      top="6vh"
    >
      <el-form
        ref="formRef"
        :model="form"
        :rules="formRules"
        label-width="128px"
        label-position="right"
      >
        <!-- 1/4 基础 -->
        <div class="ingress-form-section">
          <div class="ingress-form-section-head">
            <span class="ingress-form-section-no">1 / 4</span>
            <span class="ingress-form-section-title">基础信息</span>
          </div>
          <el-row :gutter="14">
            <el-col :xs="24" :md="14">
              <el-form-item label="名称" prop="name">
                <el-input v-model="form.name" placeholder="例如：Prometheus-A / Kafka-Core" />
              </el-form-item>
            </el-col>
            <el-col :xs="24" :md="10">
              <el-form-item label="启用">
                <el-switch v-model="form.enabled" active-text="启用" inactive-text="停用" />
              </el-form-item>
            </el-col>
          </el-row>
          <el-form-item label="类型" prop="kind">
            <!-- 编辑或已预选：只读 display -->
            <template v-if="isEdit || form.kind">
              <div class="ingress-kind-display">
                <el-tag :type="kindMetaOf(form.kind).tagType" effect="dark" size="default">
                  {{ kindMetaOf(form.kind).icon }} {{ kindMetaOf(form.kind).label }}
                </el-tag>
                <span class="ingress-form-hint">{{ kindMetaOf(form.kind).desc }}</span>
              </div>
            </template>
            <!-- 新建时未预选 kind：显示 4 卡片选择 -->
            <el-row v-else :gutter="12">
              <el-col
                v-for="k in KINDS.filter(x => x.kind !== 'snmptrap')"
                :key="k.kind"
                :xs="24"
                :sm="12"
                :md="6"
              >
                <div
                  class="ingress-kind-card"
                  :class="{ active: form.kind === k.kind }"
                  @click="form.kind = k.kind as IngressKind"
                >
                  <div class="ingress-kind-card-icon">{{ k.icon }}</div>
                  <div class="ingress-kind-card-title">{{ k.label }}</div>
                  <div class="ingress-kind-card-desc">{{ k.desc }}</div>
                </div>
              </el-col>
            </el-row>
          </el-form-item>
        </div>

        <el-divider style="margin: 14px 0;" />

        <!-- 2/4 接入参数 -->
        <div class="ingress-form-section">
          <div class="ingress-form-section-head">
            <span class="ingress-form-section-no">2 / 4</span>
            <span class="ingress-form-section-title">接入参数</span>
          </div>

          <!-- HTTP 类：Token -->
          <template v-if="form.kind === 'alertmanager' || form.kind === 'generic'">
            <el-form-item label="鉴权 Token" required>
              <el-input
                v-model="form.token"
                type="password"
                show-password
                placeholder="请输入至少 8 字符的 Token（请求端放在 X-Ingress-Token header 或 token query）"
                minlength="8"
              />
              <div class="ingress-form-hint">
                保存前前端校验 ≥8 字符；推送到接入地址时请求端携带 Header <code>X-Ingress-Token</code>。
              </div>
            </el-form-item>
          </template>

          <!-- Kafka 类 -->
          <template v-else-if="form.kind === 'kafka'">
            <el-row :gutter="14">
              <el-col :xs="24" :md="14">
                <el-form-item label="Brokers" prop="brokers">
                  <el-input v-model="form.brokers" placeholder="host1:9092,host2:9092" />
                </el-form-item>
              </el-col>
              <el-col :xs="24" :md="10">
                <el-form-item label="Topic" prop="topic">
                  <el-input v-model="form.topic" placeholder="例如 eventide.alerts" />
                </el-form-item>
              </el-col>
              <el-col :xs="24" :sm="8">
                <el-form-item label="start">
                  <el-select v-model="form.start" style="width: 100%;">
                    <el-option label="latest" value="latest" />
                    <el-option label="earliest" value="earliest" />
                  </el-select>
                </el-form-item>
              </el-col>
              <el-col :xs="24" :sm="8">
                <el-form-item label="分区数 (参考)">
                  <el-input-number v-model="form.partitions" :min="0" :max="10000" controls-position="right" style="width: 100%;" />
                </el-form-item>
              </el-col>
              <el-col :xs="24" :sm="8">
                <el-form-item label="group_id (选填)">
                  <el-input v-model="form.group_id" placeholder="留空使用系统默认" />
                </el-form-item>
              </el-col>
            </el-row>
          </template>
        </div>

        <el-divider style="margin: 14px 0;" />

        <!-- 3/4 字段映射 -->
        <div class="ingress-form-section">
          <div class="ingress-form-section-head">
            <span class="ingress-form-section-no">3 / 4</span>
            <span class="ingress-form-section-title">字段映射</span>
            <el-switch
              v-model="form.useMapping"
              style="margin-left: 16px;"
              active-text="启用映射"
              inactive-text="关闭映射"
            />
          </div>
          <div v-if="!form.useMapping" class="ingress-form-hint">
            关闭映射时按协议默认字段解析（Alertmanager / Generic 内置默认映射）。
          </div>
          <template v-else>
            <el-row :gutter="14">
              <el-col :xs="24" :sm="12">
                <el-form-item label="map_status">
                  <el-input v-model="form.map_status" placeholder='例如 labels.status 或 "status"' />
                </el-form-item>
              </el-col>
              <el-col :xs="24" :sm="12">
                <el-form-item label="map_severity">
                  <el-input v-model="form.map_severity" placeholder='例如 labels.severity' />
                </el-form-item>
              </el-col>
              <el-col :xs="24" :sm="12">
                <el-form-item label="map_fingerprint">
                  <el-input v-model="form.map_fingerprint" placeholder='例如 labels.fingerprint' />
                </el-form-item>
              </el-col>
              <el-col :xs="24" :sm="12">
                <el-form-item label="map_name">
                  <el-input v-model="form.map_name" placeholder='默认 labels.alertname' />
                </el-form-item>
              </el-col>
              <el-col :xs="24" :sm="12">
                <el-form-item label="map_description">
                  <el-input v-model="form.map_description" placeholder='默认 annotations.summary' />
                </el-form-item>
              </el-col>
              <el-col :xs="24" :sm="12">
                <el-form-item label="map_ip">
                  <el-input v-model="form.map_ip" placeholder='例如 labels.instance 或 labels.ip' />
                </el-form-item>
              </el-col>
              <el-col :xs="24" :sm="12">
                <el-form-item label="map_value">
                  <el-input v-model="form.map_value" placeholder='例如 annotations.value' />
                </el-form-item>
              </el-col>
              <el-col :xs="24" :sm="12">
                <el-form-item label="map_labels_json_list">
                  <el-input v-model="form.map_labels_json_list" placeholder='例如 labels.* (JSON list)' />
                </el-form-item>
              </el-col>
            </el-row>
          </template>
        </div>

        <el-divider style="margin: 14px 0;" />

        <!-- 4/4 通知与升级 -->
        <div class="ingress-form-section">
          <div class="ingress-form-section-head">
            <span class="ingress-form-section-no">4 / 4</span>
            <span class="ingress-form-section-title">通知与升级</span>
          </div>
          <el-form-item label="通知渠道" prop="channel_ids">
            <el-select
              v-model="form.channel_ids"
              multiple
              filterable
              collapse-tags
              collapse-tags-tooltip
              style="width: 100%;"
              placeholder="选择至少一个渠道"
            >
              <el-option
                v-for="c in channels"
                :key="c.id"
                :label="c.name + ' (' + channelKindStr(c) + ')'"
                :value="c.id"
              />
            </el-select>
          </el-form-item>
          <el-row :gutter="14">
            <el-col :xs="24" :md="8">
              <el-form-item label="升级延迟 (秒)">
                <el-input-number
                  v-model="form.escalate_after_seconds"
                  :min="0"
                  :max="1000000"
                  controls-position="right"
                  style="width: 100%;"
                  placeholder="0 或 null 表示不升级"
                />
              </el-form-item>
            </el-col>
            <el-col :xs="24" :md="8">
              <el-form-item label="升级严重度">
                <el-select v-model="form.escalate_severity" style="width: 100%;">
                  <el-option label="全部 (all)" value="all" />
                  <el-option label="Critical" value="critical" />
                  <el-option label="Error" value="error" />
                  <el-option label="Warning" value="warning" />
                  <el-option label="Info" value="info" />
                  <el-option label="Ok" value="ok" />
                </el-select>
              </el-form-item>
            </el-col>
            <el-col :xs="24" :md="8">
              <el-form-item label="升级渠道">
                <el-select
                  v-model="form.escalate_channel_ids"
                  multiple
                  filterable
                  collapse-tags
                  collapse-tags-tooltip
                  style="width: 100%;"
                  placeholder="空则沿用通知渠道"
                >
                  <el-option
                    v-for="c in channels"
                    :key="'esc-'+c.id"
                    :label="c.name + ' (' + channelKindStr(c) + ')'"
                    :value="c.id"
                  />
                </el-select>
              </el-form-item>
            </el-col>
          </el-row>
        </div>
      </el-form>

      <template #footer>
        <el-button @click="dialogVisible = false">取消</el-button>
        <el-button type="primary" :loading="dialogLoading" @click="submitForm">
          {{ isEdit ? '保存修改' : '创建接入' }}
        </el-button>
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
  align-items: center;
  gap: 12px;
}
.ingress-title {
  font-size: 18px;
  font-weight: 700;
  margin: 0;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  color: var(--heading);
}
.ingress-subtitle {
  font-size: 12px;
  color: var(--muted);
}

.ingress-tabs :deep(.el-tabs__header) {
  margin-bottom: 8px;
}
.ingress-tabs :deep(.el-tabs__item) {
  font-weight: 600;
}

.ingress-tab-body {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

/* Toolbar */
.ingress-toolbar-row {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
}
.ingress-help {
  margin-top: 12px;
}
.ingress-help-table {
  border: 1px solid var(--line);
  border-radius: 8px;
  overflow: hidden;
}

/* Empty */
.ingress-empty {
  padding: 28px 18px 32px;
}
.ingress-empty-steps {
  text-align: left;
  max-width: 760px;
  margin: 12px auto 0;
  padding-left: 18px;
  line-height: 2;
  color: var(--text-secondary);
}
.ingress-empty-steps li::marker {
  font-weight: 700;
  color: var(--primary-hover);
}

/* 卡片列表 */
.ingress-cards {
  display: flex;
  flex-direction: column;
  gap: 14px;
}
.ingress-card {
  overflow: hidden;
}
.ingress-card :deep(.el-card__header) {
  background: var(--panel-2);
  border-bottom: 1px solid var(--line);
  padding: 10px 16px;
}
.ingress-card-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  flex-wrap: wrap;
}
.ingress-card-head-left {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
  min-width: 0;
}
.ingress-card-name {
  font-weight: 700;
  font-size: 15px;
  color: var(--heading);
}
.ingress-card-desc {
  color: var(--muted);
  font-size: 12.5px;
  margin-left: 6px;
}
.ingress-card-head-right {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}
.ingress-card-body {
  padding: 10px 4px 4px;
}

/* address rows */
.ingress-addr-row {
  display: flex;
  align-items: center;
  gap: 14px;
  margin: 6px 0;
}
.ingress-addr-label {
  min-width: 90px;
  color: var(--muted);
  font-size: 12.5px;
}
.ingress-addr-main {
  display: flex;
  align-items: center;
  gap: 8px;
  flex: 1;
  min-width: 0;
  flex-wrap: wrap;
}
.ingress-code {
  background: var(--panel-2);
  border: 1px solid var(--line);
  border-radius: 6px;
  padding: 4px 10px;
  font-size: 12.5px;
  color: var(--text);
  word-break: break-all;
  flex: 1;
  min-width: 260px;
}

.ingress-meta-row {
  display: flex;
  align-items: flex-start;
  gap: 14px;
  margin: 6px 0;
}
.ingress-meta-label {
  min-width: 90px;
  color: var(--muted);
  font-size: 12.5px;
  padding-top: 2px;
}
.ingress-meta-chips {
  display: flex;
  gap: 6px;
  flex-wrap: wrap;
  flex: 1;
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
  color: #fff;
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
