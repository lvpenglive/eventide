<script setup lang="ts">
import { computed, onMounted, reactive, ref, markRaw } from 'vue'
import { useRouter } from 'vue-router'
import {
  ElAffix,
  ElButton,
  ElDescriptions,
  ElDescriptionsItem,
  ElDialog,
  ElDrawer,
  ElEmpty,
  ElMessage,
  ElMessageBox,
  ElTable,
  ElTableColumn,
  ElTag,
  ElPagination,
} from 'element-plus'
import {
  Delete,
  Edit,
  Plus,
  Refresh,
  VideoPlay,
} from '@element-plus/icons-vue'
import type { NotifyChannel } from '@/api/types'

// ============================================================
// 本地补齐类型（不改动 types.ts）
// ============================================================
export type RuleSeverity = 'disaster' | 'high' | 'average' | 'warning' | 'information' | 'not_classified'
export type Comparator = 'gt' | 'gte' | 'lt' | 'lte' | 'eq' | 'neq'

export interface DatasourceRow {
  id: string
  name: string
  kind: 'prometheus' | 'loki' | 'kafka' | string
  url: string
  enabled: boolean
  created_at: string
  updated_at: string
}

export interface LocalRule {
  id: string
  name: string
  datasource_id: string
  expr: string
  comparator: Comparator
  threshold: number
  severity: RuleSeverity
  for_seconds: number
  interval_seconds: number
  labels: Record<string, string>
  annotations: Record<string, string>
  channel_ids: string[]
  escalate_after_seconds: number
  escalate_severity: RuleSeverity | null
  escalate_channel_ids: string[]
  enabled: boolean
  created_at: string
  updated_at: string
}

export interface RuleInput {
  name: string
  datasource_id: string
  expr: string
  comparator: Comparator
  threshold: number
  severity: RuleSeverity
  for_seconds?: number
  interval_seconds?: number
  labels?: Record<string, string>
  annotations?: Record<string, string>
  channel_ids?: string[]
  escalate_after_seconds?: number
  escalate_severity?: RuleSeverity | '' | null
  escalate_channel_ids?: string[]
  enabled?: boolean
}

export interface EvaluateAlert {
  severity: RuleSeverity | string
  status: 'firing' | 'pending' | 'resolved' | string
  labels: Record<string, string>
  fingerprint: string
  starts_at: string
  summary?: string
  value?: number | null
  annotations?: Record<string, string>
}

export interface EvaluateRuleResp {
  ok: boolean
  http_method?: string
  http_path?: string
  request_body?: unknown
  response_raw?: unknown
  alerts: EvaluateAlert[]
  error?: string
}

// ============================================================
// 兜底模块绑定
// ============================================================
interface RulesApiShim {
  listRules(): Promise<LocalRule[]>
  getRule(id: string): Promise<LocalRule>
  createRule(body: RuleInput): Promise<LocalRule>
  updateRule(id: string, body: Partial<RuleInput>): Promise<LocalRule>
  deleteRule(id: string): Promise<{ ok: boolean }>
  evaluateRule(id: string): Promise<EvaluateRuleResp>
}
interface DatasourcesApiShim {
  listDatasources(): Promise<DatasourceRow[]>
}
interface CommonApiShim {
  listChannels(): Promise<NotifyChannel[]>
}
function _unimpl(name: string): never {
  throw new Error(`[API shim] 模块未实现：${name}，请等待并行任务创建对应 api/*.ts`)
}
const _shimRules: RulesApiShim = {
  listRules: () => Promise.reject(_unimpl('listRules')),
  getRule: () => Promise.reject(_unimpl('getRule')),
  createRule: () => Promise.reject(_unimpl('createRule')),
  updateRule: () => Promise.reject(_unimpl('updateRule')),
  deleteRule: () => Promise.reject(_unimpl('deleteRule')),
  evaluateRule: () => Promise.reject(_unimpl('evaluateRule')),
}
const _shimDatasources: DatasourcesApiShim = {
  listDatasources: () => Promise.resolve([]),
}
const _shimCommon: CommonApiShim = {
  listChannels: () => Promise.resolve([]),
}
// @ts-ignore 若 @/api/rules 模块尚未创建则忽略解析错误
import * as _rawRules from '@/api/rules'
// @ts-ignore 若 @/api/datasources 模块尚未创建则忽略解析错误
import * as _rawDatasources from '@/api/datasources'
// @ts-ignore 若 @/api/common 模块尚未创建则忽略解析错误
import * as _rawCommon from '@/api/common'
const _rules = markRaw(_rawRules as unknown as RulesApiShim | Record<string, unknown>)
const _datasources = markRaw(_rawDatasources as unknown as DatasourcesApiShim | Record<string, unknown>)
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
const _rulesApi = _bindApi<Record<string, unknown>, RulesApiShim>(_rules as unknown as Record<string, unknown>, _shimRules)
const _datasourcesApi = _bindApi<Record<string, unknown>, DatasourcesApiShim>(_datasources as unknown as Record<string, unknown>, _shimDatasources)
const _commonApi = _bindApi<Record<string, unknown>, CommonApiShim>(_common as unknown as Record<string, unknown>, _shimCommon)
const { listRules, getRule, createRule, updateRule, deleteRule, evaluateRule } = _rulesApi
const listDatasources = _datasourcesApi.listDatasources
const listChannels = _commonApi.listChannels

