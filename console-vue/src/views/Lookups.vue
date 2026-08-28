<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue'
import {
  ElAffix,
  ElButton,
  ElDialog,
  ElEmpty,
  ElForm,
  ElFormItem,
  ElInput,
  ElMessage,
  ElMessageBox,
  ElOption,
  ElRow,
  ElCol,
  ElSelect,
  ElSwitch,
  ElTable,
  ElTableColumn,
  ElTabs,
  ElTabPane,
  ElTooltip,
} from 'element-plus'
import type { FormInstance, FormRules } from 'element-plus'
import {
  Delete,
  Edit,
  Plus,
  Refresh,
} from '@element-plus/icons-vue'
import type { LookupTable, LookupInput } from '@/api/types'

// ============================================================
// API 绑定：lookup CRUD 函数定义在 enrichments.ts
// ============================================================
import {
  listLookups,
  getLookup,
  createLookup,
  updateLookup,
  deleteLookup,
} from '@/api/enrichments'

// ============================================================
// 常量
// ============================================================
const KEY_LABEL_OPTIONS: string[] = [
  'instance', 'ip', 'hostname', 'alertname', 'severity', 'device_id',
]

// ============================================================
// 状态：列表、筛选
// ============================================================
const loading = ref(false)
const lookups = ref<LookupTable[]>([])
const filterQ = ref('')

const filteredLookups = computed(() => {
  if (!filterQ.value.trim()) return lookups.value
  const q = filterQ.value.trim().toLowerCase()
  return lookups.value.filter((l) => l.name.toLowerCase().includes(q))
})

function asLookup(r: unknown): LookupTable { return r as LookupTable }
function formatTime(s: string | null | undefined): string {
  if (!s) return '—'
  return s.replace('T', ' ').slice(0, 19)
}
function rowsCount(r: Record<string, Record<string, string>> | undefined): number {
  if (!r) return 0
  return Object.keys(r).filter((k) => k && k.trim() !== '').length
}

async function loadAll() {
  loading.value = true
  try {
    const ls = await listLookups().catch((e) => {
      ElMessage.error(String(e?.message || e))
      return [] as LookupTable[]
    })
    lookups.value = ls
  } finally {
    loading.value = false
  }
}

onMounted(loadAll)

// ============================================================
// enabled 开关：乐观更新 + 失败回滚
// ============================================================
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

// ============================================================
// 新建 / 编辑 Dialog
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

/** Tab1 行：key + 子 attributes Map */
interface RowsMapRow {
  key: string
  attributes: KvRow[]
}
function emptyRowsMapRow(): RowsMapRow {
  return { key: '', attributes: emptyKv() }
}
function rowsRecordToRowsMap(rows: Record<string, Record<string, string>> | undefined): RowsMapRow[] {
  if (!rows || Object.keys(rows).length === 0) {
    // 至少 1 行初始
    return [emptyRowsMapRow()]
  }
  const result: RowsMapRow[] = []
  for (const [k, attrs] of Object.entries(rows)) {
    result.push({ key: k, attributes: mapToRows(attrs) })
  }
  // 保证至少一行
  if (result.length === 0) return [emptyRowsMapRow()]
  return result
}
function rowsMapToRecord(rowsMap: RowsMapRow[]): Record<string, Record<string, string>> {
  const out: Record<string, Record<string, string>> = {}
  for (const r of rowsMap) {
    if (!r.key.trim()) continue // 空 key 过滤掉
    const attrs = rowsToMap(r.attributes)
    out[r.key.trim()] = attrs
  }
  return out
}

function kvRowAdd(arr: KvRow[]) { arr.push({ key: '', value: '' }) }
function kvRowRemove(arr: KvRow[], idx: number) {
  if (arr.length <= 1) {
    arr[0] = { key: '', value: '' }
  } else {
    arr.splice(idx, 1)
  }
}

interface DialogState {
  visible: boolean
  mode: 'create' | 'edit'
  id: string
  name: string
  description: string
  key_label: string
  enabled: boolean
  // rows 编辑模式 Tab
  editMode: 'table' | 'text'
  rows: RowsMapRow[]      // Tab1 键值表
  text: string             // Tab2 lookup 原文
}
function emptyDialog(): DialogState {
  return {
    visible: false,
    mode: 'create',
    id: '',
    name: '',
    description: '',
    key_label: 'instance',
    enabled: true,
    editMode: 'table',
    rows: [emptyRowsMapRow()],
    text: '',
  }
}

