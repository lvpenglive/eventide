<script setup lang="ts">
import { computed, onMounted, reactive, ref, watch } from 'vue'
import {
  ElAffix, ElButton, ElDatePicker, ElDialog, ElEmpty, ElForm, ElFormItem,
  ElInput, ElMessage, ElMessageBox, ElOption, ElPagination, ElSelect,
  ElTable, ElTableColumn, ElTooltip, ElAlert,
} from 'element-plus'
import type { FormInstance } from 'element-plus'
import { Plus, Refresh, Search } from '@element-plus/icons-vue'
import type { Silence, SilenceInput } from '@/api/types'
import { listSilences, createSilence, deleteSilence } from '@/api/silences'
import { listRules } from '@/api/rules'

interface RuleBrief { id: string; name?: string }
const loading = ref(false)
const silences = ref<Silence[]>([])
const rules = ref<RuleBrief[]>([])
const filterQ = ref('')
const filterStatus = ref<'' | 'active' | 'pending' | 'ended'>('')
const PAGE_SIZES: number[] = [10, 20, 50, 100]
const page = ref(1)
const pageSize = ref<number>(PAGE_SIZES[1])

function asSilence(r: unknown): Silence { return r as Silence }

function mToObj(m: unknown): Record<string, string> {
  if (!m) return {}
  if (Array.isArray(m)) {
    const o: Record<string, string> = {}
    for (const x of m as Array<{ key?: string; value?: string }>) {
      if (x?.key) o[x.key] = String(x.value ?? '')
    }
    return o
  }
  if (typeof m === 'object') return m as Record<string, string>
  return {}
}

const filtered = computed(() => {
  const now = Date.now()
  return silences.value.filter((s) => {
    if (filterStatus.value) {
      const st = silenceStatus(s, now)
      if (st !== filterStatus.value) return false
    }
    if (filterQ.value.trim()) {
      const q = filterQ.value.trim().toLowerCase()
      const c = (s.comment || '').toLowerCase()
      const ms = Object.entries(mToObj(s.matchers)).map(
        ([k, v]) => k + '=' + v
      ).join(' ')
      const r = rules.value.find((x) => x.id === s.rule_id)?.name || ''
      if (c.indexOf(q) < 0 && ms.indexOf(q) < 0 && r.toLowerCase().indexOf(q) < 0) return false
    }
    return true
  })
})

const pagedSilences = computed(() => {
  const src = filtered.value
  if (src.length <= pageSize.value) return src
  const start = (page.value - 1) * pageSize.value
  return src.slice(start, start + pageSize.value)
})
function onPage(p: number) { page.value = Math.max(1, p) }
function onSize(s: number) { pageSize.value = s; page.value = 1 }
watch([filterQ, filterStatus, silences], () => { page.value = 1 })

