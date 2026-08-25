<script setup lang="ts">
import { computed, markRaw, onMounted, reactive, ref } from 'vue'
import {
  ElBadge,
  ElButton,
  ElCard,
  ElCol,
  ElCollapse,
  ElCollapseItem,
  ElDescriptions,
  ElDescriptionsItem,
  ElDialog,
  ElDivider,
  ElEmpty,
  ElForm,
  ElFormItem,
  ElInput,
  ElInputNumber,
  ElMessage,
  ElMessageBox,
  ElOption,
  ElRow,
  ElSelect,
  ElStatistic,
  ElTable,
  ElTableColumn,
  ElTag,
  ElTooltip,
} from 'element-plus'
import type { FormInstance, FormRules } from 'element-plus'
import {
  Delete,
  Document,
  FolderOpened,
  Plus,
  Refresh,
  Right,
} from '@element-plus/icons-vue'
import { useAuthStore } from '@/stores/auth'

// ============================================================
// 本地类型定义
// ============================================================
interface KafkaTopicInfo {
  name: string
  partitions: number
  is_internal?: boolean
  compacted?: boolean
}

interface KafkaClusterProbe {
  ok: true
  brokers: string
  latency_ms: number
  topic_count: number
  topics: KafkaTopicInfo[]
  topic_found?: boolean
  partitions?: number
}

interface KafkaPartitionInfo {
  partition: number | string
  earliest: number
  latest: number
  lag_approx?: number
  leader?: number | string
  replicas?: number[]
  isr?: number[]
}

interface KafkaTopicDescribe {
  topic: string
  partitions: KafkaPartitionInfo[]
}

interface KafkaMessage {
  partition: number
  offset: number
  timestamp: string
  key?: string
  value?: string
  value_bytes: number
}

interface KafkaBrowseResult {
  partition: number
  earliest: number
  latest: number
  messages: KafkaMessage[]
}

interface KafkaGroupListed {
  group_id: string
  state?: string
  total_lag?: number
  source?: 'brokers' | 'routes'
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
  }>
  coordinator?: string
}

interface KafkaProduceResult {
  ok: true
  offset?: number
  partition?: number
  key?: string
}

interface KafkaApiShim {
  probeKafkaCluster(body: { brokers: string; topic?: string }): Promise<KafkaClusterProbe>
  listKafkaGroups(body: { brokers: string }): Promise<{ groups: KafkaGroupListed[] }>
  describeKafkaGroup(body: { brokers: string; group_id: string; topic?: string }): Promise<KafkaGroupState>
  describeKafkaTopic(body: { brokers: string; topic: string }): Promise<KafkaTopicDescribe>
  browseKafkaMessages(body: {
    brokers: string
    topic: string
    partition: number
    from: 'latest' | 'earliest' | 'offset'
    max?: number
    offset?: number
  }): Promise<KafkaBrowseResult>
  produceKafkaMessage(body: {
    brokers: string
    topic: string
    partition?: number
    key?: string
    value: string
  }): Promise<KafkaProduceResult>
  createKafkaTopic(body: {
    brokers: string
    topic: string
    partitions?: number
    replication_factor?: number
    configs?: Record<string, string>
  }): Promise<{ ok: true; topic: string }>
  deleteKafkaTopic(body: { brokers: string; topic: string }): Promise<{ ok: true }>
}

function _unimpl(name: string): never {
  throw new Error(`[API shim] 模块未实现：${name}，请等待并行任务创建 @/api/kafka 或 @/api/ingress`)
}

const _shimKafka: KafkaApiShim = {
  probeKafkaCluster: () => Promise.reject(_unimpl('probeKafkaCluster')),
  listKafkaGroups: () => Promise.reject(_unimpl('listKafkaGroups')),
  describeKafkaGroup: () => Promise.reject(_unimpl('describeKafkaGroup')),
  describeKafkaTopic: () => Promise.reject(_unimpl('describeKafkaTopic')),
  browseKafkaMessages: () => Promise.reject(_unimpl('browseKafkaMessages')),
  produceKafkaMessage: () => Promise.reject(_unimpl('produceKafkaMessage')),
  createKafkaTopic: () => Promise.reject(_unimpl('createKafkaTopic')),
  deleteKafkaTopic: () => Promise.reject(_unimpl('deleteKafkaTopic')),
}

