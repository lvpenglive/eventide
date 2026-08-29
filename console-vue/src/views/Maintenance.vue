<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, reactive, ref, watch } from 'vue'
import {
  ElAffix, ElButton, ElDatePicker, ElDialog, ElEmpty, ElForm, ElFormItem,
  ElInput, ElMessage, ElMessageBox, ElOption, ElPagination, ElSelect,
  ElSwitch, ElTable, ElTableColumn, ElTag, ElTooltip,
} from 'element-plus'
import type { FormInstance, FormRules } from 'element-plus'
import { Plus, Refresh, Search, Setting } from '@element-plus/icons-vue'
import type { MaintenanceWindow, MaintenanceInput } from '@/api/types'
import { listMaintenance, createMaintenance, updateMaintenance, deleteMaintenance } from '@/api/maintenance'
import { listRules } from '@/api/rules'

interface RuleBrief { id: string; name?: string }
type MWRow = MaintenanceWindow

const PAGE_SIZES: number[] = [10, 20, 50, 100]
const page = ref(1)
const pageSize = ref<number>(PAGE_SIZES[1])
const loading = ref(false)
const list = ref<MWRow[]>([])
const filterEnabled = ref<'all' | 'on' | 'off'>('all')
const filterQ = ref('')

const ruleOptions = ref<RuleBrief[]>([])
const loadingRules = ref(false)

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
  name: '', enabled: true, rule_id: null, range: null,
  matchers: [{ key: '', value: '' }], comment: '',
})

const tick = ref(Date.now())
let tickTimer: number | null = null

// --- 过滤器 ---
const filteredList = computed(() => {
  const q = filterQ.value.trim().toLowerCase()
  return list.value.filter((row) => {
    if (filterEnabled.value === 'on' && !row.enabled) return false
    if (filterEnabled.value === 'off' && row.enabled) return false
    if (q) {
      const hay = (row.name || '') + ' ' + (row.comment || '')
      if (!hay.toLowerCase().includes(q)) return false
    }
    return true
  })
})
const pagedList = computed(() => {
  const src = filteredList.value
  if (src.length <= pageSize.value) return src
  const start = (page.value - 1) * pageSize.value
  return src.slice(start, start + pageSize.value)
})
function onPage(p: number) { page.value = Math.max(1, p) }
function onSize(s: number) { pageSize.value = s; page.value = 1 }
watch([filterEnabled, filterQ, list], () => { page.value = 1 })

