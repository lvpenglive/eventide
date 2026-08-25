<script setup lang="ts">
import { computed, markRaw, onMounted, reactive, ref } from 'vue'
import {
  ElButton,
  ElCard,
  ElCheckbox,
  ElCol,
  ElDialog,
  ElDivider,
  ElEmpty,
  ElForm,
  ElFormItem,
  ElInput,
  ElMessage,
  ElMessageBox,
  ElOption,
  ElRow,
  ElSelect,
  ElSwitch,
  ElTable,
  ElTableColumn,
  ElTag,
  ElTooltip,
  ElUpload,
} from 'element-plus'
import type { FormInstance, FormRules, UploadInstance, UploadProps, UploadRawFile } from 'element-plus'
import {
  Delete,
  Download,
  Edit,
  Plus,
  Refresh,
  Search,
  UploadFilled,
} from '@element-plus/icons-vue'
import { useAuthStore } from '@/stores/auth'

// ============================================================
// 本地类型
// ============================================================
type PolicyMatchMode = 'exact' | 'prefix' | 'oid_prefix'
type AlertSeverity = 'critical' | 'warning' | 'info' | 'error' | 'ok' | string
type ImportMode = 'merge' | 'replace' | 'keep'

interface TrapPolicy {
  id: string
  name: string
  trap_oid: string
  match_mode: PolicyMatchMode
  severity: AlertSeverity
  enabled: boolean
  summary_template?: string
  description?: string
  module?: string
  status?: string
  objects?: string[]
  object_oids?: Record<string, string>
  keywords?: string[]
  resolve_oid?: string
  resolve_values?: string[]
  fingerprint_oids?: string[]
  severity_oid?: string
  severity_map?: Record<string, AlertSeverity>
  created_at?: string
  updated_at?: string
}

interface TrapPolicyList {
  items: TrapPolicy[]
  path: string
  count: number
}

interface ApplyPolicyResult {
  inserted?: number
  updated?: number
  skipped?: number
  deleted?: number
  errors?: string[]
}

interface PoliciesApiShim {
  listTrapPolicies(): Promise<TrapPolicyList>
  getTrapPolicy(id: string): Promise<TrapPolicy>
  createTrapPolicy(body: TrapPolicy): Promise<TrapPolicy>
  updateTrapPolicy(id: string, body: TrapPolicy): Promise<TrapPolicy>
  deleteTrapPolicy(id: string): Promise<{ ok: true }>
  exportPoliciesXlsx(): Promise<Blob>
  exportPolicyXlsx(id: string): Promise<Blob>
  importPoliciesXlsx(mode: ImportMode, file: File): Promise<{ ok: true; result: ApplyPolicyResult }>
}

function _unimpl(name: string): never {
  throw new Error(`[API shim] 模块未实现：${name}，请等待并行任务创建 @/api/policies`)
}
const _shim: PoliciesApiShim = {
  listTrapPolicies: () => Promise.reject(_unimpl('listTrapPolicies')),
  getTrapPolicy: () => Promise.reject(_unimpl('getTrapPolicy')),
  createTrapPolicy: () => Promise.reject(_unimpl('createTrapPolicy')),
  updateTrapPolicy: () => Promise.reject(_unimpl('updateTrapPolicy')),
  deleteTrapPolicy: () => Promise.reject(_unimpl('deleteTrapPolicy')),
  exportPoliciesXlsx: () => Promise.reject(_unimpl('exportPoliciesXlsx')),
  exportPolicyXlsx: () => Promise.reject(_unimpl('exportPolicyXlsx')),
  importPoliciesXlsx: () => Promise.reject(_unimpl('importPoliciesXlsx')),
}

// @ts-ignore 模块未创建时忽略
import * as _raw from '@/api/policies'
const _rawApi = markRaw(_raw as unknown as Record<string, unknown>)

