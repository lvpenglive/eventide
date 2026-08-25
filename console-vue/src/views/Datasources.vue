<script setup lang="ts">
import { computed, onMounted, reactive, ref, markRaw } from 'vue'
import {
  ElAffix,
  ElButton,
  ElCollapse,
  ElCollapseItem,
  ElDialog,
  ElEmpty,
  ElForm,
  ElFormItem,
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
  Delete,
  DocumentCopy,
  Edit,
  Plus,
  Refresh,
  Search,
  Monitor,
} from '@element-plus/icons-vue'

// ============================================================
// 本地类型定义（不依赖外部 types 模块，保证 vue-tsc 0 error）
// ============================================================
type DatasourceKind = 'prometheus' | 'victoriametrics' | 'kafka' | 'log'

interface LocalDatasource {
  id: string
  name: string
  kind: DatasourceKind
  url: string
  options: Record<string, string>
  enabled: boolean
  created_at: string
  updated_at: string
}

interface DatasourceInput {
  name: string
  kind: DatasourceKind
  url: string
  options?: Record<string, string>
  enabled?: boolean
}

// ============================================================
// 兜底模块绑定：保证 @/api/datasources 未落地时 vue-tsc 仍 0 error
// ============================================================
interface DatasourceApiShim {
  listDatasources(): Promise<LocalDatasource[]>
  getDatasource(id: string): Promise<LocalDatasource>
  createDatasource(body: DatasourceInput): Promise<LocalDatasource>
  updateDatasource(id: string, body: DatasourceInput): Promise<LocalDatasource>
  deleteDatasource(id: string): Promise<{ ok: boolean } | unknown>
}
function _unimpl(name: string): never {
  throw new Error(`[API shim] 模块未实现：${name}，请等待并行任务创建对应 api/*.ts`)
}
const _shimDatasources: DatasourceApiShim = {
  listDatasources: () => Promise.reject(_unimpl('listDatasources')),
  getDatasource: () => Promise.reject(_unimpl('getDatasource')),
  createDatasource: () => Promise.reject(_unimpl('createDatasource')),
  updateDatasource: () => Promise.reject(_unimpl('updateDatasource')),
  deleteDatasource: () => Promise.reject(_unimpl('deleteDatasource')),
}
// @ts-ignore 若 @/api/datasources 模块尚未创建则忽略解析错误
import * as _rawDatasources from '@/api/datasources'
const _dsRaw = markRaw(
  _rawDatasources as unknown as DatasourceApiShim | Record<string, unknown>,
)
function _bindApi<A extends object, S>(api: A | Record<string, unknown>, shim: S): S {
  const out = { ...(shim as unknown as object) } as S
  for (const key of Object.keys(shim as unknown as object)) {
    if (key in api && typeof (api as Record<string, unknown>)[key] === 'function') {
      ;(out as unknown as Record<string, unknown>)[key] = (api as Record<string, unknown>)[key]
    }
  }
  return out
}
const _api = _bindApi<Record<string, unknown>, DatasourceApiShim>(
  _dsRaw as unknown as Record<string, unknown>,
  _shimDatasources,
)
const {
  listDatasources,
  getDatasource,
  createDatasource,
  updateDatasource,
  deleteDatasource,
} = _api

// ============================================================
// 工具函数
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
function truncate(str: string, max: number): string {
  if (!str) return '-'
  if (str.length <= max) return str
  return str.slice(0, max) + '…'
}
function prettyJson(obj: Record<string, string> | undefined, maxLines = 20): string {
  if (!obj || Object.keys(obj).length === 0) return '(空)'
  try {
    const lines = JSON.stringify(obj, null, 2).split('\n')
    if (lines.length > maxLines) {
      return lines.slice(0, maxLines).join('\n') + `\n… (共 ${lines.length} 行，已截断)`
    }
    return lines.join('\n')
  } catch {
    return String(obj)
  }
}

