<script setup lang="ts">
/* =========================================================
 * Silences.vue —— 静默策略视图（Vue 3 + TS + Element Plus）
 * 批次 2：静默策略管理。
 * 严格独立，不修改任何其他文件。
 * ========================================================= */

import { computed, onMounted, reactive, ref, watch } from 'vue'

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

interface FormRow {
  key: string
  value: string
}

const form = reactive({
  comment: '',
  rule_id: '' as string,
  starts_at: '' as string,
  ends_at: '' as string,
  matchers: [{ key: '', value: '' }] as FormRow[],
})
const dateRange = ref<[string, string]>(['', ''])
watch(
  dateRange,
  (val: [string, string] | null | undefined) => {
    form.starts_at = (val && val[0]) || ''
    form.ends_at = (val && val[1]) || ''
  },
  { deep: true }
)

function resetForm() {
  form.comment = ''
  form.rule_id = ''
  // 默认预填：此刻 ~ 此刻 + 2 小时
  const now = new Date()
  const plus2 = new Date(now.getTime() + 2 * 60 * 60 * 1000)
  form.starts_at = formatDateTimePicker(now)
  form.ends_at = formatDateTimePicker(plus2)
  dateRange.value = [form.starts_at, form.ends_at]
  form.matchers = [{ key: '', value: '' }]
  editingId.value = null
  formRef.value?.clearValidate()
}

// =================================================================
// 工具函数
// =================================================================
function pad(n: number): string {
  return String(n).padStart(2, '0')
}

// ElDatePicker value-format 要求：YYYY-MM-DDTHH:mm:ssZ（UTC）
function formatDateTimePicker(d: Date): string {
  return `${d.getUTCFullYear()}-${pad(d.getUTCMonth() + 1)}-${pad(d.getUTCDate())}T${pad(
    d.getUTCHours()
  )}:${pad(d.getUTCMinutes())}:${pad(d.getUTCSeconds())}Z`
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
  dateRange.value = [form.starts_at, form.ends_at]
  const mList = normalizeMatchers(row.matchers)
  form.matchers = mList.length > 0
    ? mList.map((m) => ({ key: m.key, value: m.value }))
    : [{ key: '', value: '' }]
  dialogVisible.value = true
}

function closeDialog() {
  dialogVisible.value = false
}

// 匹配器行增删
function addMatcherRow() {
  form.matchers.push({ key: '', value: '' })
}

function removeMatcherRow(idx: number) {
  if (form.matchers.length <= 1) {
    form.matchers[0] = { key: '', value: '' }
    return
  }
  form.matchers.splice(idx, 1)
}

