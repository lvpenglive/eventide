<script setup lang="ts">
import { onMounted, reactive, ref } from 'vue'
import {
  ElButton,
  ElCard,
  ElEmpty,
  ElInput,
  ElMessage,
  ElOption,
  ElSelect,
  ElTable,
  ElTableColumn,
  ElTag,
  ElTooltip,
} from 'element-plus'
import { Refresh, Search } from '@element-plus/icons-vue'
import { listAuditLogs } from '@/api/iam'
import type { AuditLog, AuditQuery } from '@/api/types'

// ============================================================
// 动作标签映射（对齐旧版 app.js actionLabel）
// ============================================================
const ACTION_LABEL: Record<string, string> = {
  'alert.ack': '确认告警',
  'alert.unack': '取消确认',
  'alert.close': '关闭告警',
  'silence.create': '新建静默',
  'silence.delete': '删除静默',
  'maintenance.create': '新建维护窗',
  'maintenance.update': '更新维护窗',
  'maintenance.delete': '删除维护窗',
  'rule.create': '新建规则',
  'rule.update': '更新规则',
  'rule.delete': '删除规则',
  'channel.create': '新建渠道',
  'channel.update': '更新渠道',
  'channel.delete': '删除渠道',
  'ingress.create': '新建接入',
  'ingress.update': '更新接入',
  'ingress.delete': '删除接入',
  'enrich.create': '新建丰富',
  'enrich.update': '更新丰富',
  'enrich.delete': '删除丰富',
  'user.create': '新建用户',
  'user.update': '更新用户',
  'user.delete': '删除用户',
  'user.reset_password': '重置密码',
  'role.create': '新建角色',
  'role.update': '更新角色',
  'role.delete': '删除角色',
  'auth.change_password': '修改密码',
  'license.create': '导入许可',
  'license.update': '更新许可',
  'license.delete': '清除许可',
}
function actionLabel(a?: string): string {
  if (!a) return '—'
  return ACTION_LABEL[a] || a
}

// 资源类型选项（对齐旧版 resourceTypes）
const RESOURCE_TYPES: { value: string; label: string }[] = [
  { value: '', label: '全部资源' },
  { value: 'alert', label: '告警' },
  { value: 'silence', label: '静默' },
  { value: 'maintenance', label: '维护窗' },
  { value: 'rule', label: '规则' },
  { value: 'channel', label: '渠道' },
  { value: 'ingress', label: '接入' },
  { value: 'enrich', label: '丰富' },
  { value: 'user', label: '用户' },
  { value: 'role', label: '角色' },
  { value: 'department', label: '部门' },
  { value: 'settings', label: '设置' },
  { value: 'license', label: '许可' },
  { value: 'auth', label: '认证' },
]

// 动作选项（对齐旧版 actions）
const ACTIONS: { value: string; label: string }[] = [
  { value: '', label: '全部动作' },
  { value: 'alert.ack', label: '确认告警' },
  { value: 'alert.unack', label: '取消确认' },
  { value: 'alert.close', label: '关闭告警' },
  { value: 'silence.create', label: '新建静默' },
  { value: 'silence.delete', label: '删除静默' },
  { value: 'maintenance.create', label: '新建维护窗' },
  { value: 'maintenance.update', label: '更新维护窗' },
  { value: 'maintenance.delete', label: '删除维护窗' },
  { value: 'rule.create', label: '新建规则' },
  { value: 'rule.update', label: '更新规则' },
  { value: 'rule.delete', label: '删除规则' },
  { value: 'user.reset_password', label: '重置密码' },
  { value: 'auth.change_password', label: '修改密码' },
]

// ============================================================
// 筛选与数据
// ============================================================
const filters = reactive<AuditQuery & { limit: number }>({
  actor: '',
  action: '',
  resource_type: '',
  q: '',
  limit: 200,
})
const rows = ref<AuditLog[]>([])
const loading = ref(false)

let debounceTimer: ReturnType<typeof setTimeout> | null = null
function onFilterInput() {
  if (debounceTimer) clearTimeout(debounceTimer)
  debounceTimer = setTimeout(() => load(), 280)
}

async function load(): Promise<void> {
  loading.value = true
  try {
    rows.value = await listAuditLogs({
      actor: filters.actor || undefined,
      action: filters.action || undefined,
      resource_type: filters.resource_type || undefined,
      q: filters.q || undefined,
      limit: filters.limit,
    })
  } catch (e) {
    const msg = e && typeof e === 'object' && 'message' in e ? String((e as { message: unknown }).message) : '加载审计日志失败'
    ElMessage.error(msg)
  } finally {
    loading.value = false
  }
}

function statusTagType(code?: number): 'success' | 'danger' | 'info' {
  if (code == null) return 'info'
  return code >= 200 && code < 300 ? 'success' : 'danger'
}

