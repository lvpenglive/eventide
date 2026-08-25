<script setup lang="ts">
/* =========================================================
 * Maintenance.vue —— 维护窗视图（Vue 3 + TS + Element Plus）
 * 批次 2：维护窗。单文件独立实现，不改动其他文件。
 * 本文件内所有类型仅用于补齐 vue-tsc 严格校验；
 * 与 @/api/maintenance 的模块契约按任务要求对齐。
 * ========================================================= */

import { computed, markRaw, onBeforeUnmount, onMounted, reactive, ref, watch } from 'vue'

// --- Element Plus 按需 ---
import {
  ElAffix,
  ElButton,
  ElCard,
  ElCol,
  ElDatePicker,
  ElDialog,
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
  Plus,
  Refresh,
  Search,
  Setting,
} from '@element-plus/icons-vue'

// --- 类型（本地补齐，不修改 api/types.ts） ---
import type { MaintenanceWindow as _TsMaintenanceWindow } from '@/api/types'

export interface LocalMaintenance {
  id: string
  name: string
  enabled: boolean
  rule_id: string | null
  matchers: Record<string, string> | Array<{ key: string; value: string; operator?: '=' | '!=' | '=~' | '!~' | string }>
  starts_at: string
  ends_at: string
  comment: string
  created_at: string
  updated_at: string
  created_by?: string | null
}

export interface LocalMaintenanceInput {
  name: string
  enabled: boolean
  rule_id?: string | null
  matchers: Array<{ key: string; value: string }>
  starts_at: string
  ends_at: string
  comment: string
}

type RequiredKeys<T, K extends keyof T> = T

type Maintenance = _TsMaintenanceWindow extends object
  ? _TsMaintenanceWindow & LocalMaintenance
  : LocalMaintenance

interface RuleOption { id: string; name?: string }

// --- API Shim：优先 @/api/maintenance，其次用 common.listMaintenanceWindows ---
interface MaintenanceApiShim {
  listMaintenance(): Promise<Maintenance[]>
  getMaintenance(id: string): Promise<Maintenance>
  createMaintenance(body: LocalMaintenanceInput): Promise<Maintenance>
  updateMaintenance(id: string, body: Partial<LocalMaintenanceInput> & { enabled?: boolean }): Promise<Maintenance>
  deleteMaintenance(id: string): Promise<{ ok: boolean }>
  listRules?(): Promise<RuleOption[]>
}

function _unimpl(name: string): never {
  throw new Error(`[API shim] 模块未实现：${name}，请等待并行任务创建对应 api/*.ts`)
}
const _shim: MaintenanceApiShim = {
  listMaintenance: () => Promise.reject(_unimpl('listMaintenance')),
  getMaintenance: () => Promise.reject(_unimpl('getMaintenance')),
  createMaintenance: () => Promise.reject(_unimpl('createMaintenance')),
  updateMaintenance: () => Promise.reject(_unimpl('updateMaintenance')),
  deleteMaintenance: () => Promise.reject(_unimpl('deleteMaintenance')),
  listRules: () => Promise.resolve([] as RuleOption[]),
}

// 真实 import：若模块尚未创建，使用 @ts-ignore 抑制找不到模块错误，运行时走 shim。
// @ts-ignore 若 @/api/maintenance 模块尚未创建（并行任务进行中）则忽略解析错误
import * as _raw from '@/api/maintenance'
// @ts-ignore 若 @/api/common 模块尚未创建则忽略解析错误
import * as _rawCommon from '@/api/common'
const _rawMaint = markRaw(_raw as unknown as MaintenanceApiShim | Record<string, unknown>)
const _rawCmn = markRaw(_rawCommon as unknown as { listMaintenanceWindows?: unknown } | Record<string, unknown>)

function _bindApi<A extends object, S>(api: A | Record<string, unknown>, shim: S): S {
  const out = { ...(shim as unknown as object) } as S
  for (const key of Object.keys(shim as unknown as object)) {
    if (key in api && typeof (api as Record<string, unknown>)[key] === 'function') {
      ;(out as unknown as Record<string, unknown>)[key] = (api as Record<string, unknown>)[key]
    }
  }
  return out
}
let _api = _bindApi<Record<string, unknown>, MaintenanceApiShim>(
  _rawMaint as unknown as Record<string, unknown>,
  _shim,
)
// fallback listMaintenance -> common.listMaintenanceWindows
if (_api.listMaintenance === _shim.listMaintenance && typeof _rawCmn.listMaintenanceWindows === 'function') {
  _api.listMaintenance = _rawCmn.listMaintenanceWindows as () => Promise<Maintenance[]>
}
const { listMaintenance, getMaintenance, createMaintenance, updateMaintenance, deleteMaintenance, listRules } = _api

