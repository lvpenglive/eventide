<script setup lang="ts">
import { computed, markRaw, onMounted, reactive, ref } from 'vue'
import { ElMessage } from 'element-plus'
import { Refresh } from '@element-plus/icons-vue'
import { useAuthStore } from '@/stores/auth'

// ============================================================
// 本地类型
// ============================================================
interface TrapHealth {
  ok?: true
  instance_id: string
  listen_udp: string
  ha_vip?: string
  kafka_enabled: boolean
  kafka_topic?: string
  started_at?: string
  version?: string
}

interface TrapStats {
  received: number
  parsed_ok: number
  parse_err: number
  kafka_ok: number
  kafka_err: number
  simulated: number
  dropped?: number
}

interface TrapRecentVarbind {
  oid: string
  name?: string
  value?: string
  type?: string
}

interface TrapRecentItem {
  at: string
  peer: string
  trap_oid: string
  alertname?: string
  severity?: string
  kafka?: boolean
  varbinds?: TrapRecentVarbind[]
}

interface TrapPeerItem {
  instance_id: string
  listen_udp: string
  ha_vip?: string
  received?: number
  updated_at?: string
  kafka_topic?: string
  redis_connected?: boolean
}

interface TrapClusterResp {
  items: TrapPeerItem[]
  redis?: boolean
  hint?: string
}

interface TrapSimulateReq {
  ip: string
  trap_oid: string
  alertname?: string | null
  severity?: string
  dry_run?: boolean
}

interface TrapSimulateResp {
  ok?: true
  kafka?: boolean
  alert?: Record<string, unknown>
}

interface IngressRoute {
  id: string
  name: string
  kind: string
  options: Record<string, string>
  [k: string]: unknown
}

interface KafkaGroupState {
  group_id: string
  state?: string
  total_lag?: number
  partitions: Array<{
    partition: number
    committed?: number
    latest?: number
    lag?: number
    leader?: string
    topic?: string
  }>
}

// ============================================================
// API 绑定
// ============================================================
interface TrapApiShim {
  getTrapHealth(): Promise<TrapHealth>
  getTrapStats(): Promise<TrapStats>
  getTrapRecent(limit?: number): Promise<{ items: TrapRecentItem[] }>
  listTrapInstances(): Promise<TrapClusterResp>
  simulateTrap(req: TrapSimulateReq): Promise<TrapSimulateResp>
}

interface IngressApiShim {
  listIngress(): Promise<IngressRoute[]>
}

interface KafkaApiShim {
  describeKafkaGroup(body: { brokers: string; group_id: string; topic?: string }): Promise<KafkaGroupState>
}

function _unimpl(name: string): never {
  throw new Error(`[API shim] 模块未实现：${name}`)
}

const _shimTrap: TrapApiShim = {
  getTrapHealth: () => Promise.reject(_unimpl('getTrapHealth')),
  getTrapStats: () => Promise.reject(_unimpl('getTrapStats')),
  getTrapRecent: () => Promise.reject(_unimpl('getTrapRecent')),
  listTrapInstances: () => Promise.reject(_unimpl('listTrapInstances')),
  simulateTrap: () => Promise.reject(_unimpl('simulateTrap')),
}
const _shimIngress: IngressApiShim = {
  listIngress: () => Promise.reject(_unimpl('listIngress')),
}
const _shimKafka: KafkaApiShim = {
  describeKafkaGroup: () => Promise.reject(_unimpl('describeKafkaGroup')),
}

// @ts-ignore
import * as _rawTrap from '@/api/trap'
// @ts-ignore
import * as _rawIngress from '@/api/ingress'
// @ts-ignore
import * as _rawKafka from '@/api/kafka'
const _trap = markRaw(_rawTrap as unknown as Record<string, unknown>)
const _ingress = markRaw(_rawIngress as unknown as Record<string, unknown>)
const _kafka = markRaw(_rawKafka as unknown as Record<string, unknown>)