function silenceStatus(s: Silence, nowMs = Date.now()): 'active' | 'pending' | 'ended' {
  const st = new Date(s.starts_at).getTime()
  const et = new Date(s.ends_at).getTime()
  if (nowMs < st) return 'pending'
  if (nowMs <= et) return 'active'
  return 'ended'
}
const statusStyle: Record<string, { text: string; cls: string }> = {
  active: { text: '生效中', cls: 'pill-sm pill-ok' },
  pending: { text: '未开始', cls: 'pill-sm pill-warn' },
  ended: { text: '已结束', cls: 'pill-sm' },
}
function pad(n: number) { return String(n).padStart(2, '0') }
function fmtTs(ts: string | null | undefined): string {
  if (!ts) return '-'
  const d = new Date(ts)
  if (Number.isNaN(d.getTime())) return ts
  return pad(d.getFullYear()) + '-' + pad(d.getMonth() + 1) + '-' + pad(d.getDate()) + ' ' + pad(d.getHours()) + ':' + pad(d.getMinutes()) + ':' + pad(d.getSeconds())
}
function fmtRemain(s: Silence): string {
  const now = Date.now()
  const st = new Date(s.starts_at).getTime()
  const et = new Date(s.ends_at).getTime()
  if (now < st) return fmtDur(st - now) + ' 后开始'
  if (now <= et) return '剩余 ' + fmtDur(et - now)
  return '已结束 ' + fmtDur(now - et)
}
function fmtDur(ms: number): string {
  const total = Math.max(0, Math.floor(ms / 1000))
  const d = Math.floor(total / 86400)
  const h = Math.floor((total % 86400) / 3600)
  const m = Math.floor((total % 3600) / 60)
  if (d > 0) return d + '天' + h + '小时'
  if (h > 0) return h + '小时' + m + '分'
  if (m > 0) return m + '分'
  return total + '秒'
}
function ruleName(s: Silence): string {
  if (!s.rule_id) return '全部告警规则'
  const r = rules.value.find((x) => x.id === s.rule_id)
  if (r) return r.name || s.rule_id.slice(0, 8) + '...'
  return s.rule_id.slice(0, 8) + '...'
}
function matchersText(s: Silence): string {
  const o = mToObj(s.matchers)
  const keys = Object.keys(o)
  if (keys.length === 0) return '(无匹配标签)'
  return keys.map((k) => k + '=' + o[k]).join(', ')
}

// Dialog
const dlg = reactive({
  visible: false,
  mode: 'create' as 'create' | 'edit',
  editing: null as Silence | null,
  form: {
    comment: '', rule_id: '', starts_at: '', ends_at: '', matchersJson: '{}',
  },
  formRef: null as FormInstance | null,
})
function dlgReset() {
  const now = new Date()
  const plus2 = new Date(now.getTime() + 2 * 60 * 60 * 1000)
  const fmt = (d: Date) => d.getFullYear() + '-' + pad(d.getMonth() + 1) + '-' + pad(d.getDate()) + 'T' + pad(d.getHours()) + ':' + pad(d.getMinutes()) + ':' + pad(d.getSeconds())
  dlg.form.comment = ''; dlg.form.rule_id = ''
  dlg.form.starts_at = fmt(now); dlg.form.ends_at = fmt(plus2)
  dlg.form.matchersJson = '{}'
  dlg.editing = null
  try { dlg.formRef?.clearValidate() } catch {}
}
function openCreate() { dlg.mode = 'create'; dlgReset(); dlg.visible = true }
function openEdit(row: Silence) {
  dlg.mode = 'edit'; dlg.editing = row
  dlg.form.comment = row.comment || ''
  dlg.form.rule_id = row.rule_id || ''
  dlg.form.starts_at = row.starts_at; dlg.form.ends_at = row.ends_at
  dlg.form.matchersJson = JSON.stringify(mToObj(row.matchers), null, 2)
  dlg.visible = true
}
async function dlgSave() {
  if (!dlg.formRef) return
  await dlg.formRef.validate(async (valid) => {
    if (!valid) return
    let matchersObj: Record<string, string>
    try {
      matchersObj = JSON.parse(dlg.form.matchersJson || '{}')
      if (typeof matchersObj !== 'object' || matchersObj === null || Array.isArray(matchersObj)) {
        ElMessage.error('匹配标签 JSON 必须是对象'); return
      }
      for (const k of Object.keys(matchersObj)) matchersObj[k] = String(matchersObj[k])
    } catch { ElMessage.error('匹配标签 JSON 格式错误'); return }
    const body: SilenceInput = {
      comment: dlg.form.comment, rule_id: dlg.form.rule_id || null,
      starts_at: dlg.form.starts_at, ends_at: dlg.form.ends_at, matchers: matchersObj,
    }
    try {
      if (dlg.mode === 'edit' && dlg.editing) {
        await deleteSilence(dlg.editing.id).catch(() => {})
      }
      const s = await createSilence(body)
      ElMessage.success(dlg.mode === 'create' ? '已新建静默策略' : '已更新静默策略')
      dlg.visible = false
      silences.value = silences.value.filter((x) => x.id !== s.id)
      silences.value.unshift(s)
      dlg.editing = null
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : String(e)
      ElMessage.error(msg || '保存失败')
    }
  })
}
async function handleDelete(row: Silence) {
  try {
    await ElMessageBox.confirm(
      '确定删除静默策略「' + (row.comment || row.id.slice(0, 8)) + '...」？删除后生效中的抑制立即失效。',
      '删除静默策略', { type: 'warning', confirmButtonText: '删除', cancelButtonText: '取消' }
    )
  } catch { return }
  try {
    await deleteSilence(row.id)
    silences.value = silences.value.filter((x) => x.id !== row.id)
    ElMessage.success('已删除')
  } catch (e: unknown) {
    const msg = e instanceof Error ? e.message : String(e)
    ElMessage.error(msg || '删除失败')
  }
}