// ============================================================
// 基础依赖
// ============================================================
const router = useRouter()

// ============================================================
// 常量：severity 色值 / 中文名 / comparator
// ============================================================
const SEVERITY_COLORS: Record<RuleSeverity, string> = {
  disaster: '#f56c6c',
  high: '#e6a23c',
  average: '#d48806',
  warning: '#f0c78a',
  information: '#909399',
  not_classified: '#c0c4cc',
}
const SEVERITY_NAMES: Record<RuleSeverity, string> = {
  disaster: '灾害',
  high: '严重',
  average: '一般',
  warning: '警告',
  information: '信息',
  not_classified: '未分类',
}
const SEVERITY_OPTIONS: { value: RuleSeverity; label: string }[] = [
  { value: 'not_classified', label: '未分类' },
  { value: 'information', label: '信息' },
  { value: 'warning', label: '警告' },
  { value: 'average', label: '一般' },
  { value: 'high', label: '严重' },
  { value: 'disaster', label: '灾害' },
]
const COMPARATOR_OPTIONS: { value: Comparator; label: string }[] = [
  { value: 'gt', label: '>' },
  { value: 'gte', label: '>=' },
  { value: 'lt', label: '<' },
  { value: 'lte', label: '<=' },
  { value: 'eq', label: '==' },
  { value: 'neq', label: '!=' },
]

// ============================================================
// 状态：列表、筛选、字典
// ============================================================
const loading = ref(false)
const rules = ref<LocalRule[]>([])
const datasources = ref<DatasourceRow[]>([])
const channels = ref<NotifyChannel[]>([])

const filterDatasource = ref<string>('')
const filterQ = ref('')
const filterEnabled = ref<'' | 'true' | 'false'>('')

// —— 分页
const RULES_PAGE_SIZES = [10, 20, 50, 100] as const
const rulesPage = ref(1)
const rulesPageSize = ref<number>(RULES_PAGE_SIZES[1])
const pagedRules = computed(() => {
  const src = filteredRules.value
  if (src.length <= rulesPageSize.value) return src
  const start = (rulesPage.value - 1) * rulesPageSize.value
  return src.slice(start, start + rulesPageSize.value)
})
function rulesOnPage(p: number) { rulesPage.value = Math.max(1, p) }
function rulesOnSize(s: number) { rulesPageSize.value = s; rulesPage.value = 1 }
import { watch as _rulesWatch } from 'vue'
_rulesWatch([filterDatasource, filterQ, filterEnabled, rules, datasources], () => { rulesPage.value = 1 })

const filteredRules = computed(() => {
  return rules.value.filter((r) => {
    if (filterDatasource.value && r.datasource_id !== filterDatasource.value) return false
    if (filterEnabled.value === 'true' && !r.enabled) return false
    if (filterEnabled.value === 'false' && r.enabled) return false
    if (filterQ.value.trim()) {
      const q = filterQ.value.trim().toLowerCase()
      if (!r.name.toLowerCase().includes(q) && !r.expr.toLowerCase().includes(q)) return false
    }
    return true
  })
})

const datasourceMap = computed(() => {
  const m = new Map<string, DatasourceRow>()
  for (const d of datasources.value) m.set(d.id, d)
  return m
})

const annotationsHint = computed(() => '通知前自动渲染 {{labels.x}} / {{value}} / {{severity}} 等变量。')

// 模板插槽 row 类型兜底（DefaultRow -> LocalRule / 索引类型修复）
function asRule(r: unknown): LocalRule { return r as LocalRule }
function asEval(r: unknown): EvaluateAlert { return r as EvaluateAlert }
function sev(s: unknown): RuleSeverity { return s as RuleSeverity }

function formatDatasource(id: string): string {
  const d = datasourceMap.value.get(id)
  if (d) return d.name
  return id ? id.slice(0, 8) : '—'
}
function truncateExpr(expr: string): string {
  if (!expr) return ''
  return expr.length > 50 ? expr.slice(0, 50) + '…' : expr
}
async function copyText(text: string) {
  try {
    await navigator.clipboard.writeText(text)
    ElMessage.success('已复制到剪贴板')
  } catch {
    // fallback: select + execCommand
    const ta = document.createElement('textarea')
    ta.value = text
    document.body.appendChild(ta)
    ta.select()
    try {
      document.execCommand('copy')
      ElMessage.success('已复制到剪贴板')
    } catch {
      ElMessage.error('复制失败，请手动复制')
    }
    document.body.removeChild(ta)
  }
}
function formatCondition(r: LocalRule): string {
  const map: Record<string, string> = {
    gt: '>', gte: '>=', lt: '<', lte: '<=', eq: '==', neq: '!=',
  }
  const op = map[r.comparator] || r.comparator
  return `${op} ${r.threshold ?? 0}`
}
function formatTime(s: string | null | undefined): string {
  if (!s) return '—'
  return s.replace('T', ' ').slice(0, 19)
}

