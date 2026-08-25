<script setup lang="ts">
import { computed, onMounted, reactive, ref, markRaw } from 'vue'
import { useRouter } from 'vue-router'
import {
  ElAffix,
  ElButton,
  ElCard,
  ElCol,
  ElDescriptions,
  ElDescriptionsItem,
  ElDivider,
  ElDrawer,
  ElEmpty,
  ElInput,
  ElInputNumber,
  ElMessage,
  ElOption,
  ElRow,
  ElSelect,
  ElTable,
  ElTableColumn,
  ElTabPane,
  ElTabs,
  ElTag,
  ElTooltip,
} from 'element-plus'
import {
  DocumentCopy,
  Refresh,
  View,
} from '@element-plus/icons-vue'

import type { AlertTransition, ChannelKind, NotifyChannel, NotifyLog } from '@/api/types'

// ============================================================
// 本地扩展类型：补齐与 channel 字典回填后的字段
// ============================================================
interface LocalNotifyLog extends NotifyLog {
  channel_kind?: ChannelKind
  channel_name?: string
}

type NotifyListQuery = {
  channel_id?: string
  success?: 0 | 1 | ''
  q?: string
  limit?: number
}

interface NotifiesApiShim {
  listNotifies(query: NotifyListQuery): Promise<NotifyLog[]>
}
interface ChannelsApiShim {
  listAllChannels(): Promise<NotifyChannel[]>
}
interface CommonApiShim {
  listChannels(): Promise<NotifyChannel[]>
}

function _unimpl(name: string): never {
  throw new Error(`[API shim] 模块未实现：${name}`)
}

const _shimNotifies: NotifiesApiShim = {
  listNotifies: () => Promise.reject(_unimpl('listNotifies')),
}
const _shimChannels: ChannelsApiShim = {
  listAllChannels: () => Promise.reject(_unimpl('listAllChannels')),
}
const _shimCommon: CommonApiShim = {
  listChannels: () => Promise.reject(_unimpl('listChannels')),
}

// 真实模块：若模块尚未落地，@ts-ignore 抑制解析错误，运行时走 shim
// @ts-ignore
import * as _rawNotifies from '@/api/notifies'
// @ts-ignore
import * as _rawChannels from '@/api/channels'
// @ts-ignore
import * as _rawCommon from '@/api/common'

const _notifies = markRaw(_rawNotifies as unknown as NotifiesApiShim | Record<string, unknown>)
const _channels = markRaw(_rawChannels as unknown as ChannelsApiShim | Record<string, unknown>)
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

const _notifiesApi = _bindApi<Record<string, unknown>, NotifiesApiShim>(
  _notifies as unknown as Record<string, unknown>,
  _shimNotifies,
)
const _channelsApi = _bindApi<Record<string, unknown>, ChannelsApiShim>(
  _channels as unknown as Record<string, unknown>,
  _shimChannels,
)
const _commonApi = _bindApi<Record<string, unknown>, CommonApiShim>(
  _common as unknown as Record<string, unknown>,
  _shimCommon,
)

const { listNotifies } = _notifiesApi

