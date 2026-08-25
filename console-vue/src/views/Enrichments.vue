<script setup lang="ts">
import { computed, onMounted, reactive, ref, markRaw } from 'vue'
import {
  ElAffix,
  ElButton,
  ElCollapse,
  ElCollapseItem,
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
  Delete,
  Edit,
  Plus,
  Refresh,
  VideoPlay,
} from '@element-plus/icons-vue'

// ============================================================
// 本地补齐类型（不改动外部文件）
// ============================================================
export type EnrichKind = 'AnnotationTemplate' | 'LabelMap' | 'Lookup' | 'Composite'

export interface EnrichRow {
  id: string
  name: string
  kind: EnrichKind
  matchers: Record<string, string>
  priority: number
  enabled: boolean
  templates?: Record<string, string> | null
  field_templates?: Record<string, string> | null
  label_extracts?: Record<string, string> | null
  write_labels?: boolean | null
  match_key?: string | null
  mappings?: Record<string, Record<string, string>> | null
  lookup_table_ids?: string[] | null
  lookup_match_keys?: Record<string, string> | null
  created_at: string
  updated_at: string
}

export interface EnrichInput {
  name: string
  kind?: EnrichKind
  enabled?: boolean
  priority?: number
  matchers?: Record<string, string>
  templates?: Record<string, string>
  field_templates?: Record<string, string>
  label_extracts?: Record<string, string>
  write_labels?: boolean
  match_key?: string
  mappings?: Record<string, Record<string, string>>
  lookup_table_ids?: string[]
  lookup_match_keys?: Record<string, string>
}

export interface LookupBrief {
  id: string
  name: string
  key_label?: string
  enabled?: boolean
}

export interface PreviewAlertInput {
  matchers?: Record<string, string>
  labels?: Record<string, string>
  annotations?: Record<string, string>
  severity?: string
  status?: string
  [k: string]: unknown
}

export interface PreviewAlertResult {
  labels?: Record<string, string>
  annotations?: Record<string, string>
  severity?: string
  status?: string
  summary?: string
  [k: string]: unknown
}

export interface PreviewEnrichResp {
  ok: boolean
  original?: PreviewAlertInput
  result?: PreviewAlertResult
  error?: string
  detail?: unknown
}

// ============================================================
// 兜底模块绑定
// ============================================================
interface EnrichmentsApiShim {
  listEnrich(): Promise<EnrichRow[]>
  getEnrich(id: string): Promise<EnrichRow>
  createEnrich(body: EnrichInput): Promise<EnrichRow>
  updateEnrich(id: string, body: Partial<EnrichInput>): Promise<EnrichRow>
  deleteEnrich(id: string): Promise<{ ok: boolean }>
  previewEnrich(body: PreviewAlertInput & { enrich_id?: string }): Promise<PreviewEnrichResp>
  listLookups(): Promise<LookupBrief[]>
}
function _unimpl(name: string): never {
  throw new Error(`[API shim] 模块未实现：${name}，请等待并行任务创建对应 api/*.ts`)
}
const _shimEnrich: EnrichmentsApiShim = {
  listEnrich: () => Promise.reject(_unimpl('listEnrich')),
  getEnrich: () => Promise.reject(_unimpl('getEnrich')),
  createEnrich: () => Promise.reject(_unimpl('createEnrich')),
  updateEnrich: () => Promise.reject(_unimpl('updateEnrich')),
  deleteEnrich: () => Promise.reject(_unimpl('deleteEnrich')),
  previewEnrich: () => Promise.reject(_unimpl('previewEnrich')),
  listLookups: () => Promise.resolve([]),
}
// @ts-ignore 若 @/api/enrichments 模块尚未创建则忽略解析错误
import * as _rawEnrich from '@/api/enrichments'
const _enrich = markRaw(_rawEnrich as unknown as EnrichmentsApiShim | Record<string, unknown>)
function _bindApi<A extends object, S>(api: A | Record<string, unknown>, shim: S): S {
  const out = { ...(shim as unknown as object) } as S
  for (const key of Object.keys(shim as unknown as object)) {
    if (key in api && typeof (api as Record<string, unknown>)[key] === 'function') {
      ;(out as unknown as Record<string, unknown>)[key] = (api as Record<string, unknown>)[key]
    }
  }
  return out
}
const _enrichApi = _bindApi<Record<string, unknown>, EnrichmentsApiShim>(
  _enrich as unknown as Record<string, unknown>,
  _shimEnrich,
)
const {
  listEnrich,
  getEnrich,
  createEnrich,
  updateEnrich,
  deleteEnrich,
  previewEnrich,
  listLookups,
} = _enrichApi