// --- 工具：时间/状态 ---
function pad(n: number): string { return String(n).padStart(2, '0') }
function fmtMinute(ts: string | null | undefined): string {
  if (!ts) return '-'
  const d = new Date(ts)
  if (Number.isNaN(d.getTime())) return String(ts)
  return pad(d.getFullYear()) + '-' + pad(d.getMonth() + 1) + '-' + pad(d.getDate()) + ' ' + pad(d.getHours()) + ':' + pad(d.getMinutes())
}
type MWStatus = 'ongoing' | 'pending' | 'ended'
function mwStatusOf(row: MWRow, nowMs = Date.now()): MWStatus {
  const s = new Date(row.starts_at).getTime()
  const e = new Date(row.ends_at).getTime()
  if (nowMs < s) return 'pending'
  if (nowMs >= e) return 'ended'
  return 'ongoing'
}
const statusStyle: Record<MWStatus, { text: string; cls: string }> = {
  ongoing: { text: '进行中', cls: 'pill-sm pill-ok' },
  pending: { text: '未开始', cls: 'pill-sm pill-warn' },
  ended:   { text: '已结束', cls: 'pill-sm' },
}
function fmtRemaining(row: MWRow): string {
  const st = mwStatusOf(row, tick.value)
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
    if (diff < 60) return diff + 's 前结束'
    if (diff < 3600) return Math.floor(diff / 60) + 'm 前结束'
    if (diff < 86400) return Math.floor(diff / 3600) + 'h 前结束'
    return Math.floor(diff / 86400) + 'd 前结束'
  }
  if (diff < 60) return st === 'ongoing' ? '剩余 ' + diff + 's' : diff + 's 后开始'
  if (diff < 3600) {
    const m = Math.floor(diff / 60); const ss = diff % 60
    return st === 'ongoing' ? '剩余 ' + m + 'm ' + ss + 's' : m + 'm ' + ss + 's 后开始'
  }
  if (diff < 48 * 3600) {
    const h = Math.floor(diff / 3600); const m = Math.floor((diff % 3600) / 60)
    return st === 'ongoing' ? '剩余 ' + h + 'h ' + m + 'm' : h + 'h ' + m + 'm 后开始'
  }
  const d = Math.floor(diff / 86400); const h = Math.floor((diff % 86400) / 3600)
  return st === 'ongoing' ? '剩余 ' + d + 'd ' + h + 'h' : d + 'd ' + h + 'h 后开始'
}
function shortUuid(id: string | null | undefined): string {
  if (!id) return ''
  const s = String(id).replace(/[^A-Za-z0-9]/g, '')
  return s.slice(0, 8) || String(id).slice(0, 8)
}
function ruleName(row: MWRow): string {
  if (!row.rule_id) return '全部规则'
  const r = ruleOptions.value.find((x) => x.id === row.rule_id)
  if (r) return r.name || '#' + shortUuid(row.rule_id)
  return '#' + shortUuid(row.rule_id)
}
function matchersEntries(row: MWRow): Array<{ key: string; value: string }> {
  const m = row.matchers
  if (!m) return []
  if (Array.isArray(m)) {
    return (m as Array<{ key?: string; value?: string }>)
      .map((x) => ({ key: (x?.key || '') as string, value: (x?.value || '') as string }))
      .filter((e) => e.key || e.value)
  }
  if (typeof m === 'object') {
    return Object.entries(m as Record<string, string>)
      .map(([k, v]) => ({ key: k, value: String(v) }))
  }
  return []
}
function matchersText(row: MWRow): string {
  const es = matchersEntries(row)
  if (es.length === 0) return '(无匹配器，匹配全部)'
  return es.map((e) => e.key + '=' + e.value).join(', ')
}
function errMsgOf(e: unknown, fallback: string): string {
  if (e && typeof e === 'object') {
    const anyE = e as { message?: unknown }
    if (typeof anyE.message === 'string') return anyE.message
  }
  return fallback
}

// --- 列表加载 ---
async function loadList(): Promise<void> {
  loading.value = true
  try {
    const rows = await listMaintenance()
    list.value = Array.isArray(rows) ? rows : []
  } catch (e) {
    ElMessage.error(errMsgOf(e, '加载维护窗列表失败'))
    list.value = []
  } finally { loading.value = false }
}
async function loadRules(): Promise<void> {
  loadingRules.value = true
  try {
    const r = await listRules()
    ruleOptions.value = Array.isArray(r) ? r : []
  } catch { ruleOptions.value = [] }
  finally { loadingRules.value = false }
}

// --- 新建 / 编辑 / 删除 / toggle ---
function openCreate(): void {
  dialogMode.value = 'create'
  editingId.value = null
  const now = new Date()
  const end = new Date(now.getTime() + 4 * 3600 * 1000)
  const toIso = (d: Date) => {
    function p(n: number) { return String(n).padStart(2, '0') }
    return d.getFullYear() + '-' + p(d.getMonth() + 1) + '-' + p(d.getDate()) + 'T' + p(d.getHours()) + ':' + p(d.getMinutes()) + ':' + p(d.getSeconds())
  }
  formData.name = ''; formData.enabled = true; formData.rule_id = null
  formData.range = [toIso(now), toIso(end)]
  formData.matchers = [{ key: '', value: '' }]
  formData.comment = ''
  void loadRules()
  dialogVisible.value = true
  setTimeout(() => { try { formRef.value?.clearValidate?.() } catch {} }, 0)
}

async function openEdit(row: MWRow): Promise<void> {
  dialogMode.value = 'edit'
  editingId.value = row.id
  void loadRules()
  let detail: MWRow = row
  try {
    detail = await listMaintenance().then((all) => all.find((x) => x.id === row.id) || row).catch(() => row as MWRow)
  } catch { detail = row }
  formData.name = detail.name ?? ''
  formData.enabled = Boolean(detail.enabled)
  formData.rule_id = detail.rule_id ?? null
  formData.range = [
    detail.starts_at ? detail.starts_at.replace(/Z$/, '').slice(0, 19) : new Date().toISOString().slice(0, 19),
    detail.ends_at ? detail.ends_at.replace(/Z$/, '').slice(0, 19) : new Date(Date.now() + 4 * 3600 * 1000).toISOString().slice(0, 19),
  ]
  const entries = matchersEntries(detail)
  formData.matchers = entries.length > 0 ? entries.map((x) => ({ key: x.key, value: x.value })) : [{ key: '', value: '' }]
  formData.comment = detail.comment ?? ''
  dialogVisible.value = true
  setTimeout(() => { try { formRef.value?.clearValidate?.() } catch {} }, 0)
}

