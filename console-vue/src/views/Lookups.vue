<script setup lang="ts">
import { computed, onMounted, reactive, ref, watch, nextTick } from 'vue'
import {
  ElButton, ElDialog, ElEmpty, ElForm, ElFormItem, ElInput, ElMessage, ElMessageBox,
  ElOption, ElRow, ElCol, ElSelect, ElSwitch, ElTable, ElTableColumn, ElTooltip,
  ElAlert, ElTag, ElRadio, ElRadioGroup, ElTabs, ElTabPane, ElDrawer, ElPagination,
} from 'element-plus'
import type { FormInstance, FormRules } from 'element-plus'
import { Delete, Edit, Plus, Refresh, UploadFilled, Document, Picture, Search } from '@element-plus/icons-vue'
import type { LookupTable, LookupInput } from '@/api/types'
import {
  listLookups, getLookup, createLookup, updateLookup, deleteLookup,
} from '@/api/enrichments'

const KEY_LABEL_OPTIONS: string[] = [
  'instance', 'ip', 'hostname', 'alertname', 'severity', 'device_id',
]

const loading = ref(false)
const lookups = ref<LookupTable[]>([])
const filterQ = ref('')

const filteredLookups = computed(() => {
  if (!filterQ.value.trim()) return lookups.value
  const q = filterQ.value.trim().toLowerCase()
  return lookups.value.filter((l) => l.name.toLowerCase().includes(q))
})

const PAGE_SIZES = [10, 20, 50, 100] as const
const page = ref(1)
const pageSize = ref<number>(PAGE_SIZES[1])
const pagedLookups = computed(() => {
  const src = filteredLookups.value
  if (src.length <= pageSize.value) return src
  const start = (page.value - 1) * pageSize.value
  return src.slice(start, start + pageSize.value)
})
function onPage(p: number) { page.value = Math.max(1, p) }
function onSize(s: number) { pageSize.value = s; page.value = 1 }
watch([filterQ, lookups], () => { page.value = 1 })

function asLookup(r: unknown): LookupTable { return r as LookupTable }
function formatTime(s: string | null | undefined): string {
  if (!s) return '—'
  return s.replace('T', ' ').slice(0, 19)
}
function rowsCount(r: Record<string, Record<string, string>> | undefined): number {
  if (!r) return 0
  return Object.keys(r).filter((k) => k && k.trim() !== '').length
}
function colsCount(r: Record<string, Record<string, string>> | undefined): number {
  if (!r) return 0
  const s = new Set<string>()
  Object.values(r).forEach((attrs) => Object.keys(attrs || {}).forEach((k) => s.add(k)))
  return s.size
}

async function loadAll() {
  loading.value = true
  try {
    const ls = await listLookups().catch((e) => {
      ElMessage.error(String(e?.message || e))
      return [] as LookupTable[]
    })
    lookups.value = ls
  } finally { loading.value = false }
}
onMounted(loadAll)

async function onToggleEnabled(row: LookupTable) {
  const newVal = row.enabled
  try {
    await updateLookup(row.id, { name: row.name, enabled: newVal })
    ElMessage.success(`已${newVal ? '启用' : '停用'}`)
    const fresh = await getLookup(row.id).catch(() => null)
    if (fresh) {
      const idx = lookups.value.findIndex((l) => l.id === row.id)
      if (idx >= 0) lookups.value[idx] = fresh
    }
  } catch (e) {
    row.enabled = !newVal
    ElMessage.error(`更新失败：${String((e as Error)?.message || e)}`)
  }
}

interface ParseResult {
  ok: boolean
  headerCols: string[]
  dataRows: string[][]
  rows: Record<string, Record<string, string>>
  rowCount: number
  colCount: number
  duplicatedKeys: string[]
  error?: string
  keyLabel: string
}

