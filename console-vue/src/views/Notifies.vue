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
  ElPagination,
  ElRow,
  ElSelect,
  ElTable,
  ElTableColumn,
  ElTabPane,
  ElTabs,
  ElTag,
  ElTooltip,
  ElIcon,
} from 'element-plus'
import {
  DocumentCopy,
  Refresh,
  View,
  Bell,
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
  page?: number
  limit?: number
}

interface NotifyListRespShim {
  items: NotifyLog[]
  total: number
  page: number
  limit: number
}
interface NotifiesApiShim {
  listNotifies(query: NotifyListQuery): Promise<NotifyLog[] | NotifyListRespShim>
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
// 常量：Transition / ChannelKind 的展示映射 + 柔和配色（与 Channels 列表 tag 同风格）
// ============================================================
type TransitionNormalized = 'BecameFiring' | 'BecameResolved' | 'Escalated' | 'Unchanged'
const TRANSITION_LABEL: Record<TransitionNormalized, string> = {
  BecameFiring: 'BecameFiring',
  BecameResolved: 'BecameResolved',
  Escalated: 'Escalated',
  Unchanged: 'Unchanged',
}
interface TagStyle { color: string; bg: string; border: string }
const TRANSITION_STYLE: Record<TransitionNormalized, TagStyle> = {
  BecameFiring:  { color: '#fa8c16', bg: 'rgba(250,140,22,.10)', border: 'rgba(250,140,22,.35)' },
  BecameResolved:{ color: '#19a967', bg: 'rgba(35,194,122,.10)', border: 'rgba(35,194,122,.35)' },
  Escalated:     { color: '#f5222d', bg: 'rgba(245,34,45,.10)',   border: 'rgba(245,34,45,.35)' },
  Unchanged:     { color: '#8c8c8c', bg: 'rgba(140,140,140,.10)', border: 'rgba(140,140,140,.30)' },
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

type ChannelKindKnown = 'webhook' | 'http' | 'dingtalk' | 'wecom' | 'feishu' | 'lark' | 'slack' | 'telegram' | 'sms' | 'email' | 'phone'
const CHANNEL_STYLE: Record<ChannelKindKnown, TagStyle & { label: string }> = {
  webhook:  { label: 'Webhook',  color: '#722ed1', bg: 'rgba(114,46,209,.10)', border: 'rgba(114,46,209,.30)' },
  http:     { label: 'http',     color: '#fa8c16', bg: 'rgba(250,140,22,.10)', border: 'rgba(250,140,22,.30)' },
  dingtalk: { label: '钉钉',     color: '#2f54eb', bg: 'rgba(47,84,235,.10)',  border: 'rgba(47,84,235,.30)' },
  wecom:    { label: '企微',     color: '#07c160', bg: 'rgba(7,193,96,.10)',   border: 'rgba(7,193,96,.30)' },
  feishu:   { label: '飞书',     color: '#1677ff', bg: 'rgba(22,119,255,.10)', border: 'rgba(22,119,255,.30)' },
  lark:     { label: 'Lark',     color: '#1677ff', bg: 'rgba(22,119,255,.10)', border: 'rgba(22,119,255,.30)' },
  slack:    { label: 'Slack',    color: '#6f42c1', bg: 'rgba(111,66,193,.10)', border: 'rgba(111,66,193,.30)' },
  telegram: { label: 'Telegram', color: '#229ed9', bg: 'rgba(34,158,217,.10)', border: 'rgba(34,158,217,.30)' },
  email:    { label: '邮件',     color: '#13c2c2', bg: 'rgba(19,194,194,.10)', border: 'rgba(19,194,194,.30)' },
  sms:      { label: '短信',     color: '#eb2f96', bg: 'rgba(235,47,150,.10)', border: 'rgba(235,47,150,.30)' },
  phone:    { label: '电话',     color: '#f56c6c', bg: 'rgba(245,108,108,.10)',border: 'rgba(245,108,108,.30)' },
}
const SUCCESS_STYLE: Record<'ok' | 'fail', TagStyle & { label: string }> = {
  ok:   { label: '成功', color: '#19a967', bg: 'rgba(35,194,122,.10)', border: 'rgba(35,194,122,.35)' },
  fail: { label: '失败', color: '#f5222d', bg: 'rgba(245,34,45,.10)',  border: 'rgba(245,34,45,.35)' },
}

function normalizeChannelKind(k: ChannelKind): ChannelKindKnown {
  const kind = String(k || '').toLowerCase()
  if (kind === 'dingtalk' || kind === 'ding_talk') return 'dingtalk'
  if (kind === 'wecom' || kind === 'wechat' || kind === 'wxwork' || kind === 'we_com') return 'wecom'
  if (kind === 'feishu') return 'feishu'
  if (kind === 'lark') return 'lark'
  if (kind === 'slack') return 'slack'
  if (kind === 'telegram') return 'telegram'
  if (kind === 'sms') return 'sms'
  if (kind === 'email' || kind === 'mail') return 'email'
  if (kind === 'phone') return 'phone'
  if (kind === 'http') return 'http'
  if (kind === 'webhook' || kind === 'generic' || !kind) return 'webhook'
  return 'webhook'
}
function channelKindMeta(k: ChannelKind): TagStyle & { label: string } {
  const key = normalizeChannelKind(k)
  return (
    CHANNEL_STYLE[key] ?? {
      label: key.charAt(0).toUpperCase() + key.slice(1),
      color: '#909399',
      bg: 'rgba(144,147,153,.10)',
      border: 'rgba(144,147,153,.30)',
    }
  )
}

// ============================================================
// 工具：时间格式化 / 文本截断 / JSON 美化 / 复制
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

// —— 分页：兼容后端数组 / 分页结构体两种返回
const PAGE_SIZES = [10, 20, 50, 100] as const
const defaultPageSize: number = PAGE_SIZES[1]
const page = ref(1)
const pageSize = ref<number>(defaultPageSize)
type ServerPaging = { total: number; page: number; limit: number }
const serverPaging = ref<ServerPaging | null>(null)
const tableDataRaw = ref<LocalNotifyLog[]>([])
const serverMode = computed(() => !!serverPaging.value)
const total = computed(() => (serverMode.value ? (serverPaging.value?.total ?? 0) : tableDataRaw.value.length))
const pagedRows = computed<LocalNotifyLog[]>(() => {
  if (serverMode.value) return tableDataRaw.value
  const start = (page.value - 1) * pageSize.value
  return tableDataRaw.value.slice(start, start + pageSize.value)
})
const hasAnyRow = computed(() => tableDataRaw.value.length > 0)
function onCurrentChange(p: number): void {
  const target = Math.max(1, p)
  if (serverMode.value) { page.value = target; void refresh(); }
  else { page.value = target; }
}
function onSizeChange(size: number): void {
  pageSize.value = size; page.value = 1;
  if (serverMode.value) void refresh()
}

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
    const useServer = serverMode.value
    const effLimit = useServer
      ? pageSize.value
      : Math.max(pageSize.value * 10, filters.limit || 200, 200)
    const query: NotifyListQuery = { limit: effLimit }
    if (useServer) query.page = page.value
    if (filters.channel_id) query.channel_id = filters.channel_id
    if (filters.q) query.q = filters.q
    if (filters.success !== '') query.success = filters.success as 0 | 1
    const rows = await listNotifies(query)
    let list: LocalNotifyLog[] = []
    let sp: ServerPaging | null = null
    if (Array.isArray(rows)) {
      list = rows as LocalNotifyLog[]
      sp = null
    } else if (rows && typeof rows === 'object' && 'items' in rows && Array.isArray((rows as { items?: unknown }).items)) {
      const r = rows as { items: LocalNotifyLog[]; total?: number; page?: number; limit?: number }
      list = (r.items || []) as LocalNotifyLog[]
      sp = {
        total: typeof r.total === 'number' ? r.total : list.length,
        page: typeof r.page === 'number' ? r.page : page.value,
        limit: typeof r.limit === 'number' ? r.limit : pageSize.value,
      }
      if (typeof sp.page === 'number' && sp.page > 0) page.value = sp.page
    }
    const map = channelMap.value
    for (const row of list) {
      if (!row.channel_name && row.channel_id && map[row.channel_id]) row.channel_name = map[row.channel_id].name
      if (!row.channel_kind && row.channel_id && map[row.channel_id]) row.channel_kind = map[row.channel_id].kind
    }
    serverPaging.value = sp
    tableDataRaw.value = list
    tableData.value = list
    if (!serverMode.value && tableDataRaw.value.length) {
      const maxPage = Math.max(1, Math.ceil(tableDataRaw.value.length / pageSize.value))
      if (page.value > maxPage) page.value = maxPage
    }
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
  // 懒渲染一次：切换显示即视为"加载过一次"
  setTimeout(() => {
    drawerLazyLoaded.value = true
  }, 0)
}

function handleOpenBodyDetail(row: LocalNotifyLog): void {
  handleRowClickView(row)
}

function handleCopyId(value: string, label = 'ID'): void {
  copyText(value, `${label} 已复制`)
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
    <!-- 标题 + 吸附工具栏（与 Channels 保持统一骨架） -->
    <el-affix :offset="0" class="notifies-affix" z-index="5">
      <div class="notifies-head">
        <div class="notifies-head-left">
          <h2 class="notifies-title">
            <el-icon :size="20"><Bell /></el-icon> 通知记录
          </h2>
          <span class="notifies-subtitle">查看最近通知的发送结果、transition、渠道与响应 body</span>
        </div>
      </div>
    </el-affix>

    <!-- 筛选工具栏 -->
    <div class="panel notifies-toolbar" style="padding: 14px 18px;">
      <div class="notifies-toolbar-row">
        <ElButton :icon="Refresh" @click="refresh" :loading="loading">
          刷新
        </ElButton>

        <div class="notifies-filters">
          <ElSelect
            v-model="filters.channel_id"
            filterable
            clearable
            placeholder="按渠道"
            style="width: 220px;"
          >
            <ElOption
              v-for="ch in channelOptions"
              :key="ch.id"
              :value="ch.id"
              :label="`${ch.name}（${channelKindMeta(ch.kind).label}）`"
            >
              <div style="display: flex; align-items: center; justify-content: space-between; gap: 8px">
                <span style="overflow: hidden; text-overflow: ellipsis; white-space: nowrap">
                  {{ ch.name }}
                </span>
                <ElTag
                  size="small"
                  effect="plain"
                  class="ch-kind-mini"
                  :style="{
                    color: channelKindMeta(ch.kind).color,
                    borderColor: channelKindMeta(ch.kind).border,
                    background: channelKindMeta(ch.kind).bg,
                  }"
                >
                  {{ channelKindMeta(ch.kind).label }}
                </ElTag>
              </div>
            </ElOption>
          </ElSelect>