async function onDelete(row: MWRow): Promise<void> {
  try {
    await ElMessageBox.confirm(
      '确定删除维护窗「' + (row.name || row.id.slice(0, 8)) + '」？删除后该时间窗内的抑制会立即失效。',
      '删除维护窗',
      { type: 'warning', confirmButtonText: '删除', cancelButtonText: '取消' },
    )
  } catch { return }
  try {
    await deleteMaintenance(row.id)
    list.value = list.value.filter((x) => x.id !== row.id)
    ElMessage.success('已删除')
  } catch (e) {
    ElMessage.error(errMsgOf(e, '删除失败'))
  }
}

async function toggleEnabled(row: MWRow, enabled: boolean): Promise<void> {
  try {
    const body: MaintenanceInput = {
      name: row.name,
      enabled,
      rule_id: row.rule_id,
      starts_at: row.starts_at,
      ends_at: row.ends_at,
      matchers: (row.matchers || {}) as Record<string, string>,
      comment: row.comment || '',
    }
    await updateMaintenance(row.id, body)
    row.enabled = enabled
    ElMessage.success(enabled ? '已启用' : '已停用')
  } catch (e) {
    ElMessage.error(errMsgOf(e, '状态切换失败'))
    row.enabled = !enabled
  }
}

function addMatcherRow(): void { formData.matchers.push({ key: '', value: '' }) }
function removeMatcherRow(idx: number): void {
  if (formData.matchers.length <= 1) formData.matchers = [{ key: '', value: '' }]
  else formData.matchers.splice(idx, 1)
}

// --- Dialog 校验 & 提交 ---
const formRules: FormRules = {
  name: [
    { required: true, message: '请输入维护窗名称', trigger: 'blur' },
    { max: 80, message: '最多 80 字符', trigger: 'blur' },
  ],
  range: [
    {
      type: 'array',
      required: true,
      validator: (_rule, value: unknown, cb) => {
        const v = value as [string, string] | null | undefined
        if (!v || v.length !== 2 || !v[0] || !v[1]) { cb(new Error('请选择维护窗开始和结束时间')); return }
        const s = new Date(v[0]).getTime(); const e = new Date(v[1]).getTime()
        if (Number.isNaN(s) || Number.isNaN(e)) { cb(new Error('时间格式不正确')); return }
        if (e <= s) { cb(new Error('结束时间必须晚于开始时间')); return }
        cb()
      },
      trigger: 'change',
    },
  ],
}