function fmtTime(s?: string): string {
  if (!s) return '—'
  const d = new Date(s)
  if (Number.isNaN(d.getTime())) return s
  const pad = (n: number) => String(n).padStart(2, '0')
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`
}

onMounted(() => load())
</script>

<template>
  <div class="audit-page">
    <ElCard shadow="never" class="audit-card">
      <template #header>
        <div class="card-header">
          <span class="card-title">操作审计</span>
          <ElButton :icon="Refresh" size="small" @click="load">刷新</ElButton>
        </div>
      </template>

      <!-- 筛选器 -->
      <div class="audit-toolbar">
        <ElInput
          v-model="filters.actor"
          placeholder="操作人"
          size="small"
          clearable
          style="max-width: 160px"
          @input="onFilterInput"
        >
          <template #prefix><el-icon><Search /></el-icon></template>
        </ElInput>
        <ElSelect v-model="filters.action" size="small" placeholder="全部动作" style="width: 150px" @change="load">
          <ElOption v-for="a in ACTIONS" :key="a.value" :label="a.label" :value="a.value" />
        </ElSelect>
        <ElSelect v-model="filters.resource_type" size="small" placeholder="全部资源" style="width: 150px" @change="load">
          <ElOption v-for="r in RESOURCE_TYPES" :key="r.value" :label="r.label" :value="r.value" />
        </ElSelect>
        <ElInput
          v-model="filters.q"
          placeholder="搜索路径 / 资源 ID"
          size="small"
          clearable
          style="max-width: 220px"
          @input="onFilterInput"
        >
          <template #prefix><el-icon><Search /></el-icon></template>
        </ElInput>
      </div>

      <!-- 表格 -->
      <ElTable :data="rows" v-loading="loading" stripe size="small" style="width: 100%">
        <ElTableColumn label="时间" width="170">
          <template #default="{ row }">{{ fmtTime((row as AuditLog).created_at) }}</template>
        </ElTableColumn>
        <ElTableColumn label="操作人" width="120">
          <template #default="{ row }">{{ (row as AuditLog).actor_username || (row as AuditLog).actor_uid || '—' }}</template>
        </ElTableColumn>
        <ElTableColumn label="动作" min-width="140">
          <template #default="{ row }">
            <div class="action-cell">
              <ElTag size="small" type="primary">{{ actionLabel((row as AuditLog).action) }}</ElTag>
              <span class="action-code">{{ (row as AuditLog).action || '' }}</span>
            </div>
          </template>
        </ElTableColumn>
        <ElTableColumn label="资源" min-width="120">
          <template #default="{ row }">
            <div class="resource-cell">
              <span>{{ (row as AuditLog).resource_type || '—' }}</span>
              <ElTooltip v-if="(row as AuditLog).resource_id" :content="String((row as AuditLog).resource_id)" placement="top">
                <span class="resource-id">#{{ String((row as AuditLog).resource_id).length > 12 ? String((row as AuditLog).resource_id).slice(0, 8) + '…' : (row as AuditLog).resource_id }}</span>
              </ElTooltip>
            </div>
          </template>
        </ElTableColumn>
        <ElTableColumn label="结果" width="90">
          <template #default="{ row }">
            <ElTag v-if="(row as AuditLog).status_code != null" size="small" :type="statusTagType((row as AuditLog).status_code)">
              {{ (row as AuditLog).status_code }}
            </ElTag>
            <span v-else>—</span>
          </template>
        </ElTableColumn>
        <ElTableColumn label="路径" min-width="200">
          <template #default="{ row }">
            <div class="path-cell">
              <ElTag size="small" type="info">{{ (row as AuditLog).method || '—' }}</ElTag>
              <span class="path-text" :title="(row as AuditLog).path || ''">{{ (row as AuditLog).path || '' }}</span>
            </div>
          </template>
        </ElTableColumn>
        <ElTableColumn label="IP" width="130">
          <template #default="{ row }">
            <span class="mono">{{ (row as AuditLog).client_ip || '—' }}</span>
          </template>
        </ElTableColumn>
        <template #empty>
          <ElEmpty description="暂无审计记录。确认告警、改规则、改用户等写操作后会出现在这里。" />
        </template>
      </ElTable>
    </ElCard>
  </div>
</template>

<style scoped>
.audit-page {
  display: flex;
  flex-direction: column;
  gap: 16px;
}
.audit-card {
  border: 1px solid var(--line);
  border-radius: 12px;
}
.card-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}
.card-title {
  font-weight: 600;
  font-size: 15px;
  color: var(--heading);
}
.audit-toolbar {
  display: flex;
  gap: 10px;
  flex-wrap: wrap;
  margin-bottom: 14px;
}
.action-cell {
  display: flex;
  align-items: center;
  gap: 6px;
}
.action-code {
  font-size: 11px;
  color: var(--muted);
}
.resource-cell {
  display: flex;
  align-items: center;
  gap: 6px;
}
.resource-id {
  font-family: 'SFMono-Regular', Consolas, monospace;
  font-size: 11px;
  color: var(--muted);
  cursor: help;
}
.path-cell {
  display: flex;
  align-items: center;
  gap: 6px;
}
.path-text {
  font-family: 'SFMono-Regular', Consolas, monospace;
  font-size: 12px;
  color: var(--text-secondary);
  word-break: break-all;
}
.mono {
  font-family: 'SFMono-Regular', Consolas, monospace;
  font-size: 12px;
  color: var(--text-secondary);
}
</style>
