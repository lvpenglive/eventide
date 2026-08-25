<script setup lang="ts">
/* =========================================================
 * Channels.vue —— 通知渠道视图（Vue 3 + TS + Element Plus）
 * 单文件独立实现，不改动其他任何文件；Shim/真实模块双模式。
 * ========================================================= */

import { computed, markRaw, onMounted, reactive, ref } from 'vue'
import {
  ElAffix,
  ElButton,
  ElCollapse,
  ElCollapseItem,
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
} from 'element-plus'
import type { FormInstance, FormRules } from 'element-plus'
import {
  Check,
  CircleCheck,
  CopyDocument,
  Delete,
  Edit,
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
  | 'feishu'
  | 'dingtalk'
  | 'wecom'
  | 'webhook'
  | 'email'
  | 'sms'
  | 'phone'
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

// @ts-ignore 若 @/api/channels 模块尚未创建（并行任务进行中）则忽略解析错误，运行时走 shim
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

function prettyJson(obj: unknown, maxLines = 20): string {
  try {
    const pretty = JSON.stringify(obj, null, 2)
    const lines = pretty.split('\n')
    if (lines.length > maxLines) {
      return lines.slice(0, maxLines).join('\n') + `\n… （共 ${lines.length} 行，已截断）`
    }
    return pretty
  } catch {
    return String(obj)
  }
}

// ============================================================
// Kind 元信息
// ============================================================
interface KindMeta {
  kind: ChannelKind
  label: string
  color: string
  urlPlaceholder: string
}
const KINDS: KindMeta[] = [
  { kind: 'feishu', label: '飞书', color: '#1677ff', urlPlaceholder: '自定义机器人 webhook' },
  { kind: 'dingtalk', label: '钉钉', color: '#2f54eb', urlPlaceholder: '钉钉机器人签名 URL' },
  { kind: 'wecom', label: '企业微信', color: '#07c160', urlPlaceholder: '企业微信机器人 URL' },
  { kind: 'webhook', label: 'Webhook', color: '#722ed1', urlPlaceholder: '任意 HTTP(S) POST URL' },
  { kind: 'email', label: '邮件', color: '#13c2c2', urlPlaceholder: 'smtp://user:pass@host:port' },
  { kind: 'sms', label: '短信', color: '#eb2f96', urlPlaceholder: 'HTTP 网关或 sdk 标识 URL' },
  { kind: 'phone', label: '电话', color: '#f56c6c', urlPlaceholder: 'HTTP 网关或 sip URI' },
]

function kindMetaOf(k: ChannelKind): KindMeta {
  return (
    KINDS.find((x) => x.kind === k) || {
      kind: k,
      label: String(k || '-'),
      color: '#909399',
      urlPlaceholder: '请填写渠道访问地址',
    }
  )
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

const filteredChannels = computed<LocalChannel[]>(() => {
  const q = filterQ.value.trim().toLowerCase()
  return channels.value.filter((c) => {
    if (filterKind.value && c.kind !== filterKind.value) return false
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
const formRef = ref<FormInstance>()

const form = reactive<{
  name: string
  kind: ChannelKind
  url: string
  secret: string
  enabled: boolean
  template_firing: string
  template_resolved: string
  json_firing: string
  json_resolved: string
  template: string
  extraRows: Array<{ key: string; value: string }>
}>({
  name: '',
  kind: 'feishu',
  url: '',
  secret: '',
  enabled: true,
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

const urlPlaceholder = computed(() => kindMetaOf(form.kind).urlPlaceholder)

function resetForm(): void {
  form.name = ''
  form.kind = 'feishu'
  form.url = ''
  form.secret = ''
  form.enabled = true
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
    // 若 getChannel 未实现，尝试在本地列表里找
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
  form.kind = c.kind
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
  ])
  form.template_firing = opts.template_firing || ''
  form.template_resolved = opts.template_resolved || ''
  form.json_firing = opts.json_firing || ''
  form.json_resolved = opts.json_resolved || ''
  form.template = opts.template || ''
  form.extraRows = Object.entries(opts)
    .filter(([k]) => !specialKeys.has(k))
    .map(([k, v]) => ({ key: k, value: v }))
  dialogVisible.value = true
}

function openEditRow(row: LocalChannel): void {
  void openEditById(row.id)
}

function addExtraRow(): void {
  form.extraRows.push({ key: '', value: '' })
}
function removeExtraRow(idx: number): void {
  form.extraRows.splice(idx, 1)
}

function buildPayload(): ChannelInput {
  const options: Record<string, string> = {}
  const specials: Array<[string, string]> = [
    ['template_firing', form.template_firing],
    ['template_resolved', form.template_resolved],
    ['json_firing', form.json_firing],
    ['json_resolved', form.json_resolved],
    ['template', form.template],
  ]
  for (const [k, v] of specials) {
    if (v != null && v !== '') options[k] = v
  }
  for (const r of form.extraRows) {
    const k = (r.key || '').trim()
    if (!k) continue
    options[k] = r.value == null ? '' : String(r.value)
  }
  return {
    name: form.name.trim(),
    kind: form.kind,
    url: form.url.trim(),
    secret: form.secret === '' ? null : form.secret,
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
// 测试 Drawer
// ============================================================
const drawerVisible = ref(false)
const drawerLoading = ref(false)
const testResult = ref<ChannelTestResp | null>(null)
const testChannelName = ref('')
const bodyExpanded = ref(false)
const respExpanded = ref(false)

async function runTest(row: LocalChannel): Promise<void> {
  drawerVisible.value = true
  drawerLoading.value = true
  testResult.value = null
  testChannelName.value = row.name
  bodyExpanded.value = false
  respExpanded.value = false
  try {
    testResult.value = await testChannel(row.id)
  } catch (e) {
    testResult.value = {
      ok: false,
      error: errMsgOf(e, '调用测试接口失败'),
    }
  } finally {
    drawerLoading.value = false
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
    testResult.value?.request_body,
    bodyExpanded.value,
    () => (bodyExpanded.value = !bodyExpanded.value),
    1000
  )
)
const drawerResp = computed(() =>
  longTextBlock(
    testResult.value?.response_body,
    respExpanded.value,
    () => (respExpanded.value = !respExpanded.value),
    1000
  )
)
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
          <span class="channels-subtitle">飞书 / 钉钉 / 企业微信 / Webhook / 邮件 / 短信 / 电话</span>
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
                <el-tag size="small" effect="dark" :color="k.color" style="border: none; color: #fff;">
                  {{ k.label }}
                </el-tag>
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
      v-if="!loading && channels.length === 0"
      class="panel empty channels-empty"
    >
      <el-empty description="还没有任何通知渠道，先创建一个以开始接收告警：">
        <template #image>
          <el-icon :size="64" color="var(--info-soft)"><VideoPlay /></el-icon>
        </template>
      </el-empty>

      <div class="channels-empty-hints">
        <p><b>飞书 / 钉钉 / 企业微信</b>：直接粘贴「群机器人 Webhook」地址，钉钉/飞书签名机器人再填上 secret。</p>
        <p><b>Webhook</b>：任意 HTTP(S) POST 目标，可自定义 json_firing / json_resolved body 模板。</p>
        <p><b>邮件</b>：URL 填 smtp://user:pass@host:port；options 中补充 smtp_from / smtp_to。</p>
        <p><b>短信 / 电话</b>：填写 HTTP 网关或 sip URI；options 中补充 call_to / phones 等参数。</p>
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
      :data="filteredChannels"
      stripe
      border
      class="panel channels-table"
      style="width: 100%;"
    >
      <el-table-column prop="name" label="名称" min-width="180">
        <template #default="{ row }">
          <el-link type="primary" @click="openEditRow(row as LocalChannel)">
            {{ (row as LocalChannel).name }}
          </el-link>
        </template>
      </el-table-column>

      <el-table-column prop="kind" label="类型" width="130">
        <template #default="{ row }">
          <el-tag
            size="small"
            effect="dark"
            :color="kindMetaOf((row as LocalChannel).kind).color"
            style="border: none; color: #fff;"
          >
            {{ kindMetaOf((row as LocalChannel).kind).label }}
          </el-tag>
        </template>
      </el-table-column>

      <el-table-column prop="url" label="URL" min-width="320">
        <template #default="{ row }">
          <div class="cell-url">
            <el-tooltip
              v-if="(row as LocalChannel).url"
              :content="(row as LocalChannel).url"
              placement="top"
              :show-after="200"
            >
              <span class="cell-url-text">{{ truncate((row as LocalChannel).url, 40) }}</span>
            </el-tooltip>
            <span v-else class="cell-url-text">-</span>
            <el-button
              v-if="(row as LocalChannel).url"
              link
              type="primary"
              :icon="CopyDocument"
              size="small"
              @click="copyText((row as LocalChannel).url)"
            >
              复制
            </el-button>
          </div>
        </template>
      </el-table-column>

      <el-table-column prop="secret" label="签名" width="110" align="center">
        <template #default="{ row }">
          <el-tag
            size="small"
            :type="(row as LocalChannel).secret ? 'success' : 'info'"
          >
            {{ (row as LocalChannel).secret ? '已设置' : '未设置' }}
          </el-tag>
        </template>
      </el-table-column>

      <el-table-column label="options" min-width="180">
        <template #default="{ row }">
          <el-popover
            placement="right"
            trigger="hover"
            :show-after="300"
            popper-class="channels-options-popover"
          >
            <template #reference>
              <el-tag size="small" effect="plain" type="info">
                {{ Object.keys((row as LocalChannel).options || {}).length }} 项
              </el-tag>
            </template>
            <pre class="channels-options-pre">{{
              prettyJson((row as LocalChannel).options || {}, 20)
            }}</pre>
          </el-popover>
        </template>
      </el-table-column>

      <el-table-column prop="enabled" label="启用" width="90" align="center">
        <template #default="{ row }">
          <el-switch
            v-model="(row as LocalChannel).enabled"
            inline-prompt
            active-text="开"
            inactive-text="关"
            @change="(v) => onEnabledChange(row as LocalChannel, Boolean(v))"
          />
        </template>
      </el-table-column>

      <el-table-column prop="updated_at" label="更新时间" width="170">
        <template #default="{ row }">
          {{ (row as LocalChannel).updated_at || '-' }}
        </template>
      </el-table-column>

      <el-table-column label="操作" width="220" align="right" fixed="right">
        <template #default="{ row }">
          <el-button
            size="small"
            type="success"
            plain
            @click="runTest(row as LocalChannel)"
          >
            测试
          </el-button>
          <el-button
            size="small"
            type="primary"
            plain
            :icon="Edit"
            @click="openEditRow(row as LocalChannel)"
          >
            编辑
          </el-button>
          <el-button
            size="small"
            type="danger"
            plain
            :icon="Delete"
            @click="removeChannel(row as LocalChannel)"
          >
            删除
          </el-button>
        </template>
      </el-table-column>
    </el-table>

    <!-- 新建/编辑 Dialog -->
    <el-dialog
      v-model="dialogVisible"
      :title="isEdit ? '编辑通知渠道' : '新建通知渠道'"
      width="720px"
      :close-on-click-modal="false"
      destroy-on-close
    >
      <el-form
        ref="formRef"
        :model="form"
        :rules="rules"
        label-width="110px"
        label-position="right"
      >
        <el-form-item label="名称" prop="name">
          <el-input
            v-model="form.name"
            placeholder="渠道名称，用于列表识别"
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
        </el-form-item>

        <el-form-item label="签名 secret">
          <el-input
            v-model="form.secret"
            show-password
            placeholder="钉钉/飞书 signed robot 使用；其他渠道可空"
            maxlength="256"
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

        <el-form-item label="高级 options">
          <el-collapse style="width: 100%;">
            <el-collapse-item title="展开：模板与自定义参数" name="adv">
              <el-form-item label="template_firing" label-width="150px">
                <el-input
                  v-model="form.template_firing"
                  type="textarea"
                  :rows="3"
                  placeholder="告警触发通知模板，支持 {{labels.*}} / {{annotations.summary}}"
                />
              </el-form-item>
              <el-form-item label="template_resolved" label-width="150px">
                <el-input
                  v-model="form.template_resolved"
                  type="textarea"
                  :rows="3"
                  placeholder="告警恢复通知模板，支持 {{labels.*}} / {{annotations.summary}}"
                />
              </el-form-item>
              <el-form-item label="json_firing" label-width="150px">
                <el-input
                  v-model="form.json_firing"
                  type="textarea"
                  :rows="3"
                  placeholder="自定义 Webhook firing JSON body 模板；邮件/短信/电话不适用可留空"
                />
              </el-form-item>
              <el-form-item label="json_resolved" label-width="150px">
                <el-input
                  v-model="form.json_resolved"
                  type="textarea"
                  :rows="3"
                  placeholder="自定义 Webhook resolved JSON body 模板；邮件/短信/电话不适用可留空"
                />
              </el-form-item>
              <el-form-item label="通用 template" label-width="150px">
                <el-input
                  v-model="form.template"
                  type="textarea"
                  :rows="3"
                  placeholder="通用 template，未细分 firing/resolved 时走此项"
                />
              </el-form-item>

              <el-divider content-position="left">其它 options（Map&lt;String,String&gt; 行）</el-divider>
              <div
                v-for="(r, idx) in form.extraRows"
                :key="idx"
                class="extra-row"
              >
                <el-input
                  v-model="r.key"
                  placeholder="key（如 smtp_from / smtp_to / call_to）"
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
            </el-collapse-item>
          </el-collapse>
        </el-form-item>
      </el-form>

      <template #footer>
        <el-button @click="dialogVisible = false">取消</el-button>
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
      size="560px"
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
        </div>

        <el-descriptions :column="1" border size="default" style="margin-top: 16px;">
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
            {{ testResult.http_status == null ? '（无）' : testResult.http_status }}
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

          <el-descriptions-item label="error">
            <span :class="{ 'text-danger': !!testResult.error }">
              {{ testResult.error || '（无）' }}
            </span>
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
  background: var(--bg-color, #fff);
  border-bottom: 1px solid var(--el-border-color-lighter, #ebeef5);
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
  border-radius: 6px;
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
.channels-empty {
  padding: 28px 20px 32px;
  text-align: center;
}
.channels-empty-hints {
  max-width: 760px;
  margin: 10px auto 0;
  text-align: left;
  color: var(--el-text-color-regular, #606266);
  font-size: 13px;
  line-height: 1.8;
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
.channels-table {
  padding: 0;
  overflow: hidden;
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
  font-size: 12.5px;
  line-height: 1.55;
  max-height: 280px;
  overflow: auto;
}
.text-danger {
  color: var(--el-color-danger, #f56c6c);
}
.empty {
  padding: 24px 20px 28px;
}
</style>