async function loadAll() {
  loading.value = true
  try {
    const [rs, ds, cs] = await Promise.all([
      listRules().catch((e) => { ElMessage.error(String(e?.message || e)); return [] as LocalRule[] }),
      listDatasources().catch(() => [] as DatasourceRow[]),
      listChannels().catch(() => [] as NotifyChannel[]),
    ])
    rules.value = rs
    datasources.value = ds
    channels.value = cs
  } finally {
    loading.value = false
  }
}

onMounted(loadAll)

// ============================================================
// enabled 开关：乐观更新 + 失败回滚
// ============================================================
async function onToggleEnabled(row: LocalRule) {
  const newVal = row.enabled
  try {
    await updateRule(row.id, { enabled: newVal })
    ElMessage.success(`规则已${newVal ? '启用' : '停用'}`)
    const fresh = await getRule(row.id).catch(() => null)
    if (fresh) {
      const idx = rules.value.findIndex((r) => r.id === row.id)
      if (idx >= 0) rules.value[idx] = fresh
    }
  } catch (e) {
    row.enabled = !newVal
    ElMessage.error(`更新失败：${String((e as Error)?.message || e)}`)
  }
}

// ============================================================
// 试跑 Drawer
// ============================================================
const drawerVisible = ref(false)
const evaluateLoading = ref(false)
const evaluateResult = ref<EvaluateRuleResp | null>(null)
const evaluateRuleId = ref<string>('')
const evaluateRuleName = ref('')

function shownSummaryList(alerts: EvaluateAlert[]) {
  const top = alerts.slice(0, 3).map((a, i) => {
    const s = a.summary || a.annotations?.summary || a.labels?.alertname || `告警 #${i + 1}`
    return s
  })
  return { top, more: alerts.length > 3 ? alerts.length - 3 : 0 }
}

async function onEvaluate(row: LocalRule) {
  evaluateRuleId.value = row.id
  evaluateRuleName.value = row.name
  evaluateResult.value = null
  drawerVisible.value = true
  evaluateLoading.value = true
  try {
    evaluateResult.value = await evaluateRule(row.id)
  } catch (e) {
    evaluateResult.value = {
      ok: false,
      alerts: [],
      error: String((e as Error)?.message || e),
    }
  } finally {
    evaluateLoading.value = false
  }
}

function evalHttpDisplay(): { method: string; path: string; rawJson: string } {
  const r = evaluateResult.value
  if (!r) return { method: 'GET', path: 'N/A', rawJson: '' }
  const method = r.http_method || 'GET'
  const path = r.http_path || `/api/rules/${evaluateRuleId.value}/evaluate`
  const raw =
    r.response_raw !== undefined
      ? r.response_raw
      : r.error
        ? { error: r.error, alerts: r.alerts }
        : { alerts: r.alerts }
  let j = ''
  try {
    j = JSON.stringify(raw, null, 2)
  } catch {
    j = String(raw)
  }
  if (j.length > 500) j = j.slice(0, 500) + '…'
  return { method, path, rawJson: j }
}

function fingerprintShort(fp: string): string {
  if (!fp) return ''
  return fp.length > 12 ? fp.slice(0, 12) : fp
}
function labelsToString(l: Record<string, string> | undefined): string {
  if (!l) return ''
  return Object.entries(l)
    .map(([k, v]) => `${k}=${v}`)
    .join(', ')
}

// ============================================================
// 新建 / 编辑 Dialog
// ============================================================
interface DialogState {
  visible: boolean
  mode: 'create' | 'edit'
  id: string
  name: string
  datasource_id: string
  enabled: boolean
  expr: string
  comparator: Comparator
  threshold: number | null
  severity: RuleSeverity
  for_seconds: number
  interval_seconds: number
  labelsText: string
  annotationsText: string
  channel_ids: string[]
  escalate_after_seconds: number
  escalate_severity: RuleSeverity | ''
  escalate_channel_ids: string[]
}

const dlg = reactive<DialogState>({
  visible: false,
  mode: 'create',
  id: '',
  name: '',
  datasource_id: '',
  enabled: true,
  expr: '',
  comparator: 'gt',
  threshold: null,
  severity: 'warning',
  for_seconds: 0,
  interval_seconds: 30,
  labelsText: '',
  annotationsText: '',
  channel_ids: [],
  escalate_after_seconds: 0,
  escalate_severity: '',
  escalate_channel_ids: [],
})