function parseLookupText(text: string): ParseResult {
  const out: ParseResult = {
    ok: false, headerCols: [], dataRows: [],
    rows: {}, rowCount: 0, colCount: 0, duplicatedKeys: [], keyLabel: '',
  }
  if (!text || !text.trim()) { out.ok = true; return out }
  const lines = text.split(/\r?\n/).map((l) => l.replace(/\uFEFF/g, ''))
    .filter((l) => l.trim() !== '' && !l.trim().startsWith('//') && !l.trim().startsWith('#'))
  if (!lines.length) { out.ok = true; return out }
  const headerRaw = lines[0].split(/[\t\s]+/).map((s) => s.trim()).filter(Boolean)
  if (headerRaw.length < 1) {
    out.error = '表头为空'; return out
  }
  out.keyLabel = headerRaw[0]
  out.headerCols = headerRaw
  out.colCount = headerRaw.length - 1
  const rows: Record<string, Record<string, string>> = {}
  const dup: string[] = []
  let lineNo = 1
  for (let i = 1; i < lines.length; i++) {
    lineNo++
    const line = lines[i]
    const cells = line.split(/[\t\s]+/).map((s) => s.trim()).filter(Boolean)
    if (!cells.length) continue
    if (cells.length < 2) {
      out.error = `第 ${lineNo} 行：至少需要 key + 一个属性列`; return out
    }
    const key = cells[0]
    const attrs: Record<string, string> = {}
    for (let j = 1; j < headerRaw.length; j++) {
      if (j < cells.length) attrs[headerRaw[j]] = cells[j]
    }
    if (Object.prototype.hasOwnProperty.call(rows, key)) {
      if (!dup.includes(key)) dup.push(key)
      Object.assign(rows[key], attrs)
    } else {
      rows[key] = attrs
    }
  }
  out.rows = rows
  out.rowCount = Object.keys(rows).length
  out.colCount = Math.max(out.colCount, colsCount(rows))
  out.duplicatedKeys = dup
  out.ok = true
  return out
}

function rowsToLookupText(rows: Record<string, Record<string, string>> | undefined, keyLabel: string): string {
  if (!rows || Object.keys(rows).length === 0) return `${keyLabel}\n`
  const cols = new Set<string>()
  Object.values(rows).forEach((attrs) => Object.keys(attrs || {}).forEach((c) => cols.add(c)))
  const sortedCols = [...cols].sort()
  const header = [keyLabel, ...sortedCols].join('\t')
  const lines = [header]
  Object.keys(rows).sort().forEach((key) => {
    const attrs = rows[key] || {}
    const row = [key, ...sortedCols.map((c) => attrs[c] ?? '')]
    lines.push(row.join('\t'))
  })
  return lines.join('\n') + '\n'
}

interface DialogState {
  visible: boolean
  mode: 'create' | 'edit'
  id: string
  name: string
  description: string
  key_label: string
  enabled: boolean
  text: string
  viewTab: 'text' | 'json'
}
function emptyDialog(): DialogState {
  return {
    visible: false, mode: 'create', id: '',
    name: '', description: '', key_label: 'ip', enabled: true,
    text: '', viewTab: 'text',
  }
}
const dlg = reactive<DialogState>(emptyDialog())
const dlgFormRef = ref<FormInstance | null>(null)
const dlgFormRules: FormRules<DialogState> = {
  name: [{ required: true, message: '请输入外表名称', trigger: 'blur' }],
}

const parseResult = computed(() => parseLookupText(dlg.text))
const previewRowsJson = computed(() => {
  if (!parseResult.value.ok || !parseResult.value.rowCount) return '{}'
  try { return JSON.stringify(parseResult.value.rows, null, 2) }
  catch { return '{}' }
})

