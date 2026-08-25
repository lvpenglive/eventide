<script setup lang="ts">
import { computed, markRaw, onMounted, reactive, ref } from 'vue'
import {
  ElAlert,
  ElBadge,
  ElButton,
  ElCard,
  ElCheckbox,
  ElCol,
  ElDescriptions,
  ElDescriptionsItem,
  ElDrawer,
  ElEmpty,
  ElForm,
  ElFormItem,
  ElInput,
  ElMessage,
  ElOption,
  ElRow,
  ElSelect,
  ElStatistic,
  ElTable,
  ElTableColumn,
  ElTabPane,
  ElTabs,
  ElTag,
} from 'element-plus'
import type { FormInstance, FormRules } from 'element-plus'
import {
  Refresh,
  Right,
  View,
} from '@element-plus/icons-vue'
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
// 多模块 Shim 绑定
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
  throw new Error(`[API shim] 模块未实现：${name}，请等待并行任务创建 @/api/trap`)
}

const _shimTrap: TrapApiShim = {
  getTrapHealth: () => Promise.reject(_unimpl('getTrapHealth')),
  getTrapStats: () => Promise.reject(_unimpl('getTrapStats')),
  getTrapRecent: () => Promise.reject(_unimpl('getTrapRecent')),
  listTrapInstances: () => Promise.reject(_unimpl('listTrapInstances')),
  simulateTrap: () => Promise.reject(_unimpl('simulateTrap')),
}
const _shimIngress: IngressApiShim = {
  listIngress: () => Promise.reject(_unimpl('listIngress (trap view 辅助)')),
}
const _shimKafka: KafkaApiShim = {
  describeKafkaGroup: () => Promise.reject(_unimpl('describeKafkaGroup (trap view 辅助)')),
}

// @ts-ignore 模块未创建时忽略
import * as _rawTrap from '@/api/trap'
// @ts-ignore 模块未创建时忽略
import * as _rawIngress from '@/api/ingress'
// @ts-ignore 模块未创建时忽略
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
const canRead = computed(() => auth.can('trap:read'))
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
function asVarb(r: unknown): TrapRecentVarbind { return r as TrapRecentVarbind }
function asKp(r: unknown): { partition: number; committed?: number; latest?: number; lag?: number; leader?: string; topic?: string } {
  return r as { partition: number; committed?: number; latest?: number; lag?: number; leader?: string; topic?: string }
}

type TagType = 'success' | 'warning' | 'info' | 'danger' | 'primary'
function sevMeta(s: string | undefined): { text: string; type: TagType; color: string } {
  switch (s) {
    case 'critical': return { text: '严重', type: 'danger', color: 'var(--crit)' }
    case 'error': return { text: '错误', type: 'danger', color: 'var(--crit-soft)' }
    case 'warning': return { text: '警告', type: 'warning', color: 'var(--warn)' }
    case 'info': return { text: '信息', type: 'info', color: 'var(--info)' }
    case 'ok': return { text: '正常', type: 'success', color: 'var(--ok)' }
    default: return { text: String(s || '-'), type: 'primary', color: 'var(--primary)' }
  }
}

function formatTime(s: string | undefined | null): string {
  if (!s) return '-'
  return s.replace('T', ' ').slice(0, 19)
}

// Tab 模型
const tabModel = ref('health')

// ============================================================
// 全局刷新：并行请求 4 接口
// ============================================================
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

// ============================================================
// Tab: Kafka 积压
// ============================================================
const lagLoading = ref(false)
const lagRoute = ref<IngressRoute | null>(null)
const lagGroup = ref<KafkaGroupState | null>(null)
const lagHint = ref('')