async function openCreate() {
  dlg.visible = true
  dlg.mode = 'create'
  dlg.id = ''
  dlg.name = ''
  dlg.datasource_id = ''
  dlg.enabled = true
  dlg.expr = ''
  dlg.comparator = 'gt'
  dlg.threshold = null
  dlg.severity = 'warning'
  dlg.for_seconds = 0
  dlg.interval_seconds = 30
  dlg.labelsText = ''
  dlg.annotationsText = ''
  dlg.channel_ids = []
  dlg.escalate_after_seconds = 0
  dlg.escalate_severity = ''
  dlg.escalate_channel_ids = []
  // 默认选中第一个数据源
  if (datasources.value.length > 0) {
    dlg.datasource_id = datasources.value[0].id
  }
}
async function openEdit(id: string) {
  let rule: LocalRule | null = null
  try {
    rule = await getRule(id)
  } catch (e) {
    ElMessage.error(`读取规则失败：${String((e as Error)?.message || e)}`)
    return
  }
  dlg.visible = true
  dlg.mode = 'edit'
  dlg.id = rule.id
  dlg.name = rule.name
  dlg.datasource_id = rule.datasource_id
  dlg.enabled = rule.enabled
  dlg.expr = rule.expr
  dlg.comparator = rule.comparator
  dlg.threshold = rule.threshold
  dlg.severity = rule.severity
  dlg.for_seconds = rule.for_seconds ?? 0
  dlg.interval_seconds = rule.interval_seconds ?? 30
  // labels / annotations 转为 JSON 字符串
  dlg.labelsText = rule.labels && Object.keys(rule.labels).length > 0
    ? JSON.stringify(rule.labels, null, 0)
    : ''
  dlg.annotationsText = rule.annotations && Object.keys(rule.annotations).length > 0
    ? JSON.stringify(rule.annotations, null, 2)
    : ''
  dlg.channel_ids = rule.channel_ids ? [...rule.channel_ids] : []
  dlg.escalate_after_seconds = rule.escalate_after_seconds ?? 0
  dlg.escalate_severity = rule.escalate_severity ?? ''
  dlg.escalate_channel_ids = rule.escalate_channel_ids ? [...rule.escalate_channel_ids] : []
}
async function onDlgSave() {
  // 手动校验
  if (!dlg.name.trim()) {
    ElMessage.error('请输入规则名称')
    return
  }
  if (!dlg.datasource_id) {
    ElMessage.error('请选择数据源')
    return
  }
  if (!dlg.expr.trim()) {
    ElMessage.error('请输入查询表达式')
    return
  }
  if (dlg.threshold === null || Number.isNaN(dlg.threshold)) {
    ElMessage.error('请填写阈值')
    return
  }

  // 解析 labels JSON
  let labels: Record<string, string> = {}
  const rawLabels = dlg.labelsText.trim()
  if (rawLabels) {
    try {
      labels = JSON.parse(rawLabels)
    } catch {
      ElMessage.error('标签 JSON 无效')
      return
    }
  }

  // 解析 annotations JSON
  let annotations: Record<string, string> = {}
  const rawAnn = dlg.annotationsText.trim()
  if (rawAnn) {
    try {
      annotations = JSON.parse(rawAnn)
    } catch {
      ElMessage.error('注解 JSON 无效')
      return
    }
  }

  const body: RuleInput = {
    name: dlg.name.trim(),
    datasource_id: dlg.datasource_id,
    expr: dlg.expr,
    comparator: dlg.comparator,
    threshold: dlg.threshold,
    severity: dlg.severity,
    for_seconds: dlg.for_seconds,
    interval_seconds: dlg.interval_seconds,
    labels,
    annotations,
    channel_ids: dlg.channel_ids,
    escalate_after_seconds: dlg.escalate_after_seconds,
    escalate_severity: dlg.escalate_severity || null,
    escalate_channel_ids: dlg.escalate_channel_ids,
    enabled: dlg.enabled,
  }
  try {
    if (dlg.mode === 'create') {
      await createRule(body)
      ElMessage.success('已保存')
    } else {
      await updateRule(dlg.id, body)
      ElMessage.success('已保存')
    }
    dlg.visible = false
    await loadAll()
  } catch (e) {
    ElMessage.error(`保存失败：${String((e as Error)?.message || e)}`)
  }
}

// ============================================================
// 删除（二次确认）
// ============================================================
async function onDelete(row: LocalRule) {
  try {
    await ElMessageBox.confirm(
      `确定要删除规则「${row.name}」？该操作不可恢复。`,
      '删除规则',
      { type: 'warning', confirmButtonText: '删除', cancelButtonText: '取消' }
    )
  } catch {
    return
  }
  try {
    await deleteRule(row.id)
    ElMessage.success('已删除')
    await loadAll()
  } catch (e) {
    ElMessage.error(`删除失败：${String((e as Error)?.message || e)}`)
  }
}

// ============================================================
// 跳转
// ============================================================
function goDatasources() {
  router.push('/datasources').catch(() => {})
}
</script>