async function loadAll() {
  loading.value = true
  try {
    const [listRs, rulesRs] = await Promise.all([
      listSilences().catch(() => [] as Silence[]),
      listRules().catch(() => [] as RuleBrief[]),
    ])
    silences.value = (listRs || []) as Silence[]
    rules.value = (rulesRs || []) as RuleBrief[]
  } finally { loading.value = false }
}

onMounted(loadAll)
</script>

<template>
  <div class="silences-view">
    <div class="page-header">
      <h2 class="page-title">静默策略</h2>
      <p class="page-sub">按匹配标签 + 时间窗抑制告警通知；生效中的抑制会立即阻止对应告警推送</p>
    </div>

    <ElAffix :offset="0" class="affix-wrap">
      <div class="toolbar panel">
        <div class="tb-left">
          <ElButton type="primary" :icon="Plus" @click="openCreate">新建静默策略</ElButton>
          <ElButton :icon="Refresh" @click="loadAll" :loading="loading">刷新</ElButton>
        </div>
        <div class="tb-right">
          <ElSelect v-model="filterStatus" clearable placeholder="状态" style="width:140px">
            <ElOption label="生效中" value="active" />
            <ElOption label="未开始" value="pending" />
            <ElOption label="已结束" value="ended" />
          </ElSelect>
          <ElInput v-model="filterQ" placeholder="搜索 注释/匹配标签/规则名" clearable style="width:260px" :prefix-icon="Search" />
        </div>
      </div>
    </ElAffix>

    <div v-if="!loading && silences.length === 0" class="panel silences-empty">
      <ElEmpty description="暂无静默策略">
        <template #default>
          <ElButton type="primary" :icon="Plus" @click="openCreate">新建第一个静默策略</ElButton>
        </template>
      </ElEmpty>
    </div>

    <template v-else>
      <ElTable :data="pagedSilences" v-loading="loading" stripe class="silences-table" empty-text="无匹配结果">
        <ElTableColumn label="注释" min-width="180" show-overflow-tooltip>
          <template #default="{ row }"><span class="name-link">{{ asSilence(row).comment || '-' }}</span></template>
        </ElTableColumn>
        <ElTableColumn label="告警规则" min-width="200" show-overflow-tooltip>
          <template #default="{ row }">
            <ElTooltip v-if="asSilence(row).rule_id" :content="(asSilence(row).rule_id as string) || undefined">
              <span class="rule-cell">{{ ruleName(asSilence(row)) }}</span>
            </ElTooltip>
            <span v-else class="rule-cell all-rule">全部告警规则</span>
          </template>
        </ElTableColumn>
        <ElTableColumn label="匹配标签" min-width="240" show-overflow-tooltip>
          <template #default="{ row }"><span class="mono">{{ matchersText(asSilence(row)) }}</span></template>
        </ElTableColumn>
        <ElTableColumn label="开始时间" width="150">
          <template #default="{ row }"><span class="cell-updated" :title="asSilence(row).starts_at">{{ fmtTs(asSilence(row).starts_at) }}</span></template>
        </ElTableColumn>
        <ElTableColumn label="结束时间" width="150">
          <template #default="{ row }"><span class="cell-updated" :title="asSilence(row).ends_at">{{ fmtTs(asSilence(row).ends_at) }}</span></template>
        </ElTableColumn>
        <ElTableColumn label="剩余/已结束" width="120">
          <template #default="{ row }">
            <span :class="['remain-text', silenceStatus(asSilence(row)) === 'ended' ? 'remain-ended' : '']">{{ fmtRemain(asSilence(row)) }}</span>
          </template>
        </ElTableColumn>
        <ElTableColumn label="状态" width="100" align="center">
          <template #default="{ row }">
            <span :class="statusStyle[silenceStatus(asSilence(row))].cls">{{ statusStyle[silenceStatus(asSilence(row))].text }}</span>
          </template>
        </ElTableColumn>
        <ElTableColumn label="创建时间" width="160">
          <template #default="{ row }"><span class="cell-updated" :title="asSilence(row).created_at">{{ fmtTs(asSilence(row).created_at) }}</span></template>
        </ElTableColumn>
        <ElTableColumn label="操作" width="170" align="center">
          <template #default="{ row }">
            <div class="row-actions">
              <ElButton size="small" class="act-btn act-edit" @click="openEdit(asSilence(row))">编辑</ElButton>
              <ElButton size="small" type="danger" plain class="act-btn" @click="handleDelete(asSilence(row))">删除</ElButton>
            </div>
          </template>
        </ElTableColumn>
      </ElTable>

      <div class="panel silences-pager">
        <div class="pager-tip">共 <span class="mono">{{ filtered.length }}</span> 条 / 总 <span class="mono">{{ silences.length }}</span> 条</div>
        <ElPagination v-model:current-page="page" v-model:page-size="pageSize" :page-sizes="PAGE_SIZES"
          :total="filtered.length" layout="sizes, prev, pager, next, jumper, ->, total"
          @current-change="onPage" @size-change="onSize" background />
      </div>
    </template>

    <ElDialog v-model="dlg.visible" :title="dlg.mode === 'create' ? '新建静默策略' : '编辑静默策略'"
      width="640px" :close-on-click-modal="false" destroy-on-close top="6vh" @closed="dlgReset">
      <ElAlert v-if="dlg.mode === 'edit'" type="warning" :closable="false" style="margin-bottom:12px">
        后端暂不支持更新静默策略，保存将执行「删除旧 → 新建新」。生效中策略的抑制会短暂中断。
      </ElAlert>
      <ElForm ref="dlg.formRef" label-width="100px">
        <ElFormItem label="注释"><ElInput v-model="dlg.form.comment" placeholder="例：维护窗口" maxlength="200" show-word-limit /></ElFormItem>
        <ElFormItem label="告警规则">
          <ElSelect v-model="dlg.form.rule_id" clearable filterable placeholder="留空 = 全部规则" style="width:100%">
            <ElOption v-for="r in rules" :key="r.id" :label="r.name || r.id" :value="r.id" />
          </ElSelect>
        </ElFormItem>
        <ElFormItem label="匹配标签 JSON">
          <ElInput v-model="dlg.form.matchersJson" type="textarea" :rows="4"
            placeholder='{"ip":"10.0.0.1","alertname":"CPU High"}' class="mono-font" />
          <div class="form-tip">JSON 对象；留空 {} 表示仅按规则匹配</div>
        </ElFormItem>
        <div style="display:flex;gap:12px">
          <ElFormItem label="开始时间" style="flex:1">
            <ElDatePicker v-model="dlg.form.starts_at" type="datetime" value-format="YYYY-MM-DDTHH:mm:ss"
              format="YYYY-MM-DD HH:mm:ss" style="width:100%" />
          </ElFormItem>
          <ElFormItem label="结束时间" style="flex:1">
            <ElDatePicker v-model="dlg.form.ends_at" type="datetime" value-format="YYYY-MM-DDTHH:mm:ss"
              format="YYYY-MM-DD HH:mm:ss" style="width:100%" />
          </ElFormItem>
        </div>
      </ElForm>
      <template #footer>
        <div class="dlg-footer">
          <ElButton @click="dlg.visible = false">取消</ElButton>
          <ElButton type="primary" :loading="loading" @click="dlgSave">保存</ElButton>
        </div>
      </template>
    </ElDialog>
  </div>