const dlg = reactive<DialogState>(emptyDialog())
const dlgFormRef = ref<FormInstance | null>(null)
const dlgFormRules: FormRules<DialogState> = {
  name: [{ required: true, message: '请输入字典名称', trigger: 'blur' }],
}

function openCreate() {
  Object.assign(dlg, emptyDialog())
  dlg.visible = true
}

async function openEdit(id: string) {
  let l: LookupTable | null = null
  try {
    l = await getLookup(id)
  } catch (e) {
    ElMessage.error(`读取失败：${String((e as Error)?.message || e)}`)
    return
  }
  if (!l) return
  Object.assign(dlg, emptyDialog())
  dlg.visible = true
  dlg.mode = 'edit'
  dlg.id = l.id
  dlg.name = l.name
  dlg.description = l.description || ''
  dlg.key_label = l.key_label || 'instance'
  dlg.enabled = l.enabled
  // 当 rows 非空，把数据填到 Tab1；text 同时保留（非空 rows 切换时不丢失）
  dlg.rows = rowsRecordToRowsMap(l.rows || {})
  // text 字段可能在后端响应中存在也可能不存在
  const lText = (l as LookupTable & { text?: string }).text
  dlg.text = lText || ''
  // 若 rows 非空则默认 Tab1；否则根据 text 决定
  const hasRows = Object.keys(l.rows || {}).length > 0
  dlg.editMode = hasRows ? 'table' : (lText && lText.trim() ? 'text' : 'table')
}

function rowAdd() { dlg.rows.push(emptyRowsMapRow()) }
function rowRemove(idx: number) {
  if (dlg.rows.length <= 1) {
    dlg.rows[0] = emptyRowsMapRow()
  } else {
    dlg.rows.splice(idx, 1)
  }
}

async function onDlgSave() {
  const valid = await dlgFormRef.value?.validate().catch(() => false)
  if (!valid) return

  if (dlg.description && dlg.description.length > 1000) {
    ElMessage.error(`description 最多 1000 字，当前 ${dlg.description.length} 字`)
    return
  }

  // 模式1（键值表）→ rows；模式2 → text；最终优先 rows，若 rows 空则后端尝试解析 text
  const rowsRec = dlg.editMode === 'table' ? rowsMapToRecord(dlg.rows) : rowsMapToRecord(dlg.rows)
  const textContent = dlg.editMode === 'text' ? (dlg.text || '') : (dlg.text || '')

  const body: LookupInput = {
    name: dlg.name.trim(),
    description: dlg.description,
    key_label: dlg.key_label || 'instance',
    rows: rowsRec,
    text: textContent,
    enabled: dlg.enabled,
  }

  try {
    if (dlg.mode === 'create') {
      await createLookup(body)
      ElMessage.success('查询字典已创建')
    } else {
      await updateLookup(dlg.id, body)
      ElMessage.success('查询字典已更新')
    }
    dlg.visible = false
    await loadAll()
  } catch (e) {
    const msg = (e as Error)?.message || String(e)
    let detail: unknown = undefined
    if (e && typeof e === 'object' && 'detail' in e) detail = (e as Record<string, unknown>).detail
    if (detail !== undefined) {
      ElMessage.error(`保存失败：${msg}；detail: ${JSON.stringify(detail, null, 2)}`)
    } else {
      ElMessage.error(`保存失败：${msg}`)
    }
  }
}

// ============================================================
// 删除（二次确认）
// ============================================================
async function onDelete(row: LookupTable) {
  try {
    await ElMessageBox.confirm(
      `确定要删除查询字典「${row.name}」？该操作不可恢复，且关联的 Enrich 规则将无法命中此字典。`,
      '删除查询字典',
      { type: 'warning', confirmButtonText: '删除', cancelButtonText: '取消' },
    )
  } catch {
    return
  }
  try {
    await deleteLookup(row.id)
    ElMessage.success('已删除')
    await loadAll()
  } catch (e) {
    ElMessage.error(`删除失败：${String((e as Error)?.message || e)}`)
  }
}
</script>

