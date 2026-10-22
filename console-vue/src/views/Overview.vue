<script setup lang="ts">
import {
  computed,
  onMounted,
  onBeforeUnmount,
  ref,
} from 'vue'
import { useRouter } from 'vue-router'
import {
  ElButton,
  ElCard,
  ElDescriptions,
  ElDescriptionsItem,
  ElDialog,
  ElDivider,
  ElCol,
  ElEmpty,
  ElProgress,
  ElRow,
  ElSkeleton,
  ElStatistic,
  ElTable,
  ElTableColumn,
  ElTag,
  ElMessage,
} from 'element-plus'
import { Refresh } from '@element-plus/icons-vue'
import { overview as apiOverview } from '@/api/overviewAuthLoginAndMore'
import type {
  AlertRecentItem,
  ClusterInstancesResp,
  IngressBrief,
  OverviewResp,
  ServerInstanceInfo,
  TrapInstanceInfo,
} from '@/api/types'

const router = useRouter()

const loading = ref(true)
const data = ref<OverviewResp | null>(null)
const errMsg = ref('')
const refreshHint = ref('')

async function loadData() {
  loading.value = true
  errMsg.value = ''
  try {
    data.value = await apiOverview()
  } catch (e) {
    const msg = e && typeof e === 'object' && 'message' in e ? String((e as { message: unknown }).message) : '加载总览数据失败'
    errMsg.value = msg
    ElMessage.error(msg)
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
      pressure_inflight: 0,
      ingress_max_inflight: 1,
      is_leader: false,
      leader_holder_id: '',
      cluster_enabled: false,
    }
  } finally {
    loading.value = false
    const now = new Date()
    const pad = (n: number) => String(n).padStart(2, '0')
    refreshHint.value = `刚刚刷新 ${pad(now.getHours())}:${pad(now.getMinutes())}:${pad(now.getSeconds())} · 每 30 秒自动`
  }
}

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

function goAlerts(status: string) {
  router.push({ path: '/alerts', query: status ? { status } : {} })
}
function goPage(page: string) {
  router.push({ path: `/${page}` })
}

const healthType = computed<'success' | 'warning' | 'info' | 'danger'>(() => {
  const n = data.value?.alerts_firing ?? 0
  if (n === 0) return 'success'
  if (n < 50) return 'warning'
  return 'danger'
})

const cluster = computed<ClusterInstancesResp | null>(() => data.value?.cluster ?? null)
const clusterExpanded = ref(false)
function fmtClusterTime(iso?: string) {
  if (!iso) return '—'
  try {
    const d = new Date(iso)
    if (Number.isNaN(d.getTime())) return iso
    const pad = (n: number) => String(n).padStart(2, '0')
    return `${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`
  } catch {
    return iso
  }
}
function shortId(id?: string) {
  if (!id) return '—'
  return id.length > 8 ? id.slice(0, 8) : id
}

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
    rules: !(d.enabled_rules || 0),
  }
})

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

function fmtTime(ts: string): string {
  if (!ts) return '-'
  const d = new Date(ts)
  if (Number.isNaN(d.getTime())) return ts
  const pad = (n: number) => String(n).padStart(2, '0')
  return `${d.getFullYear()}/${d.getMonth() + 1}/${d.getDate()} ${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`
}

function alertTime(item: AlertRecentItem): string {
  return item.starts_at || item.last_occurrence_at || item.pending_since || item.created_at || ''
}

function alertDesc(item: AlertRecentItem): string {
  if (item.description) return item.description
  const ann = item.annotations
  if (ann) {
    return ann.summary || ann.hint || ann.description || ann.msg || ann.content || ''
  }
  return ''
}