<template>
  <div class="rules-view">
    <!-- 标题 -->
    <div class="page-header">
      <h2 class="page-title">告警规则</h2>
      <p class="page-sub">对数据源做 PromQL/LogQL 查询并阈值判断</p>
    </div>

    <!-- 吸附工具栏 -->
    <ElAffix :offset="0" class="affix-wrap">
      <div class="toolbar">
        <ElButton type="primary" :icon="Plus" @click="openCreate">新建规则</ElButton>
        <ElButton :icon="Refresh" @click="loadAll" :loading="loading">刷新</ElButton>

        <div class="toolbar-filters">
          <ElSelect
            v-model="filterDatasource"
            placeholder="按数据源筛选"
            clearable
            style="width: 200px"
          >
            <ElOption
              v-for="d in datasources"
              :key="d.id"
              :label="d.name"
              :value="d.id"
            />
          </ElSelect>

          <ElInput
            v-model="filterQ"
            placeholder="搜索 name / expr 包含"
            clearable
            style="width: 240px"
          />

          <ElSelect
            v-model="filterEnabled"
            placeholder="启用状态"
            clearable
            style="width: 140px"
          >
            <ElOption label="已启用" value="true" />
            <ElOption label="已停用" value="false" />
          </ElSelect>
        </div>
      </div>
    </ElAffix>

    <!-- 空态：无数据源 + 无规则 -->
    <ElEmpty
      v-if="!loading && datasources.length === 0 && rules.length === 0"
      description="先到『数据源』至少建一个 Prometheus / Kafka / Loki 数据源，再新建规则"
    >
      <template #extra>
        <ElButton type="primary" @click="goDatasources">前往数据源</ElButton>
        <ElButton :icon="Plus" @click="openCreate">新建规则</ElButton>
      </template>
    </ElEmpty>

    <!-- 表格 -->
    <el-table
      v-else
      :data="pagedRules"
      v-loading="loading"
      stripe
      style="width: 100%; margin-top: 12px"
      empty-text="暂无匹配的规则，试试调整筛选条件或点击新建。"
    >
      <el-table-column label="名称" width="140">
        <template #default="{ row }">
          <span class="rule-name" :title="asRule(row).name" @click="openEdit(asRule(row).id)">{{ asRule(row).name }}</span>
        </template>
      </el-table-column>

      <el-table-column label="数据源" width="120">
        <template #default="{ row }">
          {{ formatDatasource(asRule(row).datasource_id) }}
        </template>
      </el-table-column>

      <el-table-column label="表达式" min-width="180">
        <template #default="{ row }">
          <el-tooltip
            v-if="asRule(row).expr && asRule(row).expr.length > 50"
            :content="asRule(row).expr"
            placement="top"
            :show-after="300"
          >
            <span class="mono-cell" @click="copyText(asRule(row).expr)">{{ truncateExpr(asRule(row).expr) }}</span>
          </el-tooltip>
          <span v-else class="mono-cell">{{ truncateExpr(asRule(row).expr) }}</span>
        </template>
      </el-table-column>

      <el-table-column label="条件" width="100">
        <template #default="{ row }">
          <span class="mono-cell">{{ formatCondition(asRule(row)) }}</span>
        </template>
      </el-table-column>

      <el-table-column label="间隔" width="150">
        <template #default="{ row }">
          {{ asRule(row).interval_seconds }}s / for {{ asRule(row).for_seconds }}s{{ Number(asRule(row).escalate_after_seconds) > 0 ? ' · 升级 ' + asRule(row).escalate_after_seconds + 's' : '' }}
        </template>
      </el-table-column>

      <el-table-column label="状态" width="90" align="center" class-name="col-status">
        <template #default="{ row }">
          <span :class="['badge', asRule(row).enabled ? 'on' : 'off']">
            {{ asRule(row).enabled ? '启用' : '停用' }}
          </span>
        </template>
      </el-table-column>

      <el-table-column label="操作" width="170" fixed="right" class-name="col-actions">
        <template #default="{ row }">
          <div class="actions">
            <button @click="onEvaluate(asRule(row))">试跑</button>
            <button @click="openEdit(asRule(row).id)">编辑</button>
            <button class="danger" @click="onDelete(asRule(row))">删除</button>
          </div>
        </template>
      </el-table-column>
    </el-table>

    <div class="panel rules-pager">
      <div class="pager-tip">共 <span class="mono">{{ filteredRules.length }}</span> 条</div>
      <el-pagination
        v-model:current-page="rulesPage"
        v-model:page-size="rulesPageSize"
        :page-sizes="Array.from(RULES_PAGE_SIZES)"
        :total="filteredRules.length"
        layout="sizes, prev, pager, next, jumper, ->, total"
        background
        @current-change="rulesOnPage"
        @size-change="rulesOnSize"
      />
    </div>

    <!-- 试跑 Drawer -->
    <ElDrawer
      v-model="drawerVisible"
      :title="`规则试跑结果：${evaluateRuleName}`"
      direction="rtl"
      size="560px"
    >
      <div v-loading="evaluateLoading" style="min-height: 200px">
        <template v-if="evaluateResult">
          <div class="eval-head">
            <ElTag
              v-if="evaluateResult.ok"
              type="success"
              size="large"
              effect="dark"
              style="font-weight: 600; padding: 6px 14px;"
            >
              执行成功
            </ElTag>
            <ElTag
              v-else
              type="danger"
              size="large"
              effect="dark"
              style="font-weight: 600; padding: 6px 14px;"
            >
              执行失败
            </ElTag>
            <span class="eval-counts">
              alerts: <b>{{ evaluateResult.alerts?.length ?? 0 }}</b>
            </span>
          </div>

          <ElDescriptions :column="1" border size="small" style="margin-top: 12px">
            <ElDescriptionsItem label="HTTP 请求">
              {{ evalHttpDisplay().method }} {{ evalHttpDisplay().path }}
            </ElDescriptionsItem>
            <ElDescriptionsItem label="返回（前 500 字）">
              <pre class="raw-pre">{{ evalHttpDisplay().rawJson }}</pre>
            </ElDescriptionsItem>
          </ElDescriptions>

          <div v-if="evaluateResult.alerts && evaluateResult.alerts.length > 0" style="margin-top: 16px">
            <h4>触发的告警（{{ evaluateResult.alerts.length }} 条）</h4>
            <ElTable :data="evaluateResult.alerts" size="small" border stripe>
              <ElTableColumn label="Severity" width="110">
                <template #default="{ row }">
                  <ElTag
                    v-if="SEVERITY_COLORS[sev(asEval(row).severity)]"
                    :color="SEVERITY_COLORS[sev(asEval(row).severity)]"
                    effect="dark"
                    style="color: #fff"
                  >
                    {{ SEVERITY_NAMES[sev(asEval(row).severity)] || asEval(row).severity }}
                  </ElTag>
                  <ElTag v-else type="info">{{ asEval(row).severity }}</ElTag>
                </template>
              </ElTableColumn>
              <ElTableColumn label="Status" width="90">
                <template #default="{ row }">{{ asEval(row).status }}</template>
              </ElTableColumn>
              <ElTableColumn label="Labels" min-width="180">
                <template #default="{ row }">
                  <ElTooltip :content="labelsToString(asEval(row).labels)" placement="top" :show-after="300">
                    <span class="labels-cell">{{ labelsToString(asEval(row).labels) }}</span>
                  </ElTooltip>
                </template>
              </ElTableColumn>
              <ElTableColumn label="Fingerprint" width="110">
                <template #default="{ row }">
                  <code>{{ fingerprintShort(asEval(row).fingerprint) }}</code>
                </template>
              </ElTableColumn>
              <ElTableColumn label="Starts At" width="160">
                <template #default="{ row }">
                  {{ formatTime(asEval(row).starts_at) }}
                </template>
              </ElTableColumn>
            </ElTable>
          </div>

          <div v-if="evaluateResult.alerts && evaluateResult.alerts.length > 0" style="margin-top: 12px">
            <h4>摘要</h4>
            <ul class="summary-list">
              <li v-for="(s, i) in shownSummaryList(evaluateResult.alerts).top" :key="i">
                {{ s }}
              </li>
            </ul>
            <div v-if="shownSummaryList(evaluateResult.alerts).more > 0" class="more-tip">
              …还有 {{ shownSummaryList(evaluateResult.alerts).more }} 更多
            </div>
          </div>
        </template>
      </div>
    </ElDrawer>

    <!-- 新建 / 编辑 Dialog -->
    <el-dialog
      v-model="dlg.visible"
      width="880px"
      :close-on-click-modal="false"
      append-to-body
      :show-close="false"
    >
      <template #header>
        <div class="ds-modal-head">
          <h3>{{ dlg.mode === 'create' ? '新建告警规则' : '编辑规则' }}</h3>
          <p class="ds-modal-desc">绑定数据源，配置阈值条件与通知渠道。</p>
        </div>
      </template>
      <div class="ds-modal-body">
        <!-- 规则 -->
        <div class="ds-seg">
          <div class="ds-seg-title">规则</div>
          <div class="ds-field">
            <label>名称</label>
            <input v-model="dlg.name" type="text" required placeholder="例如：API 不可用" />
          </div>
          <div class="ds-field">
            <label>数据源</label>
            <select v-model="dlg.datasource_id">
              <option
                v-for="d in datasources"
                :key="d.id"
                :value="d.id"
              >{{ d.name }} · {{ d.kind }}</option>
            </select>
          </div>
          <div class="ds-field">
            <label>查询表达式</label>
            <input v-model="dlg.expr" type="text" required placeholder="PromQL / LogQL / JSON 字段路径" />
            <div class="ds-hint">Prometheus/VM 用 PromQL；Loki 用 LogQL；Kafka 填数值字段路径（可覆盖数据源 field，如 latency_ms）。</div>
          </div>
        </div>

        <!-- 阈值条件 -->
        <div class="ds-seg">
          <div class="ds-seg-title">阈值条件</div>
          <div class="ds-row">
            <div class="ds-field">
              <label>比较符</label>
              <select v-model="dlg.comparator">
                <option
                  v-for="c in COMPARATOR_OPTIONS"
                  :key="c.value"
                  :value="c.value"
                >{{ c.label }}</option>
              </select>
            </div>
            <div class="ds-field">
              <label>阈值</label>
              <input v-model.number="dlg.threshold" type="number" step="any" required />
            </div>
          </div>
          <div class="ds-row">
            <div class="ds-field">
              <label>持续 for（秒）</label>
              <input v-model.number="dlg.for_seconds" type="number" min="0" />
            </div>
            <div class="ds-field">
              <label>评估间隔（秒）</label>
              <input v-model.number="dlg.interval_seconds" type="number" min="5" />
            </div>
          </div>
          <div class="ds-row">
            <div class="ds-field">
              <label>严重级别</label>
              <select v-model="dlg.severity">
                <option
                  v-for="s in SEVERITY_OPTIONS"
                  :key="s.value"
                  :value="s.value"
                >{{ s.label }}</option>
              </select>
            </div>
          </div>
          <div class="ds-field">
            <label class="ds-check-row">
              <input v-model="dlg.enabled" type="checkbox" />
              <span>启用此规则</span>
            </label>
          </div>
        </div>

        <!-- 通知与标签 -->
        <div class="ds-seg">
          <div class="ds-seg-title">通知与标签</div>
          <div class="ds-field">
            <label>通知渠道</label>
            <select v-model="dlg.channel_ids" multiple :size="Math.min(6, Math.max(3, channels.length || 3))">
              <option
                v-for="c in channels"
                :key="c.id"
                :value="c.id"
              >{{ c.name }} ({{ c.kind }})</option>
            </select>
            <div class="ds-ms-hint">按住 <b>Ctrl</b>（Windows）或 <b>⌘</b>（Mac）点击可多选；已选中的再点一次即取消。</div>
          </div>
          <div class="ds-field">
            <label>未接手升级（秒，0=关闭）</label>
            <input v-model.number="dlg.escalate_after_seconds" type="number" min="0" />
            <div class="ds-hint">告警中超过此时长仍未接手则发送升级通知；可选抬升级别与独立渠道。</div>
          </div>
          <div class="ds-row">
            <div class="ds-field">
              <label>升级级别（可选）</label>
              <select v-model="dlg.escalate_severity">
                <option value="">不改级别</option>
                <option
                  v-for="s in SEVERITY_OPTIONS"
                  :key="s.value"
                  :value="s.value"
                >{{ s.label }}</option>
              </select>
            </div>
          </div>
          <div class="ds-field">
            <label>升级通知渠道（可选，空=用上方渠道）</label>
            <select v-model="dlg.escalate_channel_ids" multiple :size="Math.min(6, Math.max(3, channels.length || 3))">
              <option
                v-for="c in channels"
                :key="c.id"
                :value="c.id"
              >{{ c.name }} ({{ c.kind }})</option>
            </select>
          </div>
          <div class="ds-field">
            <label>附加标签（可选，JSON）</label>
            <textarea v-model="dlg.labelsText" rows="2" placeholder='{"team":"sre"}'></textarea>
          </div>
          <div class="ds-field">
            <label>注解 annotations（可选，JSON，支持模板）</label>
            <textarea v-model="dlg.annotationsText" rows="3" placeholder='{"summary":"..."}'></textarea>
            <div class="ds-hint">{{ annotationsHint }}</div>
          </div>
        </div>
      </div>
      <template #footer>
        <div class="ds-modal-actions">
          <button type="button" class="ds-btn-ghost" @click="dlg.visible = false">取消</button>
          <button type="button" class="ds-btn-primary" @click="onDlgSave">保存</button>
        </div>
      </template>
    </el-dialog>
  </div>
