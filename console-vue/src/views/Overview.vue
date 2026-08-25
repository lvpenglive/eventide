<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import {
  ElBadge,
  ElCard,
  ElEmpty,
  ElProgress,
  ElRow,
  ElCol,
  ElSkeleton,
  ElStatistic,
  ElTable,
  ElTag,
  ElMessage,
} from 'element-plus'
import {
  CircleCheckFilled,
  CircleCloseFilled,
  WarningFilled,
  Timer,
} from '@element-plus/icons-vue'
import { overview as apiOverview } from '@/api/overviewAuthLoginAndMore'
import { useAuthStore } from '@/stores/auth'
import type { AlertRecentItem, IngressBrief, NotifySkipItem, OverviewResp } from '@/api/types'

const auth = useAuthStore()

const loading = ref(true)
const data = ref<OverviewResp | null>(null)
const errMsg = ref('')

// 保证加载态至少 1.2s 视觉骨架屏（TR-4.6）
async function loadData() {
  loading.value = true
  errMsg.value = ''
  const t0 = performance.now()
  try {
    data.value = await apiOverview()
  } catch (e) {
    const msg = e && typeof e === 'object' && 'message' in e ? String((e as { message: unknown }).message) : '加载总览数据失败'
    errMsg.value = msg
    ElMessage.error(msg)
    // 兜底空结构，方便渲染
    data.value = {
      health: 'unknown',
      datasources: 0,
      rules: 0,
      channels: 0,
      ingress_routes: 0,
      alerts_total: 0,
      alerts_firing: 0,
      alerts_pending: 0,
      alerts_resolved: 0,
      active_silences: 0,
      active_maintenance_windows: 0,
      recent_alerts: [],
      ingress: [],
      notify_skips: [],
      pressure_inflight: 0,
      ingress_max_inflight: 1,
      is_leader: false,
      leader_holder_id: '',
      cluster_enabled: false,
    }
  } finally {
    const elapsed = performance.now() - t0
    const remain = Math.max(0, 1200 - elapsed)
    setTimeout(() => (loading.value = false), remain)
  }
}
onMounted(loadData)

// ===== 派生值 =====
const isHealthOk = computed(() => (data.value?.health || '').toLowerCase() === 'ok')
const healthType = computed<'success' | 'warning' | 'info' | 'danger'>(() => {
  switch ((data.value?.health || '').toLowerCase()) {
    case 'ok':
      return 'success'
    case 'degraded':
      return 'warning'
    case 'error':
      return 'danger'
    default:
      return 'info'
  }
})
const dbLine = computed(() => {
  const d = auth.database
  if (!d) return '数据库：未获取到连接信息（登录后首次进入时拉取）'
  const db = [d.db_type, [d.host, d.port].filter(Boolean).join(':')].filter(Boolean).join(' @ ')
  const name = d.name ? `/${d.name}` : ''
  const redis = d.redis_url ? `   ·   Redis: ${d.redis_url}` : ''
  return `数据库：${db}${name}${redis}`
})

// --- 告警状态 severity/tag 映射 ---
type TagType = 'success' | 'warning' | 'info' | 'danger' | 'primary'
function severityTag(s: string): { text: string; type: TagType } {
  switch (s) {
    case 'critical':
    case 'error':
      return { text: '严重', type: 'danger' }
    case 'warning':
      return { text: '警告', type: 'warning' }
    case 'info':
      return { text: '信息', type: 'info' }
    case 'ok':
    case 'resolved':
      return { text: '恢复', type: 'success' }
    default:
      return { text: String(s || '-'), type: 'primary' }
  }
}
function statusTag(s: string): { text: string; type: TagType } {
  switch (s) {
    case 'firing':
      return { text: '触发', type: 'danger' }
    case 'pending':
      return { text: '待确认', type: 'warning' }
    case 'resolved':
      return { text: '已恢复', type: 'success' }
    default:
      return { text: String(s || '-'), type: 'info' }
  }
}

function transitionTagType(t: string): TagType {
  switch (t) {
    case 'fire':
    case 'firing':
      return 'danger'
    case 'resolve':
    case 'resolved':
      return 'success'
    case 'ack':
    case 'acknowledge':
      return 'warning'
    default:
      return 'info'
  }
}