function _bind<S extends object>(shim: S, src: Record<string, unknown>): S {
  const out = { ...(shim as unknown as object) } as S
  for (const key of Object.keys(shim as unknown as object)) {
    if (key in src && typeof (src as Record<string, unknown>)[key] === 'function') {
      ;(out as unknown as Record<string, unknown>)[key] = (src as Record<string, unknown>)[key]
    }
  }
  return out
}

const _trapApi = _bind<TrapApiShim>(_shimTrap, _trap)
const _ingressApi = _bind<IngressApiShim>(_shimIngress, _ingress)
const _kafkaApi = _bind<KafkaApiShim>(_shimKafka, _kafka)

const { getTrapHealth, getTrapStats, getTrapRecent, listTrapInstances, simulateTrap } = _trapApi
const { listIngress } = _ingressApi
const { describeKafkaGroup } = _kafkaApi

// ============================================================
// 权限 & 工具
// ============================================================
const auth = useAuthStore()
const canWrite = computed(() => auth.can('trap:write'))

function errMsg(e: unknown, fb: string): string {
  if (e && typeof e === 'object') {
    const m = (e as { message?: unknown }).message
    if (typeof m === 'string') return m
  }
  return fb
}

function asPeer(r: unknown): TrapPeerItem { return r as TrapPeerItem }
function asRecent(r: unknown): TrapRecentItem { return r as TrapRecentItem }
function asKp(r: unknown) {
  return r as { partition: number; committed?: number; latest?: number; lag?: number; leader?: string; topic?: string }
}

function fmtTime(s: string | undefined | null): string {
  if (!s) return '-'
  return s.replace('T', ' ').slice(0, 19)
}

const severityOptions = [
  { value: 'critical', label: '严重 (critical)' },
  { value: 'error', label: '错误 (error)' },
  { value: 'warning', label: '警告 (warning)' },
  { value: 'info', label: '信息 (info)' },
  { value: 'ok', label: '正常 (ok)' },
]

// ============================================================
// 数据
// ============================================================
const activeTab = ref('status')
const loadingAll = ref(false)
const healthOk = ref(false)
const healthErr = ref('')

const health = ref<TrapHealth | null>(null)
const stats = ref<TrapStats | null>(null)
const recent = ref<TrapRecentItem[]>([])
const cluster = ref<TrapClusterResp | null>(null)

async function loadAll() {
  loadingAll.value = true
  healthErr.value = ''
  try {
    const [h, s, r, c] = await Promise.all([
      getTrapHealth().catch((e) => { throw new Error(errMsg(e, 'health 失败')) }),
      getTrapStats().catch(() => ({ received: 0, parsed_ok: 0, parse_err: 0, kafka_ok: 0, kafka_err: 0, simulated: 0, dropped: 0 } as TrapStats)),
      getTrapRecent(50).catch(() => ({ items: [] as TrapRecentItem[] })),
      listTrapInstances().catch(() => ({ items: [] as TrapPeerItem[], redis: false, hint: '' } as TrapClusterResp)),
    ])
    health.value = h
    healthOk.value = true
    stats.value = s
    recent.value = r?.items || []
    cluster.value = c
    void loadKafkaLag()
  } catch (e) {
    healthOk.value = false
    healthErr.value = errMsg(e, 'Trap 服务未连通')
    health.value = null
    stats.value = null
    recent.value = []
    cluster.value = null
  } finally {
    loadingAll.value = false
  }
}

// Kafka lag
const lagLoading = ref(false)
const lagHtml = ref('')