</template>

<style scoped>
.panel {
  background: var(--el-bg-color, #fff);
  border-radius: 8px;
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.03);
  border: 1px solid var(--el-border-color-lighter, #ebeef5);
  margin-bottom: 14px;
}

/* === 分页条 === */
.rules-pager {
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex-wrap: wrap;
  gap: 12px;
  padding: 12px 18px;
  margin: 14px 0;
  border: 1px solid var(--el-border-color-lighter, #ebeef5);
  background: var(--el-bg-color, #fff);
  border-radius: 8px;
  box-shadow: 0 1px 2px rgba(0,0,0,0.03);
}
.rules-pager .pager-tip {
  font-size: 13px;
  color: var(--el-text-color-secondary, #909399);
}
.rules-pager .mono {
  font-family: Consolas, Menlo, monospace;
  font-weight: 600;
  color: var(--el-text-color-primary, #303133);
  padding: 0 2px;
}

.rules-view {
  padding: 8px 16px 24px 16px;
}
.page-header {
  margin: 4px 4px 12px 4px;
}
.page-title {
  margin: 0;
  font-size: 20px;
  font-weight: 600;
  color: var(--el-text-color-primary, var(--heading));
}
.page-sub {
  margin: 4px 0 0 0;
  color: var(--el-text-color-secondary, var(--muted));
  font-size: 13px;
}
.affix-wrap {
  z-index: 20;
}
.toolbar {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 0;
  background: var(--el-bg-color, var(--panel));
  border-bottom: 1px solid var(--el-border-color-lighter, var(--line-soft));
}
.toolbar-filters {
  display: flex;
  gap: 8px;
  margin-left: auto;
}

/* ============================================================
 * 表格样式
 * ============================================================ */
.rule-name {
  font-weight: 500;
  color: var(--el-text-color-primary, var(--heading));;
  cursor: pointer;
}
.rule-name:hover {
  color: var(--el-color-primary, var(--primary));;
}
.mono-cell {
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  font-size: 12px;
  color: var(--el-text-color-primary, var(--heading));;
}

/* ============================================================
 * Dialog 样式（与老版 .modal 一致）
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
  border-top: 1px solid var(--el-border-color, var(--line));
  display: flex;
  justify-content: flex-end;
  gap: 10px;
  margin: 0 -20px -20px -20px;
  background: var(--el-bg-color, var(--panel));
}

/* Seg (分组) */
.ds-seg {
  margin: 0 0 20px;
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

/* Field / Row */
.ds-field {
  margin-bottom: 16px;
}
.ds-field label {
  display: block;
  font-size: 13px;
  font-weight: 500;
  color: var(--el-text-color-primary, var(--heading));;
  margin-bottom: 6px;
}
.ds-field input[type="text"],
.ds-field input[type="number"],
.ds-field select,
.ds-field textarea {
  width: 100%;
  padding: 8px 12px;
  font-size: 13px;
  color: var(--el-text-color-primary, var(--heading));
  background: var(--el-bg-color, var(--panel));
  border: 1px solid var(--el-border-color, var(--line));
  border-radius: 6px;
  outline: none;
  transition: border-color 0.15s, box-shadow 0.15s;
  box-sizing: border-box;
}
.ds-field input:focus,
.ds-field select:focus,
.ds-field textarea:focus {
  border-color: var(--el-color-primary, var(--primary));;
  box-shadow: 0 0 0 2px rgba(64, 158, 255, 0.15);
}
.ds-field textarea {
  resize: vertical;
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
}
.ds-row {
  display: flex;
  gap: 16px;
  margin-bottom: 4px;
}
.ds-row .ds-field {
  flex: 1;
  min-width: 140px;
}

/* Check Row */
.ds-check-row {
  display: flex;
  align-items: center;
  gap: 8px;
  cursor: pointer;
  font-size: 13px;
  font-weight: 400;
  margin-bottom: 0 !important;
}
.ds-check-row input[type="checkbox"] {
  width: 16px;
  height: 16px;
  accent-color: var(--el-color-primary, var(--primary));;
  cursor: pointer;
}

/* Hint */
.ds-hint {
  margin-top: 6px;
  font-size: 12px;
  color: var(--el-text-color-secondary, var(--muted));;
  line-height: 1.5;
}

/* Multi-select hint */
.ds-ms-hint {
  margin-top: 6px;
  padding: 6px 10px;
  border-radius: 8px;
  background: var(--el-fill-color-light, var(--inset-bg));;
  border: 1px dashed var(--el-border-color, var(--line));;
  color: var(--el-text-color-secondary, var(--muted));;
  font-size: 12px;
  line-height: 1.5;
}
.ds-ms-hint b {
  color: var(--el-color-primary, var(--primary));;
  font-weight: 600;
}

/* Multi-select styling */
.ds-field select[multiple] {
  min-height: 140px;
  padding: 6px;
  border-radius: 10px;
  background: var(--el-fill-color-light, var(--inset-bg));
  border: 1px solid var(--el-border-color-lighter, var(--line-soft));
  line-height: 1.45;
  scrollbar-width: thin;
}
.ds-field select[multiple]:hover {
  border-color: var(--el-color-primary-light-5, var(--primary));
}
.ds-field select[multiple]:focus {
  border-color: var(--el-color-primary, var(--primary));
  box-shadow: 0 0 0 3px rgba(64, 158, 255, 0.15);
}
.ds-field select[multiple] option {
  padding: 7px 10px;
  border-radius: 6px;
  margin-bottom: 2px;
  font-size: 13px;
  color: var(--el-text-color-regular, var(--text));
  background: var(--el-bg-color, var(--panel));
  transition: background 0.12s, color 0.12s;
}
.ds-field select[multiple] option:checked {
  background: linear-gradient(180deg, rgba(64, 158, 255, 0.26), rgba(64, 158, 255, 0.18));
  color: var(--el-text-color-primary, var(--heading));
  box-shadow: inset 2px 0 0 var(--el-color-primary, var(--primary));
  font-weight: 500;
}

/* Buttons */
.ds-btn-ghost {
  padding: 8px 18px;
  font-size: 13px;
  color: var(--el-text-color-secondary, var(--muted));;
  background: transparent;
  border: 1px solid var(--el-border-color, var(--line));
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
 * Drawer (试跑结果) 样式
 * ============================================================ */
.eval-head {
  display: flex;
  align-items: center;
  gap: 12px;
}
.eval-counts {
  color: var(--el-text-color-regular, var(--text));
  font-size: 13px;
}
.raw-pre {
  margin: 0;
  max-height: 160px;
  overflow: auto;
  font-size: 12px;
  background: var(--el-fill-color-light, var(--inset-bg));
  padding: 8px;
  border-radius: 4px;
  white-space: pre-wrap;
  word-break: break-all;
}
.summary-list {
  margin: 4px 0;
  padding-left: 20px;
  color: var(--el-text-color-regular, var(--text));
}
.summary-list li {
  line-height: 1.7;
}
.more-tip {
  color: var(--el-text-color-secondary, var(--muted));
  font-size: 12px;
  margin-top: 4px;
}
.labels-cell {
  font-size: 12px;
  color: var(--el-text-color-regular, var(--text));
}
</style>