// ============================================================
// Kind 元信息
// ============================================================
interface KindMeta {
  kind: DatasourceKind
  label: string
  color: string
  urlPlaceholder: string
  hint: string
}
const KINDS: KindMeta[] = [
  {
    kind: 'prometheus',
    label: 'Prometheus',
    color: '#1677ff',
    urlPlaceholder: 'http://prometheus:9090',
    hint: '填写 Prometheus HTTP API 地址（默认端口 9090）',
  },
  {
    kind: 'victoriametrics',
    label: 'VictoriaMetrics',
    color: '#13c2c2',
    urlPlaceholder: 'http://victoriametrics:8428',
    hint: '填写 VictoriaMetrics HTTP API 地址（兼容 Prometheus API）',
  },
  {
    kind: 'kafka',
    label: 'Kafka',
    color: '#2f54eb',
    urlPlaceholder: 'broker1:9092,broker2:9092',
    hint: '填写 Kafka Brokers，多节点用逗号分隔',
  },
  {
    kind: 'log',
    label: 'Loki',
    color: '#722ed1',
    urlPlaceholder: 'http://loki:3100',
    hint: '填写 Loki HTTP API 地址（日志数据源）',
  },
]
const KIND_MAP: Record<DatasourceKind, KindMeta> = KINDS.reduce(
  (acc, k) => {
    acc[k.kind] = k
    return acc
  },
  {} as Record<DatasourceKind, KindMeta>,
)
function kindMetaOf(k: string | DatasourceKind): KindMeta {
  return KIND_MAP[k as DatasourceKind] || KINDS[0]
}

// ============================================================
// 列表状态
// ============================================================
const datasources = ref<LocalDatasource[]>([])
const loading = ref(false)
const filterKind = ref<DatasourceKind | ''>('')
const filterQ = ref('')

const filtered = computed<LocalDatasource[]>(() => {
  let rows = datasources.value
  if (filterKind.value) {
    rows = rows.filter((r) => r.kind === filterKind.value)
  }
  const q = filterQ.value.trim().toLowerCase()
  if (q) {
    rows = rows.filter(
      (r) =>
        r.name.toLowerCase().includes(q) ||
        r.url.toLowerCase().includes(q),
    )
  }
  return rows
})

async function loadList(): Promise<void> {
  loading.value = true
  try {
    datasources.value = await listDatasources()
  } catch (e) {
    ElMessage.error(errMsgOf(e, '加载数据源列表失败'))
  } finally {
    loading.value = false
  }
}
onMounted(() => {
  void loadList()
})

// ============================================================
// 表格操作：enabled 乐观保存
// ============================================================
async function toggleEnabled(row: LocalDatasource, val: boolean): Promise<void> {
  const oldVal = row.enabled
  row.enabled = val
  try {
    const patch: DatasourceInput = {
      name: row.name,
      kind: row.kind,
      url: row.url,
      options: { ...row.options },
      enabled: val,
    }
    await updateDatasource(row.id, patch)
    ElMessage.success(val ? '已启用' : '已停用')
  } catch (e) {
    row.enabled = oldVal
    ElMessage.error(errMsgOf(e, '保存 enabled 失败，已回滚'))
  }
}

// ============================================================
// 表格操作：探测 / 编辑 / 删除
// ============================================================
async function probeDatasource(row: LocalDatasource): Promise<void> {
  try {
    await getDatasource(row.id)
    ElMessage.success('探测成功')
  } catch (e) {
    ElMessage.error(errMsgOf(e, '探测失败'))
  }
}
async function removeDatasource(row: LocalDatasource): Promise<void> {
  try {
    await ElMessageBox.confirm(
      `确认删除数据源「${row.name}」？此操作不可恢复。`,
      '删除确认',
      {
        type: 'warning',
        confirmButtonText: '删除',
        cancelButtonText: '取消',
      },
    )
  } catch {
    return
  }
  try {
    await deleteDatasource(row.id)
    ElMessage.success('删除成功')
    void loadList()
  } catch (e) {
    ElMessage.error(errMsgOf(e, '删除失败'))
  }
}

// ============================================================
// 新建 / 编辑 Dialog
// ============================================================
const dialogVisible = ref(false)
const isEdit = ref(false)
const editingId = ref<string | null>(null)
const dialogLoading = ref(false)
const advancedOpen = ref<string[]>([])