function _bind<S extends object>(shim: S, src: Record<string, unknown>): S {
  const out = { ...(shim as unknown as object) } as S
  for (const key of Object.keys(shim as unknown as object)) {
    if (key in src && typeof (src as Record<string, unknown>)[key] === 'function') {
      ;(out as unknown as Record<string, unknown>)[key] = (src as Record<string, unknown>)[key]
    }
  }
  return out
}
const api = _bind<PoliciesApiShim>(_shim, {
  ..._rawApi,
  listTrapPolicies: _rawApi.listPolicies,
  getTrapPolicy: _rawApi.getPolicy,
  createTrapPolicy: _rawApi.createPolicy,
  updateTrapPolicy: _rawApi.updatePolicy,
  deleteTrapPolicy: _rawApi.deletePolicy,
  exportPoliciesXlsx: _rawApi.exportPolicies,
  importPoliciesXlsx: _rawApi.importPolicies,
})

// ============================================================
// 权限 & 工具
// ============================================================
const auth = useAuthStore()
const canRead = computed(() => auth.can('trap:read'))
const canWrite = computed(() => auth.can('trap:write'))

function errMsg(e: unknown, fb: string): string {
  if (e && typeof e === 'object') {
    const m = (e as { message?: unknown }).message
    if (typeof m === 'string') return m
  }
  return fb
}

function asPol(r: unknown): TrapPolicy { return r as TrapPolicy }

type TagType = 'success' | 'warning' | 'info' | 'danger' | 'primary'
interface SevMeta { text: string; type: TagType; color: string }
function sevMeta(s: AlertSeverity): SevMeta {
  switch (s) {
    case 'critical': return { text: '严重', type: 'danger', color: 'var(--crit)' }
    case 'error': return { text: '错误', type: 'danger', color: 'var(--crit-soft)' }
    case 'warning': return { text: '警告', type: 'warning', color: 'var(--warn)' }
    case 'info': return { text: '信息', type: 'info', color: 'var(--info)' }
    case 'ok': return { text: '正常', type: 'success', color: 'var(--ok)' }
    default: return { text: String(s || '-'), type: 'primary', color: 'var(--primary)' }
  }
}

function matchModeMeta(m: PolicyMatchMode | string): { label: string; type: TagType } {
  switch (m) {
    case 'exact': return { label: '精确', type: 'success' }
    case 'prefix': return { label: '前缀', type: 'warning' }
    case 'oid_prefix': return { label: 'OID前缀', type: 'primary' }
    default: return { label: String(m), type: 'info' }
  }
}

function formatTime(s: string | undefined | null): string {
  if (!s) return '-'
  return s.replace('T', ' ').slice(0, 19)
}

function downloadBlob(blob: Blob, filename: string) {
  const a = document.createElement('a')
  const url = URL.createObjectURL(blob)
  a.href = url
  a.download = filename
  document.body.appendChild(a)
  a.click()
  document.body.removeChild(a)
  URL.revokeObjectURL(url)
}

// ============================================================
// 列表 & 筛选
// ============================================================
const loading = ref(false)
const policies = ref<TrapPolicy[]>([])
const listMeta = reactive({ path: '', count: 0 })

const filterMatchMode = ref<PolicyMatchMode | ''>('')
const filterSeverity = ref<AlertSeverity | ''>('')
const filterEnabled = ref<'all' | 'on' | 'off'>('all')
const filterQ = ref('')

const filtered = computed<TrapPolicy[]>(() => {
  let arr = policies.value
  if (filterMatchMode.value) arr = arr.filter((p) => p.match_mode === filterMatchMode.value)
  if (filterSeverity.value) arr = arr.filter((p) => p.severity === filterSeverity.value)
  if (filterEnabled.value === 'on') arr = arr.filter((p) => p.enabled)
  if (filterEnabled.value === 'off') arr = arr.filter((p) => !p.enabled)
  const q = filterQ.value.trim().toLowerCase()
  if (q) {
    arr = arr.filter((p) => {
      const hay = `${p.name} ${p.trap_oid} ${p.module || ''}`.toLowerCase()
      return hay.includes(q)
    })
  }
  return arr
})

