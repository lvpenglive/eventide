<script setup lang="ts">
/* =========================================================
 * Silences.vue —— 静默策略视图（Vue 3 + TS + Element Plus）
 * 批次 2：静默策略管理。
 * 严格独立，不修改任何其他文件。
 * ========================================================= */

import { computed, onMounted, reactive, ref } from 'vue'

// --- Element Plus 按需 ---
import {
  ElAffix,
  ElBadge,
  ElButton,
  ElDatePicker,
  ElDialog,
  ElEmpty,
  ElForm,
  ElFormItem,
  ElIcon,
  ElInput,
  ElMessage,
  ElMessageBox,
  ElOption,
  ElSelect,
  ElTable,
  ElTableColumn,
  ElTag,
  ElTooltip,
} from 'element-plus'
import type { FormInstance } from 'element-plus'

import {
  CircleClose,
  Delete,
  Edit,
  Plus,
  Refresh,
} from '@element-plus/icons-vue'

// =================================================================
// 业务模块 —— 优先真实模块，缺失则 shim 兜底
// =================================================================
// 尝试从 @/api/silences 导入；若并行任务未创建成功则本地 shim
interface SilenceApiShim {
  listSilences: () => Promise<LocalSilence[]>
  createSilence: (payload: SilenceInput) => Promise<LocalSilence>
  updateSilence: (id: string, payload: SilenceInput) => Promise<LocalSilence>
  deleteSilence: (id: string) => Promise<{ ok: boolean }>
}

// 尝试从 @/api/common 获取 listRules
interface CommonShim {
  listRules: () => Promise<Array<{ id: string; name?: string }>>
}

// 动态尝试导入真实模块，失败时使用 fallback shim
let SilApi: SilenceApiShim
let CommonApi: CommonShim

const unimpl = (name: string) => () => {
  ElMessage.warning(`[shim] ${name} 未实现（真实 @/api 模块待接入）`)
  return Promise.reject(new Error(`${name} not implemented`))
}

const fallbackSil: SilenceApiShim = {
  listSilences: async () => [],
  createSilence: unimpl('createSilence'),
  updateSilence: unimpl('updateSilence'),
  deleteSilence: unimpl('deleteSilence'),
}

const fallbackCommon: CommonShim = {
  listRules: async () => [],
}

try {
  // eslint-disable-next-line @typescript-eslint/no-var-requires
  const m = require('@/api/silences')
  SilApi = {
    listSilences: m.listSilences ?? fallbackSil.listSilences,
    createSilence: m.createSilence ?? fallbackSil.createSilence,
    updateSilence: m.updateSilence ?? fallbackSil.updateSilence,
    deleteSilence: m.deleteSilence ?? fallbackSil.deleteSilence,
  }
} catch {
  SilApi = fallbackSil
}

try {
  // eslint-disable-next-line @typescript-eslint/no-var-requires
  const m = require('@/api/common')
  CommonApi = {
    listRules: m.listRules ?? fallbackCommon.listRules,
  }
} catch {
  CommonApi = fallbackCommon
}

// =================================================================
// 本地类型定义（不修改 api/types.ts）
// =================================================================
export interface LocalMatcher {
  key: string
  value: string
}

export interface LocalSilence {
  id: string
  comment?: string
  rule_id?: string | null
  starts_at: string
  ends_at: string
  matchers: LocalMatcher[] | Record<string, string>
  created_at?: string | null
}

export interface SilenceInput {
  comment?: string
  rule_id?: string | null
  starts_at: string
  ends_at: string
  matchers: LocalMatcher[]
}

// 与 @/api/types 中可能存在的 Silence 类型对齐（若未来添加）
type Silence = LocalSilence

// =================================================================
// 运行时状态
// =================================================================
const loading = ref(false)
const silences = ref<Silence[]>([])
const rules = ref<Array<{ id: string; name?: string }>>([])

// --- Dialog 表单 ---
const dialogVisible = ref(false)
const dialogMode = ref<'create' | 'edit'>('create')
const editingId = ref<string | null>(null)
const formRef = ref<FormInstance | null>(null)