          <ElSelect
            v-model="filters.success"
            clearable
            placeholder="按状态"
            style="width: 140px;"
          >
            <ElOption :value="''" label="全部" />
            <ElOption :value="1" label="成功" />
            <ElOption :value="0" label="失败" />
          </ElSelect>

          <ElInput
            v-model="filters.q"
            clearable
            placeholder="搜索 body / error"
            style="width: 240px;"
          />


        </div>
      </div>
    </div>

    <!-- 空态 -->
    <ElEmpty
      v-if="!loading && !hasAnyRow"
      class="panel empty notifies-empty"
      description="还没有任何通知记录"
    >
      <template #image>
        <el-icon :size="64" color="var(--info-soft)"><Bell /></el-icon>
      </template>
      <ElButton type="primary" @click="handleGoChannels">前往渠道管理</ElButton>
    </ElEmpty>

    <!-- 列表 -->
    <ElTable
      v-else
      v-loading="loading"
      :data="pagedRows"
      class="panel notifies-table"
      style="width: 100%;"
      :header-cell-style="{ background: 'transparent', color: 'var(--el-text-color-secondary)', fontWeight: 500, fontSize: '13px' }"
    >
      <!-- 时间 -->
      <ElTableColumn label="时间" width="180" fixed>
        <template #default="{ row }">
          <ElTooltip :content="String((row as LocalNotifyLog).created_at ?? '-')" placement="top" :show-after="150">
            <span class="cell-created">{{ formatLocalDateTime((row as LocalNotifyLog).created_at) }}</span>
          </ElTooltip>
        </template>
      </ElTableColumn>