async function loadKafkaLag() {
  lagLoading.value = true
  lagHint.value = ''
  lagRoute.value = null
  lagGroup.value = null
  try {
    const routes = await listIngress()
    const found = routes.find(
      (r) => r.kind === 'snmptrap' || r.kind === 'kafka' || (r.options && (r.options.kind === 'snmptrap' || (r.name && r.name.toLowerCase().includes('trap'))))
    )
    if (!found) {
      lagHint.value = '未找到 snmptrap 对应的 Ingress 路由，请先在 Ingress 页面配置 kafka 类型路由并关联 trap topic'
      return
    }
    lagRoute.value = found
    const topic = found.options?.topic || found.options?.kafka_topic || (health.value?.kafka_topic)
    const groupId = found.options?.group_id || found.options?.consumer_group || 'eventide-trap-consumer'
    const brokers = found.options?.brokers || ''
    if (!brokers) {
      lagHint.value = '找到路由但缺少 brokers 配置，无法查询消费积压'
      return
    }
    lagGroup.value = await describeKafkaGroup({ brokers, group_id: groupId, topic: topic || undefined })
  } catch (e) {
    lagHint.value = errMsg(e, '查询 Kafka 积压失败')
  } finally {
    lagLoading.value = false
  }
}

// ============================================================
// Tab: 试推送
// ============================================================
const simForm = reactive<TrapSimulateReq>({
  ip: '127.0.0.1',
  trap_oid: '.1.3.6.1.6.3.1.1.5.3',
  alertname: '',
  severity: 'warning',
  dry_run: false,
})
const simFormRef = ref<FormInstance>()
const simFormRules: FormRules = {
  ip: [{ required: true, message: '请输入设备 IP', trigger: 'blur' }],
  trap_oid: [{ required: true, message: '请输入 Trap OID', trigger: 'blur' }],
}
const simLoading = ref(false)
const simResult = ref<TrapSimulateResp | null>(null)
const simError = ref('')

async function handleSimulate() {
  if (!canWrite.value) {
    ElMessage.warning('需要 trap:write 权限')
    return
  }
  if (!simFormRef.value) return
  try { await simFormRef.value.validate() } catch { return }
  simLoading.value = true
  simError.value = ''
  simResult.value = null
  try {
    simResult.value = await simulateTrap({
      ip: simForm.ip,
      trap_oid: simForm.trap_oid,
      alertname: simForm.alertname || null,
      severity: simForm.severity,
      dry_run: simForm.dry_run,
    })
    ElMessage.success(simResult.value?.kafka ? '已推送至 Kafka' : '试推送完成（dry-run）')
  } catch (e) {
    simError.value = errMsg(e, '试推送失败')
    ElMessage.error(simError.value)
  } finally {
    simLoading.value = false
  }
}

// ============================================================
// Tab: 最近事件 Drawer 详情
// ============================================================
const detailDrawer = ref(false)
const detailItem = ref<TrapRecentItem | null>(null)

function openDetail(r: TrapRecentItem) {
  detailItem.value = r
  detailDrawer.value = true
}

// ============================================================
// parse fail rate 计算
// ============================================================
const parseFailRate = computed<number>(() => {
  const s = stats.value
  if (!s || !s.received) return 0
  return Math.round((s.parse_err / s.received) * 10000) / 100
})

onMounted(() => { void loadAll() })
</script>

<template>
  <div class="page-wrap" style="padding: 16px 20px;">
    <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 14px;">
      <h2 style="font-size: 20px; margin: 0; color: var(--heading);">
        SNMP Trap
        <span style="font-size:13px;color:var(--el-text-color-secondary);margin-left:8px">服务状态 · Kafka 积压 · 集群心跳 · 试推送 · 最近事件</span>
      </h2>
      <el-button type="primary" :icon="Refresh" :loading="loadingAll" @click="loadAll">刷新</el-button>
    </div>

    <!-- 健康失败空态 -->
    <el-card
      v-if="!healthOk && !loadingAll"
      shadow="never"
      style="border: 1px solid var(--line); background: var(--panel-bg, var(--panel));"
    >
      <el-empty description="Trap 服务未连通" :image-size="120">
        <div style="text-align: left; max-width: 520px; margin: 0 auto; color: var(--el-text-color-secondary);">
          <el-alert v-if="healthErr" type="warning" :closable="false" style="margin-bottom: 10px;">
            <template #title>{{ healthErr }}</template>
          </el-alert>
          <p style="margin: 6px 0;"><strong>步骤 1：</strong>启动 Trap 微服务</p>
          <pre style="background: var(--inset-bg); padding: 10px; border-radius: 6px; font-family: var(--font-mono); font-size: 12.5px;">cargo run -p eventide-trap</pre>
          <p style="margin: 10px 0 6px;"><strong>步骤 2：</strong>eventide.toml 中配置 Trap API</p>
          <pre style="background: var(--inset-bg); padding: 10px; border-radius: 6px; font-family: var(--font-mono); font-size: 12.5px;">[trap]