async function loadKafkaLag() {
  lagLoading.value = true
  lagHtml.value = '<p class="hint">加载积压…</p>'
  try {
    const routes = await listIngress()
    const trapRoute = (routes || []).find((r) => {
      if (r.kind !== 'kafka') return false
      const opt = r.options || {}
      const topic = String(opt.topic || '')
      const preset = String(opt._preset || '')
      return (
        preset === 'snmptrap' ||
        topic === 'eventide.snmptrap' ||
        topic === (health.value?.kafka_topic || '')
      )
    })
    if (!trapRoute) {
      lagHtml.value = '<p class="hint">未找到 SNMP Trap 的 Kafka Ingress。请在「告警接入」配置 Topic（默认 <code>eventide.snmptrap</code>）。</p>'
      return
    }
    const opt = trapRoute.options || {}
    const brokers = String(trapRoute.endpoint || '').trim()
    const topic = String(opt.topic || health.value?.kafka_topic || 'eventide.snmptrap')
    const groupId = String(opt.group_id || '').trim() || `eventide-ingress-${trapRoute.id}`
    if (!brokers) {
      lagHtml.value = `<p class="hint">Ingress「${trapRoute.name}」未配置 brokers。</p>`
      return
    }
    const data = await describeKafkaGroup({ brokers, group_id: groupId, topic })
    const parts = data.partitions || []
    const partsHtml = parts.length
      ? `<div class="alert-table-scroll"><table class="data"><thead><tr>
          <th>分区</th><th>committed</th><th>latest</th><th>lag</th>
        </tr></thead><tbody>${parts
          .map((p) => `<tr>
          <td>${p.partition}</td>
          <td class="mono">${p.committed ?? 0}</td>
          <td class="mono">${p.latest ?? 0}</td>
          <td><strong>${p.lag ?? 0}</strong></td>
        </tr>`).join('')}</tbody></table></div>`
      : `<p class="hint">无分区位点（消费组可能尚未提交 offset）。</p>`
    lagHtml.value = `
      <p class="hint" style="margin:0 0 10px">
        路由 <code>${trapRoute.name}</code>
        · Group <code>${groupId}</code>
        · Topic <code>${topic}</code>
        · 总积压 <strong>${data.total_lag ?? 0}</strong>
        · 状态 ${data.state || '—'}
      </p>
      ${partsHtml}`
  } catch (e) {
    lagHtml.value = `<p class="hint">查询积压失败：${errMsg(e, String(e))}</p>`
  } finally {
    lagLoading.value = false
  }
}

// ============================================================
// 试推送
// ============================================================
const simForm = reactive<TrapSimulateReq>({
  ip: '10.0.0.1',
  trap_oid: '1.3.6.1.6.3.1.1.5.3',
  alertname: '',
  severity: 'warning',
  dry_run: false,
})
const simLoading = ref(false)
const simOut = ref('')

async function handleSimulate() {
  if (!canWrite.value) {
    ElMessage.warning('需要 trap:write 权限才能试推送')
    return
  }
  simLoading.value = true
  simOut.value = ''
  try {
    const result = await simulateTrap({
      ip: simForm.ip,
      trap_oid: simForm.trap_oid,
      alertname: simForm.alertname || null,
      severity: simForm.severity,
      dry_run: simForm.dry_run,
    })
    simOut.value = JSON.stringify(result, null, 2)
    ElMessage.success(result.kafka ? '已推送至 Kafka' : '试推送完成（dry-run）')
  } catch (e) {
    simOut.value = JSON.stringify({ error: errMsg(e, '试推送失败') }, null, 2)
    ElMessage.error(errMsg(e, '试推送失败'))
  } finally {
    simLoading.value = false
  }
}

// ============================================================
// 最近事件详情
// ============================================================
const detailVisible = ref(false)
const detailItem = ref<TrapRecentItem | null>(null)

function showDetail(r: TrapRecentItem) {
  detailItem.value = r
  detailVisible.value = true
}
function closeDetail() {
  detailVisible.value = false
  detailItem.value = null
}

// ============================================================
// 计算属性
// ============================================================
const recv = computed(() => Number(stats.value?.received ?? 0))
const parseErr = computed(() => Number(stats.value?.parse_err ?? 0))
const errRate = computed(() => {
  const r = recv.value
  return r > 0 ? ((parseErr.value / r) * 100).toFixed(1) : '0.0'
})