      <!-- transition -->
      <ElTableColumn label="Transition" width="160">
        <template #default="{ row }">
          <ElTag
            class="nf-tag nf-transition"
            size="small"
            effect="plain"
            :style="{
              color: TRANSITION_STYLE[normalizeTransition((row as LocalNotifyLog).transition)].color,
              borderColor: TRANSITION_STYLE[normalizeTransition((row as LocalNotifyLog).transition)].border,
              background: TRANSITION_STYLE[normalizeTransition((row as LocalNotifyLog).transition)].bg,
            }"
          >
            {{ transitionText((row as LocalNotifyLog).transition) }}
          </ElTag>
        </template>
      </ElTableColumn>

      <!-- 渠道类型 -->
      <ElTableColumn label="渠道类型" width="110">
        <template #default="{ row }">
          <ElTag
            class="nf-tag nf-kind"
            size="small"
            effect="plain"
            :style="{
              color: channelKindMeta((row as LocalNotifyLog).channel_kind || 'webhook').color,
              borderColor: channelKindMeta((row as LocalNotifyLog).channel_kind || 'webhook').border,
              background: channelKindMeta((row as LocalNotifyLog).channel_kind || 'webhook').bg,
            }"
          >
            {{ channelKindMeta((row as LocalNotifyLog).channel_kind || 'webhook').label }}
          </ElTag>
        </template>
      </ElTableColumn>

      <!-- 渠道名称 -->
      <ElTableColumn label="渠道名称" min-width="180">
        <template #default="{ row }">
          <ElTooltip
            v-if="(row as LocalNotifyLog).channel_name"
            :content="String((row as LocalNotifyLog).channel_name ?? '')"
            placement="top"
            :show-after="200"
          >
            <span class="cell-channel">{{ truncate((row as LocalNotifyLog).channel_name, 28) }}</span>
          </ElTooltip>
          <span v-else class="cell-dim">-</span>
        </template>
      </ElTableColumn>

      <!-- success -->
      <ElTableColumn label="状态" width="100" align="center">
        <template #default="{ row }">
          <ElTag
            class="nf-tag nf-success"
            size="small"
            effect="plain"
            :style="{
              color: (row as LocalNotifyLog).success ? SUCCESS_STYLE.ok.color : SUCCESS_STYLE.fail.color,
              borderColor: (row as LocalNotifyLog).success ? SUCCESS_STYLE.ok.border : SUCCESS_STYLE.fail.border,
              background: (row as LocalNotifyLog).success ? SUCCESS_STYLE.ok.bg : SUCCESS_STYLE.fail.bg,
            }"
          >
            {{ (row as LocalNotifyLog).success ? SUCCESS_STYLE.ok.label : SUCCESS_STYLE.fail.label }}
          </ElTag>
        </template>
      </ElTableColumn>

      <!-- error -->
      <ElTableColumn label="错误信息" min-width="240">
        <template #default="{ row }">
          <template v-if="(row as LocalNotifyLog).error">
            <div class="cell-error-wrap">
              <ElTooltip
                :content="String((row as LocalNotifyLog).error ?? '')"
                placement="top"
                :show-after="200"
              >
                <span class="cell-error">
                  {{ truncate((row as LocalNotifyLog).error, 72) }}
                </span>
              </ElTooltip>
              <ElTooltip content="复制错误信息" placement="top">
                <ElButton
                  link
                  type="danger"
                  :icon="DocumentCopy"
                  size="small"
                  class="cell-copy"
                  @click.stop="handleCopyId((row as LocalNotifyLog).error || '', 'error')"
                />
              </ElTooltip>
            </div>
          </template>
          <span v-else class="cell-dim">-</span>
        </template>
      </ElTableColumn>

      <!-- body -->
      <ElTableColumn label="通知正文" min-width="320">
        <template #default="{ row }">
          <div class="cell-body-wrap">
            <ElTooltip
              :content="String((row as LocalNotifyLog).body ?? '-')"
              placement="top"
              :show-after="300"
            >
              <span class="cell-body">{{ truncate((row as LocalNotifyLog).body, 88) || '-' }}</span>
            </ElTooltip>
            <ElButton
              size="small"
              link
              type="primary"
              class="cell-view-btn"
              @click.stop="handleOpenBodyDetail(asLocalNotifyLog(row))"
            >查看</ElButton>
          </div>
        </template>
      </ElTableColumn>

      <!-- 操作 -->
      <ElTableColumn label="操作" width="180" align="right" fixed="right" class-name="col-actions">
        <template #default="{ row }">
          <div class="row-actions">
            <ElButton
              size="small"
              type="primary"
              plain
              class="act-btn act-view"
              :icon="View"
              @click="handleRowClickView(asLocalNotifyLog(row))"
            >查看</ElButton>
            <ElTooltip content="复制 alert_id" placement="top">
              <ElButton
                circle
                size="small"
                class="act-copyid"
                :icon="DocumentCopy"
                :disabled="!(row as LocalNotifyLog).alert_id"
                @click="handleCopyId((row as LocalNotifyLog).alert_id || '', 'alert_id')"
              />
            </ElTooltip>
          </div>
        </template>
      </ElTableColumn>
    </ElTable>

    <div class="panel notifies-pager">
      <div class="pager-tip">
        <template v-if="serverMode">总计 <span class="mono">{{ total }}</span> 条（后端分页）</template>
        <template v-else-if="total >= filters.limit">已加载最近 <span class="mono">{{ total }}</span> 条，前端分页</template>
        <template v-else>共 <span class="mono">{{ total }}</span> 条</template>
      </div>
      <ElPagination
        v-model:current-page="page"
        v-model:page-size="pageSize"
        :page-sizes="Array.from(PAGE_SIZES)"
        :total="total"
        layout="sizes, prev, pager, next, jumper, ->, total"
        background
        @current-change="onCurrentChange"
        @size-change="onSizeChange"
      />
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
            <span class="mono">{{ formatLocalDateTime(currentItem.created_at) }}</span>
            <span style="margin-left: 6px; color: var(--el-text-color-secondary); font-size: 12px">
              ({{ currentItem.created_at || '-' }})
            </span>
          </ElDescriptionsItem>

          <ElDescriptionsItem label="Transition">
            <ElTag
              size="large"
              effect="plain"
              style="font-weight: 600; padding: 6px 14px;"
              :style="{
                color: TRANSITION_STYLE[normalizeTransition(currentItem.transition)].color,
                borderColor: TRANSITION_STYLE[normalizeTransition(currentItem.transition)].border,
                background: TRANSITION_STYLE[normalizeTransition(currentItem.transition)].bg,
              }"
            >
              {{ transitionText(currentItem.transition) }}
            </ElTag>
          </ElDescriptionsItem>

          <ElDescriptionsItem label="渠道">
            <ElTag
              size="large"
              effect="plain"
              style="font-weight: 600; padding: 6px 14px;"
              :style="{
                color: channelKindMeta(currentItem.channel_kind || 'webhook').color,
                borderColor: channelKindMeta(currentItem.channel_kind || 'webhook').border,
                background: channelKindMeta(currentItem.channel_kind || 'webhook').bg,
              }"
            >
              {{ channelKindMeta(currentItem.channel_kind || 'webhook').label }}
            </ElTag>
            <span style="margin-left: 8px">{{ currentItem.channel_name || '-' }}</span>
            <span style="margin-left: 8px; color: var(--el-text-color-secondary); font-size: 12px">
              channel_id: {{ currentItem.channel_id || '-' }}
            </span>
          </ElDescriptionsItem>

          <ElDescriptionsItem label="状态">
            <ElTag
              size="large"
              effect="plain"
              style="font-weight: 600; padding: 6px 14px;"
              :style="{
                color: currentItem.success ? SUCCESS_STYLE.ok.color : SUCCESS_STYLE.fail.color,
                borderColor: currentItem.success ? SUCCESS_STYLE.ok.border : SUCCESS_STYLE.fail.border,
                background: currentItem.success ? SUCCESS_STYLE.ok.bg : SUCCESS_STYLE.fail.bg,
              }"
            >
              {{ currentItem.success ? SUCCESS_STYLE.ok.label : SUCCESS_STYLE.fail.label }}
            </ElTag>
          </ElDescriptionsItem>

          <ElDescriptionsItem label="错误">
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
              <span style="margin-left: 6px; word-break: break-word; white-space: pre-wrap; color: var(--el-color-danger);">
                {{ currentItem.error }}
              </span>
            </template>
            <span v-else>无</span>
          </ElDescriptionsItem>

          <ElDescriptionsItem label="alert_id">
            <span class="mono" style="word-break: break-all">
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
            <span class="mono" style="word-break: break-all">{{ currentItem.id || '-' }}</span>
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
  padding: 14px 20px 40px;
}
/* === 吸附标题栏（与 Channels 一致） === */
.notifies-affix {
  background: var(--el-bg-color, var(--panel));
  border-bottom: 1px solid var(--el-border-color-lighter, var(--line-soft));
  margin: -14px -20px 16px;
  padding: 12px 20px 0;
}
.notifies-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
}
.notifies-head-left {
  display: flex;
  align-items: baseline;
  gap: 14px;
}
.notifies-title {
  font-size: 18px;
  font-weight: 600;
  margin: 0;
  display: inline-flex;
  align-items: center;
  gap: 6px;
}
.notifies-subtitle {
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
.notifies-toolbar-row {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 12px;
}
.notifies-filters {
  margin-left: auto;
  display: inline-flex;
  align-items: center;
  gap: 10px;
}
.ch-kind-mini {
  font-size: 11.5px;
  padding: 0 6px;
  height: 20px;
  line-height: 18px;
  border-radius: 4px;
  font-family: Consolas, Menlo, monospace;
}
.empty {
  padding: 28px 20px 32px;
  text-align: center;
}

/* === 列表样式（与 Channels 一致） === */
.notifies-table {
  padding: 0;
  overflow: hidden;
  font-size: 13px;
}
.notifies-table :deep(.el-table__inner-wrapper::before) {
  display: none;
}
.notifies-table :deep(.el-table th.el-table__cell) {
  background: transparent;
}
.notifies-table :deep(.el-table td.el-table__cell),
.notifies-table :deep(.el-table th.el-table__cell.is-leaf) {
  border-bottom: 1px solid var(--el-border-color-lighter, #f2f3f5);
}
.notifies-table :deep(.el-table th.el-table__cell .cell) {
  padding-top: 4px;
  padding-bottom: 4px;
}
.notifies-table :deep(.el-table td.el-table__cell .cell) {
  padding-top: 8px;
  padding-bottom: 8px;
}

/* === 单元格 === */
.cell-created {
  font-family: Consolas, Menlo, monospace;
  font-size: 12.5px;
  color: var(--el-text-color-secondary, #909399);
}
.cell-channel {
  font-weight: 500;
  color: var(--el-text-color-regular, #606266);
}
.cell-dim {
  color: var(--el-text-color-secondary, #c0c4cc);
  font-size: 12.5px;
}
.cell-error-wrap,
.cell-body-wrap {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  max-width: 100%;
}
.cell-error {
  color: var(--el-color-danger, #f5222d);
  font-size: 12.5px;
  word-break: break-all;
}
.cell-body {
  font-family: Consolas, Menlo, monospace;
  font-size: 12.5px;
  color: var(--el-text-color-regular, #606266);
  word-break: break-all;
}
.cell-view-btn {
  padding-left: 4px !important;
  padding-right: 4px !important;
  font-size: 12px;
}
.cell-copy {
  padding: 0 !important;
  width: 20px;
  height: 20px;
  min-height: 20px !important;
  opacity: 0.75;
}
.cell-copy:hover {
  opacity: 1;
}

/* === Tag：柔和 pill（统一风格） === */
.nf-tag {
  font-family: Consolas, Menlo, monospace;
  font-size: 11.5px;
  padding: 0 8px;
  height: 22px;
  line-height: 20px;
  font-weight: 500;
  border-radius: 4px;
}
.nf-transition { letter-spacing: 0.1px; }
.nf-kind { font-family: inherit; }
.nf-success { font-family: inherit; }

/* === 操作列 === */
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
.act-view {
  --el-button-border-color: #3f8bff;
  --el-button-text-color: #3f8bff;
  --el-button-hover-bg-color: rgba(63, 139, 255, 0.08);
  --el-button-hover-border-color: #3077ef;
  --el-button-hover-text-color: #3077ef;
  --el-button-active-bg-color: rgba(63, 139, 255, 0.12);
}
.act-copyid {
  border: 1px solid var(--el-border-color, #dcdfe6);
  background: transparent;
  color: var(--el-text-color-secondary, #909399);
  width: 24px;
  height: 24px;
  padding: 0;
  border-radius: 4px;
}
.act-copyid:hover:not(:disabled) {
  color: var(--el-text-color-primary, #303133);
  border-color: var(--el-border-color, #c0c4cc);
}

/* === 分页条（与 Channels 一致） === */
.notifies-pager {
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex-wrap: wrap;
  gap: 12px;
  padding: 12px 18px;
  margin-bottom: 14px;
}
.pager-tip {
  font-size: 13px;
  color: var(--el-text-color-secondary, #909399);
}
.notifies-pager .mono {
  font-family: Consolas, Menlo, monospace;
  font-weight: 600;
  color: var(--el-text-color-primary, #303133);
  padding: 0 2px;
}

/* === Drawer === */
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
  font-family: Consolas, Menlo, monospace;
  font-size: 12px;
  line-height: 1.55;
  white-space: pre-wrap;
  word-break: break-word;
}
.body-pre code {
  background: transparent;
  padding: 0;
}
.mono {
  font-family: Consolas, Menlo, monospace;
}
</style>
