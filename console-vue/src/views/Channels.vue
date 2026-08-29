<script setup lang="ts">
/* =========================================================
 * Channels.vue —— 通知渠道视图（Vue 3 + TS + Element Plus）
 * 表单交互对齐老版本（editChannel / showChannelTestResult）：
 *   - 10 种渠道类型（webhook/http/dingtalk/wecom/feishu/slack/telegram/email/sms/phone）
 *   - 按类型动态显隐字段（http_method / headers_json / chat_id / msg_type / at_*）
 *   - 模板芯片一键插入（记忆焦点）
 *   - 编辑弹窗内置「测试发送」按钮
 *   - headers_json 校验 + JSON firing 软校验
 * ========================================================= */

import { computed, markRaw, onMounted, reactive, ref, watch, nextTick } from 'vue'
import {
  ElAffix,
  ElButton,
  ElCheckbox,
  ElCollapse,
  ElCollapseItem,
  ElDescriptions,
  ElDescriptionsItem,
  ElDialog,
  ElDivider,
  ElDrawer,
  ElDropdown,
  ElDropdownItem,
  ElDropdownMenu,
  ElEmpty,
  ElForm,
  ElFormItem,
  ElIcon,
  ElInput,
  ElMessage,
  ElMessageBox,
  ElOption,
  ElPopover,
  ElRow,
  ElCol,
  ElSelect,
  ElSwitch,
  ElTable,
  ElTableColumn,
  ElTag,
  ElTooltip,
  ElPagination,
} from 'element-plus'
import type { FormInstance, FormRules } from 'element-plus'
import {
  Check,
  CircleCheck,
  CopyDocument,
  Delete,
  Edit,
  MoreFilled,
  Plus,
  Refresh,
  Warning,
  VideoPlay,
  Close,
} from '@element-plus/icons-vue'

// ============================================================
// 类型补齐 + Shim/真实模块双模式
// ============================================================
type ChannelKind =
  | 'webhook'
  | 'http'
  | 'dingtalk'
  | 'wecom'
  | 'feishu'
  | 'slack'
  | 'telegram'
  | 'email'
  | 'sms'
  | 'phone'
  // 后端 serde(rename_all = "snake_case") 实际序列化别名
  | 'ding_talk'
  | 'we_com'
  | string

interface LocalChannel {
  id: string
  name: string
  kind: ChannelKind
  url: string
  secret: string | null
  options: Record<string, string>
  enabled: boolean
  created_at: string
  updated_at: string
}

interface ChannelInput {
  name: string
  kind: ChannelKind
  url: string
  secret?: string | null
  options?: Record<string, string>
  enabled?: boolean
}

interface ChannelTestResp {
  ok: boolean
  hint?: string
  text?: string
  method?: string
  path?: string
  request_url?: string
  request_body?: string
  http_status?: number | null
  response_body?: string
  error?: string
  [key: string]: unknown
}

interface ChannelsApiShim {
  listAllChannels(): Promise<LocalChannel[]>
  getChannel(id: string): Promise<LocalChannel>
  createChannel(body: ChannelInput): Promise<LocalChannel>
  updateChannel(id: string, body: ChannelInput): Promise<LocalChannel>
  deleteChannel(id: string): Promise<{ ok: boolean }>
  testChannel(id: string): Promise<ChannelTestResp>
}

function _unimpl(name: string): never {
  throw new Error(`[API shim] 模块未实现：${name}，请等待并行任务创建 @/api/channels`)
}

const _shimChannels: ChannelsApiShim = {
  listAllChannels: () => Promise.reject(_unimpl('listAllChannels')),
  getChannel: () => Promise.reject(_unimpl('getChannel')),
  createChannel: () => Promise.reject(_unimpl('createChannel')),
  updateChannel: () => Promise.reject(_unimpl('updateChannel')),
  deleteChannel: () => Promise.reject(_unimpl('deleteChannel')),
  testChannel: () => Promise.reject(_unimpl('testChannel')),
}

// @ts-ignore 若 @/api/channels 模块尚未创建则忽略解析错误，运行时走 shim
import * as _rawChannels from '@/api/channels'
const _channels = markRaw(
  _rawChannels as unknown as ChannelsApiShim | Record<string, unknown>
)

function _bindApi<A extends object, S>(
  api: A | Record<string, unknown>,
  shim: S
): S {
  const out = { ...(shim as unknown as object) } as S
  for (const key of Object.keys(shim as unknown as object)) {
    if (key in api && typeof (api as Record<string, unknown>)[key] === 'function') {
      ;(out as unknown as Record<string, unknown>)[key] = (
        api as Record<string, unknown>
      )[key]
    }
  }
  return out
}

const _api = _bindApi<Record<string, unknown>, ChannelsApiShim>(
  _channels as unknown as Record<string, unknown>,
  _shimChannels
)