// ============================================================
// 常量：kind 彩色 Tag / 中文名 / field_templates 下拉
// ============================================================
const KIND_COLORS: Record<EnrichKind, string> = {
  AnnotationTemplate: '#409eff',
  LabelMap: '#67c23a',
  Lookup: '#e6a23c',
  Composite: '#909399',
}
const KIND_NAMES: Record<EnrichKind, string> = {
  AnnotationTemplate: 'Annotation 模板',
  LabelMap: 'Label 映射',
  Lookup: 'Lookup 外表',
  Composite: '复合组合',
}
const KIND_OPTIONS: { value: EnrichKind; label: string }[] = [
  { value: 'AnnotationTemplate', label: 'Annotation 模板 (AnnotationTemplate)' },
  { value: 'LabelMap', label: 'Label 映射 (LabelMap)' },
  { value: 'Lookup', label: 'Lookup 外表 (Lookup)' },
  { value: 'Composite', label: '复合组合 (Composite)' },
]
const FIELD_TEMPLATE_KEY_OPTIONS: string[] = [
  'severity', 'ip', 'alertname', 'summary', 'status', 'fingerprint',
]

// ============================================================
// 状态：列表、筛选、字典
// ============================================================
const loading = ref(false)
const enrichments = ref<EnrichRow[]>([])
const lookups = ref<LookupBrief[]>([])

const filterKind = ref<EnrichKind | ''>('')
const filterQ = ref('')
const filterEnabled = ref<'' | 'true' | 'false'>('')

const filteredEnrichments = computed(() => {
  return enrichments.value.filter((r) => {
    if (filterKind.value && r.kind !== filterKind.value) return false
    if (filterEnabled.value === 'true' && !r.enabled) return false
    if (filterEnabled.value === 'false' && r.enabled) return false
    if (filterQ.value.trim()) {
      const q = filterQ.value.trim().toLowerCase()
      if (!r.name.toLowerCase().includes(q)) return false
    }
    return true
  })
})

const lookupMap = computed(() => {
  const m = new Map<string, LookupBrief>()
  for (const l of lookups.value) m.set(l.id, l)
  return m
})
function lookupName(id: string): string {
  const l = lookupMap.value.get(id)
  return l ? l.name : id
}

// Dialog 折叠面板状态（非 accordion：多面板同时展开，使用 string[]）
const activePanel = ref<string[]>(['basic'])

// ============================================================
// 辅助函数
// ============================================================
function asEnrich(r: unknown): EnrichRow { return r as EnrichRow }
function formatTime(s: string | null | undefined): string {
  if (!s) return '—'
  return s.replace('T', ' ').slice(0, 19)
}
function matchersSummary(m: Record<string, string> | undefined): { text: string; more: number } {
  if (!m) return { text: '', more: 0 }
  const entries = Object.entries(m)
  const top = entries.slice(0, 3).map(([k, v]) => `${k}=${v}`).join(' / ')
  return { text: top, more: entries.length > 3 ? entries.length - 3 : 0 }
}
function prettyJson(v: unknown): string {
  try {
    return JSON.stringify(v, null, 2)
  } catch {
    return String(v)
  }
}

async function loadAll() {
  loading.value = true
  try {
    const [es, ls] = await Promise.all([
      listEnrich().catch((e) => { ElMessage.error(String(e?.message || e)); return [] as EnrichRow[] }),
      listLookups().catch(() => [] as LookupBrief[]),
    ])
    enrichments.value = es
    lookups.value = ls
  } finally {
    loading.value = false
  }
}

onMounted(loadAll)

// ============================================================
// enabled 开关：乐观更新 + 失败回滚
// ============================================================
async function onToggleEnabled(row: EnrichRow) {
  const newVal = row.enabled
  try {
    await updateEnrich(row.id, { enabled: newVal })
    ElMessage.success(`已${newVal ? '启用' : '停用'}`)
    const fresh = await getEnrich(row.id).catch(() => null)
    if (fresh) {
      const idx = enrichments.value.findIndex((r) => r.id === row.id)
      if (idx >= 0) enrichments.value[idx] = fresh
    }
  } catch (e) {
    row.enabled = !newVal
    ElMessage.error(`更新失败：${String((e as Error)?.message || e)}`)
  }
}

// ============================================================
// 试跑 Drawer (direction=rtl)
// ============================================================
interface KvRow { key: string; value: string }
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

const drawerVisible = ref(false)
const previewLoading = ref(false)
const previewResult = ref<PreviewEnrichResp | null>(null)
const previewEnrichId = ref<string>('')
const previewEnrichName = ref('')

const previewForm = reactive<{
  matchers: KvRow[]
  labels: KvRow[]
  annotations: KvRow[]
  severity: string
  status: string
}>({
  matchers: emptyKv(),
  labels: emptyKv(),
  annotations: emptyKv(),
  severity: 'warning',
  status: 'firing',
})

function kvRowAdd(arr: KvRow[]) { arr.push({ key: '', value: '' }) }
function kvRowRemove(arr: KvRow[], idx: number) {
  if (arr.length <= 1) {
    arr[0] = { key: '', value: '' }
  } else {
    arr.splice(idx, 1)
  }
}

function previewOpen(row: EnrichRow) {
  previewEnrichId.value = row.id
  previewEnrichName.value = row.name
  previewResult.value = null
  // 预置 matchers 从 enrich 里取
  previewForm.matchers = mapToRows(row.matchers || {})
  previewForm.labels = emptyKv()
  previewForm.annotations = emptyKv()
  previewForm.severity = 'warning'
  previewForm.status = 'firing'
  drawerVisible.value = true
}