// 优先 @/api/ingress（旧 kafka 函数），其次 @/api/kafka（B4 新版）
// @ts-ignore 模块未创建时忽略
import * as _rawIngress from '@/api/ingress'
// @ts-ignore 模块未创建时忽略
import * as _rawKafka from '@/api/kafka'
const _ingress = markRaw(_rawIngress as unknown as Record<string, unknown>)
const _kafka = markRaw(_rawKafka as unknown as Record<string, unknown>)

function _bindApi<S extends object>(shim: S, ...sources: Record<string, unknown>[]): S {
  const out = { ...(shim as unknown as object) } as S
  for (const src of sources) {
    if (!src || typeof src !== 'object') continue
    for (const key of Object.keys(shim as unknown as object)) {
      if (key in src && typeof (src as Record<string, unknown>)[key] === 'function') {
        ;(out as unknown as Record<string, unknown>)[key] = (src as Record<string, unknown>)[key]
      }
    }
  }
  return out
}

const _api = _bindApi<KafkaApiShim>(_shimKafka, _kafka, _ingress)
const {
  probeKafkaCluster,
  listKafkaGroups,
  describeKafkaGroup,
  describeKafkaTopic,
  browseKafkaMessages,
  produceKafkaMessage,
  createKafkaTopic,
  deleteKafkaTopic,
} = _api

// ============================================================
// 权限 & 工具
// ============================================================
const auth = useAuthStore()
const canRead = computed(() => auth.can('ingress:read'))
const canWrite = computed(() => auth.can('ingress:write'))

function errMsg(e: unknown, fallback: string): string {
  if (e && typeof e === 'object') {
    const m = (e as { message?: unknown }).message
    if (typeof m === 'string') return m
  }
  return fallback
}

function asTopic(r: unknown): KafkaTopicInfo { return r as KafkaTopicInfo }
function asGroup(r: unknown): KafkaGroupListed { return r as KafkaGroupListed }
function asPart(r: unknown): KafkaPartitionInfo { return r as KafkaPartitionInfo }
function asMsg(r: unknown): KafkaMessage { return r as KafkaMessage }
function asGp(r: unknown): { partition: number; committed?: number; latest?: number; lag?: number; leader?: string } {
  return r as { partition: number; committed?: number; latest?: number; lag?: number; leader?: string }
}

// ============================================================
// 连接表单 & 集群状态
// ============================================================
const LS_BROKERS = 'eventide_kafka_brokers'
const LS_TOPIC = 'eventide_kafka_topic'
const LS_FOCUS = 'eventide_kafka_focus_topic'

const brokers = ref(localStorage.getItem(LS_BROKERS) || '')
const defaultTopic = ref(localStorage.getItem(LS_TOPIC) || '')
const focusTopic = ref(localStorage.getItem(LS_FOCUS) || '')

const connecting = ref(false)
const probeResult = ref<KafkaClusterProbe | null>(null)

async function handleConnect() {
  const b = brokers.value.trim()
  if (!b) {
    ElMessage.warning('请先填写 brokers')
    return
  }
  localStorage.setItem(LS_BROKERS, b)
  if (defaultTopic.value.trim()) localStorage.setItem(LS_TOPIC, defaultTopic.value.trim())
  connecting.value = true
  try {
    probeResult.value = await probeKafkaCluster({ brokers: b, topic: defaultTopic.value.trim() || undefined })
    ElMessage.success(`探测成功：${probeResult.value.topic_count} 个 Topic，耗时 ${probeResult.value.latency_ms}ms`)
    if (focusTopic.value && !probeResult.value.topics.some(t => t.name === focusTopic.value)) {
      focusTopic.value = ''
    }
  } catch (e) {
    ElMessage.error(errMsg(e, '连接 Kafka 失败'))
    probeResult.value = null
  } finally {
    connecting.value = false
  }
}

// ============================================================
// Topics 面板
// ============================================================
const topics = computed<KafkaTopicInfo[]>(() => probeResult.value?.topics || [])

async function handleOpenTopic(t: KafkaTopicInfo) {
  focusTopic.value = t.name
  localStorage.setItem(LS_FOCUS, t.name)
  void loadTopicDetail()
}