async function loadAll() {
  loading.value = true
  try {
    const res = await api.listTrapPolicies()
    policies.value = res.items || []
    listMeta.path = res.path || ''
    listMeta.count = res.count ?? policies.value.length
  } catch (e) {
    ElMessage.error(errMsg(e, '加载策略失败'))
    policies.value = []
  } finally {
    loading.value = false
  }
}

// ============================================================
// enabled 乐观更新
// ============================================================
async function onToggleEnabled(row: TrapPolicy) {
  if (!canWrite.value) { row.enabled = !row.enabled; ElMessage.warning('需要 trap:write 权限'); return }
  const newVal = row.enabled
  try {
    await api.updateTrapPolicy(row.id, row)
    ElMessage.success(`已${newVal ? '启用' : '禁用'}`)
  } catch (e) {
    row.enabled = !newVal
    ElMessage.error(`更新失败：${errMsg(e, '未知错误')}，已回滚`)
  }
}

// ============================================================
// 编辑 / 新建 Dialog（4 section）
// ============================================================
const dlgVisible = ref(false)
const dlgMode = ref<'create' | 'edit'>('create')
const dlgId = ref('')
const dlgLoading = ref(false)
const dlgFormRef = ref<FormInstance>()

interface DialogForm {
  name: string
  module: string
  trap_oid: string
  match_mode: PolicyMatchMode
  severity: AlertSeverity
  enabled: boolean
  resolve_oid: string
  resolve_values: string
  fingerprint_oids: string
  severity_oid: string
  severity_map: string
  summary_template: string
  description: string
}

function emptyForm(): DialogForm {
  return {
    name: '',
    module: '',
    trap_oid: '',
    match_mode: 'exact',
    severity: 'warning',
    enabled: true,
    resolve_oid: '',
    resolve_values: '',
    fingerprint_oids: '',
    severity_oid: '',
    severity_map: '',
    summary_template: '',
    description: '',
  }
}

const form = reactive<DialogForm>(emptyForm())

const formRules: FormRules<DialogForm> = {
  name: [{ required: true, message: '请输入策略名称', trigger: 'blur' }],
  trap_oid: [{ required: true, message: '请输入 Trap OID', trigger: 'blur' }],
}

function openCreate() {
  Object.assign(form, emptyForm())
  dlgMode.value = 'create'
  dlgId.value = ''
  dlgVisible.value = true
}

async function openEdit(row: TrapPolicy) {
  Object.assign(form, emptyForm())
  try {
    const p = await api.getTrapPolicy(row.id).catch(() => row as TrapPolicy)
    form.name = p.name
    form.module = p.module || ''
    form.trap_oid = p.trap_oid
    form.match_mode = p.match_mode
    form.severity = p.severity
    form.enabled = p.enabled
    form.resolve_oid = p.resolve_oid || ''
    form.resolve_values = (p.resolve_values || []).join(',')
    form.fingerprint_oids = (p.fingerprint_oids || []).join(',')
    form.severity_oid = p.severity_oid || ''
    form.severity_map = p.severity_map
      ? Object.entries(p.severity_map).map(([k, v]) => `${k}=${v}`).join(';')
      : ''
    form.summary_template = p.summary_template || ''
    form.description = p.description || ''
    dlgMode.value = 'edit'
    dlgId.value = p.id
    dlgVisible.value = true
  } catch (e) {
    ElMessage.error(errMsg(e, '获取策略详情失败'))
  }
}

function parseSeverityMap(s: string): Record<string, AlertSeverity> {
  const out: Record<string, AlertSeverity> = {}
  const parts = (s || '').split(';').map((p) => p.trim()).filter(Boolean)
  for (const part of parts) {
    const idx = part.indexOf('=')
    if (idx <= 0) continue
    const k = part.slice(0, idx).trim()
    const v = part.slice(idx + 1).trim() as AlertSeverity
    if (!k || !v) continue
    out[k] = v
  }
  return out
}

function splitCsv(s: string): string[] {
  return (s || '').split(',').map((x) => x.trim()).filter(Boolean)
}