async function previewExecute() {
  previewLoading.value = true
  previewResult.value = null
  try {
    const body: PreviewAlertInput & { enrich_id?: string } = {
      enrich_id: previewEnrichId.value || undefined,
      matchers: rowsToMap(previewForm.matchers),
      labels: rowsToMap(previewForm.labels),
      annotations: rowsToMap(previewForm.annotations),
      severity: previewForm.severity,
      status: previewForm.status,
    }
    previewResult.value = await previewEnrich(body)
  } catch (e) {
    previewResult.value = {
      ok: false,
      error: String((e as Error)?.message || e),
    }
  } finally {
    previewLoading.value = false
  }
}

// ============================================================
// 新建 / 编辑 Dialog
// ============================================================
/** 两级 Map 行 UI：第一列 match value，内嵌 kv Map */
interface MappingRow {
  match_value: string
  labels: KvRow[]
}
interface LookupMatchRow {
  table_id: string
  label_key: string
}

interface DialogState {
  visible: boolean
  mode: 'create' | 'edit'
  id: string
  name: string
  kind: EnrichKind
  enabled: boolean
  priority: number
  matchers: KvRow[]
  // 面板2：templates
  templates: KvRow[]
  // 面板3：field & extract
  field_templates: KvRow[]
  label_extracts: KvRow[]
  write_labels: boolean
  // 面板4：映射 & 外表
  match_key: string
  mappings: MappingRow[]
  lookup_table_ids: string[]
  lookup_match_keys: LookupMatchRow[]
}

function emptyMappingRow(): MappingRow {
  return { match_value: '', labels: emptyKv() }
}
function emptyDialog(): DialogState {
  return {
    visible: false,
    mode: 'create',
    id: '',
    name: '',
    kind: 'AnnotationTemplate',
    enabled: true,
    priority: 100,
    matchers: emptyKv(),
    templates: emptyKv(),
    field_templates: emptyKv(),
    label_extracts: emptyKv(),
    write_labels: false,
    match_key: '',
    mappings: [emptyMappingRow()],
    lookup_table_ids: [],
    lookup_match_keys: [],
  }
}

const dlg = reactive<DialogState>(emptyDialog())
const dlgFormRef = ref<FormInstance | null>(null)
const dlgFormRules: FormRules<DialogState> = {
  name: [{ required: true, message: '请输入丰富规则名称', trigger: 'blur' }],
  priority: [{ required: true, message: '请填写 priority（越小越先执行）', trigger: 'change' }],
}

function openCreate() {
  Object.assign(dlg, emptyDialog())
  dlg.visible = true
  activePanel.value = ['basic']
}

function mappingsRecordToRows(m: Record<string, Record<string, string>> | undefined): MappingRow[] {
  if (!m || Object.keys(m).length === 0) return [emptyMappingRow()]
  return Object.entries(m).map(([mv, labels]) => ({
    match_value: mv,
    labels: mapToRows(labels),
  }))
}
function mappingRowsToRecord(rows: MappingRow[]): Record<string, Record<string, string>> {
  const out: Record<string, Record<string, string>> = {}
  for (const r of rows) {
    if (!r.match_value.trim()) continue
    const lm = rowsToMap(r.labels)
    if (Object.keys(lm).length > 0) out[r.match_value.trim()] = lm
  }
  return out
}

function lmkRecordToRows(m: Record<string, string> | undefined): LookupMatchRow[] {
  if (!m || Object.keys(m).length === 0) return []
  return Object.entries(m).map(([table_id, label_key]) => ({ table_id, label_key }))
}
function lmkRowsToRecord(rows: LookupMatchRow[]): Record<string, string> {
  const out: Record<string, string> = {}
  for (const r of rows) {
    if (r.table_id && r.label_key.trim()) out[r.table_id] = r.label_key.trim()
  }
  return out
}

async function openEdit(id: string) {
  let e: EnrichRow | null = null
  try {
    e = await getEnrich(id)
  } catch (err) {
    ElMessage.error(`读取失败：${String((err as Error)?.message || err)}`)
    return
  }
  Object.assign(dlg, emptyDialog())
  dlg.visible = true
  dlg.mode = 'edit'
  dlg.id = e.id
  dlg.name = e.name
  dlg.kind = e.kind || 'AnnotationTemplate'
  dlg.enabled = e.enabled
  dlg.priority = typeof e.priority === 'number' ? e.priority : 100
  dlg.matchers = mapToRows(e.matchers || {})
  dlg.templates = mapToRows(e.templates || {})
  dlg.field_templates = mapToRows(e.field_templates || {})
  dlg.label_extracts = mapToRows(e.label_extracts || {})
  dlg.write_labels = Boolean(e.write_labels)
  dlg.match_key = e.match_key || ''
  dlg.mappings = mappingsRecordToRows(e.mappings || {})
  dlg.lookup_table_ids = e.lookup_table_ids ? [...e.lookup_table_ids] : []
  dlg.lookup_match_keys = lmkRecordToRows(e.lookup_match_keys || {})
  activePanel.value = ['basic']
}