const {
  listAllChannels,
  getChannel,
  createChannel,
  updateChannel,
  deleteChannel,
  testChannel,
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

function truncate(s: string, n: number): string {
  if (s == null) return ''
  const str = String(s)
  if (str.length <= n) return str
  return str.slice(0, n) + '…'
}

function prettyJson(obj: unknown, maxLines?: number): string {
  const format = (o: unknown): string => {
    try {
      return JSON.stringify(o, null, 2)
    } catch {
      return String(o ?? '')
    }
  }
  let pretty: string
  if (typeof obj === 'string') {
    const t = obj.trim()
    if (!t) return '(空)'
    try {
      pretty = JSON.stringify(JSON.parse(t), null, 2)
    } catch {
      pretty = obj
    }
  } else {
    pretty = format(obj ?? null)
  }
  if (maxLines && maxLines > 0) {
    const lines = pretty.split('\n')
    if (lines.length > maxLines) {
      return lines.slice(0, maxLines).join('\n') + `\n… （共 ${lines.length} 行，已截断）`
    }
  }
  return pretty
}

// ============================================================
// Kind 元信息（对齐老版本并集：共 10 种）
// 注：后端 serde(rename_all = "snake_case") → 列表读回来可能是 ding_talk / we_com
// parse 时同时兼容 alias；label 展示与后端序列化的原始 kind 一致
// ============================================================
interface KindMeta {
  kind: ChannelKind
  label: string
  color: string
  // 轻背景（用于 list tag）
  bg: string
  urlPlaceholder: string
}
const KINDS: KindMeta[] = [
  { kind: 'webhook', label: 'Webhook', color: '#722ed1', bg: 'rgba(114,46,209,.12)', urlPlaceholder: 'https://... 任意 HTTP(S) POST URL（固定 Eventide JSON 结构）' },
  { kind: 'http', label: 'http', color: '#fa8c16', bg: 'rgba(250,140,22,.12)', urlPlaceholder: 'https://example.com/hooks/alert（可自定义方法/请求头/JSON）' },
  { kind: 'dingtalk', label: 'dingtalk', color: '#2f54eb', bg: 'rgba(47,84,235,.12)', urlPlaceholder: '钉钉机器人 Webhook URL' },
  { kind: 'wecom', label: 'wecom', color: '#07c160', bg: 'rgba(7,193,96,.12)', urlPlaceholder: '企业微信机器人 URL' },
  { kind: 'feishu', label: 'feishu', color: '#1677ff', bg: 'rgba(22,119,255,.12)', urlPlaceholder: '飞书自定义机器人 Webhook' },
  { kind: 'slack', label: 'slack', color: '#6f42c1', bg: 'rgba(111,66,193,.12)', urlPlaceholder: 'https://hooks.slack.com/services/...（Incoming Webhook）' },
  { kind: 'telegram', label: 'telegram', color: '#229ed9', bg: 'rgba(34,158,217,.12)', urlPlaceholder: 'https://api.telegram.org/bot<token>' },
  { kind: 'email', label: 'email', color: '#13c2c2', bg: 'rgba(19,194,194,.12)', urlPlaceholder: 'smtp://user:pass@host:port' },
  { kind: 'sms', label: 'sms', color: '#eb2f96', bg: 'rgba(235,47,150,.12)', urlPlaceholder: 'HTTP 网关或 sdk 标识 URL' },
  { kind: 'phone', label: 'phone', color: '#f56c6c', bg: 'rgba(245,108,108,.12)', urlPlaceholder: 'HTTP 网关或 sip URI' },
]

function _normalizeKind(k: ChannelKind): ChannelKind {
  const s = String(k || '').toLowerCase()
  // 兼容 snake_case 别名
  if (s === 'ding_talk') return 'dingtalk'
  if (s === 'we_com') return 'wecom'
  return s as ChannelKind
}

function kindMetaOf(k: ChannelKind): KindMeta {
  const n = _normalizeKind(k)
  return (
    KINDS.find((x) => x.kind === n) || {
      kind: k,
      label: String(k || '-'),
      color: '#909399',
      bg: 'rgba(144,147,153,.12)',
      urlPlaceholder: '请填写渠道访问地址',
    }
  )
}

// 列表展示时，按后端实际序列化的 kind（snake_case）原样展示（与老版一致）
function kindDisplayOf(k: ChannelKind): string {
  const raw = String(k || '')
  if (raw) return raw
  return '-'
}

// 日期格式：把 2026-08-04T02:1... 变成紧凑版 yyyy-mm-ddTHH:MM
function formatDateShort(s: string | undefined | null): string {
  if (!s) return '-'
  const str = String(s)
  // 截取到分钟：yyyy-mm-ddThh:mm
  const m = /^(\d{4}-\d{2}-\d{2}T\d{2}:\d{2})/.exec(str)
  if (m) return m[1]
  return str
}

// 模板插入芯片（对齐老版本 14 个常用占位）
const TPL_CHIPS: string[] = [
  'title',
  'transition',
  'severity',
  'status',
  'value',
  'rule.name',
  'fingerprint',
  'labels',
  'annotations',
  'annotations.summary',
  'annotations.summary|json',
  'labels.instance',
  'labels.alertname',
]

// ============================================================
// 静态字符串常量（模板里用 :bind 引用，避免被 Vue 编译成插值）
// ============================================================
const PH_FIRING_TPL =
  '[{{severity}}] {{rule.name}}\n状态: {{transition}}\n{{annotations.summary}}\n值: {{value}}'
const PH_RESOLVED_TPL = '[恢复] {{rule.name}}\n{{annotations.summary}}'
const PH_JSON_FIRING =
  '{"msg": {{annotations.summary|json}}, "severity": {{severity|json}}, "labels": {{labels}}}'
const PH_JSON_RESOLVED = '{"msg": {{annotations.summary|json}}, "status": "resolved"}'
const HINT_JSON_SEG_RAW =
  '必须是合法 JSON。字符串字段请用 `{{annotations.summary|json}}`（自带引号与转义）；对象可用 `{{labels}}` / `{{annotations}}`。留空则发送默认 Eventide 结构。'
function HINT_JSON_SEG_HTML(): string {
  return HINT_JSON_SEG_RAW.replace(/`([^`]+)`/g, '<code>$1</code>')
}

// ============================================================
// 状态 & 加载
// ============================================================
const loading = ref(false)
const channels = ref<LocalChannel[]>([])

// 筛选
const filterKind = ref<ChannelKind | ''>('')
const filterQ = ref('')
const filterEnabled = ref<'all' | 'on' | 'off'>('all')

// —— 前端分页（渠道列表条数通常少，故用前端分页；列表先走 filteredChannels 再切片）
const CH_PAGE_SIZES = [10, 20, 50, 100] as const
const chPage = ref(1)
const chPageSize = ref<number>(CH_PAGE_SIZES[1])
const pagedChannels = computed<LocalChannel[]>(() => {
  const src = filteredChannels.value
  if (src.length <= chPageSize.value) return src
  const start = (chPage.value - 1) * chPageSize.value
  return src.slice(start, start + chPageSize.value)
})
function onChCurrentChange(p: number): void { chPage.value = Math.max(1, p) }
function onChSizeChange(s: number): void { chPageSize.value = s; chPage.value = 1 }
// 筛选变动时回到第 1 页（watch）
import { watch as _chWatch } from 'vue'
_chWatch([filterQ, filterKind, filterEnabled, channels], () => { chPage.value = 1 })

const filteredChannels = computed<LocalChannel[]>(() => {
  const q = filterQ.value.trim().toLowerCase()
  const fKind = filterKind.value ? _normalizeKind(filterKind.value) : ''
  return channels.value.filter((c) => {
    if (fKind && _normalizeKind(c.kind) !== fKind) return false
    if (filterEnabled.value === 'on' && !c.enabled) return false
    if (filterEnabled.value === 'off' && c.enabled) return false
    if (q) {
      const hay = `${c.name} ${c.url}`.toLowerCase()
      if (!hay.includes(q)) return false
    }
    return true
  })
})

async function loadChannels(force = false): Promise<void> {
  loading.value = true
  try {
    channels.value = await listAllChannels()
  } catch (e) {
    ElMessage.error(errMsgOf(e, '加载通知渠道失败'))
  } finally {
    loading.value = false
  }
}

onMounted(() => {
  void loadChannels()
})

// ============================================================
// 新建 / 编辑 Dialog
// ============================================================
const dialogVisible = ref(false)
const isEdit = ref(false)
const editingId = ref<string | null>(null)
const dialogLoading = ref(false)
const dialogTestLoading = ref(false)
const formRef = ref<FormInstance>()

const form = reactive<{
  name: string
  kind: ChannelKind
  url: string
  secret: string
  enabled: boolean
  // http 类型专用
  http_method: 'POST' | 'PUT' | 'PATCH'
  headers_json: string
  // telegram
  chat_id: string
  // 消息格式（非 webhook / http）
  msg_type: 'text' | 'markdown'
  // at（钉钉 / 企微）
  at_all: boolean
  at_mobiles: string
  // 模板
  template_firing: string
  template_resolved: string
  json_firing: string
  json_resolved: string
  template: string
  extraRows: Array<{ key: string; value: string }>
}>({
  name: '',
  kind: 'webhook',
  url: '',
  secret: '',
  enabled: true,
  http_method: 'POST',
  headers_json: '',
  chat_id: '',
  msg_type: 'text',
  at_all: false,
  at_mobiles: '',
  template_firing: '',
  template_resolved: '',
  json_firing: '',
  json_resolved: '',
  template: '',
  extraRows: [],
})

const rules: FormRules = {
  name: [
    { required: true, message: '请输入名称', trigger: 'blur' },
    { min: 2, message: '名称至少 2 个字符', trigger: 'blur' },
  ],
  url: [{ required: true, message: '请填写 URL / 访问地址', trigger: 'blur' }],
}

// ============================================================
// syncKindUi：按类型动态切换 URL/Secret 文案 + 控制字段显隐
// ============================================================
const isHttp = computed(() => _normalizeKind(form.kind) === 'http')
const isWebhook = computed(() => _normalizeKind(form.kind) === 'webhook')
const isTelegram = computed(() => _normalizeKind(form.kind) === 'telegram')
const isDingtalkOrWecom = computed(() => {
  const k = _normalizeKind(form.kind)
  return k === 'dingtalk' || k === 'wecom'
})

// secret 字段显示与否：钉钉/飞书签名，或 http（Bearer Token），或 telegram 可选
const showSecretField = computed(() => {
  const k = _normalizeKind(form.kind)
  return k === 'dingtalk' || k === 'feishu' || isHttp.value
})
const secretLabel = computed(() =>
  isHttp.value ? 'Bearer Token（可选）' : '签名密钥（可选）'
)
const secretPlaceholder = computed(() =>
  isHttp.value ? 'Authorization: Bearer <token> 中的 token 部分' : '钉钉 / 飞书 secret'
)
const secretHint = computed(() =>
  isHttp.value ? '若填写，将自动加 Authorization: Bearer <secret>。' : ''
)

// URL label / placeholder 动态
const urlLabel = computed(() => {
  if (isTelegram.value) return 'Bot API 基址'
  if (_normalizeKind(form.kind) === 'slack') return 'Slack Incoming Webhook'
  if (isHttp.value) return 'HTTP 接口地址'
  return 'Webhook / 机器人 URL'
})
const urlPlaceholder = computed(() => {
  if (isTelegram.value) return 'https://api.telegram.org/bot<token>'
  if (_normalizeKind(form.kind) === 'slack') return 'https://hooks.slack.com/services/...'
  return kindMetaOf(form.kind).urlPlaceholder
})
const urlHint = computed(() => {
  if (isTelegram.value) return '填 Bot Token 基址即可，会自动追加 /sendMessage；chat_id 必填。'
  if (_normalizeKind(form.kind) === 'slack') return 'Slack 应用 Incoming Webhooks 生成的 URL。'
  if (isHttp.value) return '向该地址发送 JSON；可用自定义请求头与 Bearer Token。'
  return ''
})

// 其它字段的显隐
const showHttpMethodField = isHttp
const showHeadersField = isHttp
const showChatIdField = isTelegram
// 消息格式：webhook / http 不走 text/markdown 选择
const showMsgTypeField = computed(() => !isWebhook.value && !isHttp.value)
// @at 字段仅钉钉/企微
const showAtField = isDingtalkOrWecom
// 文本模板段 vs JSON 模板段：http 类型用 JSON，其他类型走文本模板
const showTextTplSeg = computed(() => !isHttp.value)
const showJsonTplSeg = isHttp

function resetForm(): void {
  form.name = ''
  form.kind = 'webhook'
  form.url = ''
  form.secret = ''
  form.enabled = true
  form.http_method = 'POST'
  form.headers_json = ''
  form.chat_id = ''
  form.msg_type = 'text'
  form.at_all = false
  form.at_mobiles = ''
  form.template_firing = ''
  form.template_resolved = ''
  form.json_firing = ''
  form.json_resolved = ''
  form.template = ''
  form.extraRows = []
}

function openCreate(kind?: ChannelKind): void {
  resetForm()
  isEdit.value = false
  editingId.value = null
  if (kind) form.kind = kind
  dialogVisible.value = true
}

async function openEditById(id: string): Promise<void> {
  let c: LocalChannel
  try {
    c = await getChannel(id)
  } catch (e) {
    const local = channels.value.find((x) => x.id === id)
    if (!local) {
      ElMessage.error(errMsgOf(e, '获取渠道详情失败'))
      return
    }
    c = local
  }
  resetForm()
  isEdit.value = true
  editingId.value = c.id
  form.name = c.name
  form.kind = _normalizeKind(c.kind)
  form.url = c.url
  form.secret = c.secret || ''
  form.enabled = c.enabled

  const opts = c.options || {}
  const specialKeys = new Set([
    'template_firing',
    'template_resolved',
    'json_firing',
    'json_resolved',
    'template',
    'http_method',
    'headers_json',
    'chat_id',
    'msg_type',
    'at_all',
    'at_mobiles',
  ])
  form.template_firing = opts.template_firing || ''
  form.template_resolved = opts.template_resolved || ''
  form.json_firing = opts.json_firing || ''
  form.json_resolved = opts.json_resolved || ''
  form.template = opts.template || ''
  form.http_method = (['POST', 'PUT', 'PATCH'].includes(opts.http_method || '')
    ? (opts.http_method as 'POST' | 'PUT' | 'PATCH')
    : 'POST')
  form.headers_json = opts.headers_json || ''
  form.chat_id = opts.chat_id || ''
  form.msg_type = opts.msg_type === 'markdown' ? 'markdown' : 'text'
  form.at_all = opts.at_all === '1' || opts.at_all === 'true'
  form.at_mobiles = opts.at_mobiles || ''
  form.extraRows = Object.entries(opts)
    .filter(([k]) => !specialKeys.has(k))
    .map(([k, v]) => ({ key: k, value: v }))
  dialogVisible.value = true
}

function openEditRow(row: LocalChannel): void {
  void openEditById(row.id)
}

// ============================================================
// 模板芯片插入：记忆最后聚焦的模板框，点击芯片插入光标处
// ============================================================
const lastTplInput = ref<HTMLInputElement | HTMLTextAreaElement | null>(null)

function markTplFocus(e: FocusEvent): void {
  const el = e.target as HTMLTextAreaElement | HTMLInputElement
  if (el && (el.tagName === 'TEXTAREA' || el.tagName === 'INPUT')) {
    lastTplInput.value = el
  }
}

function insertChip(chip: string): void {
  let target = lastTplInput.value
  if (!target) {
    const selector = showJsonTplSeg.value ? 'textarea[data-role="json_firing"]' : 'textarea[data-role="template_firing"]'
    const el = document.querySelector(selector) as HTMLTextAreaElement | null
    target = el
  }
  if (!target) return
  const token = `{{${chip}}}`
  const el = target
  const start = (el.selectionStart as number) ?? el.value.length
  const end = (el.selectionEnd as number) ?? start
  const before = el.value.slice(0, start)
  const after = el.value.slice(end)
  el.value = before + token + after
  el.dispatchEvent(new Event('input', { bubbles: true }))
  const role = el.getAttribute('data-role') as keyof typeof form | null
  if (role && typeof form[role] === 'string') {
    ;(form[role] as string) = el.value
  }
  nextTick(() => {
    el.focus()
    const pos = start + token.length
    try {
      el.setSelectionRange(pos, pos)
    } catch {
      /* noop */
    }
  })
}

function addExtraRow(): void {
  form.extraRows.push({ key: '', value: '' })
}
function removeExtraRow(idx: number): void {
  form.extraRows.splice(idx, 1)
}

// ============================================================
// buildPayload：严格按照老版本 editChannel 提交逻辑
// ============================================================
function buildPayload(): ChannelInput | null {
  const kind = _normalizeKind(form.kind)
  const options: Record<string, string> = {}

  if (kind !== 'http') {
    const fire = form.template_firing
    const resolved = form.template_resolved
    if (fire.trim()) options.template_firing = fire
    if (resolved.trim()) options.template_resolved = resolved
  } else {
    const jf = form.json_firing
    const jr = form.json_resolved
    if (jf.trim()) {
      try {
        JSON.parse(jf.replace(/\{\{[^}]+\}\}/g, 'null'))
      } catch {
        ElMessage.warning('firing JSON 校验未通过（可能含未渲染的模板变量），仍将按原样保存。')
      }
      options.json_firing = jf
    }
    if (jr.trim()) options.json_resolved = jr
    const method = (form.http_method || 'POST').toUpperCase() as 'POST' | 'PUT' | 'PATCH'
    if (method && method !== 'POST') options.http_method = method
    const headers = form.headers_json.trim()
    if (headers) {
      try {
        const obj = JSON.parse(headers)
        if (!obj || typeof obj !== 'object' || Array.isArray(obj)) {
          ElMessage.error('请求头必须是 JSON 对象')
          return null
        }
        options.headers_json = JSON.stringify(obj)
      } catch {
        ElMessage.error('请求头 JSON 无效')
        return null
      }
    }
  }

  if (form.template.trim()) options.template = form.template
  if (kind !== 'webhook' && kind !== 'http') {
    if (form.msg_type && form.msg_type !== 'text') options.msg_type = form.msg_type
  }
  if (kind === 'dingtalk' || kind === 'wecom') {
    if (form.at_all) options.at_all = '1'
    const mobiles = (form.at_mobiles || '').trim()
    if (mobiles) options.at_mobiles = mobiles
  }
  if (kind === 'telegram') {
    const chatId = (form.chat_id || '').trim()
    if (!chatId) {
      ElMessage.error('Telegram 请填写 chat_id')
      return null
    }
    options.chat_id = chatId
  }

  for (const r of form.extraRows) {
    const k = (r.key || '').trim()
    if (!k) continue
    options[k] = r.value == null ? '' : String(r.value)
  }

  const secret = (form.secret || '').trim()
  return {
    name: form.name.trim(),
    kind,
    url: (form.url || '').trim(),
    secret: secret === '' ? null : secret,
    enabled: form.enabled,
    options: Object.keys(options).length === 0 ? undefined : options,
  }
}

async function saveDialog(): Promise<void> {
  if (!formRef.value) return
  try {
    await formRef.value.validate()
  } catch {
    return
  }
  const payload = buildPayload()
  if (!payload) return
  dialogLoading.value = true
  try {
    if (isEdit.value && editingId.value) {
      await updateChannel(editingId.value, payload)
      ElMessage.success('已更新渠道')
    } else {
      await createChannel(payload)
      ElMessage.success('已创建渠道')
    }
    dialogVisible.value = false
    void loadChannels(true)
  } catch (e) {
    ElMessage.error(errMsgOf(e, isEdit.value ? '更新失败' : '创建失败'))
  } finally {
    dialogLoading.value = false
  }
}

// ============================================================
// 删除
// ============================================================
async function removeChannel(row: LocalChannel): Promise<void> {
  try {
    await ElMessageBox.confirm(`确认删除通知渠道「${row.name}」？此操作不可恢复。`, '删除渠道', {
      type: 'warning',
      confirmButtonText: '删除',
      cancelButtonText: '取消',
    })
  } catch {
    return
  }
  try {
    await deleteChannel(row.id)
    ElMessage.success('已删除')
    void loadChannels(true)
  } catch (e) {
    ElMessage.error(errMsgOf(e, '删除失败'))
  }
}

// ============================================================
// 乐观更新 enabled
// ============================================================
async function onEnabledChange(row: LocalChannel, newValue: boolean): Promise<void> {
  const old = row.enabled
  row.enabled = newValue
  try {
    await updateChannel(row.id, {
      name: row.name,
      kind: row.kind,
      url: row.url,
      secret: row.secret,
      options: row.options,
      enabled: newValue,
    })
  } catch (e) {
    row.enabled = old
    ElMessage.error(errMsgOf(e, '保存启用状态失败，已回滚'))
  }
}

// ============================================================
// 测试 Drawer（列表页 + 弹窗内测试复用）
// ============================================================
const drawerVisible = ref(false)
const drawerLoading = ref(false)
const testResult = ref<ChannelTestResp | null>(null)
const testChannelName = ref('')
const bodyExpanded = ref(false)
const respExpanded = ref(false)

async function runTestById(id: string, name: string): Promise<void> {
  drawerVisible.value = true
  drawerLoading.value = true
  testResult.value = null
  testChannelName.value = name
  bodyExpanded.value = false
  respExpanded.value = false
  try {
    testResult.value = await testChannel(id)
    const r = testResult.value
    ElMessage[r?.ok ? 'success' : 'error'](
      r?.ok ? (r.hint || '测试成功') : (r?.error || '测试失败')
    )
  } catch (e) {
    testResult.value = {
      ok: false,
      error: errMsgOf(e, '调用测试接口失败'),
    }
  } finally {
    drawerLoading.value = false
  }
}

async function runTest(row: LocalChannel): Promise<void> {
  void runTestById(row.id, row.name)
}

async function runDialogTest(): Promise<void> {
  if (!editingId.value) return
  dialogTestLoading.value = true
  try {
    await runTestById(editingId.value, form.name || '(未命名渠道)')
  } finally {
    dialogTestLoading.value = false
  }
}

function longTextBlock(
  text: string | undefined | null,
  expanded: boolean,
  onToggle: () => void,
  limit = 1000
): { shown: string; toggle: (() => void) | null; label: string } {
  if (text == null || text === '') {
    return { shown: '（无）', toggle: null, label: '' }
  }
  if (text.length <= limit) {
    return { shown: text, toggle: null, label: '' }
  }
  if (expanded) {
    return { shown: text, toggle: onToggle, label: '收起' }
  }
  return { shown: text.slice(0, limit) + '…', toggle: onToggle, label: '展开' }
}

const drawerBody = computed(() =>
  longTextBlock(
    prettyJsonStr(testResult.value?.request_body),
    bodyExpanded.value,
    () => (bodyExpanded.value = !bodyExpanded.value),
    1000
  )
)
const drawerResp = computed(() =>
  longTextBlock(
    prettyJsonStr(testResult.value?.response_body),
    respExpanded.value,
    () => (respExpanded.value = !respExpanded.value),
    1000
  )
)

function prettyJsonStr(s: unknown): string | undefined | null {
  if (s == null) return s as undefined | null
  if (typeof s !== 'string') return prettyJson(s)
  return prettyJson(s)
}

// 芯片文案渲染工具：生成 {{chip}} 字面字符串（避免在 template 中被 TS 误报）
function chipLabel(c: string): string {
  return `{{${c}}}`
}

// options 条目数
function optionsCount(row: LocalChannel): number {
  const o = row.options || {}
  return Object.keys(o).length
}
</script>

<template>
  <div class="channels-page">
    <!-- 标题 + 吸附工具栏 -->
    <el-affix :offset="0" class="channels-affix" z-index="5">
      <div class="channels-head">
        <div class="channels-head-left">
          <h2 class="channels-title">
            <el-icon :size="20"><VideoPlay /></el-icon> 通知渠道
          </h2>
          <span class="channels-subtitle">Webhook / HTTP / 钉钉 / 企微 / 飞书 / Slack / Telegram / 邮件 / 短信 / 电话</span>
        </div>
      </div>
    </el-affix>

    <!-- 工具栏 + 筛选 -->
    <div class="panel channels-toolbar" style="padding: 14px 18px;">
      <div class="channels-toolbar-row">
        <el-dropdown trigger="click" @command="(k: string) => openCreate(k)">
          <el-button type="primary" :icon="Plus">
            新建渠道<el-icon class="el-icon--right"><Check /></el-icon>
          </el-button>
          <template #dropdown>
            <el-dropdown-menu>
              <el-dropdown-item
                v-for="k in KINDS"
                :key="String(k.kind)"
                :command="k.kind"
              >
                <span
                  class="kind-dot"
                  :style="{ background: k.color }"
                ></span>
                <span style="margin-left: 8px;">新建 {{ k.label }} 渠道</span>
              </el-dropdown-item>
            </el-dropdown-menu>
          </template>
        </el-dropdown>
        <el-button :icon="Refresh" link @click="loadChannels(true)">刷新</el-button>

        <div class="channels-filters">
          <el-select
            v-model="filterKind"
            placeholder="按类型"
            clearable
            style="width: 140px;"
            size="default"
          >
            <el-option
              v-for="k in KINDS"
              :key="String(k.kind)"
              :value="k.kind"
              :label="k.label"
            />
          </el-select>

          <el-input
            v-model="filterQ"
            placeholder="搜索名称 / URL"
            clearable
            style="width: 240px;"
          />

          <el-select
            v-model="filterEnabled"
            placeholder="启用状态"
            style="width: 140px;"
          >
            <el-option value="all" label="全部" />
            <el-option value="on" label="已启用" />
            <el-option value="off" label="已停用" />
          </el-select>
        </div>
      </div>
    </div>

    <!-- 空态 -->
    <div
      v-if="!loading && filteredChannels.length === 0"
      class="panel empty channels-empty"
    >
      <el-empty description="还没有通知渠道。支持 webhook / http / dingtalk / wecom / feishu / slack / telegram / email / sms / phone。">
        <template #image>
          <el-icon :size="64" color="var(--info-soft)"><VideoPlay /></el-icon>
        </template>
      </el-empty>

      <div class="channels-empty-hints">
        <p><b>飞书 / 钉钉 / 企业微信</b>：直接粘贴「群机器人 Webhook」地址，钉钉/飞书签名机器人再填上 secret；钉钉/企微还能 @所有人 或 @指定手机。</p>
        <p><b>Webhook</b>：固定 Eventide JSON 结构 POST 到该 URL；可自定义文本模板 template_firing / template_resolved。</p>
        <p><b>自定义 HTTP</b>：可自由切换 HTTP 方法、自定义请求头、自定义 JSON body（json_firing / json_resolved）。</p>
        <p><b>Slack / Telegram</b>：粘贴 Slack Webhook 或 Bot Token 基址，Telegram 需额外填 chat_id。</p>
        <p><b>邮件 / 短信 / 电话</b>：邮件 URL 填 smtp://user:pass@host:port；短信/电话填 HTTP 网关或 sip URI；options 补充 smtp_from / smtp_to / call_to / phones 等。</p>
      </div>

      <div style="margin-top: 18px;">
        <el-dropdown trigger="click" @command="(k: string) => openCreate(k)">
          <el-button type="primary" :icon="Plus">
            新建第一个渠道
            <el-icon class="el-icon--right"><Check /></el-icon>
          </el-button>
          <template #dropdown>
            <el-dropdown-menu>
              <el-dropdown-item
                v-for="k in KINDS"
                :key="String(k.kind)"
                :command="k.kind"
              >
                {{ k.label }}
              </el-dropdown-item>
            </el-dropdown-menu>
          </template>
        </el-dropdown>
      </div>
    </div>

    <!-- 列表 -->
    <el-table
      v-else
      v-loading="loading"
      :data="pagedChannels"
      class="panel channels-table"
      style="width: 100%;"
      :header-cell-style="{ background: 'transparent', color: 'var(--el-text-color-secondary)', fontWeight: 500, fontSize: '13px' }"
    >
      <el-table-column prop="name" label="名称" min-width="200">
        <template #default="{ row }">
          <div class="cell-name" @click="openEditRow(row as LocalChannel)">
            {{ (row as LocalChannel).name }}
          </div>
        </template>
      </el-table-column>

      <el-table-column prop="kind" label="类型" width="140">
        <template #default="{ row }">
          <el-tag
            class="kind-tag"
            size="small"
            effect="plain"
            round
            :style="{
              color: kindMetaOf((row as LocalChannel).kind).color,
              borderColor: kindMetaOf((row as LocalChannel).kind).color + '66',
              background: kindMetaOf((row as LocalChannel).kind).bg,
            }"
          >
            {{ kindDisplayOf((row as LocalChannel).kind) }}
          </el-tag>
        </template>
      </el-table-column>

      <el-table-column prop="url" label="URL" min-width="340">
        <template #default="{ row }">
          <div class="cell-url">
            <el-tooltip
              v-if="(row as LocalChannel).url"
              :content="(row as LocalChannel).url"
              placement="top"
              :show-after="200"
            >
              <span class="cell-url-text">{{ truncate((row as LocalChannel).url, 44) }}</span>
            </el-tooltip>
            <span v-else class="cell-url-text">-</span>
            <el-tooltip
              v-if="(row as LocalChannel).url"
              content="复制 URL"
              placement="top"
            >
              <el-button
                link
                type="primary"
                :icon="CopyDocument"
                size="small"
                class="copy-btn"
                @click.stop="copyText((row as LocalChannel).url)"
              />
            </el-tooltip>
          </div>
        </template>
      </el-table-column>

      <el-table-column prop="secret" label="签名/TOKEN" width="120" align="center">
        <template #default="{ row }">
          <el-tag
            size="small"
            round
            effect="plain"
            :type="(row as LocalChannel).secret ? 'success' : 'info'"
            class="secret-tag"
          >
            {{ (row as LocalChannel).secret ? '已设置' : '未设置' }}
          </el-tag>
        </template>
      </el-table-column>

      <el-table-column label="OPTIONS" width="110" align="center">
        <template #default="{ row }">
          <el-popover
            placement="right"
            trigger="hover"
            :show-after="200"
            popper-class="channels-options-popover"
          >
            <template #reference>
              <el-tag
                size="small"
                round
                effect="plain"
                type="info"
                class="opt-tag"
              >
                {{ optionsCount(row as LocalChannel) }} 项
              </el-tag>
            </template>
            <pre
              v-if="optionsCount(row as LocalChannel) > 0"
              class="channels-options-pre"
            >{{ prettyJson((row as LocalChannel).options || {}, 30) }}</pre>
            <div v-else class="opt-empty">（无额外 options）</div>
          </el-popover>
        </template>
      </el-table-column>

      <el-table-column prop="enabled" label="启用" width="100" align="center">
        <template #default="{ row }">
          <el-switch
            v-model="(row as LocalChannel).enabled"
            inline-prompt
            active-text="开"
            inactive-text="关"
            class="ch-switch"
            @change="(v) => onEnabledChange(row as LocalChannel, Boolean(v))"
          />
        </template>
      </el-table-column>

      <el-table-column prop="updated_at" label="更新时间" width="170">
        <template #default="{ row }">
          <span class="cell-updated" :title="(row as LocalChannel).updated_at || ''">{{ formatDateShort((row as LocalChannel).updated_at) }}</span>
        </template>
      </el-table-column>

      <el-table-column label="操作" width="220" align="right" fixed="right" class-name="col-actions">
        <template #default="{ row }">
          <div class="row-actions">
            <el-button
              size="small"
              type="success"
              plain
              round
              class="act-btn act-test"
              @click="runTest(row as LocalChannel)"
            >测试</el-button>
            <el-button
              size="small"
              type="primary"
              plain
              round
              :icon="Edit"
              class="act-btn act-edit"
              @click="openEditRow(row as LocalChannel)"
            >编辑</el-button>
            <el-dropdown trigger="click" @click.stop>
              <el-button circle size="small" class="act-more" :icon="MoreFilled" />
              <template #dropdown>
                <el-dropdown-menu>
                  <el-dropdown-item
                    class="danger-item"
                    @click="removeChannel(row as LocalChannel)"
                  >
                    <el-icon style="margin-right: 6px;"><Delete /></el-icon>删除
                  </el-dropdown-item>
                </el-dropdown-menu>
              </template>
            </el-dropdown>
          </div>
        </template>
      </el-table-column>
    </el-table>

    <div class="panel channels-pager">
      <div class="pager-tip">共 <span class="mono">{{ filteredChannels.length }}</span> 条</div>
      <el-pagination
        v-model:current-page="chPage"
        v-model:page-size="chPageSize"
        :page-sizes="Array.from(CH_PAGE_SIZES)"
        :total="filteredChannels.length"
        layout="sizes, prev, pager, next, jumper, ->, total"
        background
        @current-change="onChCurrentChange"
        @size-change="onChSizeChange"
      />
    </div>

    <!-- 新建/编辑 Dialog -->
    <el-dialog
      v-model="dialogVisible"
      :title="isEdit ? '编辑渠道' : '新建通知渠道'"
      width="820px"
      :close-on-click-modal="false"
      destroy-on-close
      top="6vh"
    >
      <div class="ch-desc">告警边沿触发时，向所选渠道发送通知。可自定义正文模板与消息格式。</div>
      <el-form
        ref="formRef"
        :model="form"
        :rules="rules"
        label-width="130px"
        label-position="right"
      >
        <el-form-item label="名称" prop="name">
          <el-input
            v-model="form.name"
            placeholder="例如：值班钉钉群"
            maxlength="64"
            show-word-limit
          />
        </el-form-item>

        <el-form-item label="类型">
          <el-select v-model="form.kind" style="width: 100%;" :disabled="isEdit">
            <el-option
              v-for="k in KINDS"
              :key="String(k.kind)"
              :value="k.kind"
              :label="k.label"
            />
          </el-select>
        </el-form-item>

        <el-form-item label="URL" prop="url">
          <el-input
            v-model="form.url"
            :placeholder="urlPlaceholder"
            maxlength="512"
            show-word-limit
          />
          <div v-if="urlHint" class="ch-hint">{{ urlHint }}</div>
        </el-form-item>

        <el-form-item v-if="showHttpMethodField" label="HTTP 方法">
          <el-select v-model="form.http_method" style="width: 100%;">
            <el-option value="POST" label="POST" />
            <el-option value="PUT" label="PUT" />
            <el-option value="PATCH" label="PATCH" />
          </el-select>
        </el-form-item>

        <el-form-item v-if="showSecretField" :label="secretLabel">
          <el-input
            v-model="form.secret"
            show-password
            :placeholder="secretPlaceholder"
            maxlength="512"
          />
          <div v-if="secretHint" class="ch-hint">{{ secretHint }}</div>
        </el-form-item>

        <el-form-item v-if="showHeadersField" label="自定义请求头">
          <el-input
            v-model="form.headers_json"
            type="textarea"
            :rows="3"
            placeholder='{"X-Token":"xxx","Content-Type":"application/json"}'
          />
          <div class="ch-hint">JSON 对象格式，保存时会做合法性校验。</div>
        </el-form-item>

        <el-form-item v-if="showChatIdField" label="Telegram chat_id">
          <el-input
            v-model="form.chat_id"
            placeholder="群/用户 ID，如 -100123... 或 123456"
          />
        </el-form-item>

        <el-form-item v-if="showMsgTypeField" label="消息格式">
          <el-select v-model="form.msg_type" style="width: 100%;">
            <el-option value="text" label="纯文本 text" />
            <el-option value="markdown" label="Markdown（钉钉/企微/飞书卡片/Slack）" />
          </el-select>
          <div class="ch-hint">
            企微 Markdown 可用 <code>&lt;font&gt;</code>；飞书为卡片 lark_md；Telegram Markdown 按 HTML 解析。
          </div>
        </el-form-item>

        <el-form-item v-if="showAtField" label="艾特">
          <div class="at-row">
            <el-checkbox v-model="form.at_all">@所有人（钉钉 / 企微 text）</el-checkbox>
          </div>
          <el-input
            v-model="form.at_mobiles"
            style="margin-top: 8px;"
            placeholder="艾特手机号，逗号分隔（钉钉 / 企微 text）"
          />
        </el-form-item>

        <el-form-item label="启用">
          <el-switch
            v-model="form.enabled"
            inline-prompt
            active-text="启用"
            inactive-text="停用"
          />
        </el-form-item>

        <!-- 文本模板段（非 http） -->
        <div v-if="showTextTplSeg" class="ch-seg">
          <div class="ch-seg-title">通知模板</div>
          <div class="ch-hint" style="margin-bottom: 10px;">
            留空则用默认正文。语法与丰富规则相同。点芯片可插入到当前模板框。
          </div>
          <div class="ch-chips">
            <el-button
              v-for="c in TPL_CHIPS"
              :key="c"
              size="small"
              class="chip-tag"
              @click="insertChip(c)"
            >{{ chipLabel(c) }}</el-button>
          </div>
          <el-form-item label="触发通知模板" label-width="130px" style="margin-top: 12px;">
            <el-input
              v-model="form.template_firing"
              type="textarea"
              :rows="6"
              data-role="template_firing"
              :placeholder="PH_FIRING_TPL"
              @focus="markTplFocus"
            />
          </el-form-item>
          <el-form-item label="恢复通知模板" label-width="130px">
            <el-input
              v-model="form.template_resolved"
              type="textarea"
              :rows="5"
              data-role="template_resolved"
              :placeholder="PH_RESOLVED_TPL"
              @focus="markTplFocus"
            />
          </el-form-item>
          <el-form-item label="通用 template" label-width="130px">
            <el-input
              v-model="form.template"
              type="textarea"
              :rows="3"
              data-role="template"
              placeholder="通用 template，未细分 firing/resolved 时走此项"
              @focus="markTplFocus"
            />
          </el-form-item>
        </div>

        <!-- JSON 模板段（http 类型） -->
        <div v-if="showJsonTplSeg" class="ch-seg">
          <div class="ch-seg-title">自定义 JSON 报文</div>
          <div class="ch-hint" style="margin-bottom: 10px;" v-html="HINT_JSON_SEG_HTML()"></div>
          <div class="ch-chips">
            <el-button
              v-for="c in TPL_CHIPS"
              :key="c"
              size="small"
              class="chip-tag"
              @click="insertChip(c)"
            >{{ chipLabel(c) }}</el-button>
          </div>
          <el-form-item label="触发 JSON" label-width="130px" style="margin-top: 12px;">
            <el-input
              v-model="form.json_firing"
              type="textarea"
              :rows="8"
              data-role="json_firing"
              class="mono-area"
              :placeholder="PH_JSON_FIRING"
              @focus="markTplFocus"
            />
          </el-form-item>
          <el-form-item label="恢复 JSON" label-width="130px">
            <el-input
              v-model="form.json_resolved"
              type="textarea"
              :rows="6"
              data-role="json_resolved"
              class="mono-area"
              :placeholder="PH_JSON_RESOLVED"
              @focus="markTplFocus"
            />
          </el-form-item>
        </div>

        <el-divider content-position="left">其它 options（Map&lt;String, String&gt; 行）</el-divider>
        <div
          v-for="(r, idx) in form.extraRows"
          :key="idx"
          class="extra-row"
        >
          <el-input
            v-model="r.key"
            placeholder="key（如 smtp_from / smtp_to / call_to / phones）"
            style="width: 44%;"
          />
          <el-input
            v-model="r.value"
            placeholder="value"
            style="width: 44%; margin-left: 2%;"
          />
          <el-button
            link
            type="danger"
            :icon="Close"
            style="margin-left: 8px;"
            @click="removeExtraRow(idx)"
          >
            移除
          </el-button>
        </div>
        <el-button size="small" plain :icon="Plus" @click="addExtraRow">
          新增一行
        </el-button>
      </el-form>

      <template #footer>
        <el-button @click="dialogVisible = false">取消</el-button>
        <el-button
          v-if="isEdit && editingId"
          type="success"
          plain
          :loading="dialogTestLoading"
          @click="runDialogTest"
        >
          测试发送
        </el-button>
        <el-button
          type="primary"
          :loading="dialogLoading"
          @click="saveDialog"
        >
          保存
        </el-button>
      </template>
    </el-dialog>

    <!-- 测试 Drawer -->
    <el-drawer
      v-model="drawerVisible"
      :title="`渠道测试结果 · ${testChannelName}`"
      direction="rtl"
      size="600px"
      destroy-on-close
    >
      <div v-if="drawerLoading" style="text-align: center; padding: 40px 0;">
        测试中…
      </div>
      <template v-else-if="testResult">
        <div class="test-head">
          <el-tag
            v-if="testResult.ok"
            type="success"
            size="large"
            effect="dark"
            round
          >
            <el-icon><CircleCheck /></el-icon>&nbsp;success
          </el-tag>
          <el-tag
            v-else
            type="danger"
            size="large"
            effect="dark"
            round
          >
            <el-icon><Warning /></el-icon>&nbsp;error
          </el-tag>
          <span v-if="testResult.hint" class="test-hint">{{ testResult.hint }}</span>
        </div>

        <el-descriptions :column="1" border size="default" style="margin-top: 16px;">
          <el-descriptions-item v-if="testResult.error" label="错误">
            <span class="text-danger">{{ testResult.error }}</span>
          </el-descriptions-item>

          <el-descriptions-item v-if="testResult.text" label="发送正文">
            <div class="long-text">
              <pre class="long-pre">{{ prettyJson(testResult.text) }}</pre>
            </div>
          </el-descriptions-item>

          <el-descriptions-item label="method / path">
            {{ testResult.method && testResult.path
              ? `${testResult.method} ${testResult.path}`
              : (testResult.method || testResult.path || '（无）') }}
          </el-descriptions-item>

          <el-descriptions-item label="request_url">
            {{ testResult.request_url || '（无）' }}
          </el-descriptions-item>

          <el-descriptions-item label="request_body">
            <div class="long-text">
              <pre class="long-pre">{{ drawerBody.shown }}</pre>
              <el-button
                v-if="drawerBody.toggle"
                link
                type="primary"
                size="small"
                @click="drawerBody.toggle!()"
              >
                {{ drawerBody.label }}
              </el-button>
            </div>
          </el-descriptions-item>

          <el-descriptions-item label="http_status">
            {{ testResult.http_status == null ? '（无响应）' : testResult.http_status }}
          </el-descriptions-item>

          <el-descriptions-item label="response_body">
            <div class="long-text">
              <pre class="long-pre">{{ drawerResp.shown }}</pre>
              <el-button
                v-if="drawerResp.toggle"
                link
                type="primary"
                size="small"
                @click="drawerResp.toggle!()"
              >
                {{ drawerResp.label }}
              </el-button>
            </div>
          </el-descriptions-item>
        </el-descriptions>
      </template>
    </el-drawer>
  </div>
</template>

<style scoped>
.channels-page {
  padding: 14px 20px 40px;
}
.channels-affix {
  background: var(--el-bg-color, var(--panel));
  border-bottom: 1px solid var(--el-border-color-lighter, var(--line-soft));
  margin: -14px -20px 16px;
  padding: 12px 20px 0;
}
.channels-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
}
.channels-head-left {
  display: flex;
  align-items: baseline;
  gap: 14px;
}
.channels-title {
  font-size: 18px;
  font-weight: 600;
  margin: 0;
  display: inline-flex;
  align-items: center;
  gap: 6px;
}
.channels-subtitle {
  color: var(--el-text-color-secondary, #909399);
  font-size: 13px;
}
.panel {
  background: var(--el-bg-color, #fff);
  border-radius: 8px;
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.03);
  border: 1px solid var(--el-border-color-lighter, #ebeef5);
  margin-bottom: 14px;
}
.channels-toolbar-row {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 12px;
}
.channels-filters {
  margin-left: auto;
  display: inline-flex;
  align-items: center;
  gap: 10px;
}
.kind-dot {
  display: inline-block;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  vertical-align: middle;
}
.channels-empty {
  padding: 28px 20px 32px;
  text-align: center;
}
.channels-empty-hints {
  max-width: 780px;
  margin: 10px auto 0;
  text-align: left;
  color: var(--el-text-color-regular, #606266);
  font-size: 13px;
  line-height: 1.8;
}

/* === 分页条 === */
.channels-pager {
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex-wrap: wrap;
  gap: 12px;
  padding: 12px 18px;
  margin-bottom: 14px;
}
.channels-pager .pager-tip {
  font-size: 13px;
  color: var(--el-text-color-secondary, #909399);
}
.channels-pager .mono {
  font-family: Consolas, Menlo, monospace;
  font-weight: 600;
  color: var(--el-text-color-primary, #303133);
  padding: 0 2px;
}

/* === 列表优化 === */
.channels-table {
  padding: 0;
  overflow: hidden;
  font-size: 13px;
  /* 去掉 Element 默认内边距，面板包裹下更紧凑 */
}
.channels-table :deep(.el-table__inner-wrapper::before) {
  display: none;
}
.channels-table :deep(.el-table th.el-table__cell) {
  background: transparent;
}
.channels-table :deep(.el-table td.el-table__cell),
.channels-table :deep(.el-table th.el-table__cell.is-leaf) {
  border-bottom: 1px solid var(--el-border-color-lighter, #f2f3f5);
}
.channels-table :deep(.el-table th.el-table__cell .cell) {
  padding-top: 4px;
  padding-bottom: 4px;
}
.channels-table :deep(.el-table td.el-table__cell .cell) {
  padding-top: 8px;
  padding-bottom: 8px;
}
.channels-table :deep(.el-table--border .el-table__inner-wrapper::after),
.channels-table :deep(.el-table__inner-wrapper::before) {
  background-color: var(--el-border-color-lighter, #f2f3f5);
}
.cell-name {
  color: var(--el-color-primary, #409eff);
  font-weight: 500;
  cursor: pointer;
  user-select: none;
}
.cell-name:hover {
  text-decoration: underline;
}
.cell-url {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  max-width: 100%;
}
.cell-url-text {
  font-family: Consolas, Menlo, monospace;
  font-size: 12.5px;
  color: var(--el-text-color-regular, #606266);
  word-break: break-all;
}
.copy-btn {
  padding: 0 !important;
  width: 20px;
  height: 20px;
  min-height: 20px !important;
  font-size: 12px;
  margin-left: 2px;
  opacity: 0.75;
  transition: opacity 0.15s;
}
.copy-btn:hover {
  opacity: 1;
}
/* 类型 tag：圆角 + 浅色底，小号紧凑款 */
.kind-tag {
  font-family: Consolas, Menlo, monospace;
  font-size: 11.5px;
  padding: 0 8px;
  height: 22px;
  line-height: 20px;
  font-weight: 500;
  border-radius: 4px;
}
.secret-tag {
  padding: 0 8px;
  height: 22px;
  line-height: 20px;
  font-size: 11.5px;
  border-radius: 4px;
}
.opt-tag {
  padding: 0 8px;
  height: 22px;
  line-height: 20px;
  font-size: 11.5px;
  border-radius: 4px;
  cursor: pointer;
}
.opt-empty {
  color: var(--el-text-color-secondary, #909399);
  font-size: 12px;
  padding: 4px 2px;
}
.cell-updated {
  font-family: Consolas, Menlo, monospace;
  font-size: 12.5px;
  color: var(--el-text-color-secondary, #909399);
}
/* 启用开关：紧凑尺寸，开蓝关灰 */
.ch-switch {
  --el-switch-on-color: var(--el-color-primary, #409eff);
  --el-switch-off-color: var(--el-border-color-darker, #c0c4cc);
}
.channels-table :deep(.ch-switch .el-switch__action) {
  height: 18px;
  width: 18px;
}
.channels-table :deep(.ch-switch) {
  --el-switch-height: 20px;
  --el-switch-width: 40px;
}
/* 操作列：紧凑 pill 按钮 */
.row-actions {
  display: inline-flex;
  align-items: center;
  gap: 6px;
}
.act-btn {
  height: 26px;
  padding: 0 12px !important;
  font-size: 12px !important;
  border-radius: 4px;
}
.act-test {
  --el-button-border-color: #23c27a;
  --el-button-text-color: #19a967;
  --el-button-hover-bg-color: rgba(35, 194, 122, 0.08);
  --el-button-hover-border-color: #19a967;
  --el-button-hover-text-color: #19a967;
  --el-button-active-bg-color: rgba(35, 194, 122, 0.12);
}
.act-edit {
  --el-button-border-color: #3f8bff;
  --el-button-text-color: #3f8bff;
  --el-button-hover-bg-color: rgba(63, 139, 255, 0.08);
  --el-button-hover-border-color: #3077ef;
  --el-button-hover-text-color: #3077ef;
  --el-button-active-bg-color: rgba(63, 139, 255, 0.12);
}
.act-more {
  border: 1px solid var(--el-border-color, #dcdfe6);
  background: transparent;
  color: var(--el-text-color-secondary, #909399);
  width: 24px;
  height: 24px;
  padding: 0;
  border-radius: 4px;
}
.act-more:hover {
  color: var(--el-text-color-primary, #303133);
  border-color: var(--el-border-color, #c0c4cc);
}
.danger-item {
  color: var(--el-color-danger, #f56c6c);
}

.channels-options-pre {
  margin: 0;
  max-width: 520px;
  max-height: 360px;
  overflow: auto;
  padding: 8px 10px;
  background: var(--el-fill-color-light, #f5f7fa);
  border-radius: 4px;
  font-size: 12px;
  line-height: 1.5;
  font-family: Consolas, Menlo, monospace;
}
.extra-row {
  display: flex;
  align-items: center;
  margin-bottom: 8px;
}
.test-head {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
}
.test-hint {
  color: var(--el-text-color-secondary, #909399);
  font-size: 13px;
}
.long-text {
  max-width: 100%;
}
.long-pre {
  margin: 0 0 6px;
  padding: 8px 10px;
  background: var(--el-fill-color-light, #f5f7fa);
  border-radius: 4px;
  white-space: pre-wrap;
  word-break: break-all;
  font-family: Consolas, Menlo, monospace;
  font-size: 12.5px;
  line-height: 1.55;
  max-height: 320px;
  overflow: auto;
}
.text-danger {
  color: var(--el-color-danger, #f56c6c);
}
.empty {
  padding: 24px 20px 28px;
}

/* 表单相关 */
.ch-desc {
  color: var(--el-text-color-secondary, #909399);
  font-size: 13px;
  margin: -4px 0 12px;
}
.ch-hint {
  color: var(--el-text-color-secondary, #909399);
  font-size: 12.5px;
  margin-top: 6px;
  line-height: 1.6;
}
.ch-hint code {
  background: var(--el-fill-color-light, #f5f7fa);
  padding: 1px 5px;
  border-radius: 3px;
  font-family: Consolas, Menlo, monospace;
  font-size: 12px;
}
.ch-seg {
  padding: 14px 14px 2px;
  margin: 10px 0 4px;
  border: 1px dashed var(--el-border-color, #dcdfe6);
  border-radius: 6px;
  background: var(--el-fill-color-lighter, #fafafa);
}
.ch-seg-title {
  font-weight: 600;
  font-size: 14px;
  margin-bottom: 6px;
  color: var(--el-text-color-primary, #303133);
}
.ch-chips {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}
.chip-tag {
  font-family: Consolas, Menlo, monospace;
  font-size: 12px !important;
  padding: 4px 8px !important;
  line-height: 1.4;
}
.at-row {
  display: flex;
  align-items: center;
}
.mono-area :deep(.el-textarea__inner),
textarea.mono-area {
  font-family: Consolas, Menlo, monospace;
  font-size: 12.5px;
}
</style>