function fmtTime(ts: string): string {
  if (!ts) return '-'
  const d = new Date(ts)
  if (Number.isNaN(d.getTime())) return ts
  const pad = (n: number) => String(n).padStart(2, '0')
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`
}

// ===== 未使用的预留列定义（已通过 template 插槽渲染，保留以语义化） =====
// @ts-ignore - 占位保留，后续切换到 tsx/配置式列可直接使用
const _unused = 0

// inflight 进度条
const inflightPercent = computed(() => {
  const d = data.value
  if (!d) return 0
  const max = Math.max(1, d.ingress_max_inflight || 1)
  return Math.min(100, Math.round((d.pressure_inflight / max) * 100))
})
const inflightText = computed(() => {
  const d = data.value
  if (!d) return '0 / 1'
  return `${d.pressure_inflight} / ${Math.max(1, d.ingress_max_inflight)}`
})
const inflightStatus = computed<'' | 'success' | 'warning' | 'exception'>(() => {
  const p = inflightPercent.value
  if (p >= 90) return 'exception'
  if (p >= 70) return 'warning'
  return 'success'
})
</script>

<template>
  <div class="overview-page">
    <!-- 全屏骨架屏（TR-4.6） -->
    <el-skeleton v-if="loading" :loading="true" animated :rows="14" class="ov-skeleton">
      <template #template>
        <el-skeleton-item variant="h1" style="width: 40%; height: 28px; margin-bottom: 20px" />
        <el-row :gutter="16">
          <el-col :xs="24" :sm="12" :md="6"><el-skeleton-item variant="rect" style="height: 120px" /></el-col>
          <el-col :xs="24" :sm="12" :md="6"><el-skeleton-item variant="rect" style="height: 120px" /></el-col>
          <el-col :xs="24" :sm="12" :md="6"><el-skeleton-item variant="rect" style="height: 120px" /></el-col>
          <el-col :xs="24" :sm="12" :md="6"><el-skeleton-item variant="rect" style="height: 120px" /></el-col>
        </el-row>
        <el-skeleton-item variant="rect" style="height: 160px; margin-top: 16px" />
        <el-skeleton-item variant="rect" style="height: 180px; margin-top: 16px" />
        <el-skeleton-item variant="rect" style="height: 140px; margin-top: 16px" />
      </template>
    </el-skeleton>

    <template v-else>
      <!-- ====== 1. 健康状态面板（2x2 栅格） ====== -->
      <el-card class="panel panel-health" shadow="never">
        <template #header>
          <div class="panel-head">
            <span class="panel-title">🏥 系统健康状态</span>
            <el-tag :type="healthType" effect="dark" size="small">
              {{ data?.health || 'unknown' }}
            </el-tag>
          </div>
        </template>
        <el-row :gutter="16" class="health-grid">
          <!-- 健康徽章 -->
          <el-col :xs="24" :sm="12" :md="6" class="health-cell">
            <div class="health-item">
              <div class="hi-label">健康状态</div>
              <el-badge
                :is-dot="false"
                :value="isHealthOk ? 'ON' : 'OFF'"
                :type="healthType || 'info'"
                class="hi-badge"
              >
                <el-icon :size="28" :class="isHealthOk ? 'hi-ok' : 'hi-err'">
                  <component :is="isHealthOk ? CircleCheckFilled : WarningFilled" />
                </el-icon>
              </el-badge>
            </div>
          </el-col>
          <!-- leader -->
          <el-col :xs="24" :sm="12" :md="6" class="health-cell">
            <div class="health-item">
              <div class="hi-label">当前节点角色</div>
              <div class="hi-value" :class="{ ok: data?.is_leader }">
                {{ data?.is_leader ? 'Leader (本节点)' : 'Follower' }}
              </div>
              <div class="hi-sub" title="leader_holder_id">
                <el-icon :size="12"><Timer /></el-icon>
                leader: {{ data?.leader_holder_id || '-' }}
              </div>
            </div>
          </el-col>
          <!-- cluster -->
          <el-col :xs="24" :sm="12" :md="6" class="health-cell">
            <div class="health-item">
              <div class="hi-label">集群模式</div>
              <div class="hi-value">
                <el-tag
                  :type="data?.cluster_enabled ? 'success' : 'info'"
                  effect="plain"
                  size="small"
                >
                  {{ data?.cluster_enabled ? '已启用 Cluster' : '单节点' }}
                </el-tag>
              </div>
            </div>
          </el-col>
          <!-- DB / Redis -->
          <el-col :xs="24" :sm="12" :md="6" class="health-cell">
            <div class="health-item">
              <div class="hi-label">数据库 / 缓存</div>
              <div class="hi-value small">{{ dbLine }}</div>
            </div>
          </el-col>
        </el-row>
      </el-card>

      <!-- ====== 2. 核心指标计数卡片区 ====== -->
      <el-card class="panel panel-stats" shadow="never">
        <template #header>
          <div class="panel-head">
            <span class="panel-title">📊 核心指标</span>
          </div>
        </template>
        <el-row :gutter="14">
          <el-col v-for="(s, i) in ([
            { k: '数据源', v: data?.datasources, t: 'primary' as const, icon: '🗄️' },
            { k: '告警规则', v: data?.rules, t: 'success' as const, icon: '📏' },
            { k: '通知渠道', v: data?.channels, t: 'warning' as const, icon: '📣' },
            { k: '接入路由', v: data?.ingress_routes, t: 'info' as const, icon: '📥' },
            { k: '告警总数', v: data?.alerts_total, t: 'primary' as const, icon: '📌' },
            { k: 'Firing', v: data?.alerts_firing, t: 'danger' as const, icon: '🔥' },
            { k: 'Pending', v: data?.alerts_pending, t: 'warning' as const, icon: '⏳' },
            { k: 'Resolved', v: data?.alerts_resolved, t: 'success' as const, icon: '✅' },
            { k: '活动静默', v: data?.active_silences, t: 'info' as const, icon: '🧘' },
            { k: '进行中维护窗', v: data?.active_maintenance_windows, t: 'info' as const, icon: '🔧' },
          ] as const)" :key="i" :xs="12" :sm="8" :md="6" :lg="5" class="stat-wrap">
            <div class="stat-card" :class="`stat-${s.t}`">
              <div class="stat-icon">{{ s.icon }}</div>
              <el-statistic :title="s.k" :value="Number(s.v ?? 0)" />
            </div>
          </el-col>
        </el-row>
      </el-card>

      <!-- ====== 3. 告警折线趋势（空占位） ====== -->
      <el-card class="panel panel-trend" shadow="never">
        <template #header>
          <div class="panel-head">
            <span class="panel-title">📈 告警趋势（近 24h）</span>
            <el-tag type="info" size="small">折线图占位</el-tag>
          </div>
        </template>
        <div class="trend-empty">
          <el-empty description="趋势图占位：批次 1 接入 ECharts（含 firing / resolved / pending 三条折线 + 对比上周）">
            <template #image>
              <el-icon :size="56" color="var(--muted)"><WarningFilled /></el-icon>
            </template>
          </el-empty>
        </div>
      </el-card>

      <!-- ====== 4. 最近告警表格 ====== -->
      <el-card class="panel panel-table" shadow="never">
        <template #header>
          <div class="panel-head">
            <span class="panel-title">🔔 最近告警</span>
            <el-tag size="small">共 {{ data?.recent_alerts?.length ?? 0 }} 条</el-tag>
          </div>
        </template>
        <el-table
          :data="(data?.recent_alerts ?? []) as AlertRecentItem[]"
          stripe
          size="small"
          height="360"
          empty-text="暂无数据"
          class="ov-table"
        >
          <el-table-column label="状态" width="110" fixed="left">
            <template #default="{ row }">
              <el-tag
                :type="statusTag((row as AlertRecentItem).status).type"
                effect="light"
                size="small"
              >
                {{ statusTag((row as AlertRecentItem).status).text }}
              </el-tag>
            </template>
          </el-table-column>
          <el-table-column prop="fingerprint" label="指纹" min-width="200" show-overflow-tooltip />
          <el-table-column prop="summary" label="摘要" min-width="280" show-overflow-tooltip />
          <el-table-column label="严重度" width="110">
            <template #default="{ row }">
              <el-tag
                :type="severityTag((row as AlertRecentItem).severity).type"
                effect="dark"
                size="small"
              >
                {{ severityTag((row as AlertRecentItem).severity).text }}
              </el-tag>
            </template>
          </el-table-column>
          <el-table-column label="产生时间" width="170">
            <template #default="{ row }">{{ fmtTime((row as AlertRecentItem).created_at) }}</template>
          </el-table-column>
          <el-table-column prop="datasource" label="数据源" min-width="140" show-overflow-tooltip />
        </el-table>
      </el-card>

      <!-- ====== 5. 接入实时流量表格 ====== -->
      <el-card class="panel panel-table" shadow="never">
        <template #header>
          <div class="panel-head">
            <span class="panel-title">📥 告警接入实时处理</span>
            <el-tag size="small" type="info">共 {{ data?.ingress?.length ?? 0 }} 个路由</el-tag>
          </div>
        </template>
        <el-table
          :data="data?.ingress ?? []"
          stripe
          size="small"
          height="320"
          empty-text="暂无接入路由"
          class="ov-table"
        >
          <el-table-column prop="id" label="ID" min-width="180" show-overflow-tooltip />
          <el-table-column prop="name" label="名称" min-width="200" show-overflow-tooltip />
          <el-table-column prop="kind" label="类型" width="140" show-overflow-tooltip />
          <el-table-column label="启用状态" width="110" align="center">
            <template #default="{ row }">
              <el-tag
                :type="(row as IngressBrief).enabled ? 'success' : 'info'"
                effect="plain"
                size="small"
              >
                {{ (row as IngressBrief).enabled ? 'on' : 'off' }}
              </el-tag>
            </template>
          </el-table-column>
        </el-table>
        <!-- v-for 遍历以满足数据驱动响应式跟踪 -->
        <template v-for="(row, _i) in (data?.ingress ?? [])" :key="(row as IngressBrief).id"></template>
      </el-card>

      <!-- ====== 6. 压力监控 + 通知跳过 ====== -->
      <el-row :gutter="16">
        <el-col :xs="24" :lg="10">
          <el-card class="panel panel-pressure" shadow="never">
            <template #header>
              <div class="panel-head">
                <span class="panel-title">🧭 接入压力监控</span>
              </div>
            </template>
            <div class="pressure-row">
              <div class="pressure-label">当前 inflight vs. 上限</div>
              <div class="pressure-value">{{ inflightText }}</div>
            </div>
            <el-progress
              :percentage="inflightPercent"
              :status="inflightStatus"
              :stroke-width="14"
              :text-inside="true"
            />
            <p class="pressure-note">
              超过 70% 建议增加 worker 或调大 ingress_max_inflight；超过 90% 会开始拒绝新请求。
            </p>
          </el-card>
        </el-col>
        <el-col :xs="24" :lg="14">
          <el-card class="panel panel-skips" shadow="never">
            <template #header>
              <div class="panel-head">
                <span class="panel-title">✉️ 通知跳过</span>
                <el-tag size="small" type="warning">共 {{ (data?.notify_skips ?? []).length }} 类</el-tag>
              </div>
            </template>
            <el-table
              :data="data?.notify_skips ?? []"
              stripe
              size="small"
              height="260"
              empty-text="暂无跳过项"
              class="ov-table"
            >
              <el-table-column prop="error" label="错误原因" min-width="320" show-overflow-tooltip />
              <el-table-column label="通知阶段" width="140" align="center">
                <template #default="{ row }">
                  <el-tag
                    :type="transitionTagType((row as NotifySkipItem).transition)"
                    effect="light"
                    size="small"
                  >
                    {{ (row as NotifySkipItem).transition }}
                  </el-tag>
                </template>
              </el-table-column>
              <el-table-column label="时间" width="170">
                <template #default="{ row }">{{ fmtTime((row as NotifySkipItem).created_at ?? '') }}</template>
              </el-table-column>
            </el-table>
          </el-card>
        </el-col>
      </el-row>
    </template>
  </div>
</template>

<style scoped>
.overview-page {
  display: flex;
  flex-direction: column;
  gap: 16px;
  color: var(--text);
}

.ov-skeleton {
  background: var(--panel-surface);
  border: 1px solid var(--line);
  border-radius: 12px;
  padding: 20px 22px;
}

.panel {
  background: var(--panel-surface);
  border: 1px solid var(--line);
  border-radius: 12px;
  box-shadow: 0 1px 0 var(--overlay-soft) inset;
}
.panel :deep(.el-card__header) {
  background: var(--panel-2);
  border-bottom: 1px solid var(--line);
  padding: 10px 16px;
}

.panel-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
}
.panel-title {
  font-weight: 600;
  color: var(--heading);
  font-size: 14.5px;
}

/* === 健康面板 === */
.health-grid {
  padding: 6px 4px;
}
.health-cell {
  margin-bottom: 4px;
}
.health-item {
  height: 100%;
  padding: 16px 14px;
  border-radius: 10px;
  background: var(--stat-bg);
  border: 1px solid var(--line);
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.hi-label {
  font-size: 12px;
  color: var(--muted);
  letter-spacing: 0.04em;
  text-transform: uppercase;
}
.hi-value {
  font-size: 18px;
  font-weight: 700;
  color: var(--heading);
}
.hi-value.ok {
  color: var(--ok);
}
.hi-value.small {
  font-size: 13px;
  line-height: 1.5;
  font-weight: 500;
  color: var(--text-secondary);
  word-break: break-all;
}
.hi-sub {
  font-size: 12px;
  color: var(--muted);
  display: flex;
  align-items: center;
  gap: 4px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.hi-badge {
  margin-top: 6px;
  align-self: flex-start;
}
.hi-ok {
  color: var(--ok);
}
.hi-err {
  color: var(--crit);
}

/* === 指标卡片 === */
.panel-stats .stat-wrap {
  margin-bottom: 12px;
}
.stat-card {
  position: relative;
  padding: 14px 14px 12px;
  border-radius: 10px;
  background: var(--stat-bg);
  border: 1px solid var(--line);
  display: flex;
  align-items: center;
  gap: 12px;
  overflow: hidden;
}
.stat-card::after {
  content: '';
  position: absolute;
  inset: 0 0 auto 0;
  height: 3px;
  opacity: 0.9;
}
.stat-card.stat-primary::after  { background: linear-gradient(90deg, var(--primary-grad-from), var(--primary-grad-to)); }
.stat-card.stat-success::after  { background: linear-gradient(90deg, var(--ok-soft), var(--ok)); }
.stat-card.stat-warning::after  { background: linear-gradient(90deg, var(--warn-soft), var(--warn)); }
.stat-card.stat-danger::after   { background: linear-gradient(90deg, var(--crit-soft), var(--crit)); }
.stat-card.stat-info::after     { background: linear-gradient(90deg, var(--info-soft), var(--info)); }
.stat-icon {
  font-size: 22px;
  width: 36px;
  height: 36px;
  display: grid;
  place-items: center;
  background: var(--overlay-mid);
  border-radius: 10px;
}
.stat-card :deep(.el-statistic__head) {
  color: var(--muted);
  font-size: 12px;
  font-weight: 500;
  margin-bottom: 2px;
}
.stat-card :deep(.el-statistic__content) {
  color: var(--heading);
  font-weight: 700;
}

/* === 趋势图占位 === */
.trend-empty {
  padding: 30px 0 16px;
}

/* === 表格通用 === */
.ov-table :deep(.el-table) {
  --el-table-bg-color: transparent;
  --el-table-tr-bg-color: transparent;
  --el-table-header-bg-color: var(--table-head-bg);
  --el-table-border-color: var(--line);
  --el-table-text-color: var(--text);
  --el-table-header-text-color: var(--text-secondary);
}
.ov-table :deep(.el-table tr:hover > td) {
  background: var(--overlay-soft) !important;
}

/* === 压力监控 === */
.pressure-row {
  display: flex;
  justify-content: space-between;
  align-items: baseline;
  margin-bottom: 8px;
}
.pressure-label {
  font-size: 13px;
  color: var(--text-secondary);
}
.pressure-value {
  font-weight: 700;
  color: var(--heading);
}
.pressure-note {
  margin-top: 10px;
  font-size: 12px;
  color: var(--muted);
}
</style>