async function handleDeleteTopic(t: KafkaTopicInfo) {
  try {
    await ElMessageBox.confirm(`确认删除 Topic「${t.name}」？此操作不可恢复。`, '删除 Topic', {
      type: 'warning',
      confirmButtonText: '删除',
      cancelButtonText: '取消',
    })
  } catch { return }
  try {
    await deleteKafkaTopic({ brokers: brokers.value.trim(), topic: t.name })
    ElMessage.success(`已删除 ${t.name}`)
    void handleConnect()
  } catch (e) {
    ElMessage.error(errMsg(e, '删除 Topic 失败'))
  }
}

// ============================================================
// Consumer Groups 面板
// ============================================================
const loadingGroups = ref(false)
const groups = ref<KafkaGroupListed[]>([])
const focusGroupId = ref('')

async function handleListGroups() {
  if (!brokers.value.trim()) {
    ElMessage.warning('请先连接集群')
    return
  }
  loadingGroups.value = true
  try {
    const res = await listKafkaGroups({ brokers: brokers.value.trim() })
    groups.value = res?.groups || []
  } catch (e) {
    ElMessage.error(errMsg(e, '列出消费组失败'))
    groups.value = []
  } finally {
    loadingGroups.value = false
  }
}

async function handleOpenGroup(g: KafkaGroupListed) {
  focusGroupId.value = g.group_id
  focusTopic.value = ''
  void loadGroupDetail()
}

// ============================================================
// 右栏模式
// ============================================================
type RightMode = 'topic' | 'group' | 'empty'
const rightMode = computed<RightMode>(() => {
  if (focusGroupId.value) return 'group'
  if (focusTopic.value) return 'topic'
  return 'empty'
})

// ============================================================
// Topic 详情 & 消息浏览
// ============================================================
const loadingDetail = ref(false)
const topicDetail = ref<KafkaTopicDescribe | null>(null)

const browseForm = reactive({
  partition: 0,
  from: 'latest' as 'latest' | 'earliest' | 'offset',
  max: 50,
  offset: 0,
})
const loadingBrowse = ref(false)
const browseResult = ref<KafkaBrowseResult | null>(null)

async function loadTopicDetail() {
  if (!focusTopic.value || !brokers.value.trim()) return
  loadingDetail.value = true
  try {
    topicDetail.value = await describeKafkaTopic({ brokers: brokers.value.trim(), topic: focusTopic.value })
    if (topicDetail.value.partitions.length > 0) {
      browseForm.partition = Number(topicDetail.value.partitions[0].partition)
    }
  } catch (e) {
    ElMessage.error(errMsg(e, '加载 Topic 详情失败'))
    topicDetail.value = null
  } finally {
    loadingDetail.value = false
  }
}

async function handleBrowse() {
  if (!focusTopic.value || !brokers.value.trim()) return
  loadingBrowse.value = true
  try {
    browseResult.value = await browseKafkaMessages({
      brokers: brokers.value.trim(),
      topic: focusTopic.value,
      partition: browseForm.partition,
      from: browseForm.from,
      max: browseForm.max,
      offset: browseForm.from === 'offset' ? browseForm.offset : undefined,
    })
  } catch (e) {
    ElMessage.error(errMsg(e, '浏览消息失败'))
    browseResult.value = null
  } finally {
    loadingBrowse.value = false
  }
}

// ============================================================
// 消费组详情
// ============================================================
const loadingGroupDetail = ref(false)
const groupDetail = ref<KafkaGroupState | null>(null)

async function loadGroupDetail() {
  if (!focusGroupId.value || !brokers.value.trim()) return
  loadingGroupDetail.value = true
  try {
    groupDetail.value = await describeKafkaGroup({
      brokers: brokers.value.trim(),
      group_id: focusGroupId.value,
      topic: focusTopic.value || undefined,
    })
  } catch (e) {
    ElMessage.error(errMsg(e, '加载消费组详情失败'))
    groupDetail.value = null
  } finally {
    loadingGroupDetail.value = false
  }
}

// ============================================================
// 试写消息
// ============================================================
const produceForm = reactive({
  partition: 0,
  key: '',
  value: '{\n  "hello": "eventide"\n}',
})
const producing = ref(false)