const form = reactive({
  comment: '',
  rule_id: '' as string,
  starts_at: '' as string,
  ends_at: '' as string,
  matchersJson: '{}' as string,
})

function resetForm() {
  form.comment = ''
  form.rule_id = ''
  // 默认预填：此刻 ~ 此刻 + 2 小时
  const now = new Date()
  const plus2 = new Date(now.getTime() + 2 * 60 * 60 * 1000)
  form.starts_at = formatDateTimePicker(now)
  form.ends_at = formatDateTimePicker(plus2)
  form.matchersJson = '{}'
  editingId.value = null
  formRef.value?.clearValidate()
}

// =================================================================
// 工具函数
// =================================================================
function pad(n: number): string {
  return String(n).padStart(2, '0')
}

// ElDatePicker value-format 要求：YYYY-MM-DDTHH:mm:ss（本地时间）
function formatDateTimePicker(d: Date): string {
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(
    d.getHours()
  )}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`
}

// 展示用截断：ISO -> YYYY-MM-DD HH:mm:ss
function fmtDisplay(ts: string | null | undefined): string {
  if (!ts) return '-'
  const d = new Date(ts)
  if (Number.isNaN(d.getTime())) return String(ts)
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(
    d.getHours()
  )}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`
}

// 统一把 matchers 规整成 [{key, value}]
function normalizeMatchers(
  m: LocalMatcher[] | Record<string, string> | undefined
): LocalMatcher[] {
  if (!m) return []
  if (Array.isArray(m)) return m.filter((x) => x && x.key)
  return Object.entries(m).map(([key, value]) => ({ key, value }))
}

type SilenceStatus = 'active' | 'pending' | 'ended'
function getStatus(s: unknown): SilenceStatus {
  const row = s as Silence
  const now = Date.now()
  const sTime = new Date(row.starts_at).getTime()
  const eTime = new Date(row.ends_at).getTime()
  if (now < sTime) return 'pending'
  if (now >= sTime && now <= eTime) return 'active'
  return 'ended'
}

const statusBadgeMap: Record<SilenceStatus, { type: 'primary' | 'success' | 'warning' | 'info' | 'danger'; text: string }> = {
  active: { type: 'primary', text: '进行中' },
  pending: { type: 'info', text: '未开始' },
  ended: { type: 'success', text: '已结束' },
}

function remainingText(s: unknown): string {
  const row = s as Silence
  const now = Date.now()
  const sTime = new Date(row.starts_at).getTime()
  const eTime = new Date(row.ends_at).getTime()
  if (now < sTime) {
    const diff = sTime - now
    return `${formatDuration(diff)} 后开始`
  }
  if (now <= eTime) {
    const diff = eTime - now
    return `剩余 ${formatDuration(diff)}`
  }
  const diff = now - eTime
  return `已结束 ${formatDuration(diff)}`
}

function formatDuration(ms: number): string {
  const total = Math.max(0, Math.floor(ms / 1000))
  const d = Math.floor(total / 86400)
  const h = Math.floor((total % 86400) / 3600)
  const m = Math.floor((total % 3600) / 60)
  const s = total % 60
  if (d > 0) return `${d}天${h}小时`
  if (h > 0) return `${h}小时${m}分`
  if (m > 0) return `${m}分${s}秒`
  return `${s}秒`
}

function shortId(id: string): string {
  if (!id) return '-'
  return id.length > 8 ? id.slice(0, 8) : id
}

// =================================================================
// 数据加载
// =================================================================
async function loadList() {
  loading.value = true
  try {
    const data = await SilApi.listSilences()
    silences.value = (data || []) as Silence[]
  } catch (e) {
    // shim 状态下静默兜底空数组
    silences.value = []
  } finally {
    loading.value = false
  }
}

async function loadRules() {
  try {
    const data = await CommonApi.listRules()
    rules.value = data || []
  } catch {
    rules.value = []
  }
}

onMounted(() => {
  loadList()
  loadRules()
})