function alertTitle(item: AlertRecentItem): string {
  if (item.summary) return item.summary
  if (item.labels?.alertname) return item.labels.alertname
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

const inflightPercent = computed(() => {
  const d = data.value
  if (!d) return 0
  const max = Math.max(1, d.ingress_max_inflight || 1)
  return Math.min(100, Math.round(((d.pressure_inflight ?? 0) / max) * 100))
})
const inflightText = computed(() => {
  const d = data.value
  if (!d) return '0 / 1'
  return `${d.pressure_inflight ?? 0} / ${Math.max(1, d.ingress_max_inflight || 1)}`
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
    <el-skeleton v-if="loading" :loading="true" animated :rows="8" class="ov-skeleton">
      <template #template>
        <el-skeleton-item variant="h1" style="width: 40%; height: 28px; margin-bottom: 20px" />
        <el-skeleton-item variant="rect" style="height: 100px; margin-bottom: 16px" />
        <el-skeleton-item variant="rect" style="height: 80px; margin-bottom: 16px" />
        <el-skeleton-item variant="rect" style="height: 220px" />
      </template>
    </el-skeleton>

    <template v-else>
      <div class="ov-page-header">
        <div class="ov-title-row">
          <div>
            <h2 class="ov-page-title">总览</h2>
            <p class="ov-page-sub">告警态势 · 集群 · 接入压力 · 自监控 — 30 秒自动刷新</p>
          </div>
          <div class="ov-actions">
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
            <ElButton v-else size="small" @click="goAlerts('')">打开告警事件</ElButton>
            <span class="ov-refresh-hint">{{ refreshHint }}</span>
            <ElButton :icon="Refresh" size="small" @click="loadData">刷新</ElButton>
          </div>
        </div>
      </div>

      <!-- 核心告警指标 -->
      <el-card class="panel panel-stats" shadow="never">
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
      </el-card>

      <!-- 集群实例 -->
      <el-card class="panel panel-cluster" shadow="never">
        <template #header>
          <div class="panel-head">
            <span class="panel-title">集群实例</span>
            <div class="health-head-right">
              <el-tag type="primary" effect="plain" size="small">
                Server {{ cluster?.server_count ?? 0 }}
              </el-tag>
              <el-tag type="warning" effect="plain" size="small">
                Trap {{ cluster?.trap_count ?? 0 }}
              </el-tag>
              <ElButton size="small" @click="clusterExpanded = !clusterExpanded">
                {{ clusterExpanded ? '收起明细' : '展开明细' }}
              </ElButton>
              <ElButton size="small" @click="goPage('trap')">Trap 集群心跳</ElButton>
            </div>
          </div>
        </template>
        <div class="cluster-summary">
          <span class="cluster-pill">
            <i class="dot" :class="(cluster?.server_count ?? 0) > 0 ? 'on' : 'off'" />
            Server {{ cluster?.server_count ?? 0 }} 台在线
          </span>
          <span class="cluster-pill">
            <i class="dot" :class="(cluster?.trap_count ?? 0) > 0 ? 'on' : 'off'" />
            Trap {{ cluster?.trap_count ?? 0 }} 台在线
          </span>
          <span v-if="cluster && cluster.trap_redis === false" class="cluster-hint">Trap Redis 未配置，Trap 心跳不可用</span>
        </div>
        <div v-if="clusterExpanded" class="cluster-detail">
          <h4 class="cluster-sub">Server</h4>
          <el-table
            :data="(cluster?.servers || []) as ServerInstanceInfo[]"
            size="small"
            stripe
            empty-text="暂无 Server 心跳（需 Redis，进程启动后约数秒登记）"
          >
            <el-table-column label="实例" min-width="120">
              <template #default="{ row }">
                <code>{{ shortId((row as ServerInstanceInfo).instance_id) }}</code>
              </template>
            </el-table-column>
            <el-table-column label="主机" prop="hostname" min-width="100" />
            <el-table-column label="Listen" prop="listen" min-width="140" />
            <el-table-column label="角色" width="110" align="center">
              <template #default="{ row }">
                <el-tag
                  :type="(row as ServerInstanceInfo).is_leader ? 'success' : 'info'"
                  size="small"
                  effect="plain"
                >
                  {{ (row as ServerInstanceInfo).is_leader ? 'Leader' : 'Follower' }}
                </el-tag>
              </template>
            </el-table-column>
            <el-table-column label="版本" prop="version" width="90" />
            <el-table-column label="心跳" min-width="130">
              <template #default="{ row }">
                {{ fmtClusterTime((row as ServerInstanceInfo).updated_at) }}
              </template>
            </el-table-column>
          </el-table>

          <h4 class="cluster-sub" style="margin-top: 16px;">Trap</h4>
          <el-table
            :data="(cluster?.traps || []) as TrapInstanceInfo[]"
            size="small"
            stripe
            empty-text="暂无 Trap 心跳（确认 redis_url 且 heartbeat_secs > 0）"
          >
            <el-table-column label="实例" min-width="120">
              <template #default="{ row }">
                <code>{{ (row as TrapInstanceInfo).instance_id }}</code>
              </template>
            </el-table-column>
            <el-table-column label="UDP" prop="listen_udp" min-width="140" />
            <el-table-column label="HTTP" prop="listen_http" min-width="140" />
            <el-table-column label="VIP" min-width="110">
              <template #default="{ row }">
                {{ (row as TrapInstanceInfo).ha_vip || '—' }}
              </template>
            </el-table-column>
            <el-table-column label="接收" width="80" align="center">
              <template #default="{ row }">
                {{ (row as TrapInstanceInfo).received ?? 0 }}
              </template>
            </el-table-column>
            <el-table-column label="心跳" min-width="130">
              <template #default="{ row }">
                {{ fmtClusterTime((row as TrapInstanceInfo).updated_at) }}
              </template>
            </el-table-column>
          </el-table>
        </div>
      </el-card>

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

      <!-- 关注中的告警 -->
      <el-card class="panel panel-alerts" shadow="never">
        <template #header>
          <div class="panel-head">
            <span class="panel-title">关注中的告警</span>
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
                <el-tag :type="statusTag(alert.status).type" effect="dark" size="small">
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

      <!-- 接入实时处理 + 压力监控 -->
      <el-row :gutter="16">
        <el-col :xs="24" :lg="14">
          <el-card class="panel panel-table" shadow="never">
            <template #header>
              <div class="panel-head">
                <span class="panel-title">告警接入实时处理</span>
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
                <span class="panel-title">接入压力监控</span>
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

      <el-card class="panel panel-runtime" shadow="never">
        <template #header>
          <div class="panel-head">
            <span class="panel-title">自监控（本进程）</span>
            <a class="hi-sub" href="/metrics" target="_blank" rel="noopener">Prometheus /metrics</a>
          </div>
        </template>
        <div class="stats-row stats-meta">
          <div class="stat-card">
            <el-statistic title="通知成功" :value="Number(data?.metrics?.notify_success ?? 0)" />
          </div>
          <div class="stat-card">
            <el-statistic title="通知失败" :value="Number(data?.metrics?.notify_failure ?? 0)" />
          </div>
          <div class="stat-card">
            <el-statistic title="节流跳过" :value="Number(data?.metrics?.notify_throttled ?? 0)" />
          </div>
          <div class="stat-card">
            <el-statistic title="降级跳过" :value="Number(data?.metrics?.notify_degraded ?? 0)" />
          </div>
          <div class="stat-card">
            <el-statistic title="接入受理" :value="Number(data?.metrics?.ingress_accepted ?? 0)" />
          </div>
          <div class="stat-card">
            <el-statistic title="接入 429" :value="Number(data?.metrics?.ingress_rejected_429 ?? 0)" />
          </div>
          <div class="stat-card">
            <el-statistic title="升级发送" :value="Number(data?.metrics?.escalation_sent ?? 0)" />
          </div>
        </div>
        <p class="pressure-note">计数自进程启动累计；抓取端点 {{ data?.metrics_endpoint || '/metrics' }}（无需登录）。</p>
      </el-card>
    </template>

    <ElDialog
      v-model="detailVisible"
      title="告警详情"
      width="900px"
      :close-on-click-modal="true"
      destroy-on-close
      class="alert-detail-dialog"
    >
      <template v-if="currentAlert">
        <div class="detail-header">
          <div class="detail-title">{{ alertTitle(currentAlert) }}</div>
          <div class="detail-badges">
            <ElTag :type="statusTag(currentAlert.status).type" effect="dark" size="large" style="font-weight: 600; padding: 4px 12px;">
              {{ statusTag(currentAlert.status).text }}
            </ElTag>
            <ElTag :type="severityTag(currentAlert.severity).type" effect="dark" size="large" style="font-weight: 600; padding: 4px 12px;">
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
        <ElDescriptions :column="2" border size="small">
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

        <template v-if="alertDesc(currentAlert)">
          <ElDivider content-position="left">告警描述</ElDivider>
          <div class="detail-summary">{{ alertDesc(currentAlert) }}</div>
        </template>

        <template v-if="detailAnnotations.length > 0">
          <ElDivider content-position="left">注解（Annotations）</ElDivider>
          <ElDescriptions :column="2" border size="small">
            <ElDescriptionsItem v-for="ann in detailAnnotations" :key="ann.key" :label="ann.key" :span="2">
              <span class="mono">{{ ann.value }}</span>
            </ElDescriptionsItem>
          </ElDescriptions>
        </template>

        <template v-if="detailLabels.length > 0">
          <ElDivider content-position="left">标签（Labels）</ElDivider>
          <ElDescriptions :column="2" border size="small">
            <ElDescriptionsItem v-for="lbl in detailLabels" :key="lbl.key" :label="lbl.key" :span="2">
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
.ov-page-header { margin-bottom: 4px; }
.ov-title-row {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
  flex-wrap: wrap;
}
.ov-page-title { margin: 0 0 4px; font-size: 22px; font-weight: 600; color: var(--heading, #303133); }
.ov-page-sub { margin: 0; font-size: 13px; color: var(--muted, #909399); }
.overview-page {
  display: flex;
  flex-direction: column;
  gap: 16px;
  color: var(--text);
}
.ov-actions {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 10px;
  flex-wrap: wrap;
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
  flex-wrap: wrap;
}
.panel-cluster { margin-bottom: 0; }
.cluster-summary {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 14px;
  padding: 4px 2px 8px;
}
.cluster-pill {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  font-size: 14px;
  font-weight: 600;
  color: var(--heading);
}
.cluster-pill .dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  display: inline-block;
  background: var(--muted);
}
.cluster-pill .dot.on { background: var(--ok); }
.cluster-pill .dot.off { background: var(--muted); }
.cluster-hint { font-size: 12px; color: var(--muted); }
.cluster-detail { margin-top: 4px; }
.cluster-sub {
  margin: 0 0 8px;
  font-size: 13px;
  color: var(--muted);
  font-weight: 600;
}
.stats-row {
  display: flex;
  flex-wrap: wrap;
  gap: 14px;
}
.stats-primary .stat-card {
  flex: 1 1 0;
  min-width: 140px;
}
.stats-meta .stat-card {
  flex: 1 1 120px;
  min-width: 110px;
}
.ov-table :deep(.el-table) {
  --el-table-bg-color: transparent;
  --el-table-tr-bg-color: transparent;
  --el-table-header-bg-color: var(--panel-2);
  --el-table-header-text-color: var(--text-secondary);
}
.ov-table :deep(.el-table tr:hover > td) {
  background: var(--overlay-soft) !important;
}
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
.hi-sub {
  font-size: 12px;
  color: var(--primary, #409eff);
  text-decoration: none;
}
.hi-sub:hover { text-decoration: underline; }
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
.stat-card.clickable { cursor: pointer; }
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
.panel-setup :deep(.el-card__body) { padding: 16px 20px; }
.setup-wrap { display: flex; flex-direction: column; gap: 8px; }
.setup-hint { margin: 0; font-size: 13px; color: var(--text-secondary); }
.setup-actions { display: flex; flex-wrap: wrap; gap: 8px; margin-top: 4px; }
.alerts-empty { padding: 20px 0; }
.alerts-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
  max-height: 480px;
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
.alert-item:hover { background: var(--overlay-soft); }
.alert-item.alert-firing { border-left-color: var(--crit); }
.alert-item.alert-pending { border-left-color: var(--warn); }
.alert-item.alert-resolved { border-left-color: var(--ok); }
.alert-item-top {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 8px;
}
.alert-item-badge { display: flex; align-items: center; gap: 6px; }
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
.meta-fingerprint { font-family: monospace; opacity: 0.7; }
.alert-clickable { cursor: pointer; }
.alert-clickable:hover { box-shadow: 0 2px 8px rgba(0, 0, 0, 0.08); }
.detail-header { display: flex; flex-direction: column; gap: 10px; margin-bottom: 8px; }
.detail-title { font-size: 16px; font-weight: 600; color: var(--heading); line-height: 1.4; }
.detail-badges { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; }
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
.dialog-footer { display: flex; justify-content: flex-end; gap: 8px; }
.mono {
  font-family: 'JetBrains Mono', 'Consolas', 'Monaco', monospace;
  font-size: 13px;
  word-break: break-all;
}
.alert-detail-dialog :deep(.el-dialog__body) {
  max-height: 70vh;
  overflow-y: auto;
  padding: 16px 20px;
}
.alert-detail-dialog :deep(.el-descriptions__label) {
  font-weight: 500;
  color: var(--text-secondary);
}
.alert-detail-dialog :deep(.el-descriptions__content) { color: var(--text); }
.alert-detail-dialog :deep(.el-descriptions) {
  --el-descriptions-table-border: var(--line);
  --el-descriptions-table-th-background: var(--panel-2);
}
</style>