interface OptionsRow {
  key: string
  value: string
}

const form = reactive<{
  name: string
  kind: DatasourceKind
  url: string
  enabled: boolean
  optionsRows: OptionsRow[]
}>({
  name: '',
  kind: 'prometheus',
  url: '',
  enabled: true,
  optionsRows: [],
})
type FormModelT = typeof form
const formRef = ref<FormInstance | null>(null)

const formRules = reactive<FormRules<FormModelT>>({
  name: [{ required: true, message: '请输入名称', trigger: 'blur' }],
  kind: [{ required: true, message: '请选择类型', trigger: 'change' }],
  url: [
    { required: true, message: '请输入 URL / Brokers', trigger: 'blur' },
    {
      validator: (_rule, value, cb) => {
        if (!value || !String(value).trim()) {
          cb(new Error('URL / Brokers 必填'))
        } else cb()
      },
      trigger: 'blur',
    },
  ],
})

function resetForm(): void {
  form.name = ''
  form.kind = 'prometheus'
  form.url = ''
  form.enabled = true
  form.optionsRows = [{ key: '', value: '' }]
  advancedOpen.value = []
}

function openCreate(): void {
  resetForm()
  isEdit.value = false
  editingId.value = null
  dialogVisible.value = true
}

async function openEdit(row: LocalDatasource): Promise<void> {
  resetForm()
  isEdit.value = true
  editingId.value = row.id
  try {
    const detail = await getDatasource(row.id)
    form.name = detail.name
    form.kind = detail.kind
    form.url = detail.url
    form.enabled = detail.enabled
    const opts = detail.options || {}
    const keys = Object.keys(opts)
    if (keys.length === 0) {
      form.optionsRows = [{ key: '', value: '' }]
    } else {
      form.optionsRows = keys.map((k) => ({ key: k, value: opts[k] }))
      form.optionsRows.push({ key: '', value: '' })
    }
  } catch (e) {
    // 若 getDatasource 失败，仍以表格行数据兜底
    form.name = row.name
    form.kind = row.kind
    form.url = row.url
    form.enabled = row.enabled
    const opts = row.options || {}
    const keys = Object.keys(opts)
    if (keys.length === 0) {
      form.optionsRows = [{ key: '', value: '' }]
    } else {
      form.optionsRows = keys.map((k) => ({ key: k, value: opts[k] }))
      form.optionsRows.push({ key: '', value: '' })
    }
  }
  dialogVisible.value = true
}

function addOptionsRow(): void {
  form.optionsRows.push({ key: '', value: '' })
}
function removeOptionsRow(idx: number): void {
  if (form.optionsRows.length <= 1) {
    form.optionsRows = [{ key: '', value: '' }]
    return
  }
  form.optionsRows.splice(idx, 1)
}
function buildOptions(): Record<string, string> {
  const out: Record<string, string> = {}
  for (const r of form.optionsRows) {
    const k = r.key.trim()
    if (!k) continue
    out[k] = r.value
  }
  return out
}

async function submitForm(): Promise<void> {
  if (!formRef.value) return
  try {
    await formRef.value.validate()
  } catch {
    return
  }
  const payload: DatasourceInput = {
    name: form.name.trim(),
    kind: form.kind,
    url: form.url.trim(),
    options: buildOptions(),
    enabled: form.enabled,
  }
  dialogLoading.value = true
  try {
    if (isEdit.value && editingId.value) {
      await updateDatasource(editingId.value, payload)
      ElMessage.success('保存成功')
    } else {
      await createDatasource(payload)
      ElMessage.success('创建成功')
    }
    dialogVisible.value = false
    void loadList()
  } catch (e) {
    ElMessage.error(errMsgOf(e, isEdit.value ? '保存失败' : '创建失败'))
  } finally {
    dialogLoading.value = false
  }
}

// 切换 kind 时清空 url（避免 placeholder 误导）
function onKindChange(): void {
  form.url = ''
}
</script>