function errMsgOf(e: unknown, fallback: string): string {
  if (e && typeof e === 'object') {
    const anyE = e as { message?: unknown }
    if (typeof anyE.message === 'string') return anyE.message
  }
  return fallback
}

// =================================================================
// 运行时状态
// =================================================================
const loading = ref(false)
const list = ref<Maintenance[]>([])
const filterEnabled = ref<'all' | 'on' | 'off'>('all')
const filterQ = ref('')

// 规则选项（来自 listRules 或 shim 空列表，并集包含「全部规则」）
const ruleOptions = ref<RuleOption[]>([])
const loadingRules = ref(false)

// Dialog 状态
const dialogVisible = ref(false)
const dialogMode = ref<'create' | 'edit'>('create')
const editingId = ref<string | null>(null)
const submitting = ref(false)

const formRef = ref<FormInstance | null>(null)
const formData = reactive<{
  name: string
  enabled: boolean
  rule_id: string | null
  range: [string, string] | null
  matchers: Array<{ key: string; value: string }>
  comment: string
}>({
  name: '',
  enabled: true,
  rule_id: null,
  range: null,
  matchers: [{ key: '', value: '' }],
  comment: '',
})

// 时间感知（每 30 秒刷新状态徽标的剩余时间）
const tick = ref(Date.now())
let tickTimer: number | null = null

// =================================================================
// 过滤器
// =================================================================
const filteredList = computed(() => {
  const q = filterQ.value.trim().toLowerCase()
  return list.value.filter((row) => {
    if (filterEnabled.value === 'on' && !row.enabled) return false
    if (filterEnabled.value === 'off' && row.enabled) return false
    if (q) {
      const hay = `${row.name || ''} ${row.comment || ''}`.toLowerCase()
      if (!hay.includes(q)) return false
    }
    return true
  })
})