onMounted(() => { void loadAll() })
</script>

<template>
  <div class="trap-page">
    <!-- 顶部工具栏 -->
    <div class="page-head">
      <h2>SNMP Trap</h2>
      <p class="page-sub">Trap 服务状态 · 试推送 · 对接 Kafka Ingress</p>
      <div class="page-actions">
        <button type="button" class="ghost" :disabled="loadingAll" @click="loadAll">
          <el-icon><Refresh /></el-icon>
          刷新
        </button>
      </div>
    </div>

    <!-- 健康失败空态 -->
    <div v-if="!healthOk && !loadingAll" class="panel trap-offline">
      <h3>Trap 服务未连通</h3>
      <p class="hint trap-offline-err">{{ healthErr || '无法连接 Trap 服务' }}</p>
      <ol class="trap-offline-steps">
        <li>启动 Trap：<code>cargo run -p eventide-trap -- eventide-trap.toml</code></li>
        <li>在 <code>eventide.toml</code> 配置 <code>[trap] api_url = "http://127.0.0.1:8081"</code></li>
        <li>Eventide 侧配置 Kafka Ingress，Topic 与 Trap 写出一致（默认 <code>eventide.snmptrap</code>）</li>
      </ol>
    </div>

    <el-tabs v-else v-model="activeTab" class="trap-tabs">
      <!-- 服务状态 -->
      <el-tab-pane label="服务状态" name="status">
        <div class="panel">
          <h3 class="trap-section-title">服务状态</h3>
          <p class="hint trap-section-meta">
            <span class="badge on">在线</span>
            实例 <code>{{ health?.instance_id || '—' }}</code>
            · UDP <code>{{ health?.listen_udp || '—' }}</code>
            <template v-if="health?.ha_vip">· VIP <code>{{ health.ha_vip }}</code></template>
            · Kafka
            <template v-if="health?.kafka_enabled">
              <span class="badge on">已启用</span> <code>{{ health?.kafka_topic || '' }}</code>
            </template>
            <template v-else>
              <span class="badge off">未配置 brokers</span>
            </template>
            · 解析失败率 {{ errRate }}%
          </p>
          <div class="trap-stat-cards">
            <div class="trap-stat"><div class="n">{{ stats?.received ?? 0 }}</div><div class="l">接收</div></div>
            <div class="trap-stat"><div class="n">{{ stats?.parsed_ok ?? 0 }}</div><div class="l">解析成功</div></div>
            <div class="trap-stat"><div class="n">{{ stats?.parse_err ?? 0 }}</div><div class="l">解析失败</div></div>
            <div class="trap-stat"><div class="n">{{ stats?.kafka_ok ?? 0 }}</div><div class="l">Kafka 成功</div></div>
            <div class="trap-stat"><div class="n">{{ stats?.kafka_err ?? 0 }}</div><div class="l">Kafka 失败</div></div>
            <div class="trap-stat"><div class="n">{{ stats?.simulated ?? 0 }}</div><div class="l">试推送</div></div>
          </div>
        </div>
      </el-tab-pane>

      <!-- Kafka 消费积压 -->
      <el-tab-pane label="Kafka 消费积压" name="lag">
        <div class="panel">
          <h3 class="trap-section-title">Kafka 消费积压</h3>
          <div v-if="lagLoading" class="hint">加载中…</div>
          <div v-else v-html="lagHtml"></div>
        </div>
      </el-tab-pane>

      <!-- 集群心跳 -->
      <el-tab-pane label="集群心跳" name="cluster">
        <div class="panel">
          <h3 class="trap-section-title">集群心跳（多机同时收）</h3>
          <p class="hint trap-section-meta">
            多机 Trap 经 Redis 上报心跳。推荐前面挂 UDP LB 分流（三台同时收）；见仓库 <code>deploy/trap-ha/</code>。
            Redis
            <span v-if="cluster?.redis" class="badge on">可用</span>
            <span v-else class="badge off">不可用</span>
          </p>
          <div v-if="(cluster?.items?.length ?? 0) > 0" class="alert-table-scroll">
            <table class="data">
              <thead>
                <tr><th>实例</th><th>UDP</th><th>VIP</th><th>接收</th><th>心跳</th></tr>
              </thead>
              <tbody>
                <tr v-for="(p, i) in cluster?.items" :key="i">
                  <td class="mono">{{ asPeer(p).instance_id }}</td>
                  <td class="mono">{{ asPeer(p).listen_udp }}</td>
                  <td class="mono">{{ asPeer(p).ha_vip || '—' }}</td>
                  <td>{{ asPeer(p).received ?? 0 }}</td>
                  <td>{{ fmtTime(asPeer(p).updated_at) }}</td>
                </tr>
              </tbody>
            </table>
          </div>
          <p v-else class="hint">暂无心跳（确认 Trap 配了 <code>redis_url</code> 且 <code>heartbeat_secs > 0</code>）。</p>
        </div>
      </el-tab-pane>

      <!-- 试推送 -->
      <el-tab-pane label="试推送" name="sim">
        <div class="panel trap-sim">
          <h3 class="trap-section-title">试推送（模拟 Trap）</h3>
          <p class="hint trap-section-meta">写出与 Kafka Ingress Generic 对齐的 JSON，用于联调（不依赖真实设备）。</p>
          <div class="trap-sim-grid">
            <div class="field"><label>设备 IP</label>
              <input v-model="simForm.ip" type="text" placeholder="10.0.0.1" :disabled="!canWrite" />
            </div>
            <div class="field"><label>Trap OID</label>
              <input v-model="simForm.trap_oid" type="text" placeholder="1.3.6.1.6.3.1.1.5.3" :disabled="!canWrite" />
            </div>
            <div class="field"><label>告警名（可选）</label>
              <input v-model="simForm.alertname" type="text" placeholder="linkDown" :disabled="!canWrite" />
            </div>
            <div class="field"><label>级别</label>
              <select v-model="simForm.severity" :disabled="!canWrite">
                <option v-for="s in severityOptions" :key="s.value" :value="s.value">{{ s.label }}</option>
              </select>
            </div>
          </div>
          <label class="check-row" style="margin:8px 0 14px">
            <input v-model="simForm.dry_run" type="checkbox" :disabled="!canWrite" />
            <span>仅预览 JSON（不写 Kafka）</span>
          </label>
          <button v-if="canWrite" type="button" class="primary" :disabled="simLoading" @click="handleSimulate">
            {{ simLoading ? '推送中…' : '试推送' }}
          </button>
          <p v-else class="hint">需要 <code>trap:write</code> 才能试推送。</p>
          <pre v-if="simOut" class="mono trap-sim-out">{{ simOut }}</pre>
        </div>
      </el-tab-pane>

      <!-- 最近事件 -->
      <el-tab-pane label="最近事件" name="recent">
        <div class="panel">
          <h3 class="trap-section-title">最近事件</h3>
          <p class="hint trap-section-meta">点击「详情」查看完整 OID / 变量名 / Varbind。</p>
          <div v-if="recent.length > 0" class="alert-table-scroll">
            <table class="data">
              <thead>
                <tr><th>时间</th><th>IP</th><th>OID</th><th>名称</th><th>Kafka</th><th></th></tr>
              </thead>
              <tbody>
                <tr v-for="(it, i) in recent" :key="i">
                  <td>{{ fmtTime(asRecent(it).at) }}</td>
                  <td class="mono">{{ asRecent(it).peer }}</td>
                  <td class="mono" :title="asRecent(it).trap_oid">
                    {{ (asRecent(it).trap_oid && asRecent(it).trap_oid.length > 36) ? asRecent(it).trap_oid.slice(0, 36) + '…' : asRecent(it).trap_oid }}
                  </td>
                  <td>{{ asRecent(it).alertname }}</td>
                  <td>{{ asRecent(it).kafka ? '✓' : '—' }}</td>
                  <td class="actions"><button type="button" @click="showDetail(asRecent(it))">详情</button></td>
                </tr>
              </tbody>
            </table>
          </div>
          <div v-else class="empty">尚无 Trap / 试推送记录。</div>
        </div>
      </el-tab-pane>
    </el-tabs>

    <!-- 详情模态框 -->
    <div v-if="detailVisible && detailItem" class="modal-mask" @click.self="closeDetail">
      <div class="modal-dialog" style="max-width: 640px;">
        <div class="modal-head">
          <h3>Trap 事件详情</h3>
          <button type="button" class="modal-close" @click="closeDetail">×</button>
        </div>
        <div class="modal-body">
          <div class="ds-form-section">
            <div class="ds-field"><label>时间</label><input :value="fmtTime(detailItem.at)" disabled /></div>
            <div class="ds-field"><label>设备 IP</label><input :value="detailItem.peer" disabled /></div>
            <div class="ds-field"><label>Trap OID</label><input :value="detailItem.trap_oid" disabled /></div>
            <div class="ds-field"><label>告警名</label><input :value="detailItem.alertname || '—'" disabled /></div>
            <div class="ds-field"><label>级别</label>
              <input :value="detailItem.severity || '—'" disabled />
            </div>
            <div class="ds-field"><label>Kafka</label>
              <input :value="detailItem.kafka ? '已写入' : '未写入'" disabled />
            </div>
          </div>

          <h4 style="margin: 16px 0 8px;">Varbinds</h4>
          <div v-if="detailItem.varbinds && detailItem.varbinds.length > 0" class="alert-table-scroll">
            <table class="data">
              <thead>
                <tr><th>OID</th><th>名称</th><th>Value</th><th>Type</th></tr>
              </thead>
              <tbody>
                <tr v-for="(vb, i) in detailItem.varbinds" :key="i">
                  <td class="mono">{{ vb.oid }}</td>
                  <td>{{ vb.name || '—' }}</td>
                  <td class="mono">{{ vb.value ?? '' }}</td>
                  <td>{{ vb.type || '—' }}</td>
                </tr>
              </tbody>
            </table>
          </div>
          <div v-else class="empty">此事件无 Varbinds。</div>
        </div>
        <div class="modal-actions">
          <button type="button" @click="closeDetail">关闭</button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.trap-page {
  display: flex;
  flex-direction: column;
  gap: 16px;
  width: 100%;
  min-width: 0;
  box-sizing: border-box;
}