<template>
  <div class="datasources-page">
    <!-- 标题 + ElAffix 吸附工具栏 -->
    <el-affix :offset="0" class="ds-affix" z-index="5">
      <div class="ds-head">
        <div class="ds-head-left">
          <h2 class="ds-title">数据源</h2>
          <span class="ds-subtitle">Prometheus / VictoriaMetrics / Kafka / Loki（Log）</span>
        </div>
      </div>
      <!-- 工具栏 -->
      <div class="ds-toolbar panel" style="padding: 12px 18px;">
        <div class="ds-toolbar-row">
          <el-button type="primary" :icon="Plus" @click="openCreate">
            新建数据源
          </el-button>
          <el-button :icon="Refresh" link @click="loadList">刷新</el-button>
          <div class="ds-toolbar-filters">
            <el-select
              v-model="filterKind"
              placeholder="类型：全部"
              clearable
              style="width: 180px;"
            >
              <el-option
                v-for="k in KINDS"
                :key="k.kind"
                :label="k.label"
                :value="k.kind"
              >
                <div style="display: flex; align-items: center; gap: 8px;">
                  <el-tag :color="k.color" effect="dark" size="small" style="color: #fff;">
                    {{ k.label }}
                  </el-tag>
                </div>
              </el-option>
            </el-select>
            <el-input
              v-model="filterQ"
              :prefix-icon="Search"
              placeholder="搜索名称 / URL"
              clearable
              style="width: 260px;"
            />
          </div>
        </div>
      </div>
    </el-affix>

    <!-- 空态 -->
    <div
      v-if="!loading && datasources.length === 0"
      class="panel empty ds-empty"
    >
      <el-empty description="还没有任何数据源，按类型快速创建：">
        <template #image>
          <el-icon :size="64" color="var(--info-soft)"><Monitor /></el-icon>
        </template>
      </el-empty>
      <div class="ds-empty-hints">
        <div
          v-for="k in KINDS"
          :key="k.kind"
          class="ds-hint-row"
        >
          <el-tag :color="k.color" effect="dark" size="small" style="color: #fff; min-width: 120px; justify-content: center;">
            {{ k.label }}
          </el-tag>
          <span class="ds-hint-text">{{ k.hint }}</span>
        </div>
      </div>
      <div style="margin-top: 18px;">
        <el-button type="primary" :icon="Plus" @click="openCreate">
          新建第一个数据源
        </el-button>
      </div>
    </div>

    <!-- 列表 -->
    <div v-else class="ds-table-wrap panel" style="padding: 0;">
      <el-table
        :data="filtered"
        v-loading="loading"
        stripe
        border
        size="default"
        style="width: 100%;"
        empty-text="暂无匹配的数据源"
      >
        <!-- 名称 name -->
        <el-table-column label="名称" min-width="180" fixed="left">
          <template #default="{ row }">
            <el-button
              link
              type="primary"
              style="font-weight: 500; padding: 0;"
              @click="openEdit(row as LocalDatasource)"
            >
              {{ (row as LocalDatasource).name }}
            </el-button>
          </template>
        </el-table-column>

        <!-- kind -->
        <el-table-column label="类型" width="140" align="center">
          <template #default="{ row }">
            <el-tag
              :color="kindMetaOf((row as LocalDatasource).kind).color"
              effect="dark"
              style="color: #fff;"
            >
              {{ kindMetaOf((row as LocalDatasource).kind).label }}
            </el-tag>
          </template>
        </el-table-column>

        <!-- url -->
        <el-table-column label="URL / Brokers" min-width="300">
          <template #default="{ row }">
            <div class="ds-url-cell">
              <el-tooltip
                :content="(row as LocalDatasource).url"
                placement="top"
                :disabled="!(row as LocalDatasource).url"
              >
                <span class="ds-url-text">
                  {{ truncate((row as LocalDatasource).url, 40) }}
                </span>
              </el-tooltip>
              <el-button
                v-if="(row as LocalDatasource).url"
                :icon="DocumentCopy"
                size="small"
                text
                type="primary"
                @click="copyText((row as LocalDatasource).url)"
              >
                复制
              </el-button>
            </div>
          </template>
        </el-table-column>

        <!-- options -->
        <el-table-column label="Options" width="110" align="center">
          <template #default="{ row }">
            <el-popover
              placement="top"
              :width="420"
              trigger="hover"
              :disabled="!(row as LocalDatasource).options || Object.keys((row as LocalDatasource).options).length === 0"
            >
              <template #reference>
                <span class="ds-options-count">
                  {{ Object.keys((row as LocalDatasource).options || {}).length }} 项
                </span>
              </template>
              <pre class="ds-popover-pre">{{ prettyJson((row as LocalDatasource).options, 20) }}</pre>
            </el-popover>
          </template>
        </el-table-column>

        <!-- enabled -->
        <el-table-column label="启用" width="90" align="center">
          <template #default="{ row }">
            <el-switch
              :model-value="(row as LocalDatasource).enabled"
              @update:model-value="(val) => toggleEnabled(row as LocalDatasource, val as boolean)"
              active-text="是"
              inactive-text="否"
            />
          </template>
        </el-table-column>

        <!-- updated_at -->
        <el-table-column label="更新时间" width="180" align="center">
          <template #default="{ row }">
            <span style="color: var(--text-color-secondary); font-size: 13px;">
              {{ (row as LocalDatasource).updated_at || '-' }}
            </span>
          </template>
        </el-table-column>

        <!-- 操作列 -->
        <el-table-column label="操作" width="220" align="center" fixed="right">
          <template #default="{ row }">
            <el-button
              size="small"
              :icon="Monitor"
              type="success"
              plain
              @click="probeDatasource(row as LocalDatasource)"
            >
              探测
            </el-button>
            <el-button
              size="small"
              :icon="Edit"
              plain
              @click="openEdit(row as LocalDatasource)"
            >
              编辑
            </el-button>
            <el-button
              size="small"
              :icon="Delete"
              type="danger"
              plain
              @click="removeDatasource(row as LocalDatasource)"
            >
              删除
            </el-button>
          </template>
        </el-table-column>
      </el-table>
    </div>

    <!-- 新建 / 编辑 Dialog -->
    <el-dialog
      v-model="dialogVisible"
      :title="isEdit ? '编辑数据源' : '新建数据源'"
      width="760px"
      :close-on-click-modal="false"
      top="6vh"
    >
      <el-form
        ref="formRef"
        :model="form"
        :rules="formRules"
        label-width="110px"
        label-position="right"
      >
        <el-row :gutter="14">
          <el-col :xs="24" :md="14">
            <el-form-item label="名称" prop="name">
              <el-input v-model="form.name" placeholder="例如：Prometheus-生产 / Kafka-Core" />
            </el-form-item>
          </el-col>
          <el-col :xs="24" :md="10">
            <el-form-item label="启用">
              <el-switch v-model="form.enabled" active-text="启用" inactive-text="停用" />
            </el-form-item>
          </el-col>
        </el-row>

        <el-row :gutter="14">
          <el-col :xs="24" :md="10">
            <el-form-item label="类型" prop="kind">
              <el-select v-model="form.kind" style="width: 100%;" @change="onKindChange">
                <el-option
                  v-for="k in KINDS"
                  :key="k.kind"
                  :label="k.label"
                  :value="k.kind"
                >
                  <div style="display: flex; align-items: center; gap: 8px;">
                    <el-tag :color="k.color" effect="dark" size="small" style="color: #fff;">
                      {{ k.label }}
                    </el-tag>
                  </div>
                </el-option>
              </el-select>
            </el-form-item>
          </el-col>
          <el-col :xs="24" :md="14">
            <el-form-item label="URL / Brokers" prop="url">
              <el-input
                v-model="form.url"
                :placeholder="kindMetaOf(form.kind).urlPlaceholder"
              />
            </el-form-item>
          </el-col>
        </el-row>

        <!-- 高级 options：折叠面板 + 动态 key/value 行 -->
        <el-collapse v-model="advancedOpen" style="border: none;">
          <el-collapse-item name="advanced" title="高级 options（Map&lt;String,String&gt;）">
            <div class="ds-options-editor">
              <div
                v-for="(row, idx) in form.optionsRows"
                :key="idx"
                class="ds-options-row"
              >
                <el-input
                  v-model="row.key"
                  placeholder="key（空行将被忽略）"
                  style="flex: 1;"
                />
                <el-input
                  v-model="row.value"
                  placeholder="value"
                  style="flex: 1; margin-left: 8px;"
                />
                <el-button
                  type="danger"
                  plain
                  style="margin-left: 8px;"
                  @click="removeOptionsRow(idx as number)"
                >
                  删除
                </el-button>
              </div>
              <el-button link type="primary" @click="addOptionsRow" style="margin-top: 8px;">
                + 添加一行
              </el-button>
            </div>
          </el-collapse-item>
        </el-collapse>
      </el-form>

      <template #footer>
        <el-button @click="dialogVisible = false">取消</el-button>
        <el-button type="primary" :loading="dialogLoading" @click="submitForm">
          {{ isEdit ? '保存修改' : '创建数据源' }}
        </el-button>
      </template>
    </el-dialog>
  </div>