function buildPayload(): TrapPolicy {
  return {
    id: dlgId.value,
    name: form.name.trim(),
    module: form.module.trim() || undefined,
    trap_oid: form.trap_oid.trim(),
    match_mode: form.match_mode,
    severity: form.severity,
    enabled: form.enabled,
    resolve_oid: form.resolve_oid.trim() || undefined,
    resolve_values: splitCsv(form.resolve_values),
    fingerprint_oids: splitCsv(form.fingerprint_oids),
    severity_oid: form.severity_oid.trim() || undefined,
    severity_map: (() => {
      const m = parseSeverityMap(form.severity_map)
      return Object.keys(m).length > 0 ? m : undefined
    })(),
    summary_template: form.summary_template || undefined,
    description: form.description || undefined,
  } as TrapPolicy
}

async function saveDialog() {
  if (!dlgFormRef.value) return
  try { await dlgFormRef.value.validate() } catch { return }
  const payload = buildPayload()
  dlgLoading.value = true
  try {
    if (dlgMode.value === 'create') {
      await api.createTrapPolicy(payload)
      ElMessage.success('已创建策略')
    } else {
      await api.updateTrapPolicy(dlgId.value, payload)
      ElMessage.success('已更新策略')
    }
    dlgVisible.value = false
    void loadAll()
  } catch (e) {
    ElMessage.error(errMsg(e, dlgMode.value === 'create' ? '创建失败' : '更新失败'))
  } finally {
    dlgLoading.value = false
  }
}

// 模板变量 chip 插入
const summaryChips: Array<{ label: string; snippet: string }> = [
  { label: 'alertname', snippet: '${alertname}' },
  { label: 'ip', snippet: '${ip}' },
  { label: 'trap_oid', snippet: '${trap_oid}' },
  { label: 'resolve_values', snippet: '${resolve_values}' },
  { label: 'severity', snippet: '${severity}' },
  { label: 'module', snippet: '${module}' },
]

function insertChip(snippet: string, field: 'summary_template' | 'description') {
  const cur = form[field] || ''
  form[field] = cur + snippet
}

// ============================================================
// 删除
// ============================================================
async function removePolicy(row: TrapPolicy) {
  try {
    await ElMessageBox.confirm(`确认删除策略「${row.name}」？此操作不可恢复。`, '删除策略', {
      type: 'warning',
      confirmButtonText: '删除',
      cancelButtonText: '取消',
    })
  } catch { return }
  try {
    await api.deleteTrapPolicy(row.id)
    ElMessage.success('已删除')
    void loadAll()
  } catch (e) {
    ElMessage.error(errMsg(e, '删除失败'))
  }
}

// ============================================================
// 导出 / 单项导出
// ============================================================
async function handleExportAll() {
  try {
    const blob = await api.exportPoliciesXlsx()
    const ts = new Date().toISOString().replace(/[:.]/g, '-').slice(0, 19)
    downloadBlob(blob, `trap-policies-${ts}.xlsx`)
    ElMessage.success('已导出全部策略')
  } catch (e) {
    ElMessage.error(errMsg(e, '导出失败'))
  }
}

async function handleExportOne(row: TrapPolicy) {
  try {
    const blob = await api.exportPolicyXlsx(row.id)
    downloadBlob(blob, `trap-policy-${row.name}-${row.id}.xlsx`)
    ElMessage.success(`已导出 ${row.name}`)
  } catch (e) {
    ElMessage.error(errMsg(e, '导出失败'))
  }
}

// ============================================================
// 导入 Dialog
// ============================================================
const importDlgVisible = ref(false)
const importMode = ref<ImportMode>('merge')
const importLoading = ref(false)
const importResult = ref<ApplyPolicyResult | null>(null)
const importErrors = ref<string[]>([])
const importUploadRef = ref<UploadInstance>()

function openImportDlg() {
  importMode.value = 'merge'
  importResult.value = null
  importErrors.value = []
  importDlgVisible.value = true
}

