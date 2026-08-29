<script setup lang="ts">
import {
  computed,
  onMounted,
  onBeforeUnmount,
  ref,
} from 'vue'
import { useRouter } from 'vue-router'
import {
  ElBadge,
  ElButton,
  ElCard,
  ElDescriptions,
  ElDescriptionsItem,
  ElDialog,
  ElDivider,
  ElEmpty,
  ElProgress,
  ElRow,
  ElCol,
  ElSkeleton,
  ElStatistic,
  ElTable,
  ElTableColumn,
  ElTag,
  ElMessage,
} from 'element-plus'
import {
  CircleCheckFilled,
  WarningFilled,
  Timer,
  Refresh,
  Close,
} from '@element-plus/icons-vue'
import { overview as apiOverview } from '@/api/overviewAuthLoginAndMore'
import { useAuthStore } from '@/stores/auth'
import type {
  AlertRecentItem,
  IngressBrief,
  NotifySkipItem,
  OverviewResp,
} from '@/api/types'

const auth = useAuthStore()
const router = useRouter()

const loading = ref(true)
const data = ref<OverviewResp | null>(null)
const errMsg = ref('')
const refreshHint = ref('')

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
      enabled_rules: 0,
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
    const now = new Date()
    const pad = (n: number) => String(n).padStart(2, '0')
    refreshHint.value = `刚刚刷新 ${pad(now.getHours())}:${pad(now.getMinutes())}:${pad(now.getSeconds())} · 每 30 秒自动`
  }
}

// 30 秒自动刷新
let refreshTimer: ReturnType<typeof setInterval> | null = null
function startAutoRefresh() {
  stopAutoRefresh()
  refreshTimer = setInterval(() => {
    loadData().catch(() => {})
  }, 30000)
}
function stopAutoRefresh() {
  if (refreshTimer) {
    clearInterval(refreshTimer)
    refreshTimer = null
  }
}

onMounted(() => {
  loadData()
  startAutoRefresh()
})
onBeforeUnmount(() => {
  stopAutoRefresh()
})

// ===== 导航跳转 =====
function goAlerts(status: string) {
  router.push({ path: '/alerts', query: { status } })
}
function goPage(page: string) {
  router.push({ path: `/${page}` })
}

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

// ===== 最小闭环检测 =====
const needsSetup = computed(() => {
  const d = data.value
  if (!d) return false
  const hasChannel = !!(d.channels || 0)
  const hasIngress = !!(d.ingress_routes || 0)
  const hasDatasources = !!(d.datasources || 0)
  const hasEnabledRules = !!(d.enabled_rules || 0)
  const hasRulePath = hasDatasources && hasEnabledRules
  const hasAlertSource = hasIngress || hasRulePath
  return !hasChannel || !hasAlertSource
})
const setupMissing = computed(() => {
  const d = data.value
  if (!d) return { channel: false, ingress: false, datasources: false, rules: false }
  return {
    channel: !(d.channels || 0),
    ingress: !(d.ingress_routes || 0),
    datasources: !(d.datasources || 0),
    rules: !((d.enabled_rules || 0)),
  }
})