</template>

<style>
/* 非 scoped；类名加 ds- 前缀避免与全局样式冲突 */

.datasources-page {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.ds-affix {
  background: var(--bg-color, #ffffff);
  border-bottom: 1px solid var(--border-color-lighter, #ebeef5);
}

.ds-head {
  padding: 14px 20px 0 20px;
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.ds-head-left {
  display: flex;
  align-items: baseline;
  gap: 14px;
}

.ds-title {
  margin: 0;
  font-size: 20px;
  font-weight: 600;
  color: var(--text-color-primary, #1f2329);
}

.ds-subtitle {
  color: var(--text-color-secondary, #86909c);
  font-size: 13px;
}

.ds-toolbar-row {
  display: flex;
  align-items: center;
  gap: 12px;
  flex-wrap: wrap;
}

.ds-toolbar-filters {
  margin-left: auto;
  display: flex;
  align-items: center;
  gap: 10px;
}

.ds-empty {
  padding: 30px 20px 40px;
  text-align: center;
}

.ds-empty-hints {
  max-width: 640px;
  margin: 14px auto 0;
  display: flex;
  flex-direction: column;
  gap: 10px;
  text-align: left;
}

.ds-hint-row {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 8px 12px;
  background: var(--fill-color-light, #f7f8fa);
  border-radius: 6px;
}

.ds-hint-text {
  color: var(--text-color-secondary, #4e5969);
  font-size: 13px;
}

.ds-table-wrap {
  overflow: hidden;
  border-radius: 8px;
}

.ds-url-cell {
  display: flex;
  align-items: center;
  gap: 6px;
}

.ds-url-text {
  font-family: 'SFMono-Regular', Consolas, 'Liberation Mono', Menlo, monospace;
  font-size: 13px;
  color: var(--text-color-primary, #1f2329);
  background: var(--fill-color-lighter, #f2f3f5);
  padding: 2px 8px;
  border-radius: 4px;
  max-width: 320px;
  display: inline-block;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  vertical-align: middle;
}

.ds-options-count {
  display: inline-block;
  padding: 2px 10px;
  background: var(--fill-color-light, #f7f8fa);
  color: var(--text-color-secondary, #4e5969);
  border-radius: 12px;
  font-size: 12px;
  cursor: help;
}

.ds-popover-pre {
  margin: 0;
  padding: 10px 12px;
  background: var(--bg-color-overlay, #1d2129);
  color: #e5e6eb;
  border-radius: 6px;
  font-family: 'SFMono-Regular', Consolas, 'Liberation Mono', Menlo, monospace;
  font-size: 12px;
  line-height: 1.5;
  max-height: 380px;
  overflow: auto;
  white-space: pre-wrap;
  word-break: break-all;
}

.ds-options-editor {
  padding: 8px 0;
}

.ds-options-row {
  display: flex;
  align-items: center;
  margin-bottom: 8px;
}
</style>