watch(() => dlg.key_label, (nv) => {
  // 当表头首列与 key_label 不一致时，询问同步（自动：只同步到未改动时）
  if (!nv) return
  if (parseResult.value.headerCols[0] && parseResult.value.headerCols[0] !== nv) {
    // 如果 user 改了 key_label，同步更新第一列表头
    const lines = (dlg.text || '').split(/\r?\n/)
    if (lines.length > 0) {
      const hdr = lines[0].split(/[\t\s]+/).map((s) => s.trim()).filter(Boolean)
      if (hdr.length > 0) {
        hdr[0] = nv
        lines[0] = hdr.join('\t')
        dlg.text = lines.join('\n')
      }
    }
  }
})
watch(parseResult, (r) => {
  if (r.ok && r.headerCols[0]) {
    dlg.key_label = r.headerCols[0]
  }
}, { deep: false })

const hiddenInputRef = ref<HTMLInputElement | null>(null)
function triggerFileImport() {
  hiddenInputRef.value?.click()
}
function onFileSelected(e: Event) {
  const t = e.target as HTMLInputElement
  const files = t.files
  if (!files || files.length === 0) return
  const f = files[0]
  const reader = new FileReader()
  reader.onload = () => {
    const txt = String(reader.result || '')
    dlg.text = (dlg.text ? dlg.text.trim() + '\n' : '') + txt
    ElMessage.success(`已导入文件：${f.name}`)
  }
  reader.onerror = () => ElMessage.error('文件读取失败')
  reader.readAsText(f)
  t.value = ''
}

function openCreate() {
  Object.assign(dlg, emptyDialog())
  dlg.text = ''
  dlg.visible = true
}

async function openEdit(id: string) {
  let l: LookupTable | null = null
  try { l = await getLookup(id) }
  catch (e) { ElMessage.error(`读取失败：${String((e as Error)?.message || e)}`); return }
  if (!l) return
  Object.assign(dlg, emptyDialog())
  dlg.visible = true; dlg.mode = 'edit'; dlg.id = l.id
  dlg.name = l.name; dlg.description = l.description || ''
  dlg.enabled = l.enabled; dlg.key_label = l.key_label || 'ip'
  const lText = (l as LookupTable & { text?: string }).text
  if (lText && lText.trim()) {
    dlg.text = lText
  } else {
    dlg.text = rowsToLookupText(l.rows || {}, l.key_label || 'ip')
  }
}

async function onDlgSave() {
  const valid = await dlgFormRef.value?.validate().catch(() => false)
  if (!valid) return
  const pr = parseResult.value
  if (!pr.ok) { ElMessage.error(`.lookup 解析失败：${pr.error || '未知错误'}`); return }
  if (!pr.rowCount && !(dlg.text && dlg.text.trim())) {
    ElMessage.error('rows 数据为空，请至少写一行'); return
  }
  const body: LookupInput = {
    name: dlg.name.trim(),
    description: dlg.description || undefined,
    key_label: dlg.key_label || 'ip',
    enabled: dlg.enabled,
    rows: pr.rowCount ? pr.rows : undefined,
    text: dlg.text || undefined,
  }
  try {
    if (dlg.mode === 'create') { await createLookup(body); ElMessage.success('已创建') }
    else { await updateLookup(dlg.id, body); ElMessage.success('已更新') }
    dlg.visible = false; await loadAll()
  } catch (e) {
    const msg = (e as Error)?.message || String(e)
    let detail: unknown = undefined
    if (e && typeof e === 'object' && 'detail' in e) detail = (e as Record<string, unknown>).detail
    if (detail !== undefined) ElMessage.error(`保存失败：${msg}；detail: ${JSON.stringify(detail, null, 2)}`)
    else ElMessage.error(`保存失败：${msg}`)
  }
}