api_url = "http://127.0.0.1:8128"</pre>
          <p style="margin: 10px 0 6px;"><strong>步骤 3：</strong>Ingress 页面为 Kafka 类型路由配置与 Trap 相同 Topic</p>
        </div>
      </el-empty>
    </el-card>

    <!-- Tab 化五大块 -->
    <el-tabs v-if="healthOk || loadingAll" v-model="tabModel">
      <!-- Tab 1: 服务状态 -->
      <el-tab-pane label="服务状态" name="health">
        <el-card
          shadow="never"
          style="border: 1px solid var(--line); background: var(--panel-bg, var(--panel));"
          v-loading="loadingAll"
        >
          <template v-if="health">
            <div style="display: flex; flex-wrap: wrap; gap: 12px; margin-bottom: 12px;">
              <el-tag type="success" effect="dark" size="large">
                <el-icon style="margin-right: 4px;"><Right /></el-icon>
                在线
              </el-tag>
              <el-tag type="primary">instance: {{ health.instance_id }}</el-tag>
              <el-tag type="info">UDP: {{ health.listen_udp }}</el-tag>
              <el-tag v-if="health.ha_vip" type="warning">VIP: {{ health.ha_vip }}</el-tag>
              <el-tag v-if="health.kafka_enabled" type="success" effect="plain">Kafka 启用</el-tag>
              <el-tag v-else type="danger" effect="plain">Kafka 未启用</el-tag>
              <el-tag v-if="health.kafka_topic" type="info" effect="plain">topic: {{ health.kafka_topic }}</el-tag>
              <el-tag type="danger" effect="plain">解析失败率: {{ parseFailRate }}%</el-tag>
              <el-tag v-if="health.version" size="small" type="info" effect="plain">v{{ health.version }}</el-tag>
            </div>

            <el-row :gutter="14">
              <el-col :span="4"><el-statistic title="接收" :value="stats?.received ?? 0" /></el-col>
              <el-col :span="4"><el-statistic title="解析成功" :value="stats?.parsed_ok ?? 0"><template #value><span style="color: var(--ok);">{{ stats?.parsed_ok ?? 0 }}</span></template></el-statistic></el-col>
              <el-col :span="4"><el-statistic title="解析失败" :value="stats?.parse_err ?? 0"><template #value><span style="color: var(--warn);">{{ stats?.parse_err ?? 0 }}</span></template></el-statistic></el-col>
              <el-col :span="4"><el-statistic title="Kafka 成功" :value="stats?.kafka_ok ?? 0"><template #value><span style="color: var(--ok);">{{ stats?.kafka_ok ?? 0 }}</span></template></el-statistic></el-col>
              <el-col :span="4"><el-statistic title="Kafka 失败" :value="stats?.kafka_err ?? 0"><template #value><span style="color: var(--crit);">{{ stats?.kafka_err ?? 0 }}</span></template></el-statistic></el-col>
              <el-col :span="4"><el-statistic title="试推送" :value="stats?.simulated ?? 0" /></el-col>
            </el-row>
          </template>
        </el-card>
      </el-tab-pane>

      <!-- Tab 2: Kafka 积压 -->
      <el-tab-pane label="Kafka 消费积压" name="lag">
        <el-card
          shadow="never"
          style="border: 1px solid var(--line); background: var(--panel-bg, var(--panel));"
          v-loading="lagLoading"
        >
          <template #header>
            <div style="display: flex; justify-content: space-between; align-items: center;">
              <span>
                <strong v-if="lagRoute">路由：{{ lagRoute.name }}（{{ lagRoute.kind }}）</strong>
                <strong v-else>Kafka 消费积压</strong>
              </span>
              <el-button size="small" :icon="Refresh" @click="loadKafkaLag">刷新</el-button>
            </div>
          </template>

          <el-alert v-if="lagHint" type="warning" :closable="false" style="margin-bottom: 12px;">
            <template #title>{{ lagHint }}</template>
          </el-alert>

          <template v-if="lagGroup">
            <el-row :gutter="14" style="margin-bottom: 10px;">
              <el-col :span="6">
                <div style="padding: 8px 12px; border: 1px solid var(--line); border-radius: 6px; background: var(--panel-2, var(--inset-bg));">
                  <div style="color: var(--el-text-color-secondary); font-size: 12.5px;">Group ID</div>
                  <div style="font-size: 18px; font-weight: 600; margin-top: 2px; font-family: var(--font-mono); overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">{{ lagGroup.group_id }}</div>
                </div>
              </el-col>
              <el-col :span="6">
                <div style="padding: 8px 12px; border: 1px solid var(--line); border-radius: 6px; background: var(--panel-2, var(--inset-bg));">
                  <div style="color: var(--el-text-color-secondary); font-size: 12.5px;">State</div>
                  <div style="font-size: 18px; font-weight: 600; margin-top: 2px;">{{ lagGroup.state || '—' }}</div>
                </div>
              </el-col>
              <el-col :span="6">
                <el-statistic title="总积压">
                  <template #value>
                    <span :style="{ color: (lagGroup.total_lag ?? 0) > 0 ? 'var(--warn)' : 'var(--ok)' }">
                      {{ lagGroup.total_lag ?? 0 }}
                    </span>
                  </template>
                </el-statistic>
              </el-col>
              <el-col :span="6"><el-statistic title="分区数" :value="lagGroup.partitions.length" /></el-col>
            </el-row>

            <el-table
              :data="lagGroup.partitions as unknown as Array<{partition:number;committed?:number;latest?:number;lag?:number;leader?:string;topic?:string}>"
              size="small"
              stripe
            >
              <el-table-column label="分区" prop="partition" width="90" align="center" />
              <el-table-column label="Committed" prop="committed" width="130" align="right" />
              <el-table-column label="Latest" prop="latest" width="130" align="right" />
              <el-table-column label="Lag" width="110" align="right">
                <template #default="{ row }">
                  <span :style="{ color: (asKp(row).lag ?? 0) > 0 ? 'var(--warn)' : 'var(--ok)' }">
                    {{ asKp(row).lag ?? 0 }}
                  </span>
                </template>
              </el-table-column>
              <el-table-column label="Leader" prop="leader" min-width="140" />
            </el-table>
          </template>
        </el-card>
      </el-tab-pane>

      <!-- Tab 3: 集群心跳 -->
      <el-tab-pane label="集群心跳" name="cluster">
        <el-card
          shadow="never"
          style="border: 1px solid var(--line); background: var(--panel-bg, var(--panel));"
          v-loading="loadingAll"
        >
          <template #header>
            <div style="display: flex; justify-content: space-between; align-items: center;">
              <strong>集群实例</strong>
              <div>
                <el-tag v-if="cluster?.redis" type="success" effect="dark">Redis 可用</el-tag>
                <el-tag v-else type="danger" effect="dark">Redis 不可用</el-tag>
              </div>
            </div>
          </template>
          <el-table
            :data="(cluster?.items ?? []) as unknown as TrapPeerItem[]"
            size="small"
            stripe
            empty-text="暂无实例心跳"
          >
            <el-table-column label="实例" min-width="170">
              <template #default="{ row }">
                <span style="font-family: var(--font-mono);">{{ asPeer(row).instance_id }}</span>
              </template>
            </el-table-column>
            <el-table-column label="UDP" width="140">
              <template #default="{ row }">{{ asPeer(row).listen_udp }}</template>
            </el-table-column>
            <el-table-column label="VIP" width="140">
              <template #default="{ row }">{{ asPeer(row).ha_vip || '—' }}</template>
            </el-table-column>
            <el-table-column label="接收" width="110" align="right">
              <template #default="{ row }">{{ asPeer(row).received ?? 0 }}</template>
            </el-table-column>
            <el-table-column label="心跳" width="180">
              <template #default="{ row }">
                <span style="color: var(--el-text-color-secondary);">{{ formatTime(asPeer(row).updated_at) }}</span>
              </template>
            </el-table-column>
          </el-table>
          <el-alert v-if="cluster?.hint" type="info" :closable="false" style="margin-top: 10px;">
            <template #title>{{ cluster.hint }}</template>
          </el-alert>
        </el-card>
      </el-tab-pane>

      <!-- Tab 4: 试推送 -->
      <el-tab-pane label="试推送" name="simulate">
        <el-card shadow="never" style="border: 1px solid var(--line); background: var(--panel-bg, var(--panel));">
          <el-form
            ref="simFormRef"
            :model="simForm"
            :rules="simFormRules"
            :disabled="!canWrite"
            label-width="110px"
            style="max-width: 680px;"
          >
            <el-form-item label="设备 IP" prop="ip">
              <el-input v-model="simForm.ip" placeholder="192.168.1.100" />
            </el-form-item>
            <el-form-item label="Trap OID" prop="trap_oid">
              <el-input v-model="simForm.trap_oid" placeholder=".1.3.6.1.6.3.1.1.5.3" style="font-family: var(--font-mono);" />
            </el-form-item>
            <el-form-item label="告警名">
              <el-input v-model="simForm.alertname" placeholder="可选，如 LinkDown" clearable />
            </el-form-item>
            <el-form-item label="级别">
              <el-select v-model="simForm.severity" style="width: 180px;">
                <el-option label="严重 (critical)" value="critical" />
                <el-option label="错误 (error)" value="error" />
                <el-option label="警告 (warning)" value="warning" />
                <el-option label="信息 (info)" value="info" />
                <el-option label="正常 (ok)" value="ok" />
              </el-select>
            </el-form-item>
            <el-form-item label="Dry Run">
              <el-checkbox v-model="simForm.dry_run">仅解析，不推送 Kafka</el-checkbox>
            </el-form-item>
            <el-form-item>
              <el-button
                type="primary"
                :disabled="!canWrite"
                :loading="simLoading"
                @click="handleSimulate"
              >
                试推送
              </el-button>
              <span v-if="!canWrite" style="margin-left: 10px; color: var(--muted);">（需要 trap:write 权限）</span>
            </el-form-item>
          </el-form>

          <el-alert v-if="simError" type="error" :closable="false" style="margin-top: 6px;">
            <template #title>{{ simError }}</template>
          </el-alert>

          <el-alert v-if="simResult" type="success" :closable="false" style="margin-top: 6px;">
            <template #title>试推送结果</template>
            <template #default>
              <pre style="margin: 0; white-space: pre-wrap; font-family: var(--font-mono); font-size: 12.5px;">{{ JSON.stringify(simResult, null, 2) }}</pre>
            </template>
          </el-alert>
        </el-card>
      </el-tab-pane>

      <!-- Tab 5: 最近事件 -->
      <el-tab-pane label="最近事件" name="recent">
        <el-card
          shadow="never"
          style="border: 1px solid var(--line); background: var(--panel-bg, var(--panel));"
          v-loading="loadingAll"
        >
          <el-table
            :data="recent as unknown as TrapRecentItem[]"
            size="small"
            stripe
            max-height="560"
            empty-text="暂无最近事件"
          >
            <el-table-column label="时间" width="170">
              <template #default="{ row }">
                <span style="color: var(--el-text-color-secondary); font-family: var(--font-mono); font-size: 12.5px;">
                  {{ formatTime(asRecent(row).at) }}
                </span>
              </template>
            </el-table-column>
            <el-table-column label="设备 IP" width="150">
              <template #default="{ row }">
                <span style="font-family: var(--font-mono);">{{ asRecent(row).peer }}</span>
              </template>
            </el-table-column>
            <el-table-column label="Trap OID" min-width="220">
              <template #default="{ row }">
                <span style="font-family: var(--font-mono); font-size: 12.5px;">{{ asRecent(row).trap_oid }}</span>
              </template>
            </el-table-column>
            <el-table-column label="名称" width="180">
              <template #default="{ row }">
                <span>{{ asRecent(row).alertname || '—' }}</span>
              </template>
            </el-table-column>
            <el-table-column label="Kafka" width="80" align="center">
              <template #default="{ row }">
                <el-tag v-if="asRecent(row).kafka === true" type="success" size="small">✓</el-tag>
                <el-tag v-else-if="asRecent(row).kafka === false" type="danger" size="small">✗</el-tag>
                <span v-else style="color: var(--muted);">—</span>
              </template>
            </el-table-column>
            <el-table-column label="级别" width="90" align="center">
              <template #default="{ row }">
                <el-tag
                  v-if="asRecent(row).severity"
                  size="small"
                  :type="sevMeta(asRecent(row).severity).type"
                  effect="dark"
                >{{ sevMeta(asRecent(row).severity).text }}</el-tag>
              </template>
            </el-table-column>
            <el-table-column label="操作" width="100" align="center" fixed="right">
              <template #default="{ row }">
                <el-button link type="primary" size="small" :icon="View" @click="openDetail(asRecent(row))">详情</el-button>
              </template>
            </el-table-column>
          </el-table>
        </el-card>
      </el-tab-pane>
    </el-tabs>

    <!-- 最近事件 Drawer -->
    <el-drawer v-model="detailDrawer" title="事件详情" size="520px">
      <template v-if="detailItem">
        <el-descriptions :column="1" border size="default" style="margin-bottom: 14px;">
          <el-descriptions-item label="时间">{{ formatTime(detailItem.at) }}</el-descriptions-item>
          <el-descriptions-item label="设备 IP"><span style="font-family: var(--font-mono);">{{ detailItem.peer }}</span></el-descriptions-item>
          <el-descriptions-item label="Trap OID"><span style="font-family: var(--font-mono);">{{ detailItem.trap_oid }}</span></el-descriptions-item>
          <el-descriptions-item label="告警名">{{ detailItem.alertname || '—' }}</el-descriptions-item>
          <el-descriptions-item label="级别">
            <el-tag v-if="detailItem.severity" :type="sevMeta(detailItem.severity).type" effect="dark">
              {{ sevMeta(detailItem.severity).text }}
            </el-tag>
            <span v-else>—</span>
          </el-descriptions-item>
          <el-descriptions-item label="Kafka">
            <el-tag v-if="detailItem.kafka === true" type="success">已写入</el-tag>
            <el-tag v-else-if="detailItem.kafka === false" type="danger">失败</el-tag>
            <span v-else style="color: var(--muted);">—</span>
          </el-descriptions-item>
        </el-descriptions>
        <h4 style="margin: 0 0 10px; color: var(--heading);">Varbinds</h4>
        <el-table
          v-if="detailItem.varbinds && detailItem.varbinds.length > 0"
          :data="detailItem.varbinds as unknown as TrapRecentVarbind[]"
          size="small"
          stripe
        >
          <el-table-column label="OID" min-width="200">
            <template #default="{ row }"><span style="font-family: var(--font-mono); font-size: 12.5px;">{{ asVarb(row).oid }}</span></template>
          </el-table-column>
          <el-table-column label="名称" min-width="120">
            <template #default="{ row }">{{ asVarb(row).name || '—' }}</template>
          </el-table-column>
          <el-table-column label="Value" min-width="140">
            <template #default="{ row }">
              <span style="font-family: var(--font-mono); font-size: 12.5px;">{{ asVarb(row).value ?? '' }}</span>
            </template>
          </el-table-column>
          <el-table-column label="Type" width="100">
            <template #default="{ row }">
              <el-tag size="small" type="info" effect="plain">{{ asVarb(row).type || '—' }}</el-tag>
            </template>
          </el-table-column>
        </el-table>
        <el-empty v-else description="此事件无 Varbinds" :image-size="60" />
      </template>
    </el-drawer>
  </div>
</template>