// =================================================================
// Dialog 操作
// =================================================================
function openCreate() {
  dialogMode.value = 'create'
  resetForm()
  dialogVisible.value = true
}

function openEdit(r: unknown) {
  const row = r as Silence
  dialogMode.value = 'edit'
  resetForm()
  editingId.value = row.id
  form.comment = row.comment || ''
  form.rule_id = row.rule_id || ''
  form.starts_at = row.starts_at || ''
  form.ends_at = row.ends_at || ''
  // 将 matchers 转为 JSON 字符串
  try {
    const mObj = matchersToObject(row.matchers)
    form.matchersJson = JSON.stringify(mObj, null, 2)
  } catch {
    form.matchersJson = '{}'
  }
  dialogVisible.value = true
}

function closeDialog() {
  dialogVisible.value = false
}

// 格式化 matchers 为 JSON 字符串显示
function formatMatchersJson(m: LocalMatcher[] | Record<string, string> | undefined): string {
  try {
    const obj = matchersToObject(m)
    return JSON.stringify(obj)
  } catch {
    return '{}'
  }
}

// 将 matchers 转为 Record 对象
function matchersToObject(m: LocalMatcher[] | Record<string, string> | undefined): Record<string, string> {
  if (!m) return {}
  if (Array.isArray(m)) {
    const obj: Record<string, string> = {}
    for (const item of m) {
      if (item && item.key) obj[item.key] = item.value
    }
    return obj
  }
  return { ...m }
}

// 保存
async function handleSave() {
  // 1. 解析 matchers JSON
  let matchersObj: Record<string, string> = {}
  try {
    const parsed = JSON.parse(form.matchersJson || '{}')
    if (typeof parsed !== 'object' || Array.isArray(parsed)) {
      ElMessage.error('匹配标签 JSON 必须是对象')
      return
    }
    matchersObj = {}
    for (const [k, v] of Object.entries(parsed)) {
      matchersObj[String(k)] = String(v)
    }
  } catch {
    ElMessage.error('匹配标签 JSON 格式错误')
    return
  }

  // 2. 至少 1 个匹配器 或 绑定 1 条规则
  const hasMatchers = Object.keys(matchersObj).length > 0
  if (!hasMatchers && !form.rule_id) {
    ElMessage.error('请至少填写一个匹配器或绑定一条规则')
    return
  }

  // 3. starts_at < ends_at
  if (!form.starts_at || !form.ends_at) {
    ElMessage.error('请选择开始时间和结束时间')
    return
  }
  const sTime = new Date(form.starts_at).getTime()
  const eTime = new Date(form.ends_at).getTime()
  if (Number.isNaN(sTime) || Number.isNaN(eTime)) {
    ElMessage.error('时间格式错误')
    return
  }
  if (sTime >= eTime) {
    ElMessage.error('开始时间必须早于结束时间')
    return
  }

  const payload: SilenceInput = {
    comment: form.comment.trim() || undefined,
    rule_id: form.rule_id || null,
    starts_at: form.starts_at,
    ends_at: form.ends_at,
    matchers: Object.entries(matchersObj).map(([key, value]) => ({ key, value })),
  }

  try {
    if (dialogMode.value === 'create') {
      await SilApi.createSilence(payload)
      ElMessage.success('静默策略创建成功')
    } else if (editingId.value) {
      await SilApi.updateSilence(editingId.value, payload)
      ElMessage.success('静默策略更新成功')
    }
    dialogVisible.value = false
    loadList()
  } catch (e) {
    // shim 状态下的兜底：不弹二次错误
  }
}

// 删除
async function handleDelete(r: unknown) {
  const row = r as Silence
  try {
    await ElMessageBox.confirm(
      `确认删除静默策略「${shortId(row.id)}」？此操作不可恢复。`,
      '删除确认',
      {
        confirmButtonText: '删除',
        cancelButtonText: '取消',
        type: 'warning',
      }
    )
  } catch {
    return
  }
  try {
    await SilApi.deleteSilence(row.id)
    ElMessage.success('删除成功')
    loadList()
  } catch {
    // shim 状态兜底
  }
}