async function onDelete(row: LookupTable) {
  try {
    await ElMessageBox.confirm(
      `确定删除外表「${row.name}」？关联的 Enrich 规则将无法命中。`,
      '删除', { type: 'warning' })
  } catch { return }
  try { await deleteLookup(row.id); ElMessage.success('已删除'); await loadAll() }
  catch (e) { ElMessage.error(`删除失败：${String((e as Error)?.message || e)}`) }
}
</script>
<template>
  <div class="lookups-view">
    <div class="page-header">
      <h2 class="page-title">外表管理</h2>
      <p class="page-sub">Lookup Table：key → attributes 映射供 Enrich Lookup 使用</p>
    </div>

    <div class="toolbar panel">
      <div class="tb-left">
        <ElButton type="primary" :icon="Plus" @click="openCreate">新建外表</ElButton>
        <ElButton :icon="Refresh" @click="loadAll" :disabled="loading">刷新</ElButton>
      </div>
      <div class="tb-right">
        <ElInput v-model="filterQ" placeholder="搜索 name 包含" clearable
          style="width:240px" :prefix-icon="Search" />
      </div>
    </div>

    <ElTable :data="pagedLookups" v-loading="loading" stripe class="lookups-table" style="width:100%;margin-top:12px"
      empty-text="暂无外表数据">
      <ElTableColumn label="名称" min-width="180">
        <template #default="{ row }">
          <ElTooltip :content="asLookup(row).name" placement="top">
            <a class="name-link" @click="openEdit(asLookup(row).id)">{{ asLookup(row).name }}</a>
          </ElTooltip>
        </template>
      </ElTableColumn>
      <ElTableColumn label="描述" min-width="240" show-overflow-tooltip>
        <template #default="{ row }">
          <span v-if="asLookup(row).description">{{ asLookup(row).description }}</span>
          <span class="empty-tip">（无描述）</span>
        </template>
      </ElTableColumn>
      <ElTableColumn label="KEY_LABEL" width="130">
        <template #default="{ row }">
          <code class="mono key-label">{{ asLookup(row).key_label }}</code>
        </template>
      </ElTableColumn>
      <ElTableColumn label="ROWS 行数" width="100" align="center">
        <template #default="{ row }">
          <code class="mono row-num">{{ rowsCount(asLookup(row).rows) }}</code>
        </template>
      </ElTableColumn>
      <ElTableColumn label="属性列" width="90" align="center">
        <template #default="{ row }">
          <code class="mono">{{ colsCount(asLookup(row).rows) }}</code>
        </template>
      </ElTableColumn>
      <ElTableColumn label="启用" width="72" align="center">
        <template #default="{ row }">
          <ElSwitch class="en-switch" :model-value="asLookup(row).enabled"
            @update:model-value="(v) => { asLookup(row).enabled = Boolean(v); onToggleEnabled(asLookup(row)) }" />
        </template>
      </ElTableColumn>
      <ElTableColumn label="最近更新" width="170">
        <template #default="{ row }">
          <span class="cell-updated" :title="asLookup(row).updated_at">{{ formatTime(asLookup(row).updated_at) }}</span>
        </template>
      </ElTableColumn>
      <ElTableColumn label="操作" width="170" fixed="right">
        <template #default="{ row }">
          <div class="row-actions">
            <ElButton size="small" class="act-btn act-edit" @click="openEdit(asLookup(row).id)">编辑</ElButton>
            <ElButton size="small" type="danger" plain class="act-btn" @click="onDelete(asLookup(row))">删除</ElButton>
          </div>
        </template>
      </ElTableColumn>
    </ElTable>

    <div class="panel lookups-pager">
      <div class="pager-tip">共 <span class="mono">{{ filteredLookups.length }}</span> 条</div>
      <ElPagination
        v-model:current-page="page"
        v-model:page-size="pageSize"
        :page-sizes="Array.from(PAGE_SIZES)"
        :total="filteredLookups.length"
        layout="sizes, prev, pager, next, jumper, ->, total"
        background
        @current-change="onPage"
        @size-change="onSize"
      />
    </div>

    <ElEmpty v-if="!loading && lookups.length === 0" description="暂无 Lookup 外表">
      <template #extra>
        <ElButton type="primary" :icon="Plus" @click="openCreate">新建外表</ElButton>
      </template>
    </ElEmpty>

    <ElDialog v-model="dlg.visible"
      :title="dlg.mode === 'create' ? '新建外表' : '编辑外表 · ' + dlg.name"
      width="980px" :close-on-click-modal="false" destroy-on-close top="5vh">
      <ElForm ref="dlgFormRef" :model="dlg" :rules="dlgFormRules" label-position="top">
        <ElRow :gutter="16">
          <ElCol :span="10">
            <ElFormItem label="名称" prop="name">
              <ElInput v-model="dlg.name" placeholder="如 cmdb_hosts / trap_ips" maxlength="128" show-word-limit />
            </ElFormItem>
          </ElCol>
          <ElCol :span="6">
            <ElFormItem label="KEY_LABEL（匹配用标签名）">
              <ElSelect v-model="dlg.key_label" allow-create filterable default-first-option style="width:100%">
                <ElOption v-for="k in KEY_LABEL_OPTIONS" :key="k" :label="k" :value="k" />
              </ElSelect>
            </ElFormItem>
          </ElCol>
          <ElCol :span="4">
            <ElFormItem label="启用">
              <ElSwitch v-model="dlg.enabled" />
            </ElFormItem>
          </ElCol>
          <ElCol :span="4" style="display:flex;align-items:flex-end;padding-bottom:20px">
            <ElButton :icon="UploadFilled" @click="triggerFileImport">导入文件</ElButton>
            <input ref="hiddenInputRef" type="file" accept=".lookup,.txt,.tsv,.csv" style="display:none" @change="onFileSelected" />
          </ElCol>
        </ElRow>
        <ElFormItem label="描述">
          <ElInput v-model="dlg.description" placeholder="外表用途（最多 1000 字）" maxlength="1000" show-word-limit />
        </ElFormItem>

        <!-- 解析结果 Alert -->
        <ElAlert v-if="parseResult.error" type="error" :closable="false" show-icon
          :title="'.lookup 解析失败：' + parseResult.error" />
        <ElAlert v-else-if="parseResult.ok && !parseResult.rowCount" type="info" :closable="false" show-icon
          title='格式说明：第一行为表头（首列即 KEY_LABEL），其余各行用空格或 Tab 分隔列。例如： ip hostname team env&#10;10.0.0.1 host-01 infra prod' />
        <ElAlert v-else-if="parseResult.duplicatedKeys.length" type="warning" :closable="false" show-icon
          :title="`已解析：${parseResult.rowCount} 行 / ${parseResult.colCount} 列；重复 key 已合并：${parseResult.duplicatedKeys.join(', ')}`" />
        <ElAlert v-else type="success" :closable="false" show-icon
          :title="`已解析：${parseResult.rowCount} 行 / ${parseResult.colCount} 列，KEY_LABEL = ${parseResult.keyLabel || dlg.key_label}`" />

        <ElTabs v-model="dlg.viewTab" style="margin-top:8px">
          <ElTabPane label=".lookup 原文" name="text">
            <ElInput v-model="dlg.text" type="textarea" :rows="14"
              placeholder="ip hostname team env&#10;10.0.0.1 host-01 infra prod&#10;10.0.0.2 host-02 data prod"
              class="mono-textarea" />
          </ElTabPane>
          <ElTabPane label="转为 JSON 预览" name="json">
            <pre class="json-pre">{{ previewRowsJson }}</pre>
          </ElTabPane>
        </ElTabs>
      </ElForm>

      <template #footer>
        <ElButton @click="dlg.visible = false">取消</ElButton>
        <ElButton type="primary" @click="onDlgSave">保存</ElButton>
      </template>
    </ElDialog>
  </div>