</template>

<style scoped>
.silences-view { padding: 16px 20px 32px; }
.page-header { margin-bottom: 10px; }
.page-title { margin: 0 0 4px; font-size: 22px; }
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
.silences-table { font-size: 13px; overflow: hidden; margin-top: 0; }
.silences-table :deep(.el-table__inner-wrapper::before) { display: none; }
.silences-table :deep(.el-table th.el-table__cell) { background: transparent; }
.silences-table :deep(.el-table td.el-table__cell),
.silences-table :deep(.el-table th.el-table__cell.is-leaf) {
  border-bottom: 1px solid var(--el-border-color-lighter, #f2f3f5);
}
.silences-table :deep(.el-table th.el-table__cell .cell) { padding-top: 4px; padding-bottom: 4px; }
.silences-table :deep(.el-table td.el-table__cell .cell) { padding-top: 8px; padding-bottom: 8px; }
.name-link { color: var(--el-color-primary, #409eff); font-weight: 500; cursor: pointer; user-select: none; }
.name-link:hover { text-decoration: underline; }
.rule-cell { font-size: 13px; color: var(--el-text-color-primary, #303133); }
.rule-cell.all-rule { font-family: Consolas, Menlo, monospace; font-size: 12px; color: var(--el-text-color-secondary, #909399); }
.mono { font-family: Consolas, Menlo, monospace; font-size: 12.5px; }
.mono-font :deep(textarea) { font-family: Consolas, Menlo, monospace; font-size: 12.5px; line-height: 1.5; }
.cell-updated { font-family: Consolas, Menlo, monospace; font-size: 12.5px; color: var(--el-text-color-secondary, #909399); }
.pill-sm {
  display: inline-flex; align-items: center; justify-content: center;
  padding: 0 10px; height: 22px; line-height: 20px;
  font-size: 11.5px; font-weight: 500; border-radius: 4px;
}
.pill-sm.pill-ok { background: var(--el-color-success-light-9, #e1f3d8); color: var(--el-color-success, #67c23a); }
.pill-sm.pill-warn { background: var(--el-color-warning-light-9, #faecd8); color: var(--el-color-warning, #e6a23c); }
.remain-text { font-size: 12.5px; font-weight: 500; color: var(--el-color-success, #67c23a); }
.remain-text.remain-ended { color: var(--el-text-color-secondary, #909399); }
.row-actions { display: inline-flex; align-items: center; gap: 6px; }
.act-btn { height: 26px; padding: 0 12px !important; font-size: 12px !important; border-radius: 4px; }
.act-edit {
  --el-button-border-color: #3f8bff; --el-button-text-color: #3f8bff;
  --el-button-hover-bg-color: rgba(63, 139, 255, 0.08); --el-button-hover-border-color: #3077ef;
  --el-button-hover-text-color: #3077ef;
}
.silences-pager {
  display: flex; align-items: center; justify-content: space-between;
  flex-wrap: wrap; gap: 12px; padding: 12px 18px; margin-top: 12px;
}
.silences-pager .pager-tip { font-size: 13px; color: var(--el-text-color-secondary, #909399); }
.silences-pager .mono { font-family: Consolas, Menlo, monospace; font-weight: 600; color: var(--el-text-color-primary, #303133); }
.silences-empty { padding: 48px 0; }
.form-tip { font-size: 12px; color: var(--el-text-color-secondary, #909399); margin-top: 4px; }
.dlg-footer { display: flex; justify-content: flex-end; gap: 8px; width: 100%; }
</style>