// mapping & lmk 行编辑
function mappingAddRow() { dlg.mappings.push(emptyMappingRow()) }
function mappingRemoveRow(idx: number) {
  if (dlg.mappings.length <= 1) {
    dlg.mappings[0] = emptyMappingRow()
  } else {
    dlg.mappings.splice(idx, 1)
  }
}
function lmkAddRow() { dlg.lookup_match_keys.push({ table_id: '', label_key: '' }) }
function lmkRemoveRow(idx: number) { dlg.lookup_match_keys.splice(idx, 1) }

// 保存校验 & 提交
function validateNonEmpty(d: DialogState): string | null {
  const tpl = rowsToMap(d.templates)
  const ft = rowsToMap(d.field_templates)
  const le = rowsToMap(d.label_extracts)
  const mappings = mappingRowsToRecord(d.mappings)
  const lti = (d.lookup_table_ids || []).filter(Boolean)

  const hasTemplates = Object.keys(tpl).length > 0
  const hasFieldTemplates = Object.keys(ft).length > 0
  const hasLabelExtracts = Object.keys(le).length > 0
  const hasLabelMap = (d.match_key || '').trim() !== '' && Object.keys(mappings).length > 0
  const hasLookup = lti.length > 0

  if (!hasTemplates && !hasFieldTemplates && !hasLabelExtracts && !hasLabelMap && !hasLookup) {
    return '必须至少填写以下内容之一：templates 非空 / field_templates 非空 / label_extracts 非空 / (match_key + mappings 非空) / lookup_table_ids 非空'
  }
  return null
}

async function onDlgSave() {
  const valid = await dlgFormRef.value?.validate().catch(() => false)
  if (!valid) return

  const errMsg = validateNonEmpty(dlg)
  if (errMsg) {
    ElMessage.error(errMsg)
    return
  }

  const body: EnrichInput = {
    name: dlg.name.trim(),
    kind: dlg.kind,
    enabled: dlg.enabled,
    priority: dlg.priority,
    matchers: rowsToMap(dlg.matchers),
    templates: rowsToMap(dlg.templates),
    field_templates: rowsToMap(dlg.field_templates),
    label_extracts: rowsToMap(dlg.label_extracts),
    write_labels: dlg.write_labels,
    match_key: dlg.match_key.trim() || undefined,
    mappings: mappingRowsToRecord(dlg.mappings),
    lookup_table_ids: (dlg.lookup_table_ids || []).filter(Boolean),
    lookup_match_keys: lmkRowsToRecord(dlg.lookup_match_keys),
  }

  try {
    if (dlg.mode === 'create') {
      await createEnrich(body)
      ElMessage.success('丰富规则已创建')
    } else {
      await updateEnrich(dlg.id, body)
      ElMessage.success('丰富规则已更新')
    }
    dlg.visible = false
    await loadAll()
  } catch (e) {
    const msg = (e as Error)?.message || String(e)
    // 尝试从 detail 展开
    let detail: unknown = undefined
    if (e && typeof e === 'object' && 'detail' in e) detail = (e as Record<string, unknown>).detail
    if (detail !== undefined) {
      ElMessage.error(`保存失败：${msg}；detail: ${prettyJson(detail)}`)
    } else {
      ElMessage.error(`保存失败：${msg}`)
    }
  }
}

// ============================================================
// 删除（二次确认）
// ============================================================
async function onDelete(row: EnrichRow) {
  try {
    await ElMessageBox.confirm(
      `确定要删除丰富规则「${row.name}」？该操作不可恢复。`,
      '删除丰富规则',
      { type: 'warning', confirmButtonText: '删除', cancelButtonText: '取消' },
    )
  } catch {
    return
  }
  try {
    await deleteEnrich(row.id)
    ElMessage.success('已删除')
    await loadAll()
  } catch (e) {
    ElMessage.error(`删除失败：${String((e as Error)?.message || e)}`)
  }
}
</script>