async function handleProduce() {
  if (!focusTopic.value || !brokers.value.trim()) {
    ElMessage.warning('请先选择 Topic 并连接')
    return
  }
  producing.value = true
  try {
    const res = await produceKafkaMessage({
      brokers: brokers.value.trim(),
      topic: focusTopic.value,
      partition: produceForm.partition,
      key: produceForm.key || undefined,
      value: produceForm.value,
    })
    ElMessage.success(`发送成功：分区 ${res.partition ?? produceForm.partition}，offset ${res.offset ?? '?'}`)
  } catch (e) {
    ElMessage.error(errMsg(e, '发送消息失败'))
  } finally {
    producing.value = false
  }
}

// ============================================================
// 创建 Topic Dialog
// ============================================================
const createDlgVisible = ref(false)
const createForm = reactive({ topic: '', partitions: 3, replication_factor: 1 })
const createFormRef = ref<FormInstance>()
const createFormRules: FormRules = {
  topic: [{ required: true, message: '请输入 Topic 名称', trigger: 'blur' }],
  partitions: [{ required: true, message: '请输入分区数', trigger: 'blur' }],
}
function openCreateDlg() {
  createForm.topic = ''
  createForm.partitions = 3
  createForm.replication_factor = 1
  createDlgVisible.value = true
}
async function handleCreateSubmit() {
  if (!createFormRef.value) return
  try { await createFormRef.value.validate() } catch { return }
  if (!brokers.value.trim()) {
    ElMessage.warning('请先填写 brokers')
    return
  }
  try {
    await createKafkaTopic({
      brokers: brokers.value.trim(),
      topic: createForm.topic,
      partitions: createForm.partitions,
      replication_factor: createForm.replication_factor,
    })
    ElMessage.success(`Topic ${createForm.topic} 已创建`)
    createDlgVisible.value = false
    void handleConnect()
  } catch (e) {
    ElMessage.error(errMsg(e, '创建 Topic 失败'))
  }
}

onMounted(() => {
  if (brokers.value.trim()) {
    // 自动连接（若 localStorage 已有值）
    void handleConnect()
  }
})
</script>

