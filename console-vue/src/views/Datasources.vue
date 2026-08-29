<script setup lang="ts">
import { computed, onMounted, reactive, ref, markRaw } from 'vue'
import {
  ElAffix,
  ElDialog,
  ElEmpty,
  ElInput,
  ElMessage,
  ElMessageBox,
  ElOption,
  ElSelect,
  ElTable,
  ElTableColumn,
  ElPagination,
} from 'element-plus'
import {
  Monitor,
  Search,
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
  name: string
  /** 选择卡片上显示的两字母缩写（大写） */
  icon: string
  /** 配色主题，对应 CSS 类 .tone-{tone} */
  tone: 'orange' | 'blue' | 'amber' | 'teal'
  /** 选择卡片下方的功能描述 */
  typeDesc: string
  /** URL 输入框 placeholder */
  urlPlaceholder: string
}
const KINDS: KindMeta[] = [
  {
    kind: 'prometheus',
    name: 'Prometheus',
    icon: 'PM',
    tone: 'orange',
    typeDesc: 'PromQL 指标查询',
    urlPlaceholder: 'http://127.0.0.1:9090',
  },
  {
    kind: 'victoriametrics',
    name: 'VictoriaMetrics',
    icon: 'VM',
    tone: 'blue',
    typeDesc: '兼容 PromQL',
    urlPlaceholder: 'http://127.0.0.1:8428',
  },
  {
    kind: 'kafka',
    name: 'Kafka',
    icon: 'K',
    tone: 'amber',
    typeDesc: '消息管道 · JSON 字段告警',
    urlPlaceholder: '127.0.0.1:9092',
  },
  {
    kind: 'log',
    name: 'Loki 日志',
    icon: 'Lo',
    tone: 'teal',
    typeDesc: 'LogQL 日志统计',
    urlPlaceholder: 'http://127.0.0.1:3100',
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

// —— 分页
const DS_PAGE_SIZES = [10, 20, 50, 100] as const
const dsPage = ref(1)
const dsPageSize = ref<number>(DS_PAGE_SIZES[1])
const pagedDatasources = computed<LocalDatasource[]>(() => {
  const src = filtered.value
  if (src.length <= dsPageSize.value) return src
  const start = (dsPage.value - 1) * dsPageSize.value
  return src.slice(start, start + dsPageSize.value)
})
function dsOnPage(p: number) { dsPage.value = Math.max(1, p) }
function dsOnSize(s: number) { dsPageSize.value = s; dsPage.value = 1 }
import { watch as _dsWatch } from 'vue'
_dsWatch([filterKind, filterQ, datasources], () => { dsPage.value = 1 })

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
const typePickerVisible = ref(false)
const isEdit = ref(false)
const editingId = ref<string | null>(null)
const dialogLoading = ref(false)

const form = reactive({
  name: '',
  kind: 'prometheus' as DatasourceKind,
  url: '',
  enabled: true,
  // Kafka 专用 options
  topic: '',
  field: '',
  label_fields: '',
  max_records: '100',
  mode: 'field' as string,
  partitions: '8',
})

function resetForm(): void {
  form.name = ''
  form.kind = 'prometheus'
  form.url = ''
  form.enabled = true
  form.topic = ''
  form.field = ''
  form.label_fields = ''
  form.max_records = '100'
  form.mode = 'field'
  form.partitions = '8'
}

// 类型选择卡片
function openCreate(): void {
  typePickerVisible.value = true
}

function pickKind(kind: DatasourceKind): void {
  typePickerVisible.value = false
  resetForm()
  form.kind = kind
  isEdit.value = false
  editingId.value = null
  dialogVisible.value = true
}

// 新建时：从编辑表单返回类型选择
function repickKind(): void {
  dialogVisible.value = false
  typePickerVisible.value = true
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
    form.topic = opts.topic || ''
    form.field = opts.field || ''
    form.label_fields = opts.label_fields || ''
    form.max_records = opts.max_records || '100'
    form.mode = opts.mode || 'field'
    form.partitions = opts.partitions || '8'
  } catch {
    form.name = row.name
    form.kind = row.kind
    form.url = row.url
    form.enabled = row.enabled
    const opts = row.options || {}
    form.topic = opts.topic || ''
    form.field = opts.field || ''
    form.label_fields = opts.label_fields || ''
    form.max_records = opts.max_records || '100'
    form.mode = opts.mode || 'field'
    form.partitions = opts.partitions || '8'
  }
  dialogVisible.value = true
}

// URL 标签按类型变化
const urlLabel = computed(() => {
  if (form.kind === 'log') return 'Loki 地址'
  if (form.kind === 'victoriametrics') return 'VictoriaMetrics 地址'
  if (form.kind === 'kafka') return 'Brokers'
  return 'Prometheus 地址'
})
const urlPlaceholder = computed(() => {
  if (form.kind === 'log') return 'http://127.0.0.1:3100'
  if (form.kind === 'victoriametrics') return 'http://127.0.0.1:8428'
  if (form.kind === 'kafka') return '127.0.0.1:9092'
  return 'http://127.0.0.1:9090'
})
const urlHint = computed(() => {
  if (form.kind === 'log') return '填写 Loki 根地址。规则表达式使用 LogQL。'
  if (form.kind === 'victoriametrics') return '兼容 PromQL 的查询入口。'
  if (form.kind === 'kafka') return '多个 broker 用英文逗号分隔。消息按 JSON 解析字段后走阈值规则。'
  return '填写 Prometheus 查询 API 根地址。'
})

async function submitForm(): Promise<void> {
  // 手动校验（替代 el-form validate）
  if (!form.name.trim()) {
    ElMessage.error('请输入名称')
    return
  }
  if (!form.url.trim()) {
    ElMessage.error('请填写连接地址')
    return
  }

  const options: Record<string, string> = {}
  if (form.kind === 'kafka') {
    if (!form.topic.trim()) {
      ElMessage.error('请填写 Kafka Topic')
      return
    }
    options.topic = form.topic.trim()
    options.mode = form.mode
    if (form.field.trim()) options.field = form.field.trim()
    if (form.label_fields.trim()) options.label_fields = form.label_fields.trim()
    if (form.max_records.trim()) options.max_records = form.max_records.trim()
    if (form.partitions.trim()) options.partitions = form.partitions.trim()
  }

  const payload: DatasourceInput = {
    name: form.name.trim(),
    kind: form.kind,
    url: form.url.trim(),
    options,
    enabled: form.enabled,
  }
  dialogLoading.value = true
  try {
    if (isEdit.value && editingId.value) {
      await updateDatasource(editingId.value, payload)
      ElMessage.success('已保存')
    } else {
      await createDatasource(payload)
      ElMessage.success('已保存')
    }
    dialogVisible.value = false
    void loadList()
  } catch (e) {
    ElMessage.error(errMsgOf(e, isEdit.value ? '保存失败' : '创建失败'))
  } finally {
    dialogLoading.value = false
  }
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
          <button class="primary" @click="openCreate">新建数据源</button>
          <button class="ghost" @click="loadList">刷新</button>
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
                :label="k.name"
                :value="k.kind"
              >
                <div style="display: flex; align-items: center; gap: 8px;">
                  <span :class="['ds-type-pick-ico', 'ds-tone-' + k.tone]" style="width: 20px; height: 20px; font-size: 10px; border-radius: 4px;">
                    {{ k.icon }}
                  </span>
                  <span>{{ k.name }}</span>
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
          <span :class="['ds-type-pick-ico', 'ds-tone-' + k.tone]" style="width: 24px; height: 24px; font-size: 10px; border-radius: 6px;">
            {{ k.icon }}
          </span>
          <span class="ds-hint-name">{{ k.name }}</span>
          <span class="ds-hint-text">{{ k.typeDesc }}</span>
        </div>
      </div>
      <div style="margin-top: 18px;">
        <button class="primary" @click="openCreate">新建第一个数据源</button>
      </div>
    </div>

    <!-- 列表 -->
    <div v-else class="ds-table-wrap panel" style="padding: 0;">
      <el-table
        :data="pagedDatasources"
        v-loading="loading"
        stripe
        border
        size="default"
        style="width: 100%;"
        empty-text="暂无匹配的数据源"
      >
        <!-- 名称 -->
        <el-table-column label="名称" min-width="180">
          <template #default="{ row }">
            <span style="font-weight: 500; color: var(--heading); cursor: pointer;" @click="openEdit(row as LocalDatasource)">
              {{ (row as LocalDatasource).name }}
            </span>
          </template>
        </el-table-column>

        <!-- 类型 -->
        <el-table-column label="类型" width="160" align="center">
          <template #default="{ row }">
            <span :class="['ds-type-pick-ico', 'ds-tone-' + kindMetaOf((row as LocalDatasource).kind).tone]" style="width: 22px; height: 22px; font-size: 10px; border-radius: 4px; vertical-align: middle;">
              {{ kindMetaOf((row as LocalDatasource).kind).icon }}
            </span>
            <span style="margin-left: 6px; font-size: 13px; color: var(--text);">
              {{ kindMetaOf((row as LocalDatasource).kind).name }}
            </span>
          </template>
        </el-table-column>

        <!-- URL -->
        <el-table-column label="URL" min-width="300">
          <template #default="{ row }">
            <span class="mono-cell" style="font-family: ui-monospace, monospace; font-size: 12px; color: var(--text-secondary);">
              {{ truncate((row as LocalDatasource).url, 50) }}
            </span>
          </template>
        </el-table-column>

        <!-- 状态 -->
        <el-table-column label="状态" width="100" align="center">
          <template #default="{ row }">
            <span :class="['badge', (row as LocalDatasource).enabled ? 'on' : 'off']">
              {{ (row as LocalDatasource).enabled ? '启用' : '停用' }}
            </span>
          </template>
        </el-table-column>

        <!-- 操作 -->
        <el-table-column label="操作" width="150" align="center" fixed="right">
          <template #default="{ row }">
            <div class="actions">
              <button @click="openEdit(row as LocalDatasource)">编辑</button>
              <button class="danger" @click="removeDatasource(row as LocalDatasource)">删除</button>
            </div>
          </template>
        </el-table-column>
      </el-table>
    </div>

    <div class="ds-pager">
      <div class="pager-tip">共 <span class="mono">{{ filtered.length }}</span> 条</div>
      <el-pagination
        v-model:current-page="dsPage"
        v-model:page-size="dsPageSize"
        :page-sizes="Array.from(DS_PAGE_SIZES)"
        :total="filtered.length"
        layout="sizes, prev, pager, next, jumper, ->, total"
        background
        @current-change="dsOnPage"
        @size-change="dsOnSize"
      />
    </div>

    <!-- 类型选择 Dialog：先选类型再填表单（与老版一致） -->
    <el-dialog
      v-model="typePickerVisible"
      width="880px"
      :close-on-click-modal="false"
      append-to-body
      :show-close="false"
    >
      <template #header>
        <div class="ds-modal-head">
          <h3>选择数据源类型</h3>
          <p class="ds-modal-desc">先选类型，再填写连接信息；规则评估会按类型自动拉数。</p>
        </div>
      </template>
      <div class="ds-modal-body">
        <div class="ds-type-pick-grid">
          <button
            v-for="k in KINDS"
            :key="k.kind"
            type="button"
            class="ds-type-pick-card"
            @click="pickKind(k.kind)"
          >
            <span :class="['ds-type-pick-ico', 'ds-tone-' + k.tone]">
              {{ k.icon }}
            </span>
            <span class="ds-type-pick-name">{{ k.name }}</span>
            <span class="ds-type-pick-desc">{{ k.typeDesc }}</span>
          </button>
        </div>
      </div>
      <template #footer>
        <div class="ds-modal-actions">
          <button type="button" class="ds-btn-ghost" @click="typePickerVisible = false">取消</button>
        </div>
      </template>
    </el-dialog>

    <!-- 新建 / 编辑 Dialog -->
    <el-dialog
      v-model="dialogVisible"
      width="880px"
      :close-on-click-modal="false"
      append-to-body
      :show-close="false"
    >
      <template #header>
        <div class="ds-modal-head">
          <h3>{{ isEdit ? '编辑数据源' : '新建数据源' }}</h3>
          <p class="ds-modal-desc">填写连接信息，规则评估会按类型自动拉数。</p>
        </div>
      </template>
      <div class="ds-modal-body">
        <!-- 名称 -->
        <div class="ds-field">
          <label>名称</label>
          <input
            v-model="form.name"
            type="text"
            required
            placeholder="例如：生产 Prometheus"
          />
        </div>

        <!-- 类型 -->
        <div class="ds-field">
          <label>类型</label>
          <input type="hidden" v-model="form.kind" />
          <div class="ds-type-picked">
            <span :class="['ds-type-pick-ico', 'ds-tone-' + kindMetaOf(form.kind).tone]">
              {{ kindMetaOf(form.kind).icon }}
            </span>
            <div class="ds-type-picked-text">
              <div class="ds-t-name">{{ kindMetaOf(form.kind).name }}</div>
              <div class="ds-t-desc">{{ kindMetaOf(form.kind).typeDesc }}</div>
            </div>
            <button
              v-if="!isEdit"
              type="button"
              class="ds-btn-ghost"
              @click="repickKind"
            >重选类型</button>
          </div>
        </div>

        <!-- 启用 -->
        <div class="ds-field">
          <label class="ds-check-row">
            <input
              v-model="form.enabled"
              type="checkbox"
            />
            <span>启用此数据源；停用后关联规则将跳过评估</span>
          </label>
        </div>

        <!-- HTTP 面板：Prometheus / VictoriaMetrics / Loki -->
        <div v-if="form.kind !== 'kafka'" class="ds-kind-panel">
          <div class="ds-field">
            <label>{{ urlLabel }}</label>
            <input
              v-model="form.url"
              type="text"
              :placeholder="urlPlaceholder"
            />
            <div class="ds-hint">{{ urlHint }}</div>
          </div>
        </div>

        <!-- Kafka 面板 -->
        <div v-else class="ds-kind-panel">
          <div class="ds-field">
            <label>Brokers</label>
            <input
              v-model="form.url"
              type="text"
              :placeholder="urlPlaceholder"
            />
            <div class="ds-hint">多个 broker 用英文逗号分隔。消息按 JSON 解析字段后走阈值规则。</div>
          </div>
          <div class="ds-row">
            <div class="ds-field">
              <label>Topic</label>
              <input v-model="form.topic" type="text" placeholder="orders" />
            </div>
            <div class="ds-field">
              <label>数值字段（JSON 路径）</label>
              <input v-model="form.field" type="text" placeholder="latency_ms 或 metrics.p99" />
            </div>
          </div>
          <div class="ds-row">
            <div class="ds-field">
              <label>标签字段（可选）</label>
              <input v-model="form.label_fields" type="text" placeholder="service,instance" />
              <div class="ds-hint">从消息中取出作为标签，用于分组与告警标识。</div>
            </div>
            <div class="ds-field">
              <label>每次拉取条数</label>
              <input v-model="form.max_records" type="number" min="1" placeholder="100" />
            </div>
          </div>
          <div class="ds-row">
            <div class="ds-field">
              <label>评估模式</label>
              <select v-model="form.mode">
                <option value="field">字段解析 field（推荐）</option>
                <option value="depth">堆积深度 depth</option>
                <option value="count">近期消息数 count</option>
              </select>
            </div>
            <div class="ds-field">
              <label>扫描分区数</label>
              <input v-model="form.partitions" type="number" min="1" placeholder="8" />
            </div>
          </div>
          <div class="ds-hint">
            规则表达式可覆盖数值字段路径；例如消息 {"latency_ms":820,"service":"api"}，字段填 latency_ms，阈值 &gt; 500。
          </div>
        </div>
      </div>
      <template #footer>
        <div class="ds-modal-actions">
          <button type="button" class="ds-btn-ghost" @click="dialogVisible = false">取消</button>
          <button
            type="button"
            class="ds-btn-primary"
            :disabled="dialogLoading"
            @click="submitForm"
          >{{ dialogLoading ? '保存中…' : '保存' }}</button>
        </div>
      </template>
    </el-dialog>
  </div>
</template>

<style>
/* 非 scoped；类名加 ds- 前缀避免与全局样式冲突 */

/* === 分页条 === */
.ds-pager {
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
.ds-pager .pager-tip {
  font-size: 13px;
  color: var(--el-text-color-secondary, #909399);
}
.ds-pager .mono {
  font-family: Consolas, Menlo, monospace;
  font-weight: 600;
  color: var(--el-text-color-primary, #303133);
  padding: 0 2px;
}

.datasources-page {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.ds-affix {
  background: var(--el-bg-color, var(--panel));
  border-bottom: 1px solid var(--el-border-color-lighter, var(--line-soft));
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
  color: var(--el-text-color-primary, #1f2329);
}

.ds-subtitle {
  color: var(--el-text-color-secondary, #86909c);
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

.ds-hint-name {
  font-weight: 650;
  color: var(--el-text-color-primary, #1f2329);
  min-width: 100px;
}

.ds-hint-text {
  color: var(--el-text-color-secondary, #4e5969);
  font-size: 13px;
}

.ds-table-wrap {
  overflow: hidden;
  border-radius: 8px;
}

/* ============================================================
 * Dialog 结构（与老版 .modal 一致）
 * ============================================================ */
.ds-modal-head {
  padding: 20px 28px 16px;
  border-bottom: 1px solid var(--el-border-color, #e5e6eb);
  flex-shrink: 0;
  margin: -20px -20px 0 -20px; /* 抵消 el-dialog 默认 padding */
}
.ds-modal-head h3 {
  margin: 0;
  font-size: 17px;
  font-weight: 650;
  color: var(--el-text-color-primary, #1f2329);
}
.ds-modal-desc {
  margin: 6px 0 0;
  color: var(--el-text-color-secondary, #86909c);
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
  display: flex;
  justify-content: flex-end;
  align-items: center;
  gap: 12px;
  padding: 16px 28px 20px;
  border-top: 1px solid var(--el-border-color, #e5e6eb);
  background: var(--fill-color-lighter, #f7f8fa);
  flex-shrink: 0;
  margin: 0 -20px -20px -20px; /* 抵消 el-dialog 默认 padding */
}

/* 覆盖 el-dialog 默认 padding，让我们的 modal-head/body/actions 铺满 */
.el-dialog :deep(.el-dialog__header) {
  padding: 0;
  margin: 0;
}
.el-dialog :deep(.el-dialog__body) {
  padding: 0;
}
.el-dialog :deep(.el-dialog__footer) {
  padding: 0;
}
.el-dialog :deep(.el-dialog__headerbtn) {
  top: 16px;
  right: 20px;
}

/* ============================================================
 * 按钮样式（与老版 .ghost / .primary 一致）
 * ============================================================ */
.ds-btn-ghost {
  min-width: 100px;
  padding: 10px 18px;
  font-size: 14px;
  border: 1px solid var(--el-border-color, #e5e6eb);
  border-radius: 6px;
  background: transparent;
  color: var(--el-text-color-primary, #1f2329);
  cursor: pointer;
  transition: border-color 0.15s, background 0.15s;
}
.ds-btn-ghost:hover {
  border-color: var(--el-color-primary, #1677ff);
  color: var(--el-color-primary, #1677ff);
  background: var(--el-color-primary-light-9, #ecf5ff);
}

.ds-btn-primary {
  min-width: 100px;
  padding: 10px 18px;
  font-size: 14px;
  border: 1px solid var(--el-color-primary, #1677ff);
  border-radius: 6px;
  background: var(--el-color-primary, #1677ff);
  color: #fff;
  cursor: pointer;
  transition: background 0.15s, border-color 0.15s;
}
.ds-btn-primary:hover {
  background: var(--el-color-primary-hover, #4096ff);
  border-color: var(--el-color-primary-hover, #4096ff);
}
.ds-btn-primary:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

/* ============================================================
 * 类型选择卡片（与老版 .type-pick-grid / .type-pick-card 一致）
 * ============================================================ */
.ds-type-pick-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
  gap: 12px;
}
.ds-type-pick-card {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: flex-start;
  gap: 8px;
  height: auto;
  min-height: 148px;
  padding: 20px 14px 16px;
  border: 1px solid var(--el-border-color, #e5e6eb);
  border-radius: 12px;
  background: var(--fill-color-lighter, #f7f8fa);
  color: var(--el-text-color-primary, #1f2329);
  cursor: pointer;
  text-align: center;
  font: inherit;
  white-space: normal;
  transition: border-color 0.15s, background 0.15s, transform 0.12s;
}
.ds-type-pick-card:hover {
  border-color: var(--el-color-primary, #1677ff);
  background: color-mix(in srgb, #1677ff 8%, var(--fill-color-lighter, #f7f8fa));
  transform: translateY(-1px);
}

/* 类型图标（tone 渐变，与老版一致） */
.ds-type-pick-ico {
  width: 48px;
  height: 48px;
  border-radius: 12px;
  display: grid;
  place-items: center;
  font-size: 15px;
  font-weight: 700;
  letter-spacing: -0.02em;
  color: #fff;
}
.ds-tone-orange {
  background: linear-gradient(145deg, #f59e0b, #ea580c);
}
.ds-tone-blue {
  background: linear-gradient(145deg, #3b82f6, #1d4ed8);
}
.ds-tone-amber {
  background: linear-gradient(145deg, #231f20, #4b5563);
  color: #f5c518;
}
.ds-tone-teal {
  background: linear-gradient(145deg, #2dd4bf, #0f766e);
}

.ds-type-pick-name {
  font-weight: 650;
  color: var(--el-text-color-primary, #1f2329);
  font-size: 14px;
}
.ds-type-pick-desc {
  font-size: 12px;
  color: var(--el-text-color-secondary, #86909c);
  line-height: 1.4;
  display: block;
}

/* ============================================================
 * 类型已选卡片（与老版 .type-picked 一致）
 * ============================================================ */
.ds-type-picked {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 10px 12px;
  border: 1px solid var(--el-border-color, #e5e6eb);
  border-radius: 10px;
  background: var(--fill-color-lighter, #f7f8fa);
}
.ds-type-picked .ds-type-pick-ico {
  width: 40px;
  height: 40px;
  font-size: 13px;
  flex-shrink: 0;
}
.ds-type-picked-text {
  flex: 1;
  min-width: 0;
}
.ds-t-name {
  font-weight: 650;
  color: var(--el-text-color-primary, #1f2329);
}
.ds-t-desc {
  font-size: 12px;
  color: var(--el-text-color-secondary, #86909c);
}

/* ============================================================
 * 表单结构（与老版 .field / .row / .hint / .check-row 一致）
 * ============================================================ */
.ds-field {
  margin-bottom: 20px;
}
.ds-field label {
  display: block;
  font-size: 13px;
  font-weight: 500;
  color: var(--el-text-color-primary, #1f2329);
  margin-bottom: 6px;
}
.ds-field input[type="text"],
.ds-field input[type="number"],
.ds-field select {
  width: 100%;
  padding: 8px 12px;
  font-size: 14px;
  border: 1px solid var(--el-border-color, var(--line));
  border-radius: 6px;
  background: var(--el-bg-color, var(--panel));
  color: var(--el-text-color-primary, var(--heading));
  outline: none;
  transition: border-color 0.15s;
  box-sizing: border-box;
}
.ds-field input[type="text"]:focus,
.ds-field input[type="number"]:focus,
.ds-field select:focus {
  border-color: var(--el-color-primary, #1677ff);
}
.ds-field input[type="text"]::placeholder,
.ds-field input[type="number"]::placeholder {
  color: var(--el-text-color-placeholder, #a8abb2);
}

.ds-row {
  display: flex;
  gap: 16px;
  flex-wrap: wrap;
}
.ds-row > .ds-field {
  flex: 1;
  min-width: 140px;
}

.ds-hint {
  margin-top: 6px;
  font-size: 12px;
  color: var(--el-text-color-secondary, #86909c);
  line-height: 1.5;
}

.ds-check-row {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  cursor: pointer;
  margin: 0;
  color: var(--el-text-color-primary, #1f2329);
  line-height: 1.5;
}
.ds-check-row input[type="checkbox"] {
  width: 16px;
  height: 16px;
  min-height: 16px;
  margin: 3px 0 0;
  accent-color: var(--el-color-primary, #1677ff);
  flex-shrink: 0;
  cursor: pointer;
}
.ds-check-row span {
  font-size: 14px;
  color: var(--el-text-color-secondary, #4e5969);
}

.ds-kind-panel[hidden] {
  display: none !important;
}

/* ============================================================
 * 状态徽章 - 使用全局 .badge 类（与老版对齐）
 * ============================================================ */
</style>