<template>
  <div class="enrichments-view">
    <!-- 标题 -->
    <div class="page-header">
      <h2 class="page-title">告警丰富</h2>
      <p class="page-sub">Annotation 模板 / Label 映射 / Lookup 外表</p>
    </div>

    <!-- 吸附工具栏 -->
    <ElAffix :offset="0" class="affix-wrap">
      <div class="toolbar">
        <ElButton type="primary" :icon="Plus" @click="openCreate">新建丰富规则</ElButton>
        <ElButton :icon="Refresh" @click="loadAll" :loading="loading">刷新</ElButton>

        <div class="toolbar-filters">
          <ElSelect
            v-model="filterKind"
            placeholder="按 kind 筛选"
            clearable
            style="width: 220px"
          >
            <ElOption
              v-for="k in KIND_OPTIONS"
              :key="k.value"
              :label="k.label"
              :value="k.value"
            />
          </ElSelect>

          <ElInput
            v-model="filterQ"
            placeholder="搜索 name 包含"
            clearable
            style="width: 220px"
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

    <!-- 表格 -->
    <ElTable
      :data="filteredEnrichments"
      v-loading="loading"
      stripe
      style="width: 100%; margin-top: 12px"
      empty-text="暂无丰富规则，试试调整筛选或点击新建。"
    >
      <ElTableColumn label="名称" min-width="180">
        <template #default="{ row }">
          <ElTooltip :content="asEnrich(row).name" placement="top">
            <a class="name-link" @click="openEdit(asEnrich(row).id)">{{ asEnrich(row).name }}</a>
          </ElTooltip>
        </template>
      </ElTableColumn>

      <ElTableColumn label="kind" width="180">
        <template #default="{ row }">
          <ElTag
            :color="KIND_COLORS[asEnrich(row).kind]"
            effect="dark"
            style="color: #fff"
          >
            {{ KIND_NAMES[asEnrich(row).kind] || asEnrich(row).kind }}
          </ElTag>
        </template>
      </ElTableColumn>

      <ElTableColumn label="matchers" min-width="260">
        <template #default="{ row }">
          <div class="matchers-cell">
            <ElTag
              v-if="matchersSummary(asEnrich(row).matchers).text"
              type="info"
              effect="plain"
              size="small"
              style="margin-right: 4px; margin-bottom: 4px"
            >
              {{ matchersSummary(asEnrich(row).matchers).text }}
            </ElTag>
            <ElTag
              v-if="matchersSummary(asEnrich(row).matchers).more > 0"
              type="info"
              size="small"
            >
              +{{ matchersSummary(asEnrich(row).matchers).more }} 更多
            </ElTag>
            <ElTag v-else-if="!matchersSummary(asEnrich(row).matchers).text" type="info" size="small" effect="plain">
              （空 matchers：对所有告警执行）
            </ElTag>
          </div>
        </template>
      </ElTableColumn>

      <ElTableColumn label="priority" width="100" align="center">
        <template #default="{ row }">
          <code>{{ asEnrich(row).priority }}</code>
        </template>
      </ElTableColumn>

      <ElTableColumn label="启用" width="80" align="center">
        <template #default="{ row }">
          <ElSwitch
            :model-value="asEnrich(row).enabled"
            @update:model-value="(v) => { asEnrich(row).enabled = Boolean(v); onToggleEnabled(asEnrich(row)) }"
          />
        </template>
      </ElTableColumn>

      <ElTableColumn label="最近更新" width="170">
        <template #default="{ row }">
          {{ formatTime(asEnrich(row).updated_at) }}
        </template>
      </ElTableColumn>

      <ElTableColumn label="操作" width="240" fixed="right">
        <template #default="{ row }">
          <ElButton
            link
            type="primary"
            :icon="VideoPlay"
            @click="previewOpen(asEnrich(row))"
          >
            试跑
          </ElButton>
          <ElButton link type="primary" :icon="Edit" @click="openEdit(asEnrich(row).id)">
            编辑
          </ElButton>
          <ElButton link type="danger" :icon="Delete" @click="onDelete(asEnrich(row))">
            删除
          </ElButton>
        </template>
      </ElTableColumn>
    </ElTable>

    <!-- 空态 -->
    <ElEmpty
      v-if="!loading && enrichments.length === 0"
      description="尚无告警丰富规则，可先创建 AnnotationTemplate / LabelMap / Lookup 类型。"
    >
      <template #extra>
        <ElButton type="primary" :icon="Plus" @click="openCreate">新建丰富规则</ElButton>
      </template>
    </ElEmpty>

    <!-- 试跑 Drawer (direction=rtl) -->
    <ElDrawer
      v-model="drawerVisible"
      :title="`丰富规则试跑：${previewEnrichName}`"
      direction="rtl"
      size="900px"
      destroy-on-close
    >
      <ElRow :gutter="16">
        <!-- 左侧 Form：输入 matchers / labels / annotations ... -->
        <ElCol :span="11">
          <h3 style="margin: 0 0 8px 0">执行预览 · 输入</h3>
          <ElForm label-width="110px" label-position="right">
            <ElFormItem label="kind">
              <ElTag :color="KIND_COLORS[(enrichments.find(e => e.id === previewEnrichId)?.kind || 'AnnotationTemplate') as EnrichKind]" effect="dark" style="color:#fff">
                {{ KIND_NAMES[(enrichments.find(e => e.id === previewEnrichId)?.kind || 'AnnotationTemplate') as EnrichKind] }}
              </ElTag>
            </ElFormItem>

            <div class="kv-section">
              <div class="kv-title">
                <span>matchers (k/v)</span>
                <ElButton size="small" :icon="Plus" link type="primary" @click="kvRowAdd(previewForm.matchers)">新增</ElButton>
              </div>
              <div v-for="(r, idx) in previewForm.matchers" :key="'pm-'+idx" class="kv-row">
                <ElInput v-model="r.key" placeholder="key" style="width: 40%; margin-right: 8px" />
                <ElInput v-model="r.value" placeholder="value" style="width: 40%; margin-right: 8px" />
                <ElButton link type="danger" :icon="Delete" @click="kvRowRemove(previewForm.matchers, idx)">删除</ElButton>
              </div>
            </div>

            <ElFormItem label="severity">
              <ElInput v-model="previewForm.severity" placeholder="warning / critical / ..." />
            </ElFormItem>
            <ElFormItem label="status">
              <ElInput v-model="previewForm.status" placeholder="firing / resolved / ..." />
            </ElFormItem>

            <div class="kv-section">
              <div class="kv-title">
                <span>原始 labels</span>
                <ElButton size="small" :icon="Plus" link type="primary" @click="kvRowAdd(previewForm.labels)">新增</ElButton>
              </div>
              <div v-for="(r, idx) in previewForm.labels" :key="'pl-'+idx" class="kv-row">
                <ElInput v-model="r.key" placeholder="key" style="width: 40%; margin-right: 8px" />
                <ElInput v-model="r.value" placeholder="value" style="width: 40%; margin-right: 8px" />
                <ElButton link type="danger" :icon="Delete" @click="kvRowRemove(previewForm.labels, idx)">删除</ElButton>
              </div>
            </div>

            <div class="kv-section">
              <div class="kv-title">
                <span>原始 annotations</span>
                <ElButton size="small" :icon="Plus" link type="primary" @click="kvRowAdd(previewForm.annotations)">新增</ElButton>
              </div>
              <div v-for="(r, idx) in previewForm.annotations" :key="'pa-'+idx" class="kv-row">
                <ElInput v-model="r.key" placeholder="key" style="width: 40%; margin-right: 8px" />
                <ElInput v-model="r.value" placeholder="value" style="width: 40%; margin-right: 8px" />
                <ElButton link type="danger" :icon="Delete" @click="kvRowRemove(previewForm.annotations, idx)">删除</ElButton>
              </div>
            </div>

            <ElFormItem>
              <ElButton type="primary" :icon="VideoPlay" :loading="previewLoading" @click="previewExecute">
                执行预览
              </ElButton>
            </ElFormItem>
          </ElForm>
        </ElCol>

        <!-- 右侧：原始 / 结果 并排 -->
        <ElCol :span="13">
          <h3 style="margin: 0 0 8px 0">执行结果 · Diff</h3>
          <div v-loading="previewLoading" style="min-height: 400px">
            <template v-if="previewResult">
              <div class="eval-head">
                <ElTag v-if="previewResult.ok" type="success" effect="light">执行成功</ElTag>
                <ElTag v-else type="danger" effect="light">执行失败</ElTag>
                <span v-if="previewResult.error" style="margin-left: 8px; color: #f56c6c">
                  {{ previewResult.error }}
                </span>
              </div>

              <ElRow :gutter="8" style="margin-top: 12px">
                <ElCol v-if="previewResult.original" :span="12">
                  <div class="diff-title">原始 alert</div>
                  <pre class="json-pre">{{ prettyJson(previewResult.original) }}</pre>
                </ElCol>
                <ElCol :span="previewResult.original ? 12 : 24">
                  <div class="diff-title">{{ previewResult.original ? '丰富后 alert' : '返回 alert' }}</div>
                  <pre class="json-pre">{{ prettyJson(previewResult.result ?? previewResult.detail ?? {}) }}</pre>
                </ElCol>
              </ElRow>
            </template>
            <template v-else-if="!previewLoading">
              <ElEmpty description="点击左侧『执行预览』查看试跑结果。" />
            </template>
          </div>
        </ElCol>
      </ElRow>
    </ElDrawer>

    <!-- 新建 / 编辑 Dialog (ElCollapse 四面板) -->
    <ElDialog
      v-model="dlg.visible"
      :title="dlg.mode === 'create' ? '新建丰富规则' : '编辑丰富规则'"
      width="880px"
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
        <ElCollapse v-model="activePanel">
          <!-- 面板1：基础 -->
          <ElCollapseItem title="① 基础信息" name="basic">
            <ElRow :gutter="16">
              <ElCol :span="14">
                <ElFormItem label="名称" prop="name">
                  <ElInput v-model="dlg.name" placeholder="例如：给主机告警补充 CMDB 信息" />
                </ElFormItem>
              </ElCol>
              <ElCol :span="4">
                <ElFormItem label="启用" prop="enabled">
                  <ElSwitch v-model="dlg.enabled" />
                </ElFormItem>
              </ElCol>
              <ElCol :span="6">
                <ElFormItem label="priority" prop="priority">
                  <ElInputNumber
                    v-model="dlg.priority"
                    :min="0"
                    :step="10"
                    style="width: 100%"
                    controls-position="right"
                  />
                </ElFormItem>
              </ElCol>
            </ElRow>
            <ElFormItem label="kind">
              <ElSelect v-model="dlg.kind" style="width: 100%">
                <ElOption
                  v-for="k in KIND_OPTIONS"
                  :key="k.value"
                  :label="k.label"
                  :value="k.value"
                />
              </ElSelect>
            </ElFormItem>

            <!-- matchers 动态行 -->
            <div class="kv-section">
              <div class="kv-title">
                <span>matchers（k/v 匹配为空时对所有告警执行）</span>
                <ElButton size="small" :icon="Plus" link type="primary" @click="kvRowAdd(dlg.matchers)">新增一行</ElButton>
              </div>
              <div v-for="(r, idx) in dlg.matchers" :key="'dm-'+idx" class="kv-row">
                <ElInput v-model="r.key" placeholder="key" style="width: 40%; margin-right: 8px" />
                <ElInput v-model="r.value" placeholder="value" style="width: 40%; margin-right: 8px" />
                <ElButton link type="danger" :icon="Delete" @click="kvRowRemove(dlg.matchers, idx)">删除</ElButton>
              </div>
            </div>
          </ElCollapseItem>

          <!-- 面板2：模板 (AnnotationTemplate) -->
          <ElCollapseItem title="② 模板（AnnotationTemplate）" name="tpl">
            <div class="panel-tip">
              定义 annotations：key 是 annotation 名（如 summary / runbook_url），value 是模板字符串（可用 <code v-pre>{{ .Label.instance }}</code> 这类占位符，具体语法视后端模板引擎而定）。
            </div>
            <div class="kv-section">
              <div class="kv-title">
                <span>templates（annotation 名 → 模板字符串）</span>
                <ElButton size="small" :icon="Plus" link type="primary" @click="kvRowAdd(dlg.templates)">新增一行</ElButton>
              </div>
              <div v-for="(r, idx) in dlg.templates" :key="'dt-'+idx" class="kv-row">
                <ElInput v-model="r.key" placeholder="annotation key，如 summary" style="width: 26%; margin-right: 8px" />
                <ElInput v-model="r.value" placeholder="模板字符串" style="width: 60%; margin-right: 8px" />
                <ElButton link type="danger" :icon="Delete" @click="kvRowRemove(dlg.templates, idx)">删除</ElButton>
              </div>
            </div>
          </ElCollapseItem>

          <!-- 面板3：字段覆盖 (Field & Extract) -->
          <ElCollapseItem title="③ 字段覆盖（Field &amp; Extract）" name="field">
            <div class="panel-tip">
              field_templates：覆盖顶层字段（severity / ip / alertname / summary 等常见值，也可自由填写）。<br />
              label_extracts：从原始字段提取/派生新 label，执行顺序先于 Lookup。
            </div>

            <div class="kv-section">
              <div class="kv-title">
                <span>field_templates（顶层字段 → 模板）</span>
                <ElButton size="small" :icon="Plus" link type="primary" @click="kvRowAdd(dlg.field_templates)">新增一行</ElButton>
              </div>
              <div v-for="(r, idx) in dlg.field_templates" :key="'df-'+idx" class="kv-row">
                <ElSelect
                  v-model="r.key"
                  placeholder="选择或输入字段名"
                  allow-create
                  filterable
                  default-first-option
                  style="width: 26%; margin-right: 8px"
                >
                  <ElOption v-for="fk in FIELD_TEMPLATE_KEY_OPTIONS" :key="fk" :label="fk" :value="fk" />
                </ElSelect>
                <ElInput v-model="r.value" placeholder="模板字符串 / 固定值" style="width: 60%; margin-right: 8px" />
                <ElButton link type="danger" :icon="Delete" @click="kvRowRemove(dlg.field_templates, idx)">删除</ElButton>
              </div>
            </div>

            <div class="kv-section" style="margin-top: 8px">
              <div class="kv-title">
                <span>label_extracts（新 label 名 → 模板）</span>
                <ElButton size="small" :icon="Plus" link type="primary" @click="kvRowAdd(dlg.label_extracts)">新增一行</ElButton>
              </div>
              <div v-for="(r, idx) in dlg.label_extracts" :key="'dl-'+idx" class="kv-row">
                <ElInput v-model="r.key" placeholder="新 label 名，如 owner_team" style="width: 26%; margin-right: 8px" />
                <ElInput v-model="r.value" placeholder="模板字符串" style="width: 60%; margin-right: 8px" />
                <ElButton link type="danger" :icon="Delete" @click="kvRowRemove(dlg.label_extracts, idx)">删除</ElButton>
              </div>
            </div>

            <ElFormItem label="write_labels">
              <ElSwitch v-model="dlg.write_labels" />
              <span class="form-tip" style="margin-left: 10px">是否把 label_extracts 结果写回 labels（默认关）</span>
            </ElFormItem>
          </ElCollapseItem>

          <!-- 面板4：映射 & 外表 -->
          <ElCollapseItem title="④ 映射 &amp; 外表（LabelMap / Lookup）" name="mapping">
            <ElFormItem label="match_key">
              <ElInput
                v-model="dlg.match_key"
                placeholder="仅 LabelMap 需要：用来匹配 mappings 第一级 key 的原始 label 名，如 instance"
              />
            </ElFormItem>

            <div class="kv-section">
              <div class="kv-title">
                <span>mappings：match value → label 映射表（两级 Map）</span>
                <ElButton size="small" :icon="Plus" link type="primary" @click="mappingAddRow()">新增 match 行</ElButton>
              </div>
              <div
                v-for="(row, idx) in dlg.mappings"
                :key="'mr-'+idx"
                class="mapping-block"
              >
                <div class="mapping-head">
                  <span class="mapping-match-label">匹配值 match_value：</span>
                  <ElInput
                    v-model="row.match_value"
                    placeholder="如 host-01 / prod-web 等"
                    style="width: 260px; margin-right: 12px"
                  />
                  <ElButton link type="danger" :icon="Delete" @click="mappingRemoveRow(idx)">删除整行</ElButton>
                </div>
                <div class="mapping-inner">
                  <div class="kv-title">
                    <span>命中后的 label 映射（label 名 → 值）</span>
                    <ElButton size="small" :icon="Plus" link type="primary" @click="kvRowAdd(row.labels)">新增 kv</ElButton>
                  </div>
                  <div v-for="(r, i2) in row.labels" :key="'mrk-'+idx+'-'+i2" class="kv-row">
                    <ElInput v-model="r.key" placeholder="label key，如 team" style="width: 40%; margin-right: 8px" />
                    <ElInput v-model="r.value" placeholder="label value，如 infra" style="width: 40%; margin-right: 8px" />
                    <ElButton link type="danger" :icon="Delete" @click="kvRowRemove(row.labels, i2)">删除</ElButton>
                  </div>
                </div>
              </div>
            </div>

            <ElFormItem label="lookup_table_ids" style="margin-top: 12px">
              <ElSelect
                v-model="dlg.lookup_table_ids"
                multiple
                filterable
                placeholder="选择要关联的 Lookup 字典（可多选，按顺序执行）"
                style="width: 100%"
              >
                <ElOption
                  v-for="l in lookups"
                  :key="l.id"
                  :label="`${l.name} (key=${l.key_label || '?'})`"
                  :value="l.id"
                />
              </ElSelect>
            </ElFormItem>

            <div class="kv-section">
              <div class="kv-title">
                <span>lookup_match_keys：table → label key 覆盖（默认用字典自身 key_label）</span>
                <ElButton size="small" :icon="Plus" link type="primary" @click="lmkAddRow()">新增覆盖</ElButton>
              </div>
              <div
                v-for="(r, idx) in dlg.lookup_match_keys"
                :key="'lmk-'+idx"
                class="kv-row"
              >
                <ElSelect
                  v-model="r.table_id"
                  placeholder="Lookup 字典"
                  filterable
                  style="width: 46%; margin-right: 8px"
                >
                  <ElOption
                    v-for="l in lookups"
                    :key="l.id"
                    :label="l.name"
                    :value="l.id"
                  />
                </ElSelect>
                <ElInput v-model="r.label_key" placeholder="覆盖后的 label key，如 ip" style="width: 38%; margin-right: 8px" />
                <ElButton link type="danger" :icon="Delete" @click="lmkRemoveRow(idx)">删除</ElButton>
              </div>
              <div v-if="dlg.lookup_match_keys.length === 0" class="form-tip">
                （空：全部使用 Lookup 字典自身的 key_label 做匹配）
              </div>
            </div>
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
.enrichments-view { padding: 16px 20px 32px; }
.page-header { margin-bottom: 12px; }
.page-title { margin: 0 0 4px; font-size: 22px; }
.page-sub { margin: 0; color: #606266; font-size: 13px; }
.affix-wrap { z-index: 10; }
.toolbar {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 12px;
  background: #fff;
  border: 1px solid #ebeef5;
  border-radius: 4px;
}
.toolbar-filters {
  margin-left: auto;
  display: flex;
  gap: 8px;
}
.name-link {
  color: #409eff;
  cursor: pointer;
}
.matchers-cell { display: flex; flex-wrap: wrap; }
.kv-section { margin-top: 4px; }
.kv-title {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 6px;
  font-size: 13px;
  color: #303133;
}
.kv-row {
  display: flex;
  align-items: center;
  margin-bottom: 6px;
}
.form-tip { font-size: 12px; color: #909399; margin-top: 4px; }
.panel-tip {
  background: #f4f8fc;
  padding: 8px 10px;
  border-radius: 4px;
  margin-bottom: 8px;
  font-size: 12px;
  color: #606266;
}
.panel-tip code { background: #eef; padding: 1px 4px; border-radius: 3px; }
.eval-head { display: flex; align-items: center; gap: 8px; margin-bottom: 8px; }
.diff-title {
  font-size: 12px;
  color: #606266;
  margin-bottom: 4px;
  padding: 2px 6px;
  background: #f5f7fa;
  border-radius: 3px;
}
.json-pre {
  background: #1e1e1e;
  color: #d4d4d4;
  padding: 10px;
  border-radius: 4px;
  max-height: 500px;
  overflow: auto;
  font-size: 12px;
  line-height: 1.4;
}
.mapping-block {
  border: 1px dashed #dcdfe6;
  border-radius: 4px;
  padding: 10px;
  margin-bottom: 8px;
  background: #fafafa;
}
.mapping-head {
  display: flex;
  align-items: center;
  margin-bottom: 8px;
  flex-wrap: wrap;
  gap: 6px;
}
.mapping-match-label { font-size: 13px; color: #303133; }
.mapping-inner { padding-left: 4px; }
.raw-pre {
  background: #f5f7fa;
  padding: 6px 8px;
  border-radius: 3px;
  max-height: 240px;
  overflow: auto;
  font-size: 12px;
  line-height: 1.4;
}
</style>