// =================================================================
// 工具函数：时间/状态
// =================================================================
function pad(n: number): string {
  return String(n).padStart(2, '0')
}
function fmtMinute(ts: string | null | undefined): string {
  if (!ts) return '-'
  const d = new Date(ts)
  if (Number.isNaN(d.getTime())) return String(ts)
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`
}

type MWStatus = 'ongoing' | 'pending' | 'ended'
function mwStatusOf(row: Maintenance): MWStatus {
  const now = tick.value
  const s = new Date(row.starts_at).getTime()
  const e = new Date(row.ends_at).getTime()
  if (now < s) return 'pending'
  if (now >= e) return 'ended'
  return 'ongoing'
}
function mwStatusMeta(st: MWStatus): { text: string; type: 'primary' | 'success' | 'info' | 'warning' | 'danger' } {
  if (st === 'ongoing') return { text: '进行中', type: 'success' }
  if (st === 'pending') return { text: '未开始', type: 'warning' }
  return { text: '已结束', type: 'info' }
}
function fmtRemaining(row: Maintenance): string {
  const st = mwStatusOf(row)
  const now = tick.value
  const s = new Date(row.starts_at).getTime()
  const e = new Date(row.ends_at).getTime()
  let ms = 0
  if (st === 'pending') ms = s - now
  else if (st === 'ongoing') ms = e - now
  else ms = now - e
  if (ms < 0) ms = 0
  const diff = Math.floor(ms / 1000)
  if (st === 'ended') {
    const ago = diff
    if (ago < 60) return `${ago}s 前结束`
    if (ago < 3600) return `${Math.floor(ago / 60)}m 前结束`
    if (ago < 86400) return `${Math.floor(ago / 3600)}h 前结束`
    return `${Math.floor(ago / 86400)}d 前结束`
  }
  if (diff < 60) return st === 'ongoing' ? `剩余 ${diff}s` : `${diff}s 后开始`
  if (diff < 3600) {
    const m = Math.floor(diff / 60)
    const ss = diff % 60
    return st === 'ongoing' ? `剩余 ${m}m ${ss}s` : `${m}m ${ss}s 后开始`
  }
  if (diff < 48 * 3600) {
    const h = Math.floor(diff / 3600)
    const m = Math.floor((diff % 3600) / 60)
    return st === 'ongoing' ? `剩余 ${h}h ${m}m` : `${h}h ${m}m 后开始`
  }
  const d = Math.floor(diff / 86400)
  const h = Math.floor((diff % 86400) / 3600)
  return st === 'ongoing' ? `剩余 ${d}d ${h}h` : `${d}d ${h}h 后开始`
}

// =================================================================
// 匹配器：支持 Record<string,string> 或 [{key,value}] 两种形状
// =================================================================
function matcherEntries(row: Maintenance): Array<{ key: string; value: string }> {
  const m = row.matchers
  if (!m) return []
  if (Array.isArray(m)) {
    return m.map((x) => ({
      key: (x as { key: string }).key || '',
      value: (x as { value: string }).value || '',
    })).filter((e) => e.key || e.value)
  }
  return Object.entries(m as Record<string, string>).map(([k, v]) => ({ key: k, value: String(v) }))
}
function shortUuid(id: string | null | undefined): string {
  if (!id) return ''
  const s = String(id).replace(/[^A-Za-z0-9]/g, '')
  return s.slice(0, 8) || String(id).slice(0, 8)
}

// =================================================================
// 列表加载
// =================================================================
async function loadList(): Promise<void> {
  loading.value = true
  try {
    const rows = await listMaintenance()
    list.value = Array.isArray(rows) ? (rows as Maintenance[]) : []
  } catch (e) {
    ElMessage.error(errMsgOf(e, '加载维护窗列表失败'))
    list.value = []
  } finally {
    loading.value = false
  }
}

async function loadRules(): Promise<void> {
  if (typeof listRules !== 'function') {
    ruleOptions.value = []
    return
  }
  loadingRules.value = true
  try {
    const r = await listRules()
    ruleOptions.value = Array.isArray(r) ? (r as RuleOption[]) : []
  } catch {
    ruleOptions.value = []
  } finally {
    loadingRules.value = false
  }
}

// =================================================================
// 新建 / 编辑 / 删除
// =================================================================
function openCreate(): void {
  dialogMode.value = 'create'
  editingId.value = null
  const now = new Date()
  const end = new Date(now.getTime() + 4 * 3600 * 1000)
  const toIsoZ = (d: Date) => {
    function p(n: number) { return String(n).padStart(2, '0') }
    return `${d.getUTCFullYear()}-${p(d.getUTCMonth() + 1)}-${p(d.getUTCDate())}T${p(d.getUTCHours())}:${p(d.getUTCMinutes())}:${p(d.getUTCSeconds())}Z`
  }
  formData.name = ''
  formData.enabled = true
  formData.rule_id = null
  formData.range = [toIsoZ(now), toIsoZ(end)]
  formData.matchers = [{ key: '', value: '' }]
  formData.comment = ''
  void loadRules()
  dialogVisible.value = true
  nextTickClearValidate()
}

async function openEdit(row: Maintenance): Promise<void> {
  dialogMode.value = 'edit'
  editingId.value = row.id
  void loadRules()
  // 优先 getMaintenance 拿详情；失败则回退到行内数据
  let detail: Maintenance = row
  try {
    if (typeof getMaintenance === 'function') {
      detail = (await getMaintenance(row.id)) as Maintenance
    }
  } catch {
    detail = row
  }
  formData.name = detail.name ?? ''
  formData.enabled = Boolean(detail.enabled)
  formData.rule_id = detail.rule_id ?? null
  const s = detail.starts_at ? new Date(detail.starts_at).toISOString() : new Date().toISOString()
  const e = detail.ends_at ? new Date(detail.ends_at).toISOString() : new Date(Date.now() + 4 * 3600 * 1000).toISOString()
  formData.range = [s, e]
  const entries = matcherEntries(detail as Maintenance)
  formData.matchers = entries.length > 0 ? entries.map((x) => ({ key: x.key, value: x.value })) : [{ key: '', value: '' }]
  formData.comment = detail.comment ?? ''
  dialogVisible.value = true
  nextTickClearValidate()
}

function nextTickClearValidate(): void {
  // 稍后清除校验（保证 formRef 已绑定）
  setTimeout(() => {
    try {
      formRef.value?.clearValidate?.()
    } catch {
      /* ignore */
    }
  }, 0)
}

async function toggleEnabled(row: Maintenance, val: boolean): Promise<void> {
  // 乐观 UI
  const prev = row.enabled
  row.enabled = val
  try {
    await updateMaintenance(row.id, { enabled: val })
    ElMessage.success(val ? '已启用' : '已停用')
  } catch (e) {
    row.enabled = prev
    ElMessage.error(errMsgOf(e, '保存失败'))
  }
}

async function onDelete(row: Maintenance): Promise<void> {
  try {
    await ElMessageBox.confirm(
      `确定删除维护窗「${row.name}」？此操作不可撤销。`,
      '删除确认',
      { type: 'warning', confirmButtonText: '删除', cancelButtonText: '取消' },
    )
  } catch {
    return
  }
  try {
    await deleteMaintenance(row.id)
    ElMessage.success('已删除')
    void loadList()
  } catch (e) {
    ElMessage.error(errMsgOf(e, '删除失败'))
  }
}

// --- 表单校验 ---
const formRules = reactive<FormRules<typeof formData>>({
  name: [
    { required: true, message: '请输入名称', trigger: 'blur' },
    { min: 2, message: '名称至少 2 个字符', trigger: 'blur' },
    {
      validator: (_rule, value: unknown, cb) => {
        if (typeof value === 'string' && value.trim().length < 2) {
          cb(new Error('名称至少 2 个字符（已去除首尾空格）'))
        } else {
          cb()
        }
      },
      trigger: 'blur',
    },
  ],
  range: [
    {
      type: 'array',
      required: true,
      validator: (_rule, value: unknown, cb) => {
        const v = value as unknown as [string, string] | null | undefined
        if (!v || v.length !== 2 || !v[0] || !v[1]) {
          cb(new Error('请选择维护窗开始和结束时间'))
          return
        }
        const s = new Date(v[0]).getTime()
        const e = new Date(v[1]).getTime()
        if (Number.isNaN(s) || Number.isNaN(e)) {
          cb(new Error('时间格式不正确'))
          return
        }
        if (e <= s) {
          cb(new Error('结束时间必须晚于开始时间'))
          return
        }
        cb()
      },
      trigger: 'change',
    },
  ],
})

// matchers 动态行
function addMatcherRow(): void {
  formData.matchers.push({ key: '', value: '' })
}
function removeMatcherRow(idx: number): void {
  if (formData.matchers.length <= 1) {
    formData.matchers = [{ key: '', value: '' }]
  } else {
    formData.matchers.splice(idx, 1)
  }
}

async function onSubmit(): Promise<void> {
  if (!formRef.value) return
  try {
    await formRef.value.validate()
  } catch {
    return
  }
  // 校验 matchers：不能两列都空；至少一行有 key
  const cleaned = formData.matchers
    .map((r) => ({ key: (r.key || '').trim(), value: (r.value || '').trim() }))
    .filter((r) => r.key !== '' || r.value !== '')
  if (cleaned.length === 0) {
    ElMessage.warning('请至少填写一个匹配器（key/value 不能都为空）')
    return
  }
  if (cleaned.some((r) => !r.key)) {
    ElMessage.warning('匹配器 key 不能为空')
    return
  }
  if (!formData.range || formData.range.length !== 2) {
    ElMessage.warning('请选择维护窗时间窗')
    return
  }
  submitting.value = true
  try {
    const body: LocalMaintenanceInput = {
      name: formData.name.trim(),
      enabled: formData.enabled,
      rule_id: formData.rule_id || null,
      starts_at: formData.range[0],
      ends_at: formData.range[1],
      matchers: cleaned,
      comment: (formData.comment || '').slice(0, 500),
    }
    if (dialogMode.value === 'create') {
      await createMaintenance(body)
      ElMessage.success('创建成功')
    } else if (editingId.value) {
      await updateMaintenance(editingId.value, body)
      ElMessage.success('保存成功')
    }
    dialogVisible.value = false
    void loadList()
  } catch (e) {
    ElMessage.error(errMsgOf(e, '保存失败'))
  } finally {
    submitting.value = false
  }
}

// =================================================================
// 生命周期
// =================================================================
onMounted(() => {
  void loadList()
  tickTimer = window.setInterval(() => {
    tick.value = Date.now()
  }, 30_000)
})
onBeforeUnmount(() => {
  if (tickTimer != null) {
    window.clearInterval(tickTimer)
    tickTimer = null
  }
})

// 过滤器变化时无需重载
watch([filterEnabled, filterQ], () => {
  /* noop, computed 已响应 */
})
</script>

<template>
  <div class="maintenance-view">
    <!-- 标题区 -->
    <div class="page-head">
      <div class="page-head-left">
        <h2 class="page-title">
          <el-icon :size="22" color="var(--ep-color-primary)"><Setting /></el-icon>
          维护窗
        </h2>
        <div class="page-subtitle">计划性维护抑制通知</div>
      </div>
    </div>

    <!-- 吸附工具栏 -->
    <el-affix :offset="0" class="toolbar-affix">
      <el-card shadow="never" class="toolbar-card">
        <div class="toolbar-row">
          <div class="toolbar-left">
            <el-button type="primary" :icon="Plus" @click="openCreate">新建维护窗</el-button>
            <el-button :icon="Refresh" :loading="loading" @click="loadList">刷新</el-button>
          </div>
          <div class="toolbar-right">
            <el-select v-model="filterEnabled" style="width:140px;" placeholder="状态">
              <el-option label="全部" value="all" />
              <el-option label="已启用" value="on" />
              <el-option label="已停用" value="off" />
            </el-select>
            <el-input
              v-model="filterQ"
              clearable
              placeholder="关键词：名称 / 备注"
              style="width:260px;margin-left:12px;"
            >
              <template #prefix>
                <el-icon><Search /></el-icon>
              </template>
            </el-input>
          </div>
        </div>
      </el-card>
    </el-affix>

    <!-- 列表区 -->
    <el-card shadow="never" class="list-card" v-loading="loading">
      <template v-if="filteredList.length > 0">
        <el-table :data="filteredList" stripe style="width:100%;">
          <!-- 名称 + enabled 开关 -->
          <el-table-column label="名称" min-width="180">
            <template #default="{ row }">
              <div class="name-cell">
                <el-switch
                  :model-value="row.enabled"
                  inline-prompt
                  size="small"
                  style="margin-right:10px;"
                  @change="(v: unknown) => toggleEnabled(row as Maintenance, Boolean(v))"
                />
                <span class="name-text" :class="{ muted: !row.enabled }">{{ row.name }}</span>
              </div>
            </template>
          </el-table-column>

          <!-- 状态：三态 + 剩余时间 -->
          <el-table-column label="状态" width="200">
            <template #default="{ row }">
              <div class="status-cell">
                <el-tag :type="mwStatusMeta(mwStatusOf(row as Maintenance)).type" effect="light" size="small" style="margin-right:8px;">
                  {{ mwStatusMeta(mwStatusOf(row as Maintenance)).text }}
                </el-tag>
                <el-tooltip placement="top" content="时间基于浏览器本地时区">
                  <span class="remaining">{{ fmtRemaining(row as Maintenance) }}</span>
                </el-tooltip>
              </div>
            </template>
          </el-table-column>

          <!-- 时间段（截到分） -->
          <el-table-column label="时间段" width="290">
            <template #default="{ row }">
              <div class="range-cell">
                <span>{{ fmtMinute(row.starts_at) }}</span>
                <span class="range-arrow">→</span>
                <span>{{ fmtMinute(row.ends_at) }}</span>
              </div>
            </template>
          </el-table-column>

          <!-- 匹配器 -->
          <el-table-column label="匹配器" min-width="220">
            <template #default="{ row }">
              <div class="matchers-cell">
                <template v-if="matcherEntries(row as Maintenance).length === 0">
                  <el-tag type="info" effect="plain" size="small">全部标签</el-tag>
                </template>
                <template v-else-if="matcherEntries(row as Maintenance).length <= 3">
                  <el-tag
                    v-for="(m, i) in matcherEntries(row as Maintenance)"
                    :key="i"
                    size="small"
                    effect="light"
                    style="margin-right:4px;margin-bottom:4px;"
                  >
                    {{ m.key }}={{ m.value }}
                  </el-tag>
                </template>
                <template v-else>
                  <el-popover placement="top" trigger="hover" :width="320">
                    <template #reference>
                      <span>
                        <el-tag
                          v-for="(m, i) in matcherEntries(row as Maintenance).slice(0, 3)"
                          :key="i"
                          size="small"
                          effect="light"
                          style="margin-right:4px;margin-bottom:4px;"
                        >
                          {{ m.key }}={{ m.value }}
                        </el-tag>
                        <el-tag size="small" effect="plain" type="info">
                          +{{ matcherEntries(row as Maintenance).length - 3 }}
                        </el-tag>
                      </span>
                    </template>
                    <div class="matchers-popover">
                      <div
                        v-for="(m, i) in matcherEntries(row as Maintenance)"
                        :key="i"
                        class="matchers-popover-row"
                      >
                        <span class="matchers-popover-k">{{ m.key }}</span>
                        <span class="matchers-popover-eq">=</span>
                        <span class="matchers-popover-v">{{ m.value }}</span>
                      </div>
                    </div>
                  </el-popover>
                </template>
              </div>
            </template>
          </el-table-column>

          <!-- 规则绑定 -->
          <el-table-column label="规则绑定" width="150">
            <template #default="{ row }">
              <template v-if="!row.rule_id">
                <el-tag type="primary" effect="plain" size="small">全部规则</el-tag>
              </template>
              <template v-else>
                <el-tooltip placement="top" :content="row.rule_id">
                  <span class="rule-chip">#{{ shortUuid(row.rule_id) }}</span>
                </el-tooltip>
              </template>
            </template>
          </el-table-column>

          <!-- 备注 -->
          <el-table-column label="备注" min-width="180" show-overflow-tooltip>
            <template #default="{ row }">
              <span v-if="row.comment">{{ row.comment }}</span>
              <span v-else class="muted">-</span>
            </template>
          </el-table-column>

          <!-- updated_at -->
          <el-table-column label="更新时间" width="160">
            <template #default="{ row }">{{ fmtMinute(row.updated_at) }}</template>
          </el-table-column>

          <!-- 操作列 -->
          <el-table-column label="操作" width="140" fixed="right" align="center">
            <template #default="{ row }">
              <el-button link type="primary" size="small" :icon="Edit" @click="openEdit(row as Maintenance)">编辑</el-button>
              <el-button link type="danger" size="small" :icon="Delete" @click="onDelete(row as Maintenance)">删除</el-button>
            </template>
          </el-table-column>
        </el-table>
      </template>

      <!-- 空态 -->
      <template v-else>
        <div class="empty-wrap" v-loading="loading">
          <el-empty description="暂无维护窗">
            <el-button type="primary" :icon="Plus" @click="openCreate">新建第一个维护窗</el-button>
          </el-empty>
        </div>
      </template>
    </el-card>

    <!-- 新建 / 编辑 Dialog -->
    <el-dialog
      v-model="dialogVisible"
      :title="dialogMode === 'create' ? '新建维护窗' : '编辑维护窗'"
      width="640px"
      :close-on-click-modal="false"
      destroy-on-close
    >
      <el-form
        ref="formRef"
        :model="formData"
        :rules="formRules"
        label-width="92px"
        label-position="right"
      >
        <el-form-item label="名称" prop="name">
          <el-input
            v-model="formData.name"
            placeholder="例如：数据库每周日凌晨升级"
            maxlength="80"
            show-word-limit
            clearable
          />
        </el-form-item>

        <el-form-item label="状态">
          <el-switch v-model="formData.enabled" inline-prompt active-text="启用" inactive-text="停用" />
        </el-form-item>

        <el-form-item label="规则">
          <el-select
            v-model="formData.rule_id"
            placeholder="选择或输入规则 ID；不选=全部规则"
            style="width:100%;"
            clearable
            filterable
            allow-create
            default-first-option
            :loading="loadingRules"
          >
            <el-option label="全部规则（不绑定）" value="" />
            <el-option
              v-for="r in ruleOptions"
              :key="r.id"
              :label="r.name ? `${r.name} (#${shortUuid(r.id)})` : `#${shortUuid(r.id)}`"
              :value="r.id"
            />
          </el-select>
        </el-form-item>

        <el-form-item label="时间窗" prop="range">
          <el-date-picker
            v-model="formData.range"
            type="datetimerange"
            start-placeholder="开始时间"
            end-placeholder="结束时间"
            value-format="YYYY-MM-DDTHH:mm:ssZ"
            style="width:100%;"
            range-separator="→"
          />
        </el-form-item>

        <el-form-item label="匹配器">
          <div class="matchers-form">
            <div
              v-for="(_row, idx) in formData.matchers"
              :key="idx"
              class="matcher-row"
            >
              <el-input
                v-model="formData.matchers[idx].key"
                placeholder="key，例如：alertname"
                style="flex:1 1 0;"
                clearable
              />
              <span class="matcher-eq">=</span>
              <el-input
                v-model="formData.matchers[idx].value"
                placeholder="value，例如：HighLoad"
                style="flex:1.2 1 0;"
                clearable
              />
              <el-button
                link
                type="danger"
                style="margin-left:8px;"
                @click="removeMatcherRow(idx)"
              >移除</el-button>
            </div>
            <el-button plain :icon="Plus" size="small" @click="addMatcherRow">添加匹配器</el-button>
          </div>
        </el-form-item>

        <el-form-item label="备注">
          <el-input
            v-model="formData.comment"
            type="textarea"
            :rows="3"
            maxlength="500"
            show-word-limit
            placeholder="维护窗说明 / 变更单号 / 负责人等"
          />
        </el-form-item>
      </el-form>

      <template #footer>
        <el-button @click="dialogVisible = false">取消</el-button>
        <el-button type="primary" :loading="submitting" @click="onSubmit">保存</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<style scoped>
.maintenance-view {
  padding: 16px 20px 32px 20px;
  color: var(--ep-text-color-primary, #303133);
}
.page-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  margin: 4px 4px 14px 4px;
}
.page-head-left { display: flex; flex-direction: column; gap: 4px; }
.page-title {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 22px;
  font-weight: 600;
  margin: 0;
  line-height: 1.2;
}
.page-subtitle {
  font-size: 13px;
  color: var(--ep-text-color-regular, #606266);
}
.toolbar-affix { z-index: 10; }
.toolbar-card { margin-bottom: 12px; border-radius: 8px; }
.toolbar-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex-wrap: wrap;
  gap: 12px;
}
.toolbar-left, .toolbar-right {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}
.list-card { border-radius: 8px; }
.name-cell { display: flex; align-items: center; }
.name-text { font-weight: 500; word-break: break-all; }
.name-text.muted { color: var(--ep-text-color-placeholder, #a8abb2); }
.muted { color: var(--ep-text-color-placeholder, #a8abb2); }
.status-cell { display: flex; align-items: center; }
.remaining { font-size: 12px; color: var(--ep-text-color-regular, #606266); }
.range-cell {
  display: flex;
  align-items: center;
  gap: 6px;
  font-variant-numeric: tabular-nums;
  font-size: 13px;
}
.range-arrow { color: var(--ep-border-color, #dcdfe6); }
.matchers-cell { line-height: 1.8; }
.matchers-popover { max-height: 320px; overflow: auto; }
.matchers-popover-row {
  display: grid;
  grid-template-columns: 1fr auto 1.2fr;
  gap: 8px;
  padding: 4px 0;
  font-size: 12px;
  border-bottom: 1px dashed var(--ep-border-color-lighter, #ebeef5);
}
.matchers-popover-row:last-child { border-bottom: none; }
.matchers-popover-k { color: var(--ep-color-primary, #409eff); }
.matchers-popover-eq { color: var(--ep-border-color, #dcdfe6); }
.matchers-popover-v {
  color: var(--ep-text-color-primary, #303133);
  word-break: break-all;
}
.rule-chip {
  display: inline-block;
  padding: 2px 8px;
  background: var(--ep-fill-color-light, #f5f7fa);
  color: var(--ep-color-primary, #409eff);
  border-radius: 10px;
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  font-size: 12px;
}
.empty-wrap { padding: 60px 0; }

.matchers-form { width: 100%; }
.matcher-row {
  display: flex;
  align-items: center;
  margin-bottom: 8px;
  gap: 8px;
}
.matcher-eq { color: var(--ep-border-color, #dcdfe6); }
</style>