// --- 告警状态 severity/tag 映射 ---
type TagType = 'success' | 'warning' | 'info' | 'danger' | 'primary'
function severityTag(s: string): { text: string; type: TagType } {
  switch (s) {
    case 'disaster':
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
      return { text: '告警中', type: 'danger' }
    case 'pending':
      return { text: '待处理', type: 'warning' }
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
  return `${d.getFullYear()}/${d.getMonth() + 1}/${d.getDate()} ${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`
}

function alertTime(item: AlertRecentItem): string {
  // 按优先级获取时间
  return item.starts_at || item.last_occurrence_at || item.pending_since || item.created_at || ''
}

function alertDesc(item: AlertRecentItem): string {
  // 优先使用 description 字段
  if (item.description) return item.description
  // 从 annotations 中提取
  const ann = item.annotations
  if (ann) {
    // 优先使用 summary 字段（包含告警的简要描述）
    return ann.summary || ann.hint || ann.description || ann.msg || ann.content || ''
  }
  return ''
}

function alertTitle(item: AlertRecentItem): string {
  // 优先使用 summary 字段
  if (item.summary) return item.summary
  // 从 labels 中获取 alertname
  if (item.labels?.alertname) return item.labels.alertname
  // 从 annotations 中提取 summary
  if (item.annotations?.summary) return item.annotations.summary
  return item.fingerprint || '-'
}

function ruleName(item: AlertRecentItem): string {
  const ann = item.annotations
  if (ann?.rule_name) return ann.rule_name
  const lbl = item.labels
  if (lbl?.rule_name) return lbl.rule_name
  return ''
}

// ===== 告警详情弹窗 =====
const detailVisible = ref(false)
const currentAlert = ref<AlertRecentItem | null>(null)

function openAlertDetail(alert: AlertRecentItem) {
  currentAlert.value = alert
  detailVisible.value = true
}

function closeAlertDetail() {
  detailVisible.value = false
  currentAlert.value = null
}

const detailAnnotations = computed(() => {
  if (!currentAlert.value?.annotations) return []
  return Object.entries(currentAlert.value.annotations)
    .filter(([k]) => !['summary', 'description', 'rule_name'].includes(k))
    .map(([k, v]) => ({ key: k, value: v }))
})

const detailLabels = computed(() => {
  if (!currentAlert.value?.labels) return []
  return Object.entries(currentAlert.value.labels)
    .map(([k, v]) => ({ key: k, value: v }))
})

function formatDuration(starts: string, ends?: string): string {
  const start = new Date(starts)
  if (Number.isNaN(start.getTime())) return '-'
  const end = ends ? new Date(ends) : new Date()
  if (Number.isNaN(end.getTime())) return '-'
  const diff = Math.max(0, end.getTime() - start.getTime())
  const s = Math.floor(diff / 1000)
  if (s < 60) return `${s} 秒`
  const m = Math.floor(s / 60)
  if (m < 60) return `${m} 分 ${s % 60} 秒`
  const h = Math.floor(m / 60)
  if (h < 24) return `${h} 时 ${m % 60} 分`
  const d = Math.floor(h / 24)
  return `${d} 天 ${h % 24} 时`
}

function getSourceLabel(item: AlertRecentItem): string {
  if (item.datasource) return item.datasource
  const ann = item.annotations
  if (ann?.source) return ann.source
  if (ann?.source_type) return ann.source_type
  return '-'
}

// ===== 演示数据（仅开发环境使用） =====


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
      <div class="ov-page-header">
        <h2 class="ov-page-title">总览</h2>
        <p class="ov-page-sub">系统健康 · 核心指标 · 实时告警 · 接入压力 — 30 秒自动刷新</p>
      </div>

      <!-- 顶部操作栏 -->
      <div class="ov-actions">
        <span class="ov-refresh-hint">{{ refreshHint }}</span>
        <ElButton :icon="Refresh" size="small" @click="loadData">刷新</ElButton>
      </div>

      <!-- ====== 1. 健康状态面板（2x2 栅格） ====== -->
      <el-card class="panel panel-health" shadow="never">
        <template #header>
          <div class="panel-head">
            <span class="panel-title">🏥 系统健康状态</span>
            <div class="health-head-right">
              <el-tag :type="healthType" effect="dark" size="large" style="font-weight: 600; padding: 6px 14px;">
                {{ data?.health || 'unknown' }}
              </el-tag>
              <ElButton
                v-if="data?.alerts_firing"
                type="danger"
                size="small"
                @click="goAlerts('firing')"
              >
                查看正在告警
              </ElButton>
              <ElButton
                v-else
                size="small"
                @click="goAlerts('')"
              >
                打开告警事件
              </ElButton>
            </div>
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
                leader: {{ data?.leader_holder_id ? String(data.leader_holder_id).slice(0, 8) : '-' }}
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

      <!-- ====== 2. 核心指标计数卡片区（可点击跳转） ====== -->
      <el-card class="panel panel-stats" shadow="never">
        <template #header>
          <div class="panel-head">
            <span class="panel-title">📊 核心指标</span>
          </div>
        </template>
        <!-- 主指标：告警状态 -->
        <div class="stats-row stats-primary">
          <div
            v-for="(s, i) in ([
              { k: '正在告警', v: data?.alerts_firing, t: 'danger', icon: '🔥', action: () => goAlerts('firing') },
              { k: '等待中', v: data?.alerts_pending, t: 'warning', icon: '⏳', action: () => goAlerts('pending') },
              { k: '已恢复', v: data?.alerts_resolved, t: 'success', icon: '✅', action: () => goAlerts('resolved') },
              { k: '生效静默', v: data?.active_silences, t: 'info', icon: '🧘', action: () => goPage('silences') },
              { k: '生效维护窗', v: data?.active_maintenance_windows, t: 'info', icon: '🔧', action: () => goPage('maintenance') },
            ] as const)"
            :key="i"
            class="stat-card clickable"
            :class="`stat-${s.t}`"
            @click="s.action()"
          >
            <div class="stat-icon">{{ s.icon }}</div>
            <el-statistic :title="s.k" :value="Number(s.v ?? 0)" />
          </div>
        </div>
        <!-- 元指标：配置项 -->
        <div class="stats-row stats-meta">
          <div
            v-for="(s, i) in ([
              { k: '启用规则', v: `${data?.enabled_rules ?? 0}/${data?.rules ?? 0}`, t: 'success', icon: '📏', action: () => goPage('rules') },
              { k: '数据源', v: data?.datasources, t: 'primary', icon: '🗄️', action: () => goPage('datasources') },
              { k: '通知渠道', v: data?.channels, t: 'warning', icon: '📣', action: () => goPage('channels') },
              { k: '告警接入', v: data?.ingress_routes, t: 'info', icon: '📥', action: () => goPage('ingress') },
            ] as const)"
            :key="i"
            class="stat-card clickable"
            :class="`stat-${s.t}`"
            @click="s.action()"
          >
            <div class="stat-icon">{{ s.icon }}</div>
            <div class="stat-meta">
              <div class="stat-meta-label">{{ s.k }}</div>
              <div class="stat-meta-value">{{ s.v }}</div>
            </div>
          </div>
        </div>
      </el-card>

      <!-- ====== 最小闭环引导 ====== -->
      <el-card v-if="needsSetup" class="panel panel-setup" shadow="never">
        <div class="setup-wrap">
          <strong>尚未完成最小闭环</strong>
          <p class="setup-hint">
            需要<strong>通知渠道</strong>；告警来源任选其一：① 告警接入（Webhook / Kafka / Trap）② 数据源 + 启用规则。
          </p>
          <div class="setup-actions">
            <ElButton v-if="setupMissing.channel" size="small" @click="goPage('channels')">新建通知渠道</ElButton>
            <ElButton v-if="setupMissing.ingress" size="small" @click="goPage('ingress')">配置告警接入</ElButton>
            <ElButton v-if="setupMissing.datasources" size="small" @click="goPage('datasources')">新建数据源</ElButton>
            <ElButton v-if="setupMissing.rules" size="small" @click="goPage('rules')">新建规则</ElButton>
          </div>
        </div>
      </el-card>


      <!-- ====== 4. 最近告警 + 通知跳过（两栏） ====== -->
      <el-row :gutter="16">
        <el-col :xs="24" :lg="14">
          <el-card class="panel panel-alerts" shadow="never">
            <template #header>
              <div class="panel-head">
                <span class="panel-title">🔔 关注中的告警</span>
                <div class="panel-head-right">
                  <el-tag size="small">共 {{ data?.recent_alerts?.length ?? 0 }} 条</el-tag>
                  <ElButton v-if="data?.alerts_firing" type="danger" size="small" link @click="goAlerts('firing')">全部告警中</ElButton>
                </div>
              </div>
            </template>
            <div v-if="(data?.recent_alerts ?? []).length === 0" class="alerts-empty">
              <el-empty description="暂无告警。可去「告警接入」试推送，或等待规则触发。">
                <template #extra>
                  <ElButton type="primary" @click="goPage('ingress')">去告警接入</ElButton>
                </template>
              </el-empty>
            </div>
            <div v-else class="alerts-list">
              <div
                v-for="(alert, idx) in (data?.recent_alerts ?? []) as AlertRecentItem[]"
                :key="alert.id || alert.fingerprint + idx"
                class="alert-item alert-clickable"
                :class="`alert-${alert.status}`"
                @click="openAlertDetail(alert)"
              >
                <div class="alert-item-top">
                  <div class="alert-item-badge">
                    <el-tag
                      :type="statusTag(alert.status).type"
                      effect="dark"
                      size="small"
                    >
                      {{ statusTag(alert.status).text }}
                    </el-tag>
                  </div>
                  <span class="alert-item-time">{{ fmtTime(alertTime(alert)) }}</span>
                </div>
                <div class="alert-item-title" :title="alertTitle(alert)">
                  {{ alertTitle(alert) }}
                </div>
                <div v-if="alertDesc(alert)" class="alert-item-desc" :title="alertDesc(alert)">
                  {{ alertDesc(alert) }}
                </div>
                <div class="alert-item-meta">
                  <span class="meta-fingerprint" :title="alert.fingerprint">
                    #{{ alert.fingerprint?.slice(0, 8) }}
                  </span>
                </div>
              </div>
            </div>
          </el-card>
        </el-col>
        <el-col :xs="24" :lg="10">
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
              height="360"
              empty-text="暂无跳过记录"
              class="ov-table"
            >
              <el-table-column prop="error" label="错误原因" min-width="240" show-overflow-tooltip />
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

      <!-- ====== 5. 接入实时流量 + 压力监控 ====== -->
      <el-row :gutter="16">
        <el-col :xs="24" :lg="14">
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
              height="280"
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
          </el-card>
        </el-col>
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
      </el-row>
    </template>

    <!-- ====== 告警详情弹窗 ====== -->
    <ElDialog
      v-model="detailVisible"
      title="告警详情"
      width="800px"
      :close-on-click-modal="true"
      destroy-on-close
      class="alert-detail-dialog"
    >
      <template v-if="currentAlert">
        <!-- 头部信息 -->
        <div class="detail-header">
          <div class="detail-title">{{ alertTitle(currentAlert) }}</div>
          <div class="detail-badges">
            <ElTag
              :type="statusTag(currentAlert.status).type"
              effect="dark"
              size="large"
              style="font-weight: 600; padding: 4px 12px;"
            >
              {{ statusTag(currentAlert.status).text }}
            </ElTag>
            <ElTag
              :type="severityTag(currentAlert.severity).type"
              effect="dark"
              size="large"
              style="font-weight: 600; padding: 4px 12px;"
            >
              {{ severityTag(currentAlert.severity).text }}
            </ElTag>
            <ElTag v-if="ruleName(currentAlert)" type="info" effect="plain" size="default">
              {{ ruleName(currentAlert) }}
            </ElTag>
            <ElTag type="info" effect="plain" size="default">
              {{ getSourceLabel(currentAlert) }}
            </ElTag>
          </div>
        </div>

        <ElDivider content-position="left">基本信息</ElDivider>

        <ElDescriptions :column="3" border size="small">
          <ElDescriptionsItem label="指纹">
            <span class="mono">{{ currentAlert.fingerprint }}</span>
          </ElDescriptionsItem>
          <ElDescriptionsItem label="产生时间">
            {{ fmtTime(alertTime(currentAlert)) }}
          </ElDescriptionsItem>
          <ElDescriptionsItem label="数据源">
            {{ currentAlert.datasource || '-' }}
          </ElDescriptionsItem>
          <ElDescriptionsItem v-if="currentAlert.id" label="事件 ID">
            <span class="mono">{{ currentAlert.id }}</span>
          </ElDescriptionsItem>
          <ElDescriptionsItem v-if="currentAlert.rule_id" label="规则 ID">
            <span class="mono">{{ currentAlert.rule_id }}</span>
          </ElDescriptionsItem>
          <ElDescriptionsItem label="持续时长">
            {{ formatDuration(alertTime(currentAlert)) }}
          </ElDescriptionsItem>
        </ElDescriptions>

        <!-- 告警描述 -->
        <template v-if="alertDesc(currentAlert)">
          <ElDivider content-position="left">告警描述</ElDivider>
          <div class="detail-summary">
            {{ alertDesc(currentAlert) }}
          </div>
        </template>

        <!-- 注解 -->
        <template v-if="detailAnnotations.length > 0">
          <ElDivider content-position="left">注解（Annotations）</ElDivider>
          <ElDescriptions :column="2" border size="small">
            <ElDescriptionsItem
              v-for="ann in detailAnnotations"
              :key="ann.key"
              :label="ann.key"
              :span="2"
            >
              <span class="mono">{{ ann.value }}</span>
            </ElDescriptionsItem>
          </ElDescriptions>
        </template>

        <!-- 标签 -->
        <template v-if="detailLabels.length > 0">
          <ElDivider content-position="left">标签（Labels）</ElDivider>
          <ElDescriptions :column="2" border size="small">
            <ElDescriptionsItem
              v-for="lbl in detailLabels"
              :key="lbl.key"
              :label="lbl.key"
              :span="2"
            >
              <span class="mono">{{ lbl.value }}</span>
            </ElDescriptionsItem>
          </ElDescriptions>
        </template>
      </template>

      <template #footer>
        <div class="dialog-footer">
          <ElButton
            v-if="currentAlert?.status === 'firing' || currentAlert?.status === 'pending'"
            type="primary"
            @click="goAlerts(currentAlert?.status || 'firing')"
          >
            去告警处理
          </ElButton>
          <ElButton @click="closeAlertDetail">关闭</ElButton>
        </div>
      </template>
    </ElDialog>
  </div>