/* Tabs */
.trap-tabs {
  background: transparent;
}
.trap-tabs :deep(.el-tabs__header) {
  margin: 0;
  border-bottom: 1px solid var(--el-border-color, var(--line));
}
.trap-tabs :deep(.el-tabs__nav-wrap::after) {
  display: none;
}
.trap-tabs :deep(.el-tabs__item) {
  font-size: 14px;
  font-weight: 500;
  color: var(--el-text-color-regular, var(--muted));
}
.trap-tabs :deep(.el-tabs__item.is-active) {
  color: var(--el-color-primary);
}
.trap-tabs :deep(.el-tabs__active-bar) {
  height: 2px;
  background: var(--el-color-primary);
}
.trap-tabs :deep(.el-tabs__content) {
  padding-top: 16px;
}

.page-head {
  display: flex;
  align-items: baseline;
  gap: 10px;
  margin-bottom: 4px;
}
.page-head h2 {
  font-size: 20px;
  margin: 0;
  color: var(--el-text-color-primary, var(--heading));
}
.page-head .page-sub {
  font-size: 13px;
  color: var(--el-text-color-secondary, var(--muted));
  margin: 0;
}
.page-actions {
  margin-left: auto;
  display: flex;
  gap: 8px;
}

/* 面板 */
.panel {
  background: var(--el-bg-color, var(--panel));
  border: 1px solid var(--el-border-color, var(--line));
  border-radius: 10px;
  padding: 16px 18px;
}