// ============================================================
// 常量：Transition / ChannelKind 的展示映射
// ============================================================
type TransitionNormalized = 'BecameFiring' | 'BecameResolved' | 'Escalated' | 'Unchanged'
const TRANSITION_LABEL: Record<TransitionNormalized, string> = {
  BecameFiring: 'BecameFiring',
  BecameResolved: 'BecameResolved',
  Escalated: 'Escalated',
  Unchanged: 'Unchanged',
}
const TRANSITION_TAG_TYPE: Record<TransitionNormalized, 'warning' | 'success' | 'danger' | 'info'> = {
  BecameFiring: 'warning',
  BecameResolved: 'success',
  Escalated: 'danger',
  Unchanged: 'info',
}
function normalizeTransition(t: AlertTransition): TransitionNormalized {
  const v = String(t || '').toLowerCase().replace(/[^a-z]/g, '')
  if (v.includes('becamefiring')) return 'BecameFiring'
  if (v === 'becamefiring' || v === 'becamefire' || v.includes('fire')) {
    if (v.includes('recover') || v.includes('resolve')) return 'BecameResolved'
    return 'BecameFiring'
  }
  if (v.includes('becameresolved') || v.includes('resolv') || v.includes('recover')) return 'BecameResolved'
  if (v.includes('escalat')) return 'Escalated'
  if (v.includes('unchang')) return 'Unchanged'
  // 原始值回退匹配
  const raw = String(t || '')
  if (/became[_\s-]*firing/i.test(raw)) return 'BecameFiring'
  if (/became[_\s-]*resolved/i.test(raw)) return 'BecameResolved'
  if (/escalated/i.test(raw)) return 'Escalated'
  if (/unchanged/i.test(raw)) return 'Unchanged'
  return 'Unchanged'
}
function transitionText(t: AlertTransition): string {
  const raw = String(t || '').trim()
  if (!raw) return TRANSITION_LABEL.Unchanged
  const norm = normalizeTransition(t)
  // 保留用户自定义/后端原文的可读性：若非标准 4 类但能被归一，则仍显示归一标签；否则显示原值
  if (raw === norm.toLowerCase() || raw === norm || raw.includes('_')) {
    const pieces = raw.split(/[_\s-]+/).filter(Boolean)
    if (pieces.length) {
      return pieces
        .map((p) => (p ? p.charAt(0).toUpperCase() + p.slice(1).toLowerCase() : ''))
        .join('')
    }
  }
  return TRANSITION_LABEL[norm] ?? raw
}

type ChannelKindKnown = 'webhook' | 'dingtalk' | 'wecom' | 'feishu' | 'lark' | 'sms' | 'email'
const CHANNEL_LABEL: Record<ChannelKindKnown, string> = {
  webhook: 'Webhook',
  dingtalk: '钉钉',
  wecom: '企业微信',
  feishu: '飞书',
  lark: 'Lark',
  sms: '短信',
  email: '邮件',
}
const CHANNEL_TAG_TYPE: Record<ChannelKindKnown, 'primary' | 'success' | 'warning' | 'danger' | 'info'> = {
  webhook: 'primary',
  dingtalk: 'warning',
  wecom: 'success',
  feishu: 'info',
  lark: 'info',
  sms: 'danger',
  email: 'warning',
}
function normalizeChannelKind(k: ChannelKind): ChannelKindKnown {
  const kind = String(k || '').toLowerCase()
  if (kind === 'dingtalk') return 'dingtalk'
  if (kind === 'wecom' || kind === 'wechat' || kind === 'wxwork') return 'wecom'
  if (kind === 'feishu') return 'feishu'
  if (kind === 'lark') return 'lark'
  if (kind === 'sms') return 'sms'
  if (kind === 'email' || kind === 'mail') return 'email'
  if (kind === 'webhook' || kind === 'generic' || !kind) return 'webhook'
  return 'webhook'
}
function channelKindLabel(k: ChannelKind): string {
  const kind = String(k || '').toLowerCase()
  const known = ['webhook', 'dingtalk', 'wecom', 'feishu', 'lark', 'sms', 'email'] as const
  if (known.includes(kind as ChannelKindKnown)) {
    return CHANNEL_LABEL[kind as ChannelKindKnown]
  }
  // 未知 kind 做 title-case
  if (!kind) return 'Webhook'
  return kind.charAt(0).toUpperCase() + kind.slice(1)
}
function channelKindTagType(k: ChannelKind): 'primary' | 'success' | 'warning' | 'danger' | 'info' {
  return CHANNEL_TAG_TYPE[normalizeChannelKind(k)] ?? 'primary'
}