<template>
  <div class="page-wrap" style="padding: 16px 20px;">
    <h2 style="font-size: 20px; margin: 0 0 14px 0; color: var(--heading);">
      Kafka 接入
      <span style="font-size:13px;color:var(--el-text-color-secondary);margin-left:8px">集群连接 · Topic · 消费组 · 消息浏览 · 试写</span>
    </h2>

    <!-- 连接表单 -->
    <el-card shadow="never" style="margin-bottom: 14px; border: 1px solid var(--line); background: var(--panel-bg, var(--panel));">
      <el-form :inline="true" :model="{ brokers, defaultTopic }" label-width="90px" size="default">
        <el-form-item label="Brokers">
          <el-input
            v-model="brokers"
            placeholder="host1:9092,host2:9092"
            style="width: 360px;"
            clearable
          />
        </el-form-item>
        <el-form-item label="默认 Topic">
          <el-input
            v-model="defaultTopic"
            placeholder="可留空"
            style="width: 240px;"
            clearable
          />
        </el-form-item>
        <el-form-item>
          <el-button type="primary" :icon="Refresh" :loading="connecting" @click="handleConnect">
            连接并列出 Topic
          </el-button>
          <el-button :icon="Document" :loading="loadingGroups" @click="handleListGroups">
            列出消费组
          </el-button>
          <el-button
            v-if="canWrite"
            type="success"
            :icon="Plus"
            @click="openCreateDlg"
          >
            创建 Topic
          </el-button>
        </el-form-item>
      </el-form>

      <div v-if="probeResult" style="margin-top: 4px;">
        <el-tag type="success" effect="dark">已连接</el-tag>
        <span style="margin: 0 8px; color: var(--el-text-color-secondary);">延迟 {{ probeResult.latency_ms }} ms</span>
        <el-tag type="info">Topic 数 {{ probeResult.topic_count }}</el-tag>
        <template v-if="defaultTopic && probeResult.topic_found !== undefined">
          <el-tag v-if="probeResult.topic_found" type="success" style="margin-left: 8px;">
            默认 Topic 存在 · 分区 {{ probeResult.partitions ?? '?' }}
          </el-tag>
          <el-tag v-else type="danger" style="margin-left: 8px;">默认 Topic 不存在</el-tag>
        </template>
      </div>
    </el-card>

    <!-- 两栏布局 -->
    <el-row :gutter="14">
      <!-- 左栏 -->
      <el-col :span="9">
        <!-- Topics 面板 -->
        <el-card shadow="never" style="margin-bottom: 14px; border: 1px solid var(--line); background: var(--panel-bg, var(--panel));">
          <template #header>
            <div style="display: flex; justify-content: space-between; align-items: center;">
              <strong>Topics</strong>
              <el-tag size="small" type="info">{{ topics.length }}</el-tag>
            </div>
          </template>
          <el-table
            :data="topics as unknown as KafkaTopicInfo[]"
            size="small"
            stripe
            height="320"
            v-loading="connecting"
            empty-text="未连接或无 Topic"
          >
            <el-table-column label="名称" min-width="160">
              <template #default="{ row }">
                <div style="display: flex; align-items: center; gap: 6px;">
                  <el-icon><Right /></el-icon>
                  <span>{{ asTopic(row).name }}</span>
                  <el-tag v-if="asTopic(row).is_internal" size="small" type="warning" effect="plain">internal</el-tag>
                </div>
              </template>
            </el-table-column>
            <el-table-column label="分区" prop="partitions" width="70" align="center" />
            <el-table-column label="操作" width="150" align="center" fixed="right">
              <template #default="{ row }">
                <el-button
                  link
                  type="primary"
                  size="small"
                  :icon="FolderOpened"
                  @click="handleOpenTopic(asTopic(row))"
                >打开</el-button>
                <el-button
                  v-if="canWrite"
                  link
                  type="danger"
                  size="small"
                  :icon="Delete"
                  @click="handleDeleteTopic(asTopic(row))"
                >删除</el-button>
              </template>
            </el-table-column>
          </el-table>
        </el-card>

        <!-- Consumer Groups 面板 -->
        <el-card shadow="never" style="border: 1px solid var(--line); background: var(--panel-bg, var(--panel));">
          <template #header>
            <div style="display: flex; justify-content: space-between; align-items: center;">
              <strong>Consumer Groups</strong>
              <el-button link size="small" :icon="Refresh" @click="handleListGroups">刷新</el-button>
            </div>
          </template>
          <el-table
            :data="groups as unknown as KafkaGroupListed[]"
            size="small"
            stripe
            height="300"
            v-loading="loadingGroups"
            empty-text="尚未列出消费组"
          >
            <el-table-column label="Group ID" min-width="180">
              <template #default="{ row }">
                <el-link type="primary" @click="handleOpenGroup(asGroup(row))">
                  {{ asGroup(row).group_id }}
                </el-link>
              </template>
            </el-table-column>
            <el-table-column label="状态" width="90" align="center">
              <template #default="{ row }">
                <el-tag v-if="asGroup(row).state" size="small" :type="asGroup(row).state === 'Stable' ? 'success' : 'info'">
                  {{ asGroup(row).state }}
                </el-tag>
                <span v-else style="color: var(--el-text-color-secondary);">-</span>
              </template>
            </el-table-column>
            <el-table-column label="总 Lag" width="100" align="right">
              <template #default="{ row }">
                <span :style="{ color: (asGroup(row).total_lag ?? 0) > 0 ? 'var(--warn)' : 'inherit' }">
                  {{ asGroup(row).total_lag ?? '0' }}
                </span>
              </template>
            </el-table-column>
          </el-table>
        </el-card>
      </el-col>

      <!-- 右栏 -->
      <el-col :span="15">
        <!-- 空态 -->
        <el-empty v-if="rightMode === 'empty'" description="请在左侧打开 Topic 或消费组" :image-size="100" />

        <!-- Topic 详情 -->
        <div v-else-if="rightMode === 'topic'">
          <el-card shadow="never" style="margin-bottom: 14px; border: 1px solid var(--line); background: var(--panel-bg, var(--panel));">
            <template #header>
              <div style="display: flex; justify-content: space-between; align-items: center;">
                <strong>Topic：{{ focusTopic }}</strong>
                <el-button size="small" :icon="Refresh" @click="loadTopicDetail">刷新</el-button>
              </div>
            </template>

            <h4 style="margin: 4px 0 10px; color: var(--heading);">分区概览</h4>
            <el-table
              :data="(topicDetail?.partitions ?? []) as unknown as KafkaPartitionInfo[]"
              size="small"
              stripe
              v-loading="loadingDetail"
              max-height="220"
            >
              <el-table-column label="分区" prop="partition" width="80" align="center" />
              <el-table-column label="Earliest" prop="earliest" width="120" align="right" />
              <el-table-column label="Latest" prop="latest" width="120" align="right" />
              <el-table-column label="约数" width="120" align="right">
                <template #default="{ row }">
                  <span :style="{ color: (asPart(row).lag_approx ?? 0) > 10000 ? 'var(--warn)' : 'inherit' }">
                    {{ asPart(row).lag_approx ?? '0' }}
                  </span>
                </template>
              </el-table-column>
            </el-table>

            <el-divider />

            <!-- 浏览消息 -->
            <h4 style="margin: 4px 0 10px; color: var(--heading);">浏览消息</h4>
            <el-form :inline="true" :model="browseForm" size="small" label-width="70px">
              <el-form-item label="分区">
                <el-select v-model="browseForm.partition" style="width: 120px;">
                  <el-option
                    v-for="p in (topicDetail?.partitions ?? [])"
                    :key="String(asPart(p).partition)"
                    :label="String(asPart(p).partition)"
                    :value="Number(asPart(p).partition)"
                  />
                </el-select>
              </el-form-item>
              <el-form-item label="From">
                <el-select v-model="browseForm.from" style="width: 120px;">
                  <el-option label="Latest" value="latest" />
                  <el-option label="Earliest" value="earliest" />
                  <el-option label="Offset" value="offset" />
                </el-select>
              </el-form-item>
              <el-form-item label="Max 条">
                <el-input-number v-model="browseForm.max" :min="1" :max="500" style="width: 110px;" />
              </el-form-item>
              <el-form-item v-if="browseForm.from === 'offset'" label="Offset">
                <el-input-number v-model="browseForm.offset" :min="0" style="width: 140px;" />
              </el-form-item>
              <el-form-item>
                <el-button type="primary" :icon="Refresh" :loading="loadingBrowse" @click="handleBrowse">加载</el-button>
              </el-form-item>
            </el-form>

            <div v-if="browseResult && browseResult.messages.length > 0" style="margin-top: 10px;">
              <el-collapse>
                <el-collapse-item
                  v-for="(m, i) in browseResult.messages"
                  :key="i"
                  :name="i"
                >
                  <template #title>
                    <span style="display: inline-flex; align-items: center; gap: 10px; font-family: var(--font-mono); font-size: 12.5px;">
                      <el-tag size="small" type="primary">p{{ asMsg(m).partition }}@{{ asMsg(m).offset }}</el-tag>
                      <span style="color: var(--el-text-color-secondary);">{{ asMsg(m).timestamp }}</span>
                      <span style="color: var(--el-text-color-secondary);">{{ asMsg(m).value_bytes }} bytes</span>
                      <el-tag v-if="asMsg(m).key" size="small" effect="plain" type="warning">key: {{ asMsg(m).key }}</el-tag>
                    </span>
                  </template>
                  <el-descriptions :column="1" size="small" border>
                    <el-descriptions-item label="Value">
                      <pre style="margin: 0; white-space: pre-wrap; font-family: var(--font-mono); font-size: 12.5px; max-height: 260px; overflow: auto;">{{ asMsg(m).value ?? '' }}</pre>
                    </el-descriptions-item>
                  </el-descriptions>
                </el-collapse-item>
              </el-collapse>
            </div>
            <el-empty
              v-else-if="browseResult && browseResult.messages.length === 0"
              description="没有消息"
              :image-size="60"
              style="margin-top: 6px;"
            />

            <!-- 试写一条 -->
            <template v-if="canWrite">
              <el-divider />
              <h4 style="margin: 4px 0 10px; color: var(--heading);">试写一条</h4>
              <el-form :model="produceForm" size="small" label-width="80px">
                <el-row :gutter="10">
                  <el-col :span="8">
                    <el-form-item label="分区">
                      <el-select v-model="produceForm.partition" style="width: 100%;">
                        <el-option
                          v-for="p in (topicDetail?.partitions ?? [])"
                          :key="String(asPart(p).partition)"
                          :label="String(asPart(p).partition)"
                          :value="Number(asPart(p).partition)"
                        />
                      </el-select>
                    </el-form-item>
                  </el-col>
                  <el-col :span="16">
                    <el-form-item label="Key">
                      <el-input v-model="produceForm.key" placeholder="可选" clearable />
                    </el-form-item>
                  </el-col>
                </el-row>
                <el-form-item label="Value (JSON)">
                  <el-input
                    v-model="produceForm.value"
                    type="textarea"
                    :rows="5"
                    placeholder='{"key":"value"}'
                    style="font-family: var(--font-mono);"
                  />
                </el-form-item>
                <el-form-item>
                  <el-button type="warning" :loading="producing" @click="handleProduce">发送</el-button>
                </el-form-item>
              </el-form>
            </template>
          </el-card>
        </div>

        <!-- 消费组详情 -->
        <div v-else-if="rightMode === 'group'">
          <el-card shadow="never" style="border: 1px solid var(--line); background: var(--panel-bg, var(--panel));">
            <template #header>
              <div style="display: flex; justify-content: space-between; align-items: center;">
                <strong>消费组：{{ focusGroupId }}</strong>
                <el-button size="small" :icon="Refresh" @click="loadGroupDetail">刷新</el-button>
              </div>
            </template>
            <el-row v-if="groupDetail" :gutter="14" style="margin-bottom: 10px;">
              <el-col :span="8">
                <div style="padding: 8px 12px; border: 1px solid var(--line); border-radius: 6px; background: var(--panel-2, var(--inset-bg));">
                  <div style="color: var(--el-text-color-secondary); font-size: 12.5px;">状态</div>
                  <div style="font-size: 18px; font-weight: 600; margin-top: 2px;">{{ groupDetail.state || '—' }}</div>
                </div>
              </el-col>
              <el-col :span="8">
                <el-statistic title="总 Lag" :value="groupDetail.total_lag ?? 0">
                  <template #value>
                    <span :style="{ color: (groupDetail.total_lag ?? 0) > 0 ? 'var(--warn)' : 'var(--ok)' }">
                      {{ groupDetail.total_lag ?? 0 }}
                    </span>
                  </template>
                </el-statistic>
              </el-col>
              <el-col :span="8">
                <div style="padding: 8px 12px; border: 1px solid var(--line); border-radius: 6px; background: var(--panel-2, var(--inset-bg));">
                  <div style="color: var(--el-text-color-secondary); font-size: 12.5px;">Coordinator</div>
                  <div style="font-size: 18px; font-weight: 600; margin-top: 2px; font-family: var(--font-mono);">{{ groupDetail.coordinator || '—' }}</div>
                </div>
              </el-col>
            </el-row>
            <h4 style="margin: 10px 0 10px; color: var(--heading);">分区 Lag</h4>
            <el-table
              :data="(groupDetail?.partitions ?? []) as unknown as Array<{partition:number;committed?:number;latest?:number;lag?:number;leader?:string}>"
              size="small"
              stripe
              v-loading="loadingGroupDetail"
              max-height="460"
            >
              <el-table-column label="分区" prop="partition" width="80" align="center" />
              <el-table-column label="Committed" prop="committed" width="120" align="right" />
              <el-table-column label="Latest" prop="latest" width="120" align="right" />
              <el-table-column label="Lag" width="100" align="right">
                <template #default="{ row }">
                  <span :style="{ color: (asGp(row).lag ?? 0) > 0 ? 'var(--warn)' : 'var(--ok)' }">
                    {{ asGp(row).lag ?? 0 }}
                  </span>
                </template>
              </el-table-column>
              <el-table-column label="Leader" prop="leader" min-width="120" />
            </el-table>
          </el-card>
        </div>
      </el-col>
    </el-row>

    <!-- 创建 Topic Dialog -->
    <el-dialog v-model="createDlgVisible" title="创建 Kafka Topic" width="480px">
      <el-form ref="createFormRef" :model="createForm" :rules="createFormRules" label-width="130px">
        <el-form-item label="Topic 名称" prop="topic">
          <el-input v-model="createForm.topic" placeholder="如 events.trap.raw" />
        </el-form-item>
        <el-form-item label="分区数" prop="partitions">
          <el-input-number v-model="createForm.partitions" :min="1" :max="1000" />
        </el-form-item>
        <el-form-item label="复制因子">
          <el-input-number v-model="createForm.replication_factor" :min="1" :max="10" />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="createDlgVisible = false">取消</el-button>
        <el-button type="primary" @click="handleCreateSubmit">创建</el-button>
      </template>
    </el-dialog>
  </div>
</template>