</template>

<style scoped>
.lookups-view { padding: 16px 20px 32px; }
.page-header { margin-bottom: 12px; }
.page-title { margin: 0 0 4px; font-size: 22px; }
.page-sub { margin: 0; color: var(--el-text-color-secondary, #909399); font-size: 13px; }
.panel {
  background: var(--el-bg-color, #fff);
  border: 1px solid var(--el-border-color-lighter, #ebeef5);
  border-radius: 4px;
}
.toolbar {
  display: flex; align-items: center; justify-content: space-between;
  padding: 10px 12px; gap: 8px; flex-wrap: wrap;
}
.tb-left, .tb-right { display: flex; gap: 8px; align-items: center; }
.name-link {
  color: var(--el-color-primary, #409eff); cursor: pointer; font-weight: 600;
}
.empty-tip { color: var(--el-text-color-secondary, #c0c4cc); }
.mono {
  font-family: 'SFMono-Regular', Consolas, Menlo, monospace;
  font-size: 12.5px;
}
.row-num { font-weight: 700; color: var(--el-color-primary, #409eff); }
.key-label {
  background: var(--el-fill-color-light, #f5f7fa);
  padding: 1px 5px; border-radius: 3px;
  color: var(--el-color-primary, #409eff);
}
.mono-textarea :deep(textarea) {
  font-family: 'SFMono-Regular', Consolas, Menlo, monospace;
  font-size: 12.5px; line-height: 1.6;
}
.json-pre {
  background: #1e1e1e; color: #d4d4d4;
  padding: 12px; border-radius: 4px;
  max-height: 440px; overflow: auto;
  font-size: 12px; line-height: 1.5;
  white-space: pre-wrap; word-break: break-all;
  margin: 0;
}
/* === 列表紧凑样式（与通知渠道一致） === */
.lookups-table { font-size: 13px; overflow: hidden; }
.lookups-table :deep(.el-table__inner-wrapper::before) { display: none; }
.lookups-table :deep(.el-table th.el-table__cell) { background: transparent; }
.lookups-table :deep(.el-table td.el-table__cell),
.lookups-table :deep(.el-table th.el-table__cell.is-leaf) {
  border-bottom: 1px solid var(--el-border-color-lighter, #f2f3f5);
}
.lookups-table :deep(.el-table th.el-table__cell .cell) { padding-top: 4px; padding-bottom: 4px; }
.lookups-table :deep(.el-table td.el-table__cell .cell) { padding-top: 8px; padding-bottom: 8px; }
.cell-updated { font-family: Consolas, Menlo, monospace; font-size: 12.5px; color: var(--el-text-color-secondary, #909399); }
/* 开关：紧凑 40x20 */
.en-switch {
  --el-switch-on-color: var(--el-color-primary, #409eff);
  --el-switch-off-color: var(--el-border-color-darker, #c0c4cc);
  --el-switch-height: 20px;
  --el-switch-width: 40px;
}
.en-switch :deep(.el-switch__action) { height: 18px; width: 18px; }
/* 操作按钮：小胶囊 4px 圆角 */
.row-actions { display: inline-flex; align-items: center; gap: 6px; }
.act-btn {
  height: 26px; padding: 0 12px !important;
  font-size: 12px !important; border-radius: 4px;
}
.act-edit {
  --el-button-border-color: #3f8bff;
  --el-button-text-color: #3f8bff;
  --el-button-hover-bg-color: rgba(63, 139, 255, 0.08);
  --el-button-hover-border-color: #3077ef;
  --el-button-hover-text-color: #3077ef;
}
.lookups-pager {
  display: flex; align-items: center; justify-content: space-between;
  flex-wrap: wrap; gap: 12px;
  padding: 12px 18px; margin: 14px 0;
  border: 1px solid var(--el-border-color-lighter, #ebeef5);
  background: var(--el-bg-color, #fff);
  border-radius: 8px;
  box-shadow: 0 1px 2px rgba(0,0,0,0.03);
}
.lookups-pager .pager-tip { font-size: 13px; color: var(--el-text-color-secondary, #909399); }
.lookups-pager .mono {
  font-family: Consolas, Menlo, monospace;
  font-weight: 600; color: var(--el-text-color-primary, #303133); padding: 0 2px;
}
</style>