async function onSubmit(): Promise<void> {
  if (!formRef.value) return
  try { await formRef.value.validate() } catch { return }
  const cleaned = formData.matchers
    .map((r) => ({ key: (r.key || '').trim(), value: (r.value || '').trim() }))
    .filter((r) => r.key !== '' || r.value !== '')
  if (cleaned.length === 0) { ElMessage.warning('请至少填写一个匹配器'); return }
  if (cleaned.some((r) => !r.key)) { ElMessage.warning('匹配器 key 不能为空'); return }
  if (!formData.range || formData.range.length !== 2) { ElMessage.warning('请选择维护窗时间窗'); return }

  submitting.value = true
  try {
    const body: MaintenanceInput = {
      name: formData.name.trim(),
      enabled: formData.enabled,
      rule_id: formData.rule_id || null,
      starts_at: formData.range[0],
      ends_at: formData.range[1],
      matchers: Object.fromEntries(cleaned.map((m) => [m.key, m.value])),
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
  } finally { submitting.value = false }
}

// --- 生命周期 ---
onMounted(() => {
  void loadList()
  tickTimer = window.setInterval(() => { tick.value = Date.now() }, 30_000)
})
onBeforeUnmount(() => {
  if (tickTimer != null) { window.clearInterval(tickTimer); tickTimer = null }
})
</script>

<template>
  <div class="maint-view">
    <div class="page-header">
      <h2 class="page-title">
        <el-icon :size="22" color="var(--ep-color-primary)"><Setting /></el-icon>
        维护窗
      </h2>
      <p class="page-sub">计划性维护抑制告警通知；与静默策略区别：维护窗针对已知计划停机，可长期启用并周期性复用</p>
    </div>

    <ElAffix :offset="0" class="affix-wrap">
      <div class="toolbar panel">
        <div class="tb-left">
          <ElButton type="primary" :icon="Plus" @click="openCreate">新建维护窗</ElButton>
          <ElButton :icon="Refresh" :loading="loading" @click="loadList">刷新</ElButton>
        </div>
        <div class="tb-right">
          <ElSelect v-model="filterEnabled" style="width:140px">
            <ElOption label="全部" value="all" />
            <ElOption label="已启用" value="on" />
            <ElOption label="已停用" value="off" />
          </ElSelect>
          <ElInput v-model="filterQ" clearable placeholder="关键词：名称 / 备注" style="width:260px" :prefix-icon="Search" />
        </div>
      </div>
    </ElAffix>

    <div v-if="!loading && list.length === 0" class="panel maint-empty">
      <ElEmpty description="暂无维护窗">
        <template #default>
          <ElButton type="primary" :icon="Plus" @click="openCreate">新建第一个维护窗</ElButton>
        </template>
      </ElEmpty>
    </div>

    <template v-else>
      <ElTable :data="pagedList" v-loading="loading" stripe class="maint-table" empty-text="无匹配结果">
        <ElTableColumn label="名称" min-width="200" show-overflow-tooltip>
          <template #default="{ row }">
            <div class="name-cell">
              <ElSwitch
                :model-value="(row as MWRow).enabled"
                inline-prompt size="small"
                style="margin-right:10px"
                @change="(v: unknown) => toggleEnabled(row as MWRow, Boolean(v))"
              />
              <span class="name-text" :class="{ muted: !(row as MWRow).enabled }">{{ (row as MWRow).name }}</span>
            </div>
          </template>
        </ElTableColumn>

        <ElTableColumn label="状态" width="180">
          <template #default="{ row }">
            <div class="status-cell">
              <span :class="statusStyle[mwStatusOf(row as MWRow)].cls">{{ statusStyle[mwStatusOf(row as MWRow)].text }}</span>
              <span class="remain-text-brief">{{ fmtRemaining(row as MWRow) }}</span>
            </div>
          </template>
        </ElTableColumn>

        <ElTableColumn label="时间段" width="290">
          <template #default="{ row }">
            <div class="range-cell">
              <span class="mono">{{ fmtMinute((row as MWRow).starts_at) }}</span>
              <span class="range-arrow">→</span>
              <span class="mono">{{ fmtMinute((row as MWRow).ends_at) }}</span>
            </div>
          </template>
        </ElTableColumn>

        <ElTableColumn label="匹配器" min-width="220" show-overflow-tooltip>
          <template #default="{ row }">
            <template v-if="matchersEntries(row as MWRow).length === 0">
              <ElTag type="info" effect="plain" size="small">全部标签</ElTag>
            </template>
            <template v-else-if="matchersEntries(row as MWRow).length <= 3">
              <ElTag
                v-for="(m, i) in matchersEntries(row as MWRow)" :key="i"
                size="small" effect="light" style="margin-right:4px;margin-bottom:4px"
              >{{ m.key }}={{ m.value }}</ElTag>
            </template>
            <template v-else>
              <ElTooltip placement="top" :content="matchersText(row as MWRow)">
                <span>
                  <ElTag
                    v-for="(m, i) in matchersEntries(row as MWRow).slice(0, 3)" :key="i"
                    size="small" effect="light" style="margin-right:4px;margin-bottom:4px"
                  >{{ m.key }}={{ m.value }}</ElTag>
                  <ElTag size="small" effect="plain" type="info">+{{ matchersEntries(row as MWRow).length - 3 }}</ElTag>
                </span>
              </ElTooltip>
            </template>
          </template>
        </ElTableColumn>

        <ElTableColumn label="规则绑定" width="140">
          <template #default="{ row }">
            <template v-if="!(row as MWRow).rule_id">
              <ElTag type="primary" effect="plain" size="small">全部规则</ElTag>
            </template>
            <template v-else>
              <ElTooltip :content="((row as MWRow).rule_id as string) || undefined" placement="top">
                <span class="rule-chip mono">{{ ruleName(row as MWRow) }}</span>
              </ElTooltip>
            </template>
          </template>
        </ElTableColumn>

        <ElTableColumn label="备注" min-width="180" show-overflow-tooltip>
          <template #default="{ row }">
            <span v-if="(row as MWRow).comment">{{ (row as MWRow).comment }}</span>
            <span v-else class="muted">-</span>
          </template>
        </ElTableColumn>

        <ElTableColumn label="更新时间" width="160">
          <template #default="{ row }">
            <span class="cell-updated">{{ fmtMinute((row as MWRow).updated_at) }}</span>
          </template>
        </ElTableColumn>

        <ElTableColumn label="操作" width="170" align="center">
          <template #default="{ row }">
            <div class="row-actions">
              <ElButton size="small" class="act-btn act-edit" @click="openEdit(row as MWRow)">编辑</ElButton>
              <ElButton size="small" type="danger" plain class="act-btn" @click="onDelete(row as MWRow)">删除</ElButton>
            </div>
          </template>
        </ElTableColumn>
      </ElTable>

      <div class="panel maint-pager">
        <div class="pager-tip">
          共 <span class="mono">{{ filteredList.length }}</span> 条
          <span v-if="filteredList.length !== list.length">/ 总 <span class="mono">{{ list.length }}</span> 条</span>
        </div>
        <ElPagination v-model:current-page="page" v-model:page-size="pageSize" :page-sizes="PAGE_SIZES"
          :total="filteredList.length" layout="sizes, prev, pager, next, jumper, ->, total"
          @current-change="onPage" @size-change="onSize" background />
      </div>
    </template>

    <ElDialog
      v-model="dialogVisible"
      :title="dialogMode === 'create' ? '新建维护窗' : '编辑维护窗'"
      width="680px" :close-on-click-modal="false" destroy-on-close top="6vh"
    >
      <ElForm ref="formRef" :model="formData" :rules="formRules" label-width="92px" label-position="right">
        <ElFormItem label="名称" prop="name">
          <ElInput v-model="formData.name" placeholder="例如：数据库每周日凌晨升级" maxlength="80" show-word-limit clearable />
        </ElFormItem>
        <ElFormItem label="状态">
          <ElSwitch v-model="formData.enabled" inline-prompt active-text="启用" inactive-text="停用" />
        </ElFormItem>
        <ElFormItem label="规则">
          <ElSelect
            v-model="formData.rule_id" placeholder="留空=全部规则"
            style="width:100%" clearable filterable :loading="loadingRules"
          >
            <ElOption label="全部规则（不绑定）" value="" />
            <ElOption
              v-for="r in ruleOptions" :key="r.id"
              :label="r.name ? r.name + ' (#' + shortUuid(r.id) + ')' : '#' + shortUuid(r.id)"
              :value="r.id"
            />
          </ElSelect>
        </ElFormItem>
        <ElFormItem label="时间窗" prop="range">
          <ElDatePicker
            v-model="formData.range" type="datetimerange"
            start-placeholder="开始时间" end-placeholder="结束时间"
            value-format="YYYY-MM-DDTHH:mm:ss" format="YYYY-MM-DD HH:mm:ss"
            style="width:100%" range-separator="→"
          />
        </ElFormItem>
        <ElFormItem label="匹配器">
          <div class="matchers-form">
            <div v-for="(_row, idx) in formData.matchers" :key="idx" class="matcher-row">
              <ElInput v-model="formData.matchers[idx].key" placeholder="key，例如：alertname" style="flex:1 1 0" clearable />
              <span class="matcher-eq">=</span>
              <ElInput v-model="formData.matchers[idx].value" placeholder="value，例如：HighLoad" style="flex:1.2 1 0" clearable />
              <ElButton link type="danger" style="margin-left:8px" @click="removeMatcherRow(idx)">移除</ElButton>
            </div>
            <ElButton plain :icon="Plus" size="small" @click="addMatcherRow">添加匹配器</ElButton>
          </div>
        </ElFormItem>
        <ElFormItem label="备注">
          <ElInput
            v-model="formData.comment" type="textarea" :rows="3"
            maxlength="500" show-word-limit
            placeholder="维护窗说明 / 变更单号 / 负责人等"
          />
        </ElFormItem>
      </ElForm>
      <template #footer>
        <div class="dlg-footer">
          <ElButton @click="dialogVisible = false">取消</ElButton>
          <ElButton type="primary" :loading="submitting" @click="onSubmit">保存</ElButton>
        </div>
      </template>
    </ElDialog>
  </div>
</template>

<style scoped>
.maint-view { padding: 16px 20px 32px; }
.page-header { margin-bottom: 10px; }
.page-title {
  display: flex; align-items: center; gap: 8px;
  margin: 0 0 4px; font-size: 22px;
}
.page-sub { margin: 0; color: var(--el-text-color-secondary, #909399); font-size: 13px; }

.panel {
  background: var(--el-bg-color, #fff);
  border: 1px solid var(--el-border-color-lighter, #ebeef5);
  border-radius: 8px;
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.03);
}
.toolbar {
  display: flex; align-items: center; justify-content: space-between;
  padding: 10px 14px; gap: 8px; flex-wrap: wrap; margin-bottom: 12px;
}
.tb-left, .tb-right { display: flex; gap: 8px; align-items: center; }
.affix-wrap { z-index: 10; margin-bottom: 0; }

.maint-table { font-size: 13px; overflow: hidden; margin-top: 0; }
.maint-table :deep(.el-table__inner-wrapper::before) { display: none; }
.maint-table :deep(.el-table th.el-table__cell) { background: transparent; }
.maint-table :deep(.el-table td.el-table__cell),
.maint-table :deep(.el-table th.el-table__cell.is-leaf) {
  border-bottom: 1px solid var(--el-border-color-lighter, #f2f3f5);
}
.maint-table :deep(.el-table th.el-table__cell .cell) { padding-top: 4px; padding-bottom: 4px; }
.maint-table :deep(.el-table td.el-table__cell .cell) { padding-top: 8px; padding-bottom: 8px; }

.name-cell { display: flex; align-items: center; }
.name-text { font-weight: 500; word-break: break-all; }
.name-text.muted { color: var(--el-text-color-placeholder, #a8abb2); }
.muted { color: var(--el-text-color-placeholder, #a8abb2); }
.mono { font-family: Consolas, Menlo, monospace; font-size: 12.5px; }
.cell-updated { font-family: Consolas, Menlo, monospace; font-size: 12.5px; color: var(--el-text-color-secondary, #909399); }

.pill-sm {
  display: inline-flex; align-items: center; justify-content: center;
  padding: 0 10px; height: 22px; line-height: 20px;
  font-size: 11.5px; font-weight: 500; border-radius: 4px;
  margin-right: 6px;
}
.pill-sm.pill-ok { background: var(--el-color-success-light-9, #e1f3d8); color: var(--el-color-success, #67c23a); }
.pill-sm.pill-warn { background: var(--el-color-warning-light-9, #faecd8); color: var(--el-color-warning, #e6a23c); }
.remain-text-brief { font-size: 12px; color: var(--el-text-color-secondary, #909399); }
.status-cell { display: flex; align-items: center; }

.range-cell {
  display: flex; align-items: center; gap: 6px;
  font-variant-numeric: tabular-nums; font-size: 13px;
}
.range-arrow { color: var(--el-border-color, #dcdfe6); }

.rule-chip {
  display: inline-block; padding: 2px 8px;
  background: var(--el-fill-color-light, #f5f7fa);
  color: var(--el-color-primary, #409eff);
  border-radius: 10px; font-size: 12px;
}

.row-actions { display: inline-flex; align-items: center; gap: 6px; }
.act-btn { height: 26px; padding: 0 12px !important; font-size: 12px !important; border-radius: 4px; }
.act-edit {
  --el-button-border-color: #3f8bff; --el-button-text-color: #3f8bff;
  --el-button-hover-bg-color: rgba(63, 139, 255, 0.08); --el-button-hover-border-color: #3077ef;
  --el-button-hover-text-color: #3077ef;
}

.maint-pager {
  display: flex; align-items: center; justify-content: space-between;
  flex-wrap: wrap; gap: 12px; padding: 12px 18px; margin-top: 12px;
}
.maint-pager .pager-tip { font-size: 13px; color: var(--el-text-color-secondary, #909399); }
.maint-pager .mono { font-family: Consolas, Menlo, monospace; font-weight: 600; color: var(--el-text-color-primary, #303133); }

.maint-empty { padding: 48px 0; }

.matchers-form { width: 100%; }
.matcher-row {
  display: flex; align-items: center; margin-bottom: 8px; gap: 8px;
}
.matcher-eq { color: var(--el-border-color, #dcdfe6); }

.dlg-footer { display: flex; justify-content: flex-end; gap: 8px; width: 100%; }
</style>