</template>

<style scoped>
/* === 页面标题区 === */
.ov-page-header { margin-bottom: 8px; }
.ov-page-title { margin: 0 0 4px; font-size: 22px; font-weight: 600; color: var(--heading, #303133); }
.ov-page-sub { margin: 0; font-size: 13px; color: var(--muted, #909399); }
.overview-page {
  display: flex;
  flex-direction: column;
  gap: 16px;
  color: var(--text);
}

/* 顶部操作栏 */
.ov-actions {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 12px;
  min-height: 32px;
}
.ov-refresh-hint {
  color: var(--muted);
  font-size: 12px;
}

.ov-skeleton {
  background: var(--panel-surface);
  border: 1px solid var(--line);
  border-radius: 8px;
  padding: 20px 22px;
}

.panel {
  background: var(--panel-surface);
  border: 1px solid var(--line);
  border-radius: 8px;
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
.panel-head-right {
  display: flex;
  align-items: center;
  gap: 8px;
}
.panel-title {
  font-weight: 600;
  color: var(--heading);
  font-size: 14.5px;
}

.health-head-right {
  display: flex;
  align-items: center;
  gap: 10px;
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
.stats-row {
  display: flex;
  flex-wrap: wrap;
  gap: 14px;
  margin-bottom: 14px;
}
.stats-row:last-child {
  margin-bottom: 0;
}
.stats-primary .stat-card {
  flex: 1 1 0;
  min-width: 140px;
}
.stats-meta .stat-card {
  flex: 1 1 0;
  min-width: 160px;
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
  transition: border-color 0.15s, transform 0.12s;
}
.stat-card.clickable {
  cursor: pointer;
}
.stat-card.clickable:hover {
  border-color: var(--primary-border, var(--primary));
  transform: translateY(-1px);
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
  flex-shrink: 0;
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
.stat-meta {
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.stat-meta-label {
  color: var(--muted);
  font-size: 12px;
  font-weight: 500;
}
.stat-meta-value {
  color: var(--heading);
  font-weight: 700;
  font-size: 22px;
}

/* === 最小闭环引导 === */
.panel-setup :deep(.el-card__body) {
  padding: 16px 20px;
}
.setup-wrap {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.setup-hint {
  margin: 0;
  font-size: 13px;
  color: var(--text-secondary);
}
.setup-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-top: 4px;
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

/* === 告警卡片列表 === */
.alerts-empty {
  padding: 20px 0;
}

.alerts-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
  max-height: 400px;
  overflow-y: auto;
}

.alert-item {
  padding: 12px 14px;
  border-radius: 8px;
  background: var(--stat-bg);
  border: 1px solid var(--line);
  border-left: 4px solid var(--crit);
  transition: background 0.15s;
}

.alert-item:hover {
  background: var(--overlay-soft);
}

.alert-item.alert-firing {
  border-left-color: var(--crit);
}
.alert-item.alert-firing .alert-item-badge .el-tag {
  background-color: var(--crit) !important;
  border-color: var(--crit) !important;
}

.alert-item.alert-pending {
  border-left-color: var(--warn);
}
.alert-item.alert-pending .alert-item-badge .el-tag {
  background-color: var(--warn) !important;
  border-color: var(--warn) !important;
}

.alert-item.alert-resolved {
  border-left-color: var(--ok);
}
.alert-item.alert-resolved .alert-item-badge .el-tag {
  background-color: var(--ok) !important;
  border-color: var(--ok) !important;
}

.alert-item-top {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 8px;
}

.alert-item-badge {
  display: flex;
  align-items: center;
  gap: 6px;
}

.alert-item-badge .el-tag {
  font-weight: 500;
}

.alert-item-time {
  font-size: 12px;
  color: var(--muted);
  font-variant-numeric: tabular-nums;
}

.alert-item-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--heading);
  line-height: 1.5;
  margin-bottom: 6px;
  overflow: hidden;
  text-overflow: ellipsis;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
}

.alert-item-desc {
  font-size: 13px;
  color: var(--text-secondary);
  line-height: 1.5;
  overflow: hidden;
  text-overflow: ellipsis;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  margin-bottom: 8px;
}

.alert-item-meta {
  display: flex;
  align-items: center;
  gap: 12px;
  font-size: 12px;
  color: var(--muted);
}

.meta-fingerprint {
  font-family: monospace;
  opacity: 0.7;
}

/* === 可点击告警卡片 === */
.alert-clickable {
  cursor: pointer;
}
.alert-clickable:hover {
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.08);
}

/* === 告警详情弹窗 === */
.detail-header {
  display: flex;
  flex-direction: column;
  gap: 10px;
  margin-bottom: 8px;
}

.detail-title {
  font-size: 16px;
  font-weight: 600;
  color: var(--heading);
  line-height: 1.4;
}

.detail-badges {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
}

.detail-summary {
  padding: 12px 14px;
  background: var(--stat-bg);
  border: 1px solid var(--line);
  border-radius: 8px;
  font-size: 14px;
  color: var(--text);
  line-height: 1.6;
  white-space: pre-wrap;
  word-break: break-word;
}

.dialog-footer {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}

/* mono 字体样式 */
.mono {
  font-family: 'JetBrains Mono', 'Consolas', 'Monaco', monospace;
  font-size: 13px;
  word-break: break-all;
}

/* 对话框内容样式覆盖 */
.alert-detail-dialog :deep(.el-dialog__body) {
  max-height: 70vh;
  overflow-y: auto;
  padding: 16px 20px;
}

.alert-detail-dialog :deep(.el-descriptions__label) {
  font-weight: 500;
  color: var(--text-secondary);
}

.alert-detail-dialog :deep(.el-descriptions__content) {
  color: var(--text);
}

.alert-detail-dialog :deep(.el-descriptions) {
  --el-descriptions-table-border: var(--line);
  --el-descriptions-table-th-background: var(--panel-2);
}
</style>
