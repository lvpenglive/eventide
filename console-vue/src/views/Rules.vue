<script setup lang="ts">
import { computed, onMounted, reactive, ref, markRaw } from 'vue'
import { useRouter } from 'vue-router'
import {
  ElAffix,
  ElButton,
  ElCollapse,
  ElCollapseItem,
  ElDescriptions,
  ElDescriptionsItem,
  ElDialog,
  ElDrawer,
  ElEmpty,
  ElForm,
  ElFormItem,
  ElInput,
  ElInputNumber,
  ElMessage,
  ElMessageBox,
  ElOption,
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
  CircleCheck,
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
export type Comparator = '>' | '>=' | '<' | '<=' | '==' | '!='

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
  { value: 'disaster', label: '灾害 (disaster)' },
  { value: 'high', label: '严重 (high)' },
  { value: 'average', label: '一般 (average)' },
  { value: 'warning', label: '警告 (warning)' },
  { value: 'information', label: '信息 (information)' },
  { value: 'not_classified', label: '未分类 (not_classified)' },
]
const COMPARATOR_OPTIONS: { value: Comparator; label: string }[] = [
  { value: '>', label: '> (大于)' },
  { value: '>=', label: '>= (大于等于)' },
  { value: '<', label: '< (小于)' },
  { value: '<=', label: '<= (小于等于)' },
  { value: '==', label: '== (等于)' },
  { value: '!=', label: '!= (不等于)' },
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

// Dialog 折叠面板状态
const activePanel = ref<string>('basic')
const escalateActive = ref<string>('')

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
function formatCondition(r: LocalRule): string {
  const thr = (r.threshold ?? 0).toFixed(2)
  return `${r.comparator} ${thr}`
}
function formatForInterval(r: LocalRule): string {
  return `持续 ${r.for_seconds ?? 0}s / 每 ${r.interval_seconds ?? 0}s 评估`
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
interface KvRow { key: string; value: string }
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
  labels: KvRow[]
  annotations: KvRow[]
  channel_ids: string[]
  escalate_after_seconds: number
  escalate_severity: RuleSeverity | ''
  escalate_channel_ids: string[]
}
function emptyKv(): KvRow[] { return [{ key: '', value: '' }] }
function mapToRows(m: Record<string, string> | undefined): KvRow[] {
  if (!m || Object.keys(m).length === 0) return emptyKv()
  return Object.entries(m).map(([k, v]) => ({ key: k, value: v }))
}
function rowsToMap(rows: KvRow[]): Record<string, string> {
  const out: Record<string, string> = {}
  for (const r of rows) {
    if (r.key.trim()) out[r.key.trim()] = r.value
  }
  return out
}

const dlg = reactive<DialogState>({
  visible: false,
  mode: 'create',
  id: '',
  name: '',
  datasource_id: '',
  enabled: true,
  expr: '',
  comparator: '>=',
  threshold: null,
  severity: 'warning',
  for_seconds: 60,
  interval_seconds: 30,
  labels: emptyKv(),
  annotations: emptyKv(),
  channel_ids: [],
  escalate_after_seconds: 0,
  escalate_severity: '',
  escalate_channel_ids: [],
})

const dlgFormRef = ref<FormInstance | null>(null)
const dlgFormRules: FormRules<DialogState> = {
  name: [{ required: true, message: '请输入规则名称', trigger: 'blur' }],
  datasource_id: [{ required: true, message: '请选择数据源', trigger: 'change' }],
  expr: [{ required: true, message: '请输入 expr（PromQL / LogQL 等）', trigger: 'blur' }],
  threshold: [{ required: true, message: '请输入阈值 threshold', trigger: 'change' }],
}

async function openCreate() {
  dlg.visible = true
  dlg.mode = 'create'
  dlg.id = ''
  dlg.name = ''
  dlg.datasource_id = ''
  dlg.enabled = true
  dlg.expr = ''
  dlg.comparator = '>='
  dlg.threshold = null
  dlg.severity = 'warning'
  dlg.for_seconds = 60
  dlg.interval_seconds = 30
  dlg.labels = emptyKv()
  dlg.annotations = emptyKv()
  dlg.channel_ids = []
  dlg.escalate_after_seconds = 0
  dlg.escalate_severity = ''
  dlg.escalate_channel_ids = []
  activePanel.value = 'basic'
  escalateActive.value = ''
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
  dlg.for_seconds = rule.for_seconds ?? 60
  dlg.interval_seconds = rule.interval_seconds ?? 30
  dlg.labels = mapToRows(rule.labels)
  dlg.annotations = mapToRows(rule.annotations)
  dlg.channel_ids = rule.channel_ids ? [...rule.channel_ids] : []
  dlg.escalate_after_seconds = rule.escalate_after_seconds ?? 0
  dlg.escalate_severity = rule.escalate_severity ?? ''
  dlg.escalate_channel_ids = rule.escalate_channel_ids ? [...rule.escalate_channel_ids] : []
  activePanel.value = 'basic'
  escalateActive.value = rule.escalate_after_seconds > 0 ? 'escalate' : ''
}
function kvAdd(target: 'labels' | 'annotations') {
  dlg[target].push({ key: '', value: '' })
}
function kvRemove(target: 'labels' | 'annotations', idx: number) {
  const arr = dlg[target]
  if (arr.length <= 1) {
    arr[0] = { key: '', value: '' }
  } else {
    arr.splice(idx, 1)
  }
}

async function onDlgSave() {
  const valid = await dlgFormRef.value?.validate().catch(() => false)
  if (!valid) return
  if (dlg.threshold === null || Number.isNaN(dlg.threshold)) {
    ElMessage.warning('请填写阈值 threshold')
    return
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
    labels: rowsToMap(dlg.labels),
    annotations: rowsToMap(dlg.annotations),
    channel_ids: dlg.channel_ids,
    escalate_after_seconds: dlg.escalate_after_seconds,
    escalate_severity: dlg.escalate_severity || null,
    escalate_channel_ids: dlg.escalate_channel_ids,
    enabled: dlg.enabled,
  }
  try {
    if (dlg.mode === 'create') {
      await createRule(body)
      ElMessage.success('规则已创建')
    } else {
      await updateRule(dlg.id, body)
      ElMessage.success('规则已更新')
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
    <ElTable
      v-else
      :data="filteredRules"
      v-loading="loading"
      stripe
      style="width: 100%; margin-top: 12px"
      empty-text="暂无匹配的规则，试试调整筛选条件或点击新建。"
    >
      <ElTableColumn label="名称" min-width="180">
        <template #default="{ row }">
          <ElTooltip :content="asRule(row).name" placement="top">
            <a class="name-link" @click="openEdit(asRule(row).id)">{{ asRule(row).name }}</a>
          </ElTooltip>
        </template>
      </ElTableColumn>

      <ElTableColumn label="数据源" min-width="160">
        <template #default="{ row }">
          {{ formatDatasource(asRule(row).datasource_id) }}
        </template>
      </ElTableColumn>

      <ElTableColumn label="表达式 (expr)" min-width="260">
        <template #default="{ row }">
          <ElTooltip :content="asRule(row).expr" placement="top" :show-after="300">
            <code class="expr-cell">{{ truncateExpr(asRule(row).expr) }}</code>
          </ElTooltip>
        </template>
      </ElTableColumn>

      <ElTableColumn label="条件" width="130">
        <template #default="{ row }">
          <code>{{ formatCondition(asRule(row)) }}</code>
        </template>
      </ElTableColumn>

      <ElTableColumn label="严重级别" width="120">
        <template #default="{ row }">
          <ElTag
            :color="SEVERITY_COLORS[sev(asRule(row).severity)]"
            effect="dark"
            style="color: #fff"
          >
            {{ SEVERITY_NAMES[sev(asRule(row).severity)] }}
          </ElTag>
        </template>
      </ElTableColumn>

      <ElTableColumn label="持续 / 评估周期" width="190">
        <template #default="{ row }">
          {{ formatForInterval(asRule(row)) }}
        </template>
      </ElTableColumn>

      <ElTableColumn label="启用" width="80" align="center">
        <template #default="{ row }">
          <ElSwitch
            :model-value="asRule(row).enabled"
            @update:model-value="(v) => { asRule(row).enabled = Boolean(v); onToggleEnabled(asRule(row)) }"
          />
        </template>
      </ElTableColumn>

      <ElTableColumn label="最近更新" width="170">
        <template #default="{ row }">
          {{ formatTime(asRule(row).updated_at) }}
        </template>
      </ElTableColumn>

      <ElTableColumn label="操作" width="220" fixed="right">
        <template #default="{ row }">
          <ElButton
            link
            type="primary"
            :icon="VideoPlay"
            @click="onEvaluate(asRule(row))"
          >
            试跑
          </ElButton>
          <ElButton link type="primary" :icon="Edit" @click="openEdit(asRule(row).id)">
            编辑
          </ElButton>
          <ElButton link type="danger" :icon="Delete" @click="onDelete(asRule(row))">
            删除
          </ElButton>
        </template>
      </ElTableColumn>
    </ElTable>

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
              :icon="CircleCheck"
              effect="light"
            >
              执行成功
            </ElTag>
            <ElTag v-else type="danger" effect="light">
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
    <ElDialog
      v-model="dlg.visible"
      :title="dlg.mode === 'create' ? '新建规则' : '编辑规则'"
      width="780px"
      :close-on-click-modal="false"
      destroy-on-close
    >
      <ElForm
        ref="dlgFormRef"
        :model="dlg"
        :rules="dlgFormRules"
        label-width="130px"
        label-position="right"
      >
        <ElCollapse v-model="activePanel" accordion>
          <!-- 基础 -->
          <ElCollapseItem title="基础信息" name="basic">
            <ElRow :gutter="16">
              <ElCol :span="16">
                <ElFormItem label="规则名称" prop="name">
                  <ElInput v-model="dlg.name" placeholder="例如：CPU 使用率过高" />
                </ElFormItem>
              </ElCol>
              <ElCol :span="8">
                <ElFormItem label="启用" prop="enabled">
                  <ElSwitch v-model="dlg.enabled" />
                </ElFormItem>
              </ElCol>
            </ElRow>
            <ElFormItem label="数据源" prop="datasource_id">
              <ElSelect
                v-model="dlg.datasource_id"
                placeholder="请选择数据源（Prometheus / Kafka / Loki 等）"
                clearable
                filterable
                style="width: 100%"
              >
                <ElOption
                  v-for="d in datasources"
                  :key="d.id"
                  :label="`${d.name} (${d.kind})`"
                  :value="d.id"
                />
              </ElSelect>
            </ElFormItem>
          </ElCollapseItem>

          <!-- 触发与评估 -->
          <ElCollapseItem title="触发与评估" name="trigger">
            <ElFormItem label="表达式 expr" prop="expr">
              <ElInput
                v-model="dlg.expr"
                type="textarea"
                :rows="4"
                placeholder="PromQL / LogQL / Kafka topic:tag-key"
              />
            </ElFormItem>
            <ElRow :gutter="16">
              <ElCol :span="8">
                <ElFormItem label="比较符" prop="comparator">
                  <ElSelect v-model="dlg.comparator" style="width: 100%">
                    <ElOption
                      v-for="c in COMPARATOR_OPTIONS"
                      :key="c.value"
                      :label="c.label"
                      :value="c.value"
                    />
                  </ElSelect>
                </ElFormItem>
              </ElCol>
              <ElCol :span="8">
                <ElFormItem label="阈值 threshold" prop="threshold">
                  <ElInputNumber
                    v-model="dlg.threshold"
                    :step="0.01"
                    :precision="2"
                    style="width: 100%"
                    controls-position="right"
                  />
                </ElFormItem>
              </ElCol>
              <ElCol :span="8">
                <ElFormItem label="严重级别" prop="severity">
                  <ElSelect v-model="dlg.severity" style="width: 100%">
                    <ElOption
                      v-for="s in SEVERITY_OPTIONS"
                      :key="s.value"
                      :label="s.label"
                      :value="s.value"
                    />
                  </ElSelect>
                </ElFormItem>
              </ElCol>
            </ElRow>
            <ElRow :gutter="16">
              <ElCol :span="12">
                <ElFormItem label="持续 (for_seconds)">
                  <ElInputNumber
                    v-model="dlg.for_seconds"
                    :min="0"
                    :step="5"
                    style="width: 100%"
                    controls-position="right"
                  />
                  <div class="form-tip">达到阈值后持续 N 秒才触发，默认 60s</div>
                </ElFormItem>
              </ElCol>
              <ElCol :span="12">
                <ElFormItem label="评估周期 (interval)">
                  <ElInputNumber
                    v-model="dlg.interval_seconds"
                    :min="5"
                    :step="5"
                    style="width: 100%"
                    controls-position="right"
                  />
                  <div class="form-tip">每隔 N 秒评估一次，最小 5s，默认 30s</div>
                </ElFormItem>
              </ElCol>
            </ElRow>

            <!-- Labels 动态行 -->
            <div class="kv-section">
              <div class="kv-title">
                <span>标签 Labels（如 instance、team 等）</span>
                <ElButton size="small" :icon="Plus" link type="primary" @click="kvAdd('labels')">新增一行</ElButton>
              </div>
              <div v-for="(r, idx) in dlg.labels" :key="'lb-'+idx" class="kv-row">
                <ElInput v-model="r.key" placeholder="key" style="width: 40%; margin-right: 8px" />
                <ElInput v-model="r.value" placeholder="value" style="width: 40%; margin-right: 8px" />
                <ElButton link type="danger" :icon="Delete" @click="kvRemove('labels', idx)">删除</ElButton>
              </div>
            </div>

            <!-- Annotations 动态行 -->
            <div class="kv-section" style="margin-top: 8px">
              <div class="kv-title">
                <span>注解 Annotations（如 summary、runbook_url）</span>
                <ElButton size="small" :icon="Plus" link type="primary" @click="kvAdd('annotations')">新增一行</ElButton>
              </div>
              <div v-for="(r, idx) in dlg.annotations" :key="'an-'+idx" class="kv-row">
                <ElInput v-model="r.key" placeholder="key" style="width: 40%; margin-right: 8px" />
                <ElInput v-model="r.value" placeholder="value" style="width: 40%; margin-right: 8px" />
                <ElButton link type="danger" :icon="Delete" @click="kvRemove('annotations', idx)">删除</ElButton>
              </div>
            </div>
          </ElCollapseItem>

          <!-- 通知与升级 -->
          <ElCollapseItem title="通知与升级" name="notify">
            <ElFormItem label="通知渠道">
              <ElSelect
                v-model="dlg.channel_ids"
                multiple
                filterable
                placeholder="选择要投递的渠道（可多选）"
                style="width: 100%"
              >
                <ElOption
                  v-for="c in channels"
                  :key="c.id"
                  :label="`${c.name} (${c.kind})`"
                  :value="c.id"
                />
              </ElSelect>
            </ElFormItem>

            <ElCollapse v-model="escalateActive" accordion class="nested-collapse">
              <ElCollapseItem title="升级策略（默认折叠）" name="escalate">
                <ElFormItem label="升级延迟秒数">
                  <ElInputNumber
                    v-model="dlg.escalate_after_seconds"
                    :min="0"
                    :step="300"
                    style="width: 100%"
                    controls-position="right"
                  />
                  <div class="form-tip">默认 0 = 关闭；填写 300 表示持续 5 分钟未恢复则升级（step=5min）</div>
                </ElFormItem>
                <ElRow :gutter="16">
                  <ElCol :span="12">
                    <ElFormItem label="升级后严重级别">
                      <ElSelect
                        v-model="dlg.escalate_severity"
                        clearable
                        placeholder="不选则保留 rule.severity"
                        style="width: 100%"
                      >
                        <ElOption
                          v-for="s in SEVERITY_OPTIONS"
                          :key="s.value"
                          :label="s.label"
                          :value="s.value"
                        />
                      </ElSelect>
                    </ElFormItem>
                  </ElCol>
                  <ElCol :span="12">
                    <ElFormItem label="升级渠道">
                      <ElSelect
                        v-model="dlg.escalate_channel_ids"
                        multiple
                        filterable
                        placeholder="空 = 沿用通知渠道"
                        style="width: 100%"
                      >
                        <ElOption
                          v-for="c in channels"
                          :key="c.id"
                          :label="`${c.name} (${c.kind})`"
                          :value="c.id"
                        />
                      </ElSelect>
                    </ElFormItem>
                  </ElCol>
                </ElRow>
              </ElCollapseItem>
            </ElCollapse>
          </ElCollapseItem>
        </ElCollapse>
      </ElForm>

      <template #footer>
        <ElButton @click="dlg.visible = false">取消</ElButton>
        <ElButton type="primary" @click="onDlgSave">保存</ElButton>
      </template>
    </ElDialog>
  </div>
</template>

<style scoped>
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
  color: #303133;
}
.page-sub {
  margin: 4px 0 0 0;
  color: #909399;
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
  background: #fff;
  border-bottom: 1px solid #ebeef5;
}
.toolbar-filters {
  display: flex;
  gap: 8px;
  margin-left: auto;
}
.name-link {
  color: #409eff;
  cursor: pointer;
  text-decoration: none;
}
.name-link:hover {
  text-decoration: underline;
}
.expr-cell {
  font-family: Consolas, Monaco, monospace;
  background: #f5f7fa;
  padding: 2px 6px;
  border-radius: 4px;
  font-size: 12px;
  color: #606266;
}
.labels-cell {
  font-size: 12px;
  color: #606266;
}
.eval-head {
  display: flex;
  align-items: center;
  gap: 12px;
}
.eval-counts {
  color: #606266;
  font-size: 13px;
}
.raw-pre {
  margin: 0;
  max-height: 160px;
  overflow: auto;
  font-size: 12px;
  background: #f5f7fa;
  padding: 8px;
  border-radius: 4px;
  white-space: pre-wrap;
  word-break: break-all;
}
.summary-list {
  margin: 4px 0;
  padding-left: 20px;
  color: #606266;
}
.summary-list li {
  line-height: 1.7;
}
.more-tip {
  color: #909399;
  font-size: 12px;
  margin-top: 4px;
}
.kv-section {
  margin-top: 8px;
}
.kv-title {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 6px;
  font-weight: 500;
  color: #303133;
  font-size: 13px;
}
.kv-row {
  display: flex;
  align-items: center;
  margin-bottom: 6px;
}
.form-tip {
  font-size: 12px;
  color: #909399;
  margin-top: 4px;
}
.nested-collapse {
  margin-top: 8px;
  border: 1px solid #ebeef5;
  border-radius: 6px;
  padding: 0 8px;
}
</style>