const beforeImportUpload: UploadProps['beforeUpload'] = (raw: UploadRawFile) => {
  if (!canWrite.value) { ElMessage.warning('需要 trap:write 权限导入'); return false }
  const name = raw.name.toLowerCase()
  if (!name.endsWith('.xlsx')) {
    ElMessage.warning('仅支持 .xlsx 文件')
    return false
  }
  return true
}

const importHttpRequest: UploadProps['httpRequest'] = async (opts) => {
  const f = opts.file as File
  importLoading.value = true
  importResult.value = null
  importErrors.value = []
  try {
    const res = await api.importPoliciesXlsx(importMode.value, f)
    importResult.value = res.result
    importErrors.value = res.result?.errors || []
    ElMessage.success('导入完成')
    void loadAll()
    opts.onSuccess?.(res)
  } catch (e) {
    const m = errMsg(e, '导入失败')
    importErrors.value = [m]
    ElMessage.error(m)
    const faux = Object.assign(new Error(m), {
      name: 'UploadAjaxError',
      status: 0,
      method: 'POST',
      url: '/api/policies/import',
    })
    opts.onError?.(faux)
  } finally {
    importLoading.value = false
  }
}

onMounted(() => { void loadAll() })
</script>

<template>
  <div class="page-wrap" style="padding: 16px 20px;">
    <!-- 顶部标题 & 动作 -->
    <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 14px; flex-wrap: wrap; gap: 8px;">
      <h2 style="font-size: 20px; margin: 0; color: var(--heading);">
        Trap 策略
        <span style="font-size:13px;color:var(--el-text-color-secondary);margin-left:8px">匹配条件 · 严重度映射 · 告警内容模板 · 导出 / 导入</span>
        <el-tag size="small" type="info" effect="plain" style="margin-left: 10px;">共 {{ listMeta.count }} 条</el-tag>
      </h2>
      <div style="display: flex; gap: 8px; flex-wrap: wrap;">
        <el-button :icon="Refresh" @click="loadAll">刷新</el-button>
        <el-button :icon="Download" type="primary" plain @click="handleExportAll">导出（xlsx）</el-button>
        <el-button v-if="canWrite" :icon="UploadFilled" type="success" plain @click="openImportDlg">导入 Excel</el-button>
        <el-button v-if="canWrite" type="primary" :icon="Plus" @click="openCreate">新建策略</el-button>
      </div>
    </div>

    <!-- 筛选条 -->
    <el-card shadow="never" style="margin-bottom: 14px; border: 1px solid var(--line); background: var(--panel-bg, var(--panel));">
      <el-form :inline="true" :model="{ filterMatchMode, filterSeverity, filterEnabled, filterQ }" size="default">
        <el-form-item label="Match Mode">
          <el-select v-model="filterMatchMode" style="width: 140px;" clearable placeholder="全部">
            <el-option label="精确 (exact)" value="exact" />
            <el-option label="前缀 (prefix)" value="prefix" />
            <el-option label="OID前缀" value="oid_prefix" />
          </el-select>
        </el-form-item>
        <el-form-item label="Severity">
          <el-select v-model="filterSeverity" style="width: 140px;" clearable placeholder="全部">
            <el-option label="严重 (critical)" value="critical" />
            <el-option label="错误 (error)" value="error" />
            <el-option label="警告 (warning)" value="warning" />
            <el-option label="信息 (info)" value="info" />
            <el-option label="正常 (ok)" value="ok" />
          </el-select>
        </el-form-item>
        <el-form-item label="Enabled">
          <el-select v-model="filterEnabled" style="width: 120px;">
            <el-option label="全部" value="all" />
            <el-option label="启用" value="on" />
            <el-option label="禁用" value="off" />
          </el-select>
        </el-form-item>
        <el-form-item label="搜索">
          <el-input
            v-model="filterQ"
            placeholder="名称 / trap_oid / module"
            style="width: 280px;"
            clearable
          >
            <template #prefix><el-icon><Search /></el-icon></template>
          </el-input>
        </el-form-item>
      </el-form>
    </el-card>

    <!-- 列表 -->
    <el-card shadow="never" style="border: 1px solid var(--line); background: var(--panel-bg, var(--panel));">
      <el-table
        :data="filtered as unknown as TrapPolicy[]"
        size="small"
        stripe
        v-loading="loading"
        empty-text="暂无策略"
        max-height="640"
      >
        <el-table-column label="名称" min-width="170">
          <template #default="{ row }">
            <div style="display: flex; flex-direction: column;">
              <strong>{{ asPol(row).name }}</strong>
              <span v-if="asPol(row).module" style="color: var(--muted); font-size: 12px;">模块: {{ asPol(row).module }}</span>
            </div>
          </template>
        </el-table-column>
        <el-table-column label="Trap OID" min-width="240">
          <template #default="{ row }">
            <el-tooltip :content="asPol(row).trap_oid" placement="top">
              <span
                style="font-family: var(--font-mono); font-size: 12.5px; display: inline-block; max-width: 220px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis;"
              >{{ asPol(row).trap_oid }}</span>
            </el-tooltip>
          </template>
        </el-table-column>
        <el-table-column label="Match" width="100" align="center">
          <template #default="{ row }">
            <el-tag size="small" :type="matchModeMeta(asPol(row).match_mode).type" effect="plain">
              {{ matchModeMeta(asPol(row).match_mode).label }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column label="Severity" width="100" align="center">
          <template #default="{ row }">
            <el-tag size="small" :type="sevMeta(asPol(row).severity).type" effect="dark">
              {{ sevMeta(asPol(row).severity).text }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column label="Enabled" width="90" align="center">
          <template #default="{ row }">
            <el-switch
              v-model="(asPol(row) as TrapPolicy).enabled"
              :disabled="!canWrite"
              @change="onToggleEnabled(asPol(row))"
            />
          </template>
        </el-table-column>
        <el-table-column label="恢复 Resolve" width="130" align="center">
          <template #default="{ row }">
            <template v-if="asPol(row).resolve_oid">
              <el-tag type="success" size="small" effect="dark">ON</el-tag>
              <el-tag size="small" type="info" effect="plain" style="margin-top: 4px;">Resolve</el-tag>
            </template>
            <span v-else style="color: var(--muted);">-</span>
          </template>
        </el-table-column>
        <el-table-column label="更新时间" width="160">
          <template #default="{ row }">
            <span style="color: var(--el-text-color-secondary); font-family: var(--font-mono); font-size: 12.5px;">
              {{ formatTime(asPol(row).updated_at) }}
            </span>
          </template>
        </el-table-column>
        <el-table-column label="操作" width="220" align="center" fixed="right">
          <template #default="{ row }">
            <el-button
              v-if="canWrite"
              link
              type="primary"
              size="small"
              :icon="Edit"
              @click="openEdit(asPol(row))"
            >编辑</el-button>
            <el-button
              link
              type="primary"
              size="small"
              :icon="Download"
              @click="handleExportOne(asPol(row))"
            >导出</el-button>
            <el-button
              v-if="canWrite"
              link
              type="danger"
              size="small"
              :icon="Delete"
              @click="removePolicy(asPol(row))"
            >删除</el-button>
          </template>
        </el-table-column>
      </el-table>
    </el-card>

    <!-- 编辑 / 新建 Dialog (xl) -->
    <el-dialog v-model="dlgVisible" :title="dlgMode === 'create' ? '新建策略' : '编辑策略'" width="1100px" top="6vh">
      <el-form ref="dlgFormRef" :model="form" :rules="formRules" label-width="150px" size="default">
        <!-- ① 基本 -->
        <h3 style="margin: 0 0 10px; color: var(--heading); font-size: 15px;">① 基本信息</h3>
        <el-row :gutter="14">
          <el-col :span="12">
            <el-form-item label="策略名称" prop="name">
              <el-input v-model="form.name" placeholder="如 LinkDown-IF-MIB" />
            </el-form-item>
          </el-col>
          <el-col :span="12">
            <el-form-item label="模块">
              <el-input v-model="form.module" placeholder="可选，如 IF-MIB" clearable />
            </el-form-item>
          </el-col>
          <el-col :span="12">
            <el-form-item label="Trap OID" prop="trap_oid">
              <el-input v-model="form.trap_oid" placeholder=".1.3.6.1.6.3.1.1.5.3" style="font-family: var(--font-mono);" />
            </el-form-item>
          </el-col>
          <el-col :span="6">
            <el-form-item label="Match Mode">
              <el-select v-model="form.match_mode" style="width: 100%;">
                <el-option label="精确 (exact)" value="exact" />
                <el-option label="前缀 (prefix)" value="prefix" />
                <el-option label="OID前缀" value="oid_prefix" />
              </el-select>
            </el-form-item>
          </el-col>
          <el-col :span="6">
            <el-form-item label="默认 Severity">
              <el-select v-model="form.severity" style="width: 100%;">
                <el-option label="严重 (critical)" value="critical" />
                <el-option label="错误 (error)" value="error" />
                <el-option label="警告 (warning)" value="warning" />
                <el-option label="信息 (info)" value="info" />
                <el-option label="正常 (ok)" value="ok" />
              </el-select>
            </el-form-item>
          </el-col>
          <el-col :span="24">
            <el-form-item label="启用">
              <el-switch v-model="form.enabled" />
            </el-form-item>
          </el-col>
        </el-row>

        <el-divider />

        <!-- ② 告警与恢复 -->
        <h3 style="margin: 0 0 10px; color: var(--heading); font-size: 15px;">② 告警与恢复</h3>
        <el-row :gutter="14">
          <el-col :span="12">
            <el-form-item label="Resolve OID">
              <el-input
                v-model="form.resolve_oid"
                placeholder="对应的恢复 Trap OID，留空表示无显式恢复"
                style="font-family: var(--font-mono);"
                clearable
              />
            </el-form-item>
          </el-col>
          <el-col :span="24">
            <el-form-item label="Resolve Values (逗号分隔)">
              <el-input
                v-model="form.resolve_values"
                placeholder="up,ok,1  （若匹配恢复 OID 且 resolve values 命中则判定为恢复）"
                clearable
              />
            </el-form-item>
          </el-col>
          <el-col :span="24">
            <el-form-item label="Fingerprint OIDs (逗号分隔)">
              <el-input
                v-model="form.fingerprint_oids"
                placeholder=".1.3.6.1.2.1.2.2.1.1,.1.3.6.1.2.1.2.2.1.3 （用作去重指纹）"
                style="font-family: var(--font-mono);"
                clearable
              />
            </el-form-item>
          </el-col>
        </el-row>

        <el-divider />

        <!-- ③ 动态级别 -->
        <h3 style="margin: 0 0 10px; color: var(--heading); font-size: 15px;">③ 动态级别</h3>
        <el-row :gutter="14">
          <el-col :span="12">
            <el-form-item label="Severity OID">
              <el-input
                v-model="form.severity_oid"
                placeholder="用于提取 severity 的 varbind OID"
                style="font-family: var(--font-mono);"
                clearable
              />
            </el-form-item>
          </el-col>
          <el-col :span="24">
            <el-form-item label="Severity Map (k=v;k=v)">
              <el-input
                v-model="form.severity_map"
                :rows="3"
                type="textarea"
                placeholder="1=critical;2=warning;3=info&#10;或&#10;critical=1;warning=2;info=3"
                style="font-family: var(--font-mono); font-size: 12.5px;"
              />
            </el-form-item>
          </el-col>
        </el-row>

        <el-divider />

        <!-- ④ 告警内容 -->
        <h3 style="margin: 0 0 10px; color: var(--heading); font-size: 15px;">④ 告警内容</h3>
        <el-form-item label="Summary 模板变量">
          <div style="display: flex; flex-wrap: wrap; gap: 6px;">
            <el-tag
              v-for="c in summaryChips"
              :key="c.snippet"
              size="small"
              effect="plain"
              type="primary"
              style="cursor: pointer; user-select: none;"
              @click="insertChip(c.snippet, 'summary_template')"
            >
              {{ c.label }} → {{ c.snippet }}
            </el-tag>
            <el-tag
              size="small"
              effect="plain"
              type="success"
              style="cursor: pointer; user-select: none;"
              @click="insertChip('${resolve_values}', 'summary_template')"
            >resolve_values → ${'$'}{resolve_values}</el-tag>
          </div>
        </el-form-item>
        <el-form-item label="Summary 模板">
          <el-input
            v-model="form.summary_template"
            type="textarea"
            :rows="3"
            placeholder="${alertname} on ${ip} (severity ${severity})"
            style="font-family: var(--font-mono); font-size: 12.5px;"
          />
        </el-form-item>
        <el-form-item label="Description">
          <el-input
            v-model="form.description"
            type="textarea"
            :rows="5"
            placeholder="支持与 Summary 相同的 ${var} 模板变量"
            style="font-family: var(--font-mono); font-size: 12.5px;"
          />
          <div style="margin-top: 4px;">
            <el-tag
              v-for="c in summaryChips"
              :key="'d-' + c.snippet"
              size="small"
              effect="plain"
              type="primary"
              style="cursor: pointer; user-select: none; margin: 2px;"
              @click="insertChip(c.snippet, 'description')"
            >{{ c.label }} → {{ c.snippet }}</el-tag>
          </div>
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="dlgVisible = false">取消</el-button>
        <el-button type="primary" :loading="dlgLoading" @click="saveDialog">保存</el-button>
      </template>
    </el-dialog>

    <!-- 导入 Dialog -->
    <el-dialog v-model="importDlgVisible" title="导入策略 Excel" width="560px">
      <el-form label-width="120px">
        <el-form-item label="导入模式">
          <el-radio-group v-model="importMode">
            <el-radio label="merge">合并（插入/更新同名策略）</el-radio>
            <el-radio label="replace">替换（删除未在 Excel 中的策略）</el-radio>
            <el-radio label="keep">保留（仅插入不存在的）</el-radio>
          </el-radio-group>
        </el-form-item>
        <el-form-item label="选择文件">
          <el-upload
            ref="importUploadRef"
            :show-file-list="false"
            accept=".xlsx"
            :before-upload="beforeImportUpload"
            :http-request="importHttpRequest"
            drag
          >
            <el-button :icon="UploadFilled" type="primary" :loading="importLoading">选择 .xlsx 并上传</el-button>
            <template #tip>
              <div style="color: var(--muted); font-size: 12px; margin-top: 6px;">
                支持列：name / module / trap_oid / match_mode / severity / enabled /
                resolve_oid / resolve_values / fingerprint_oids / severity_oid /
                severity_map / summary_template / description
              </div>
            </template>
          </el-upload>
        </el-form-item>
      </el-form>

      <template v-if="importResult">
        <el-divider />
        <h4 style="margin: 4px 0 8px; color: var(--heading);">导入结果</h4>
        <el-row :gutter="12">
          <el-col :span="6"><el-statistic title="Inserted" :value="importResult.inserted ?? 0" /></el-col>
          <el-col :span="6"><el-statistic title="Updated" :value="importResult.updated ?? 0" /></el-col>
          <el-col :span="6"><el-statistic title="Skipped" :value="importResult.skipped ?? 0" /></el-col>
          <el-col :span="6"><el-statistic title="Deleted" :value="importResult.deleted ?? 0" /></el-col>
        </el-row>
        <el-alert
          v-if="importErrors.length > 0"
          type="error"
          :closable="false"
          style="margin-top: 10px;"
          :title="`共 ${importErrors.length} 个错误`"
        >
          <template #default>
            <ul style="margin: 0; padding-left: 18px;">
              <li v-for="(err, i) in importErrors" :key="i" style="font-size: 12.5px; color: var(--crit-soft);">{{ err }}</li>
            </ul>
          </template>
        </el-alert>
      </template>

      <template #footer>
        <el-button @click="importDlgVisible = false">关闭</el-button>
      </template>
    </el-dialog>
  </div>
</template>