// =================================================================
// 渲染辅助
// =================================================================
const matcherColumns = computed(() => {
  return silences.value.map((s) => normalizeMatchers(s.matchers))
})

function getMatchersByIndex(idx: number): LocalMatcher[] {
  return matcherColumns.value[idx] || []
}
</script>

<template>
  <div class="silences-view" style="padding: 16px 20px 32px">
    <!-- 标题 -->
    <div style="margin-bottom: 16px">
      <h2 style="margin: 0 0 4px; font-size: 20px; font-weight: 600; color: #1f2937">
        静默策略
      </h2>
      <p style="margin: 0; font-size: 13px; color: #6b7280">
        按匹配器 + 时间窗抑制告警通知
      </p>
    </div>

    <!-- 吸附工具栏 -->
    <ElAffix :offset="0" style="margin-bottom: 12px">
      <div
        style="
          display: flex;
          justify-content: space-between;
          align-items: center;
          padding: 10px 0;
          background: #fff;
          border-bottom: 1px solid #eef0f3;
        "
      >
        <div>
          <ElButton type="primary" :icon="Plus" @click="openCreate">
            新建静默策略
          </ElButton>
        </div>
        <div>
          <ElButton :icon="Refresh" @click="loadList" :loading="loading">
            刷新
          </ElButton>
        </div>
      </div>
    </ElAffix>

    <!-- 空态 -->
    <template v-if="!loading && silences.length === 0">
      <div
        style="
          background: #fff;
          border-radius: 8px;
          border: 1px solid #eef0f3;
          padding: 64px 0;
        "
      >
        <ElEmpty description="暂无静默策略">
          <template #default>
            <ElButton type="primary" :icon="Plus" @click="openCreate">
              新建第一个静默策略
            </ElButton>
          </template>
        </ElEmpty>
      </div>
    </template>

    <!-- 列表 -->
    <template v-else>
      <ElTable
        :data="silences"
        v-loading="loading"
        stripe
        border
        style="width: 100%; background: #fff; border-radius: 8px; overflow: hidden"
      >
        <!-- 注释 -->
        <ElTableColumn label="注释" min-width="160" show-overflow-tooltip>
          <template #default="{ row }">
            <span style="font-size: 13px; color: #1f2937">
              {{ row.comment || '—' }}
            </span>
          </template>
        </ElTableColumn>

        <!-- 规则 -->
        <ElTableColumn label="规则" width="140">
          <template #default="{ row }">
            <template v-if="!row.rule_id">
              <span class="mono-cell" style="font-family: ui-monospace, monospace; font-size: 12px; color: #6b7280">全部</span>
            </template>
            <template v-else>
              <ElTooltip :content="row.rule_id" :show-after="300">
                <span class="mono-cell" style="font-family: ui-monospace, monospace; font-size: 12px; color: #1f2937; cursor: pointer">
                  {{ shortId(row.rule_id) }}…
                </span>
              </ElTooltip>
            </template>
          </template>
        </ElTableColumn>

        <!-- 匹配标签 -->
        <ElTableColumn label="匹配标签" min-width="200" show-overflow-tooltip>
          <template #default="{ row }">
            <span class="mono-cell" style="font-family: ui-monospace, monospace; font-size: 12px; color: #1f2937">
              {{ formatMatchersJson(row.matchers) }}
            </span>
          </template>
        </ElTableColumn>

        <!-- 时间窗 -->
        <ElTableColumn label="时间窗" min-width="300">
          <template #default="{ row }">
            <span style="font-size: 13px; color: #1f2937">
              {{ fmtDisplay(row.starts_at) }}
              <span style="color: #9ca3af; margin: 0 4px">→</span>
              {{ fmtDisplay(row.ends_at) }}
            </span>
          </template>
        </ElTableColumn>

        <!-- 状态 -->
        <ElTableColumn label="状态" width="100" align="center">
          <template #default="{ row }">
            <span :class="['status-badge', getStatus(row) === 'active' ? 'on' : 'off']">
              {{ getStatus(row) === 'active' ? '生效中' : '未生效' }}
            </span>
          </template>
        </ElTableColumn>

        <!-- 操作 -->
        <ElTableColumn label="操作" width="130" fixed="right" align="center">
          <template #default="{ row }">
            <ElButton
              link
              type="primary"
              size="small"
              :icon="Edit"
              @click="openEdit(row)"
            >
              编辑
            </ElButton>
            <ElButton
              link
              type="danger"
              size="small"
              :icon="Delete"
              @click="handleDelete(row)"
            >
              删除
            </ElButton>
          </template>
        </ElTableColumn>
      </ElTable>
    </template>

    <!-- 新建/编辑 Dialog -->
    <ElDialog
      v-model="dialogVisible"
      :title="dialogMode === 'create' ? '新建静默' : '编辑静默'"
      width="560px"
      :close-on-click-modal="false"
      @closed="resetForm"
    >
      <div class="dialog-desc">在时间窗内抑制匹配标签的通知。</div>
      <ElForm
        ref="formRef"
        label-position="top"
        style="padding-top: 4px"
      >
        <!-- 注释 -->
        <ElFormItem label="注释">
          <ElInput
            v-model="form.comment"
            placeholder="维护窗口"
            maxlength="200"
            show-word-limit
          />
        </ElFormItem>

        <!-- 规则 ID -->
        <ElFormItem label="规则 ID（可选，留空匹配全部）">
          <ElInput
            v-model="form.rule_id"
            placeholder="uuid"
          />
        </ElFormItem>

        <!-- 匹配标签 JSON -->
        <ElFormItem label="匹配标签 JSON" required>
          <ElInput
            v-model="form.matchersJson"
            type="textarea"
            :rows="4"
            placeholder='{"ip":"10.0.0.1"}'
          />
          <div class="form-hint">例：{"ip":"10.0.0.1"} 或 {"alertname":"CPU High"}</div>
        </ElFormItem>

        <!-- 时间窗 -->
        <div class="form-row">
          <ElFormItem label="开始时间" required style="flex: 1">
            <ElDatePicker
              v-model="form.starts_at"
              type="datetime"
              placeholder="开始时间"
              value-format="YYYY-MM-DDTHH:mm:ss"
              style="width: 100%"
              format="YYYY-MM-DD HH:mm:ss"
            />
          </ElFormItem>
          <ElFormItem label="结束时间" required style="flex: 1">
            <ElDatePicker
              v-model="form.ends_at"
              type="datetime"
              placeholder="结束时间"
              value-format="YYYY-MM-DDTHH:mm:ss"
              style="width: 100%"
              format="YYYY-MM-DD HH:mm:ss"
            />
          </ElFormItem>
        </div>
      </ElForm>

      <template #footer>
        <div style="display: flex; justify-content: flex-end; gap: 8px">
          <ElButton @click="closeDialog">取消</ElButton>
          <ElButton type="primary" @click="handleSave">保存</ElButton>
        </div>
      </template>
    </ElDialog>
  </div>
</template>

<style scoped>
/* 局部样式，避免污染全局，保持克制 */
.silences-view :deep(.el-affix) {
  z-index: 10;
}

/* 状态徽章 - 与老版对齐 */
.status-badge {
  display: inline-block;
  padding: 2px 10px;
  font-size: 12px;
  border-radius: 10px;
  font-weight: 500;
}
.status-badge.on {
  background: #dcfce7;
  color: #16a34a;
}
.status-badge.off {
  background: #f3f4f6;
  color: #6b7280;
}

/* 对话框描述 */
.dialog-desc {
  font-size: 13px;
  color: #6b7280;
  margin-bottom: 12px;
}

/* 表单行 */
.form-row {
  display: flex;
  gap: 16px;
}

/* 表单提示 */
.form-hint {
  font-size: 12px;
  color: #9ca3af;
  margin-top: 4px;
}
</style>