/* Trap section */
.trap-section-title {
  margin: 0 0 0.75rem;
  font-size: 1rem;
}
.trap-section-meta {
  margin: 0 0 12px;
}

/* 分栏 */
.trap-split {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 16px;
  align-items: start;
}

/* 统计卡片 */
.trap-stat-cards {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
  gap: 10px;
}
.trap-stat {
  background: var(--el-fill-color-light, var(--inset-bg));
  border: 1px solid var(--el-border-color-lighter, var(--line));
  border-radius: 8px;
  padding: 14px 16px;
}
.trap-stat .n {
  font-size: 1.35rem;
  font-weight: 650;
  font-variant-numeric: tabular-nums;
  line-height: 1.2;
  color: var(--el-text-color-primary);
}
.trap-stat .l {
  margin-top: 4px;
  font-size: 12px;
  color: var(--el-text-color-secondary, var(--muted));
}

/* 离线面板 */
.trap-offline {
  min-height: min(420px, calc(100vh - 220px));
  padding: 36px 40px;
  display: flex;
  flex-direction: column;
  justify-content: center;
}
.trap-offline h3 {
  margin: 0 0 12px;
  font-size: 1.35rem;
  font-weight: 650;
}
.trap-offline-err {
  margin: 0;
  word-break: break-all;
  font-size: 14px;
  line-height: 1.55;
  max-width: 72rem;
}
.trap-offline-steps {
  margin: 20px 0 0;
  padding-left: 1.35rem;
  color: var(--el-text-color-secondary, var(--muted));
  line-height: 1.85;
  font-size: 14px;
  max-width: 72rem;
}
.trap-offline-steps code {
  font-size: 13px;
}