// ============================================================
// 工具：时间格式化 / 文本截断 / JSON 美化
// ============================================================
function pad(n: number, w = 2): string {
  return n.toString().padStart(w, '0')
}
function formatLocalDateTime(iso: string | null | undefined): string {
  if (!iso) return '-'
  const d = new Date(iso)
  if (Number.isNaN(d.getTime())) return String(iso)
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(
    d.getHours(),
  )}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`
}
function truncate(s: string | null | undefined, max: number): string {
  if (s == null) return ''
  const str = String(s)
  if (str.length <= max) return str
  return str.slice(0, max) + '…'
}
function prettifyJsonMaybe(body: string): string {
  if (!body) return ''
  const t = body.trim()
  if (!(t.startsWith('{') || t.startsWith('['))) return body
  try {
    return JSON.stringify(JSON.parse(body), null, 2)
  } catch {
    return body
  }
}

// ============================================================
// 页面状态
// ============================================================
const router = useRouter()

const filters = reactive<{
  channel_id: string
  success: '' | 0 | 1
  q: string
  limit: number
}>({
  channel_id: '',
  success: '',
  q: '',
  limit: 100,
})

const loading = ref(false)
const tableData = ref<LocalNotifyLog[]>([])
const channelOptions = ref<NotifyChannel[]>([])
const channelMap = computed<Record<string, NotifyChannel>>(() => {
  const out: Record<string, NotifyChannel> = {}
  for (const c of channelOptions.value) out[c.id] = c
  return out
})

const drawerVisible = ref(false)
const drawerLazyLoaded = ref(false)
const currentItem = ref<LocalNotifyLog | null>(null)
const bodyTab = ref<'text' | 'raw'>('text')

const formattedBodyText = computed(() => (currentItem.value ? prettifyJsonMaybe(currentItem.value.body) : ''))
const rawBody = computed(() => currentItem.value?.body ?? '')

// ============================================================
// 数据加载
// ============================================================
async function loadChannels(): Promise<NotifyChannel[]> {
  try {
    const res = await _channelsApi.listAllChannels()
    if (Array.isArray(res)) return res
  } catch {
    // 忽略，走下一个兜底
  }
  try {
    const res = await _commonApi.listChannels()
    if (Array.isArray(res)) return res
  } catch {
    // 最终兜底：空数组
  }
  return []
}

async function loadChannelsIfEmpty(): Promise<void> {
  if (channelOptions.value.length > 0) return
  try {
    channelOptions.value = await loadChannels()
  } catch (e) {
    // 兜底空数组
    channelOptions.value = []
    console.warn('[Notifies] 加载渠道字典失败，回退空数组：', e)
  }
}

async function refresh(): Promise<void> {
  loading.value = true
  try {
    await loadChannelsIfEmpty()
    const query: NotifyListQuery = {
      limit: filters.limit || 100,
    }
    if (filters.channel_id) query.channel_id = filters.channel_id
    if (filters.q) query.q = filters.q
    if (filters.success !== '') query.success = filters.success as 0 | 1
    const rows = await listNotifies(query)
    const list: LocalNotifyLog[] = Array.isArray(rows) ? (rows as LocalNotifyLog[]) : []
    // 回填渠道字典
    const map = channelMap.value
    for (const row of list) {
      if (!row.channel_name && row.channel_id && map[row.channel_id]) {
        row.channel_name = map[row.channel_id].name
      }
      if (!row.channel_kind && row.channel_id && map[row.channel_id]) {
        row.channel_kind = map[row.channel_id].kind
      }
    }
    tableData.value = list
  } catch (e: unknown) {
    const msg = e instanceof Error ? e.message : String(e)
    ElMessage.error(`加载通知记录失败：${msg}`)
    tableData.value = []
  } finally {
    loading.value = false
  }
}

function asLocalNotifyLog(row: unknown): LocalNotifyLog {
  return row as LocalNotifyLog
}

function handleRowClickView(row: LocalNotifyLog): void {
  currentItem.value = row
  drawerLazyLoaded.value = false
  drawerVisible.value = true
  // 懒渲染一次：切换显示即视为“加载过一次”
  setTimeout(() => {
    drawerLazyLoaded.value = true
  }, 0)
}

function handleOpenBodyDetail(row: LocalNotifyLog): void {
  handleRowClickView(row)
}

async function handleCopyId(value: string, label = 'ID'): Promise<void> {
  try {
    if (navigator.clipboard && window.isSecureContext) {
      await navigator.clipboard.writeText(value)
    } else {
      const ta = document.createElement('textarea')
      ta.value = value
      ta.style.position = 'fixed'
      ta.style.opacity = '0'
      document.body.appendChild(ta)
      ta.select()
      document.execCommand('copy')
      document.body.removeChild(ta)
    }
    ElMessage.success(`${label} 已复制`)
  } catch (e: unknown) {
    const msg = e instanceof Error ? e.message : String(e)
    ElMessage.error(`复制失败：${msg}`)
  }
}

function handleGoChannels(): void {
  try {
    router.push('/channels')
  } catch {
    // 兜底：静默；router 路径未注册时避免崩溃
    Promise.resolve()
  }
}

onMounted(() => {
  void refresh()
})
</script>

<template>
  <div class="notifies-page">
    <div class="page-header">
      <h2 class="page-title">通知记录</h2>
      <p class="page-subtitle">最近 100/自定义条目的通知发送结果</p>
    </div>

    <ElAffix class="notifies-affix">
      <ElCard shadow="never" class="filter-card">
        <ElRow :gutter="16" align="middle">
          <ElCol :span="6">
            <label class="filter-label">channel_id</label>
            <ElSelect
              v-model="filters.channel_id"
              filterable
              clearable
              placeholder="选择渠道"
              class="filter-select"
              style="width: 100%"
            >
              <ElOption
                v-for="ch in channelOptions"
                :key="ch.id"
                :value="ch.id"
                :label="`${ch.name}（${channelKindLabel(ch.kind)}）`"
              >
                <div style="display: flex; align-items: center; justify-content: space-between; gap: 8px">
                  <span style="overflow: hidden; text-overflow: ellipsis; white-space: nowrap">
                    {{ ch.name }}
                  </span>
                  <ElTag :type="channelKindTagType(ch.kind)" size="small" effect="light">
                    {{ channelKindLabel(ch.kind) }}
                  </ElTag>
                </div>
              </ElOption>
            </ElSelect>
          </ElCol>
          <ElCol :span="6">
            <label class="filter-label">success</label>
            <ElSelect
              v-model="filters.success"
              clearable
              placeholder="全部 / 成功 / 失败"
              style="width: 100%"
            >
              <ElOption :value="''" label="全部" />
              <ElOption :value="1" label="成功" />
              <ElOption :value="0" label="失败" />
            </ElSelect>
          </ElCol>
          <ElCol :span="6">
            <label class="filter-label">q</label>
            <ElInput
              v-model="filters.q"
              clearable
              placeholder="搜索 body/error 文本"
            />
          </ElCol>
          <ElCol :span="6">
            <ElRow :gutter="8" align="middle" wrap="nowrap">
              <ElCol :flex="'min-content'">
                <label class="filter-label">limit</label>
                <ElInputNumber
                  v-model="filters.limit"
                  :min="10"
                  :max="500"
                  :step="10"
                  controls-position="right"
                  style="width: 140px"
                />
              </ElCol>
              <ElCol :flex="'auto'" style="text-align: right">
                <ElButton
                  type="primary"
                  plain
                  :loading="loading"
                  @click="refresh"
                >
                  <template #icon><Refresh /></template>
                  刷新
                </ElButton>
              </ElCol>
            </ElRow>
          </ElCol>
        </ElRow>
      </ElCard>
    </ElAffix>

    <div class="table-wrap">
      <ElEmpty v-if="!loading && tableData.length === 0" description="还没有任何通知记录">
        <ElButton type="primary" @click="handleGoChannels">前往渠道管理</ElButton>
      </ElEmpty>

      <ElTable
        v-else
        v-loading="loading"
        :data="tableData"
        stripe
        border
        style="width: 100%"
        :virtualized="false"
        empty-text="还没有任何通知记录"
      >
        <ElTableColumn label="created_at" width="180" fixed>
          <template #default="{ row }">
            <ElTooltip :content="row.created_at || '-'" placement="top">
              <span>{{ formatLocalDateTime(row.created_at) }}</span>
            </ElTooltip>
          </template>
        </ElTableColumn>

        <ElTableColumn label="transition" width="160">
          <template #default="{ row }">
            <ElTag :type="TRANSITION_TAG_TYPE[normalizeTransition(row.transition)]">
              {{ transitionText(row.transition) }}
            </ElTag>
          </template>
        </ElTableColumn>

        <ElTableColumn label="channel_kind" width="120">
          <template #default="{ row }">
            <ElTag
              :type="channelKindTagType(row.channel_kind || 'webhook')"
              effect="light"
            >
              {{ channelKindLabel(row.channel_kind || 'webhook') }}
            </ElTag>
          </template>
        </ElTableColumn>

        <ElTableColumn label="channel_name" min-width="180" show-overflow-tooltip>
          <template #default="{ row }">
            <span>{{ row.channel_name || '-' }}</span>
          </template>
        </ElTableColumn>

        <ElTableColumn label="success" width="100">
          <template #default="{ row }">
            <ElTag :type="row.success ? 'success' : 'danger'">
              {{ row.success ? '成功' : '失败' }}
            </ElTag>
          </template>
        </ElTableColumn>

        <ElTableColumn label="error" min-width="220" show-overflow-tooltip>
          <template #default="{ row }">
            <template v-if="row.error">
              <ElTooltip :content="row.error" placement="top" :show-after="200">
                <span class="error-cell">
                  {{ truncate(row.error, 80) || '-' }}
                </span>
              </ElTooltip>
            </template>
            <span v-else>-</span>
          </template>
        </ElTableColumn>

        <ElTableColumn label="body" min-width="320">
          <template #default="{ row }">
            <ElTooltip
              :content="row.body || '-'"
              placement="top"
              :show-after="300"
            >
              <span class="body-cell">{{ truncate(row.body, 100) || '-' }}</span>
            </ElTooltip>
            <span style="margin-left: 6px">
              <ElButton size="small" link type="primary" @click="handleOpenBodyDetail(asLocalNotifyLog(row))">
                查看完整
              </ElButton>
            </span>
          </template>
        </ElTableColumn>

        <ElTableColumn label="action" width="200" fixed="right">
          <template #default="{ row }">
            <ElButton size="small" @click="handleRowClickView(asLocalNotifyLog(row))">
              <template #icon><View /></template>
              查看
            </ElButton>
            <ElButton
              size="small"
              :disabled="!row.alert_id"
              @click="handleCopyId(row.alert_id || '', 'alert_id')"
            >
              <template #icon><DocumentCopy /></template>
              复制 alert_id
            </ElButton>
          </template>
        </ElTableColumn>
      </ElTable>
    </div>

    <ElDrawer
      v-model="drawerVisible"
      :title="currentItem ? `通知详情 ${String(currentItem.id || '').slice(0, 8)}` : '通知详情'"
      direction="rtl"
      size="560px"
      destroy-on-close
    >
      <template v-if="drawerLazyLoaded && currentItem">
        <ElDescriptions :column="1" border size="default">
          <ElDescriptionsItem label="时间">
            <span>{{ formatLocalDateTime(currentItem.created_at) }}</span>
            <span style="margin-left: 6px; color: var(--el-text-color-secondary); font-size: 12px">
              ({{ currentItem.created_at || '-' }})
            </span>
          </ElDescriptionsItem>

          <ElDescriptionsItem label="transition">
            <ElTag :type="TRANSITION_TAG_TYPE[normalizeTransition(currentItem.transition)]">
              {{ transitionText(currentItem.transition) }}
            </ElTag>
          </ElDescriptionsItem>

          <ElDescriptionsItem label="渠道">
            <ElTag :type="channelKindTagType(currentItem.channel_kind || 'webhook')" effect="light">
              {{ channelKindLabel(currentItem.channel_kind || 'webhook') }}
            </ElTag>
            <span style="margin-left: 8px">{{ currentItem.channel_name || '-' }}</span>
            <span style="margin-left: 8px; color: var(--el-text-color-secondary); font-size: 12px">
              channel_id: {{ currentItem.channel_id || '-' }}
            </span>
          </ElDescriptionsItem>

          <ElDescriptionsItem label="success">
            <ElTag :type="currentItem.success ? 'success' : 'danger'">
              {{ currentItem.success ? '成功' : '失败' }}
            </ElTag>
          </ElDescriptionsItem>

          <ElDescriptionsItem label="error">
            <template v-if="currentItem.error">
              <ElButton
                size="small"
                link
                type="primary"
                @click="handleCopyId(currentItem.error || '', 'error')"
              >
                <template #icon><DocumentCopy /></template>
                复制
              </ElButton>
              <span style="margin-left: 6px; word-break: break-word; white-space: pre-wrap">
                {{ currentItem.error }}
              </span>
            </template>
            <span v-else>无</span>
          </ElDescriptionsItem>

          <ElDescriptionsItem label="alert_id">
            <span style="word-break: break-all">
              {{ currentItem.alert_id ? currentItem.alert_id : '-' }}
            </span>
            <ElButton
              size="small"
              link
              type="primary"
              :disabled="!currentItem.alert_id"
              style="margin-left: 8px"
              @click="handleCopyId(currentItem.alert_id || '', 'alert_id')"
            >
              <template #icon><DocumentCopy /></template>
              复制
            </ElButton>
          </ElDescriptionsItem>

          <ElDescriptionsItem label="id">
            <span style="word-break: break-all">{{ currentItem.id || '-' }}</span>
            <ElButton
              size="small"
              link
              type="primary"
              style="margin-left: 8px"
              @click="handleCopyId(currentItem.id || '', 'id')"
            >
              <template #icon><DocumentCopy /></template>
              复制
            </ElButton>
          </ElDescriptionsItem>
        </ElDescriptions>

        <ElDivider />

        <section class="drawer-section">
          <div class="drawer-section-title">渲染 Body</div>
          <ElCard shadow="never" class="body-card">
            <ElTabs v-model="bodyTab" class="body-tabs">
              <ElTabPane label="文本" name="text">
                <pre class="body-pre"><code>{{ formattedBodyText || '-' }}</code></pre>
              </ElTabPane>
              <ElTabPane label="原始字节" name="raw">
                <pre class="body-pre"><code>{{ rawBody || '-' }}</code></pre>
              </ElTabPane>
            </ElTabs>
          </ElCard>
        </section>
      </template>
    </ElDrawer>
  </div>
</template>

<style scoped>
.notifies-page {
  padding: 16px 24px 24px;
}
.page-header {
  margin-bottom: 12px;
}
.page-title {
  margin: 0 0 4px;
  font-size: 22px;
  font-weight: 600;
  line-height: 1.3;
}
.page-subtitle {
  margin: 0;
  color: var(--el-text-color-secondary);
  font-size: 13px;
}
.notifies-affix {
  z-index: 10;
}
.filter-card {
  padding: 12px 8px 4px;
  background-color: var(--el-bg-color-page);
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 6px;
}
.filter-label {
  display: inline-block;
  margin-right: 8px;
  margin-bottom: 4px;
  font-size: 12px;
  color: var(--el-text-color-secondary);
}
.filter-select :deep(.el-select) {
  width: 100%;
}
.table-wrap {
  margin-top: 12px;
}
.error-cell {
  color: var(--el-color-danger);
  word-break: break-all;
}
.body-cell {
  color: var(--el-text-color-regular);
  word-break: break-all;
}
.drawer-section-title {
  margin: 0 0 8px;
  font-size: 14px;
  font-weight: 600;
}
.body-card {
  border-radius: 6px;
}
.body-tabs :deep(.el-tabs__header) {
  margin-bottom: 8px;
}
.body-pre {
  margin: 0;
  padding: 12px;
  background: var(--el-fill-color-light);
  border: 1px solid var(--el-border-color-lighter);
  border-radius: 4px;
  max-height: 480px;
  overflow: auto;
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, 'Liberation Mono', 'Courier New', monospace;
  font-size: 12px;
  line-height: 1.55;
  white-space: pre-wrap;
  word-break: break-word;
}
.body-pre code {
  background: transparent;
  padding: 0;
}
</style>