<template>
  <div class="lookups-view">
    <!-- 标题 -->
    <div class="page-header">
      <h2 class="page-title">查询字典</h2>
      <p class="page-sub">Lookup Table：key → attributes 映射供 Enrich Lookup 使用</p>
    </div>

    <!-- 吸附工具栏 -->
    <ElAffix :offset="0" class="affix-wrap">
      <div class="toolbar">
        <ElButton type="primary" :icon="Plus" @click="openCreate">新建字典</ElButton>
        <ElButton :icon="Refresh" @click="loadAll" :loading="loading">刷新</ElButton>

        <div class="toolbar-filters">
          <ElInput
            v-model="filterQ"
            placeholder="搜索 name 包含"
            clearable
            style="width: 240px"
          />
        </div>
      </div>
    </ElAffix>

    <!-- 表格 -->
    <ElTable
      :data="filteredLookups"
      v-loading="loading"
      stripe
      style="width: 100%; margin-top: 12px"
      empty-text="暂无查询字典，试试调整搜索或点击新建。"
    >
      <ElTableColumn label="名称" min-width="180">
        <template #default="{ row }">
          <ElTooltip :content="asLookup(row).name" placement="top">
            <a class="name-link" @click="openEdit(asLookup(row).id)">{{ asLookup(row).name }}</a>
          </ElTooltip>
        </template>
      </ElTableColumn>

      <ElTableColumn label="描述" min-width="220" show-overflow-tooltip>
        <template #default="{ row }">
          <span v-if="asLookup(row).description">{{ asLookup(row).description }}</span>
          <span v-else style="color: #c0c4cc">（无描述）</span>
        </template>
      </ElTableColumn>

      <ElTableColumn label="key_label" width="140">
        <template #default="{ row }">
          <code>{{ asLookup(row).key_label }}</code>
        </template>
      </ElTableColumn>

      <ElTableColumn label="rows 行数" width="110" align="center">
        <template #default="{ row }">
          <b>{{ rowsCount(asLookup(row).rows) }}</b>
        </template>
      </ElTableColumn>

      <ElTableColumn label="启用" width="80" align="center">
        <template #default="{ row }">
          <ElSwitch
            :model-value="asLookup(row).enabled"
            @update:model-value="(v) => { asLookup(row).enabled = Boolean(v); onToggleEnabled(asLookup(row)) }"
          />
        </template>
      </ElTableColumn>

      <ElTableColumn label="最近更新" width="170">
        <template #default="{ row }">
          {{ formatTime(asLookup(row).updated_at) }}
        </template>
      </ElTableColumn>

      <ElTableColumn label="操作" width="180" fixed="right">
        <template #default="{ row }">
          <ElButton link type="primary" :icon="Edit" @click="openEdit(asLookup(row).id)">
            编辑
          </ElButton>
          <ElButton link type="danger" :icon="Delete" @click="onDelete(asLookup(row))">
            删除
          </ElButton>
        </template>
      </ElTableColumn>
    </ElTable>

    <!-- 空态 -->
    <ElEmpty
      v-if="!loading && lookups.length === 0"
      description="尚无 Lookup 字典，可先创建后在 Enrich 规则中关联。"
    >
      <template #extra>
        <ElButton type="primary" :icon="Plus" @click="openCreate">新建字典</ElButton>
      </template>
    </ElEmpty>

    <!-- 新建 / 编辑 Dialog -->
    <ElDialog
      v-model="dlg.visible"
      :title="dlg.mode === 'create' ? '新建查询字典' : '编辑查询字典'"
      width="820px"
      :close-on-click-modal="false"
      destroy-on-close
    >
      <ElForm
        ref="dlgFormRef"
        :model="dlg"
        :rules="dlgFormRules"
        label-width="100px"
        label-position="top"
      >
        <ElRow :gutter="16">
          <ElCol :span="16">
            <ElFormItem label="名称" prop="name">
              <ElInput v-model="dlg.name" placeholder="例如：cmdb_host_lookup" maxlength="128" show-word-limit />
            </ElFormItem>
          </ElCol>
          <ElCol :span="8">
            <ElFormItem label="key_label">
              <ElSelect
                v-model="dlg.key_label"
                placeholder="选择或输入 key_label"
                allow-create
                filterable
                default-first-option
                style="width: 100%"
              >
                <ElOption
                  v-for="k in KEY_LABEL_OPTIONS"
                  :key="k"
                  :label="k"
                  :value="k"
                />
              </ElSelect>
            </ElFormItem>
          </ElCol>
        </ElRow>

        <ElFormItem label="描述">
          <ElInput
            v-model="dlg.description"
            type="textarea"
            :rows="3"
            maxlength="1000"
            show-word-limit
            placeholder="字典用途说明（最多 1000 字）"
          />
        </ElFormItem>

        <ElFormItem label="启用">
          <ElSwitch v-model="dlg.enabled" />
        </ElFormItem>

        <!-- rows 两种编辑模式 Tab 切换 -->
        <ElFormItem label="rows 数据">
          <ElTabs v-model="dlg.editMode" class="rows-tabs" type="card">
            <!-- Tab1 键值表模式 -->
            <ElTabPane label="键值表模式" name="table">
              <div class="panel-tip">
                每行一个 <b>key</b> + 任意数量的 attributes（<code>attr_name → attr_value</code>）。<b>空 key 会在保存时被自动过滤掉。</b>
              </div>

              <div
                v-for="(row, idx) in dlg.rows"
                :key="'r-'+idx"
                class="rows-block"
              >
                <div class="rows-head">
                  <span class="rows-head-label">#{{ idx + 1 }} key：</span>
                  <ElInput
                    v-model="row.key"
                    placeholder="如 host-01 / 10.0.0.8 等"
                    style="width: 300px; margin-right: 12px"
                  />
                  <ElButton link type="danger" :icon="Delete" @click="rowRemove(idx)">删除此行</ElButton>
                </div>
                <div class="rows-attrs">
                  <div class="kv-title">
                    <span>attributes (attr_name → attr_value)</span>
                    <ElButton size="small" :icon="Plus" link type="primary" @click="kvRowAdd(row.attributes)">
                      新增 attribute
                    </ElButton>
                  </div>
                  <div v-for="(kv, i2) in row.attributes" :key="'ra-'+idx+'-'+i2" class="kv-row">
                    <ElInput v-model="kv.key" placeholder="attr 名，如 team" style="width: 35%; margin-right: 8px" />
                    <ElInput v-model="kv.value" placeholder="attr 值，如 infra" style="width: 45%; margin-right: 8px" />
                    <ElButton link type="danger" :icon="Delete" @click="kvRowRemove(row.attributes, i2)">删除</ElButton>
                  </div>
                </div>
              </div>

              <div style="margin-top: 8px">
                <ElButton size="small" :icon="Plus" @click="rowAdd">新增 key 行</ElButton>
              </div>
            </ElTabPane>

            <!-- Tab2 lookup 原文（text） -->
            <ElTabPane label="lookup 原文 (text)" name="text">
              <div class="panel-tip">
                粘贴 <code>.lookup</code> 格式文本（第一行为表头：<code>key_label col1 col2 ...</code>，后续每行用<b>空格 / tab</b>分隔）。<br />
                <b>说明：</b>当 rows 为空时后端会自动解析 text；若 rows 非空则保存时后端优先 rows。切换 Tab 时不会清空另一个 Tab 的内容。
              </div>
              <ElInput
                v-model="dlg.text"
                type="textarea"
                :rows="12"
                placeholder="示例：&#10;instance team owner env&#10;host-01 infra zhangsan prod&#10;host-02 data lisi prod"
                style="font-family: Consolas, Monaco, monospace; font-size: 13px;"
              />
            </ElTabPane>
          </ElTabs>
        </ElFormItem>
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
.rows-tabs { width: 100%; }
.panel-tip {
  background: #f4f8fc;
  padding: 8px 10px;
  border-radius: 4px;
  margin-bottom: 8px;
  font-size: 12px;
  color: #606266;
}
.panel-tip code { background: #eef; padding: 1px 4px; border-radius: 3px; }
.rows-block {
  border: 1px dashed #dcdfe6;
  border-radius: 4px;
  padding: 10px;
  margin-bottom: 8px;
  background: #fafafa;
}
.rows-head {
  display: flex;
  align-items: center;
  margin-bottom: 8px;
  flex-wrap: wrap;
  gap: 6px;
}
.rows-head-label { font-size: 13px; color: #303133; }
.rows-attrs { padding-left: 4px; }
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
</style>