/* 试推送 */
.trap-sim-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 12px 16px;
}
.trap-sim-out {
  margin-top: 12px;
  white-space: pre-wrap;
  font-size: 0.8rem;
  max-height: 240px;
  overflow: auto;
  background: var(--el-fill-color-light, var(--inset-bg));
  border: 1px solid var(--el-border-color-lighter, var(--line));
  border-radius: 6px;
  padding: 10px 12px;
  color: var(--el-text-color-primary);
}

/* 模态框 */
.modal-mask {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.5);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 2000;
}
.modal-dialog {
  background: var(--el-bg-color, var(--panel));
  border-radius: 12px;
  border: 1px solid var(--el-border-color, var(--line));
  max-width: 560px;
  width: 90%;
  max-height: 85vh;
  display: flex;
  flex-direction: column;
}
.modal-head {
  padding: 16px 20px;
  border-bottom: 1px solid var(--el-border-color-lighter, var(--line));
  display: flex;
  align-items: center;
  justify-content: space-between;
}
.modal-head h3 {
  margin: 0;
  font-size: 16px;
  font-weight: 600;
}
.modal-close {
  background: none;
  border: none;
  font-size: 22px;
  cursor: pointer;
  color: var(--el-text-color-secondary);
  padding: 0 4px;
  line-height: 1;
}
.modal-close:hover {
  color: var(--el-text-color-primary);
}
.modal-body {
  padding: 16px 20px;
  overflow: auto;
  flex: 1 1 auto;
}
.modal-actions {
  padding: 12px 20px;
  border-top: 1px solid var(--el-border-color-lighter, var(--line));
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}

/* 响应式 */
@media (max-width: 1100px) {
  .trap-split {
    grid-template-columns: 1fr;
  }
}
@media (max-width: 720px) {
  .trap-sim-grid {
    grid-template-columns: 1fr;
  }
}
</style>