// 保存
async function handleSave() {
  // 基础校验
  // 1. matchers：至少 1 条 或 rule_id 至少有值（至少 1 个条件）
  const validMatchers = form.matchers.filter((m) => m.key.trim() !== '')
  if (validMatchers.length === 0 && !form.rule_id) {
    ElMessage.error('请至少填写一个匹配器或绑定一条规则')
    return
  }
  // 2. starts_at < ends_at
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
    matchers: validMatchers.map((m) => ({ key: m.key.trim(), value: m.value })),
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
        <!-- 时间段 + 剩余/状态 -->
        <ElTableColumn label="时间段" min-width="320">
          <template #default="{ row, $index }">
            <div style="line-height: 1.5">
              <div style="font-size: 13px; color: #1f2937; margin-bottom: 2px">
                {{ fmtDisplay(row.starts_at) }}
                <span style="color: #9ca3af; margin: 0 4px">～</span>
                {{ fmtDisplay(row.ends_at) }}
              </div>
              <div style="display: flex; align-items: center; gap: 8px">
                <ElBadge
                  :type="statusBadgeMap[getStatus(row)].type"
                  is-dot
                  style="margin-right: 4px"
                />
                <span
                  style="
                    display: inline-block;
                    padding: 1px 8px;
                    font-size: 12px;
                    border-radius: 10px;
                    background: #f3f4f6;
                    color: #4b5563;
                  "
                >
                  {{ remainingText(row) }}
                </span>
                <ElTag :type="statusBadgeMap[getStatus(row)].type" size="small" effect="light">
                  {{ statusBadgeMap[getStatus(row)].text }}
                </ElTag>
              </div>
            </div>
            <!-- 消除未使用变量警告（实际上面没用 $index，这里占位即可） -->
            <span style="display: none">{{ $index }}</span>
          </template>
        </ElTableColumn>

        <!-- 匹配器 Matchers -->
        <ElTableColumn label="匹配器" min-width="280">
          <template #default="{ $index }">
            <div style="display: flex; flex-wrap: wrap; gap: 4px; align-items: center">
              <template
                v-for="(m, i) in getMatchersByIndex($index).slice(0, 5)"
                :key="`${m.key}-${m.value}-${i}`"
              >
                <ElTag size="small" type="info" effect="plain">
                  {{ m.key }}={{ m.value }}
                </ElTag>
              </template>
              <ElTooltip
                v-if="getMatchersByIndex($index).length > 5"
                :show-after="200"
              >
                <template #content>
                  <div style="max-width: 320px">
                    <div
                      v-for="(m, i) in getMatchersByIndex($index)"
                      :key="`tip-${m.key}-${i}`"
                      style="padding: 2px 0; font-size: 12px"
                    >
                      <span style="color: #60a5fa">{{ m.key }}</span>
                      <span style="color: #9ca3af"> = </span>
                      <span style="color: #fbbf24">{{ m.value }}</span>
                    </div>
                  </div>
                </template>
                <ElTag size="small" type="warning" effect="light">
                  +{{ getMatchersByIndex($index).length - 5 }} 更多
                </ElTag>
              </ElTooltip>
              <span
                v-if="getMatchersByIndex($index).length === 0"
                style="color: #9ca3af; font-size: 12px"
              >
                无匹配器（按规则绑定生效）
              </span>
            </div>
          </template>
        </ElTableColumn>

        <!-- 规则绑定 -->
        <ElTableColumn label="规则" width="140">
          <template #default="{ row }">
            <template v-if="!row.rule_id">
              <ElTag size="small" effect="plain">全部规则</ElTag>
            </template>
            <template v-else>
              <ElTooltip :content="row.rule_id" :show-after="200">
                <span
                  style="
                    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
                    font-size: 12px;
                    background: #eff6ff;
                    color: #1d4ed8;
                    padding: 2px 8px;
                    border-radius: 4px;
                    cursor: pointer;
                  "
                >
                  {{ shortId(row.rule_id) }}…
                </span>
              </ElTooltip>
            </template>
          </template>
        </ElTableColumn>

        <!-- 备注 -->
        <ElTableColumn label="备注" min-width="180" show-overflow-tooltip>
          <template #default="{ row }">
            <span style="font-size: 13px; color: #374151">
              {{ row.comment || '-' }}
            </span>
          </template>
        </ElTableColumn>

        <!-- 创建时间 -->
        <ElTableColumn label="创建时间" width="170">
          <template #default="{ row }">
            <span style="font-size: 12px; color: #6b7280">
              {{ fmtDisplay(row.created_at) }}
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
      :title="dialogMode === 'create' ? '新建静默策略' : '编辑静默策略'"
      width="640px"
      :close-on-click-modal="false"
      @closed="resetForm"
    >
      <ElForm
        ref="formRef"
        label-width="96px"
        style="padding-top: 4px"
      >
        <!-- 备注 -->
        <ElFormItem label="备注">
          <ElInput
            v-model="form.comment"
            type="textarea"
            :rows="2"
            placeholder="备注说明，建议填写此次静默的变更单号或原因"
            maxlength="200"
            show-word-limit
          />
        </ElFormItem>

        <!-- 规则绑定 -->
        <ElFormItem label="绑定规则">
          <ElSelect
            v-model="form.rule_id"
            placeholder="选择或输入规则 ID，不选则对全部规则生效"
            style="width: 100%"
            filterable
            allow-create
            default-first-option
            clearable
          >
            <ElOption value="" label="全部规则（不绑定特定规则）" />
            <ElOption
              v-for="r in rules"
              :key="r.id"
              :value="r.id"
              :label="r.name ? `${r.name} (${shortId(r.id)})` : shortId(r.id)"
            />
          </ElSelect>
        </ElFormItem>

        <!-- 时间窗 -->
        <ElFormItem label="时间窗" required>
          <ElDatePicker
            v-model="dateRange"
            type="datetimerange"
            range-separator="至"
            start-placeholder="开始时间"
            end-placeholder="结束时间"
            value-format="YYYY-MM-DDTHH:mm:ss[Z]"
            style="width: 100%"
            format="YYYY-MM-DD HH:mm:ss"
          />
        </ElFormItem>

        <!-- 匹配器动态行 -->
        <ElFormItem label="匹配器" required>
          <div style="width: 100%">
            <div
              v-for="(m, idx) in form.matchers"
              :key="idx"
              style="display: flex; gap: 8px; margin-bottom: 8px; align-items: center"
            >
              <ElInput
                v-model="m.key"
                placeholder="标签键，如 alertname / instance"
                style="flex: 1"
              />
              <ElInput
                v-model="m.value"
                placeholder="精确匹配值"
                style="flex: 1"
              />
              <ElButton
                :icon="CircleClose"
                circle
                size="small"
                type="danger"
                link
                @click="removeMatcherRow(idx)"
                :title="form.matchers.length <= 1 ? '清空此行为空' : '删除此行'"
              />
            </div>
            <ElButton
              type="primary"
              plain
              link
              :icon="Plus"
              size="small"
              @click="addMatcherRow"
            >
              添加匹配器
            </ElButton>
            <div
              style="font-size: 12px; color: #9ca3af; margin-top: 4px"
            >
              至少需要 1 个匹配器（键名非空） 或 绑定 1 条规则
            </div>
          </div>
        </ElFormItem>
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
</style>
