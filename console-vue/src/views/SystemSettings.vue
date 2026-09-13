<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue'
import {
  ElAffix,
  ElAlert,
  ElButton,
  ElCard,
  ElDescriptions,
  ElDescriptionsItem,
  ElDivider,
  ElForm,
  ElFormItem,
  ElIcon,
  ElInput,
  ElInputNumber,
  ElMessage,
  ElMessageBox,
  ElOption,
  ElRow,
  ElCol,
  ElSelect,
  ElSwitch,
  ElTag,
  ElTooltip,
  ElUpload,
} from 'element-plus'
import {
  Refresh,
  UploadFilled,
  ArrowDown,
  ArrowRight,
} from '@element-plus/icons-vue'
import {
  getAlertHistory,
  putAlertHistory,
  getTrapToken,
  putTrapToken,
  getLookupSync,
  putLookupSync,
  getStorm,
  putStorm,
  getUiBetaToggle,
  putUiBetaToggle,
  getLicense,
  getLicenseRequest,
  importLicense,
  clearLicense,
} from '@/api/settings'
import type {
  AlertHistorySettingsView,
  AlertHistorySettingsUpdate,
  TrapTokenSettingsView,
  LookupSyncSettingsView,
  StormSettingsView,
  StormSettingsUpdate,
  UiBetaSettingsView,
  LicenseSnapshot,
  LicenseRequest,
  LicenseImportBody,
} from '@/api/types'

// ============================================================
// 通用工具
// ============================================================
function errMsgOf(e: unknown, fallback: string): string {
  if (e && typeof e === 'object') {
    const anyE = e as { message?: unknown }
    if (typeof anyE.message === 'string') return anyE.message
  }
  return fallback
}

function formatDate(iso: string | undefined): string {
  if (!iso) return ''
  const d = new Date(iso)
  if (isNaN(d.getTime())) return iso
  const pad = (n: number) => String(n).padStart(2, '0')
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`
}
function copyText(text: string, okLabel = '已复制'): void {
  const done = () => ElMessage.success(okLabel)
  if (navigator.clipboard && window.isSecureContext) {
    navigator.clipboard.writeText(text).then(done).catch(() => fallbackCopy(text, done))
  } else {
    fallbackCopy(text, done)
  }
}
function fallbackCopy(text: string, onDone: () => void): void {
  const ta = document.createElement('textarea')
  ta.value = text
  ta.style.position = 'fixed'
  ta.style.opacity = '0'
  document.body.appendChild(ta)
  ta.select()
  try {
    document.execCommand('copy')
    onDone()
  } catch {
    ElMessage.error('复制失败，请手动复制')
  } finally {
    document.body.removeChild(ta)
  }
}

// ============================================================
// 区块收起/展开状态
// ============================================================
const sectionCollapsed = reactive({
  alertHistory: false,
  trapToken: false,
  lookupSync: false,
  storm: false,
  uiBeta: false,
  license: false,
})

function toggleSection(key: keyof typeof sectionCollapsed): void {
  sectionCollapsed[key] = !sectionCollapsed[key]
}

// ============================================================
// 分组 a) 历史事件 Alert History
// ============================================================
type SearchStore = 'mysql' | 'es'
const alertHistory = reactive({
  write_to_es: false,
  search_store: 'mysql' as SearchStore,
  es_configured: false,
  es_url: '',
  es_index: '',
  es_username: '',
  es_password: '',
})
const alertHistorySaving = ref(false)
const alertHistoryLoading = ref(false)

async function loadAlertHistory(force = false): Promise<void> {
  if (alertHistoryLoading.value && !force) return
  alertHistoryLoading.value = true
  try {
    const resp = await getAlertHistory()
    Object.assign(alertHistory, {
      write_to_es: !!resp.write_to_es,
      search_store: resp.search_store === 'es' ? 'es' : 'mysql',
      es_configured: !!resp.es_configured,
      es_url: resp.es_url ?? '',
      es_index: resp.es_index ?? '',
      es_username: resp.es_username ?? '',
      es_password: '',
    })
  } catch (e) {
    ElMessage.error(errMsgOf(e, '加载历史事件设置失败'))
  } finally {
    alertHistoryLoading.value = false
  }
}

async function saveAlertHistory(): Promise<boolean> {
  alertHistorySaving.value = true
  try {
    const body: AlertHistorySettingsUpdate = {
      write_to_es: alertHistory.write_to_es,
      search_store: alertHistory.search_store,
      es_url: alertHistory.es_url || undefined,
      es_index: alertHistory.es_index || undefined,
      es_username: alertHistory.es_username || undefined,
      es_password: alertHistory.es_password || undefined,
    }
    const resp = await putAlertHistory(body)
    alertHistory.es_configured = !!resp.es_configured
    alertHistory.es_password = ''
    ElMessage.success('历史事件设置已保存')
    return true
  } catch (e) {
    ElMessage.error(errMsgOf(e, '保存历史事件设置失败'))
    return false
  } finally {
    alertHistorySaving.value = false
  }
}

// ============================================================
// 分组 b) Trap HTTP API Token
// ============================================================
const trapToken = reactive<TrapTokenSettingsView>({
  configured: false,
  token_set: false,
  token_preview: '',
  source: 'empty',
} as TrapTokenSettingsView)
const trapTokenLoading = ref(false)
const trapTokenRegenerating = ref(false)
const trapTokenCleartext = ref<string | null>(null)

async function loadTrapToken(force = false): Promise<void> {
  if (trapTokenLoading.value && !force) return
  trapTokenLoading.value = true
  try {
    const resp = await getTrapToken()
    Object.assign(trapToken, resp)
    trapTokenCleartext.value = null
  } catch (e) {
    ElMessage.error(errMsgOf(e, '加载 Trap Token 失败'))
  } finally {
    trapTokenLoading.value = false
  }
}

async function handleRegenerateTrapToken(): Promise<boolean> {
  try {
    await ElMessageBox.confirm(
      '重新生成将立即使旧的 Trap Token 失效，确定继续？',
      '重新生成 Trap Token',
      { type: 'warning', confirmButtonText: '确定生成' }
    )
  } catch {
    return false
  }
  trapTokenRegenerating.value = true
  try {
    const resp = await putTrapToken({ regenerate: true })
    Object.assign(trapToken, resp)
    const cleartext = resp.token
    if (!cleartext) {
      ElMessage.warning('Token 已重新生成，但服务端未返回一次性明文，请联系管理员')
      return false
    }
    trapTokenCleartext.value = cleartext
    await ElMessageBox.alert(
      `新的 Trap HTTP API Token 已生成：\n\n${cleartext}\n\n请立即复制保存，此窗口关闭后将不再显示明文。`,
      'Trap Token 已重新生成',
      { confirmButtonText: '我已复制并关闭', dangerouslyUseHTMLString: false }
    )
    return true
  } catch (e) {
    ElMessage.error(errMsgOf(e, '重新生成 Trap Token 失败'))
    return false
  } finally {
    trapTokenRegenerating.value = false
  }
}

// ============================================================
// 分组 b2) Lookup Sync Token（MeridianOps 外表同步）
// ============================================================
const lookupSync = reactive<LookupSyncSettingsView>({
  configured: false,
  allowlist: [],
} as LookupSyncSettingsView)
const lookupSyncLoading = ref(false)
const lookupSyncRegenerating = ref(false)
const lookupSyncCleartext = ref<string | null>(null)
const lookupSyncAllowlistText = ref('')

async function loadLookupSync(force = false): Promise<void> {
  if (lookupSyncLoading.value && !force) return
  lookupSyncLoading.value = true
  try {
    const resp = await getLookupSync()
    Object.assign(lookupSync, resp)
    lookupSyncAllowlistText.value = (resp.allowlist || []).join('\n')
    lookupSyncCleartext.value = null
  } catch (e) {
    ElMessage.error(errMsgOf(e, '加载外表同步 Token 失败'))
  } finally {
    lookupSyncLoading.value = false
  }
}

async function handleRegenerateLookupSync(): Promise<boolean> {
  try {
    await ElMessageBox.confirm(
      '重新生成后，MeridianOps 需更新为新 Token。继续？',
      '重新生成外表同步 Token',
      { type: 'warning', confirmButtonText: '确定生成' },
    )
  } catch {
    return false
  }
  lookupSyncRegenerating.value = true
  try {
    const resp = await putLookupSync({ rotate: true })
    Object.assign(lookupSync, resp)
    lookupSyncCleartext.value = resp.token || null
    if (lookupSyncCleartext.value) {
      await ElMessageBox.alert(
        `新的外表同步 Token：\n\n${lookupSyncCleartext.value}\n\n请立即复制，关闭后不再显示明文。`,
        '外表同步 Token 已重新生成',
        { confirmButtonText: '我已复制并关闭' },
      )
    }
    return true
  } catch (e) {
    ElMessage.error(errMsgOf(e, '重新生成失败'))
    return false
  } finally {
    lookupSyncRegenerating.value = false
  }
}

async function saveLookupSyncAllowlist(): Promise<boolean> {
  const allowlist = lookupSyncAllowlistText.value
    .split(/[\n,]+/)
    .map((s) => s.trim())
    .filter(Boolean)
  try {
    const resp = await putLookupSync({ allowlist })
    Object.assign(lookupSync, resp)
    lookupSyncAllowlistText.value = (resp.allowlist || []).join('\n')
    ElMessage.success('allowlist 已保存')
    return true
  } catch (e) {
    ElMessage.error(errMsgOf(e, '保存 allowlist 失败'))
    return false
  }
}

// ============================================================
// 分组 c) 抗告警风暴 Storm（与旧版 / README §11.8 对齐）
// ============================================================
const stormAggModeOptions = [
  { label: 'head+summary（首条+摘要）', value: 'head+summary' },
  { label: 'summary_only（仅摘要）', value: 'summary_only' },
]
const storm = reactive<StormSettingsView>({
  throttle_enabled: true,
  min_interval_seconds: 60,
  max_per_window: 20,
  window_seconds: 60,
  throttle_key: 'fingerprint',
  resolve_max_per_window: null,
  aggregate_enabled: false,
  aggregate_window_seconds: 30,
  group_by: 'alertname',
  aggregate_mode: 'head+summary',
  aggregate_sample_labels: 'ip,instance,alertIp',
  aggregate_sample_limit: 10,
  ingress_max_inflight: 100,
  degrade_skip_notify: false,
  degrade_notify_per_sec: 50,
  source: 'toml',
})
const stormSaving = ref(false)
const stormLoading = ref(false)

function applyStormResp(resp: StormSettingsView): void {
  storm.throttle_enabled = !!resp.throttle_enabled
  storm.min_interval_seconds = Number(resp.min_interval_seconds) || 60
  storm.max_per_window = Number(resp.max_per_window) || 20
  storm.window_seconds = Number(resp.window_seconds) || 60
  storm.throttle_key = String(resp.throttle_key || 'fingerprint')
  storm.resolve_max_per_window =
    resp.resolve_max_per_window == null
      ? null
      : Number(resp.resolve_max_per_window)
  storm.aggregate_enabled = !!resp.aggregate_enabled
  storm.aggregate_window_seconds = Number(resp.aggregate_window_seconds) || 30
  storm.group_by = String(resp.group_by || 'alertname')
  storm.aggregate_mode = String(resp.aggregate_mode || 'head+summary')
  storm.aggregate_sample_labels = String(resp.aggregate_sample_labels || '')
  storm.aggregate_sample_limit = Number(resp.aggregate_sample_limit) || 10
  storm.ingress_max_inflight = Number(resp.ingress_max_inflight ?? 100)
  storm.degrade_skip_notify = !!resp.degrade_skip_notify
  storm.degrade_notify_per_sec = Number(resp.degrade_notify_per_sec) || 50
  storm.source = resp.source || 'toml'
}

async function loadStorm(force = false): Promise<void> {
  if (stormLoading.value && !force) return
  stormLoading.value = true
  try {
    const resp = await getStorm()
    applyStormResp(resp)
  } catch (e) {
    ElMessage.error(errMsgOf(e, '加载抗风暴设置失败'))
  } finally {
    stormLoading.value = false
  }
}

async function saveStorm(): Promise<boolean> {
  stormSaving.value = true
  try {
    const body: StormSettingsUpdate = {
      throttle_enabled: !!storm.throttle_enabled,
      min_interval_seconds: Number(storm.min_interval_seconds) || 60,
      max_per_window: Number(storm.max_per_window) || 20,
      window_seconds: Number(storm.window_seconds) || 60,
      throttle_key: String(storm.throttle_key || '').trim() || 'fingerprint',
      resolve_max_per_window:
        storm.resolve_max_per_window == null || Number.isNaN(Number(storm.resolve_max_per_window))
          ? null
          : Number(storm.resolve_max_per_window),
      aggregate_enabled: !!storm.aggregate_enabled,
      aggregate_window_seconds: Number(storm.aggregate_window_seconds) || 30,
      group_by: String(storm.group_by || '').trim() || 'alertname',
      aggregate_mode: storm.aggregate_mode || 'head+summary',
      aggregate_sample_labels: String(storm.aggregate_sample_labels || '').trim(),
      aggregate_sample_limit: Number(storm.aggregate_sample_limit) || 10,
      ingress_max_inflight: Number(storm.ingress_max_inflight ?? 100),
      degrade_skip_notify: !!storm.degrade_skip_notify,
      degrade_notify_per_sec: Number(storm.degrade_notify_per_sec) || 50,
    }
    const resp = await putStorm(body)
    applyStormResp(resp)
    ElMessage.success('抗风暴配置已保存并生效')
    return true
  } catch (e) {
    ElMessage.error(errMsgOf(e, '保存抗风暴设置失败'))
    return false
  } finally {
    stormSaving.value = false
  }
}

async function resetStorm(): Promise<void> {
  try {
    await ElMessageBox.confirm(
      '清除控制台覆盖，恢复为 eventide.toml [storm] 默认值？',
      '恢复 toml 默认',
      { type: 'warning' },
    )
  } catch {
    return
  }
  stormSaving.value = true
  try {
    const resp = await putStorm({ reset: true })
    applyStormResp(resp)
    ElMessage.success('已恢复 toml [storm]')
  } catch (e) {
    ElMessage.error(errMsgOf(e, '重置失败'))
  } finally {
    stormSaving.value = false
  }
}

// ============================================================
// 分组 d) UI Beta 开关
// ============================================================
const uiBeta = reactive<UiBetaSettingsView>({ enabled: false })
const uiBetaSaving = ref(false)
const uiBetaLoading = ref(false)

async function loadUiBeta(force = false): Promise<void> {
  if (uiBetaLoading.value && !force) return
  uiBetaLoading.value = true
  try {
    const resp = await getUiBetaToggle()
    Object.assign(uiBeta, resp)
  } catch (e) {
    ElMessage.error(errMsgOf(e, '加载 UI Beta 开关失败'))
  } finally {
    uiBetaLoading.value = false
  }
}

async function saveUiBeta(): Promise<boolean> {
  uiBetaSaving.value = true
  try {
    const resp = await putUiBetaToggle({ enabled: !!uiBeta.enabled })
    Object.assign(uiBeta, resp)
    ElMessage.success('UI Beta 开关已保存')
    return true
  } catch (e) {
    ElMessage.error(errMsgOf(e, '保存 UI Beta 开关失败'))
    return false
  } finally {
    uiBetaSaving.value = false
  }
}

// ============================================================
// 分组 e) 产品许可 License
// ============================================================
const license = reactive<LicenseSnapshot>({
  kind: 'trial',
  writable: true,
  install_id: '',
  has_license_token: false,
  expires_at: '',
  days_left: undefined,
  customer: '',
  edition: '',
})
const licenseLoading = ref(false)
const licenseImporting = ref(false)
const licenseRequesting = ref(false)
const licenseClearing = ref(false)
const licenseImportText = ref('')
const licenseRequestDialogVisible = ref(false)
const licenseRequestCustomer = ref('')
const licenseRequestResult = ref<LicenseRequest | null>(null)

async function loadLicense(force = false): Promise<void> {
  if (licenseLoading.value && !force) return
  licenseLoading.value = true
  try {
    const resp = await getLicense()
    Object.assign(license, resp)
  } catch (e) {
    ElMessage.error(errMsgOf(e, '加载许可证信息失败'))
  } finally {
    licenseLoading.value = false
  }
}

async function handleImportLicense(): Promise<void> {
  if (!licenseImportText.value.trim()) {
    ElMessage.error('请粘贴授权文件内容')
    return
  }
  licenseImporting.value = true
  try {
    const body: LicenseImportBody = { content: licenseImportText.value.trim() }
    const resp = await importLicense(body)
    Object.assign(license, resp)
    licenseImportText.value = ''
    ElMessage.success('许可证已导入')
  } catch (e) {
    ElMessage.error(errMsgOf(e, '导入许可证失败'))
  } finally {
    licenseImporting.value = false
  }
}

async function handleGenerateRequest(): Promise<void> {
  licenseRequesting.value = true
  try {
    const resp = await getLicenseRequest(licenseRequestCustomer.value || undefined)
    licenseRequestResult.value = resp
    licenseRequestDialogVisible.value = true
  } catch (e) {
    ElMessage.error(errMsgOf(e, '生成授权申请失败'))
  } finally {
    licenseRequesting.value = false
  }
}

async function handleClearLicense(): Promise<void> {
  try {
    await ElMessageBox.confirm(
      '确定要清除当前许可证吗？清除后将恢复为试用模式。',
      '清除许可证',
      { type: 'warning' }
    )
  } catch {
    return
  }
  licenseClearing.value = true
  try {
    const resp = await clearLicense()
    Object.assign(license, resp)
    ElMessage.success('许可证已清除')
  } catch (e) {
    ElMessage.error(errMsgOf(e, '清除许可证失败'))
  } finally {
    licenseClearing.value = false
  }
}

function handleLicenseFileUpload(file: File): void {
  const reader = new FileReader()
  reader.onload = () => {
    const content = reader.result as string
    licenseImportText.value = content
  }
  reader.readAsText(file)
}

const licenseKindTag = computed(() => {
  switch (license.kind) {
    case 'trial': return { text: '试用', type: 'warning' as const }
    case 'commercial': return { text: '商业', type: 'success' as const }
    case 'perpetual': return { text: '永久', type: 'primary' as const }
    default: return { text: license.kind, type: 'info' as const }
  }
})

// ============================================================
// 全局操作
// ============================================================
const globalSaving = ref(false)
async function saveAll(): Promise<void> {
  globalSaving.value = true
  try {
    const results = await Promise.allSettled([
      saveAlertHistory(),
      saveStorm(),
      saveUiBeta(),
    ])
    const failed = results.filter((r) => r.status === 'rejected' || (r.status === 'fulfilled' && !r.value))
    if (failed.length === 0) {
      ElMessage.success('全部设置已保存')
    } else {
      ElMessage.warning(`部分分组保存失败，请查看上方提示（失败 ${failed.length}/3）`)
    }
  } finally {
    globalSaving.value = false
  }
}

async function refreshAll(): Promise<void> {
  await Promise.all([
    loadAlertHistory(true),
    loadTrapToken(true),
    loadLookupSync(true),
    loadStorm(true),
    loadUiBeta(true),
    loadLicense(true),
  ])
  ElMessage.success('已刷新')
}

const anyLoading = computed(
  () =>
    alertHistoryLoading.value ||
    trapTokenLoading.value ||
    lookupSyncLoading.value ||
    stormLoading.value ||
    uiBetaLoading.value ||
    licenseLoading.value
)

function scrollToLicense(): void {
  const el = document.getElementById('license-section')
  if (el) {
    el.scrollIntoView({ behavior: 'smooth', block: 'start' })
  }
}

onMounted(() => {
  void Promise.all([
    loadAlertHistory(),
    loadTrapToken(),
    loadLookupSync(),
    loadStorm(),
    loadUiBeta(),
    loadLicense(),
  ])
})
</script>

<template>
  <div class="system-settings-page">
    <h2 class="page-title">系统设置</h2>
    <p class="page-subtitle">历史事件 / Trap Token / 抗告警风暴 / UI Beta / 产品许可</p>

    <ElAffix :offset="0" class="toolbar-affix">
      <div class="toolbar-bar">
        <ElButton :icon="Refresh" :loading="anyLoading" @click="refreshAll">刷新全部</ElButton>
        <ElButton type="primary" :icon="UploadFilled" :loading="globalSaving" @click="saveAll">
          全部保存
        </ElButton>
      </div>
    </ElAffix>

    <ElAlert
      type="info"
      show-icon
      class="top-alert"
      title="设置更改立即保存到系统数据库（KV），大部分无需重启服务。"
    />

    <!-- a) 历史事件 -->
    <ElCard shadow="never" class="section-card" v-loading="alertHistoryLoading">
      <template #header>
        <div class="card-header" @click="toggleSection('alertHistory')">
          <div class="card-header-left">
            <span class="collapse-icon" :class="{ 'is-collapsed': sectionCollapsed.alertHistory }">
              <el-icon :size="14"><ArrowDown v-if="!sectionCollapsed.alertHistory" /><ArrowRight v-else /></el-icon>
            </span>
            <span class="card-title">a) 历史事件（Alert History）</span>
          </div>
          <ElButton type="primary" :icon="UploadFilled" :loading="alertHistorySaving" @click.stop="saveAlertHistory">保存</ElButton>
        </div>
      </template>

      <div v-show="!sectionCollapsed.alertHistory">

      <ElDescriptions :column="1" border>
        <ElDescriptionsItem label="写入 ES">
          <ElSwitch v-model="alertHistory.write_to_es" />
        </ElDescriptionsItem>
        <ElDescriptionsItem label="查询存储 search_store">
          <ElSelect
            v-model="alertHistory.search_store"
            style="width: 240px"
            placeholder="选择查询存储"
          >
            <ElOption value="mysql" label="MySQL (alert_history 表)" />
            <ElOption value="es" label="Elasticsearch (按 es_index)" />
          </ElSelect>
        </ElDescriptionsItem>
        <ElDescriptionsItem label="ES 已配置">
          <ElTag
            :type="alertHistory.es_configured ? 'success' : 'info'"
            size="large"
            effect="dark"
            style="font-weight: 600; padding: 6px 14px;"
          >
            {{ alertHistory.es_configured ? '已配置' : '未配置' }}
          </ElTag>
        </ElDescriptionsItem>
        <ElDescriptionsItem label="ES URL es_url">
          <ElInput
            v-model="alertHistory.es_url"
            placeholder="如 https://es.internal:9200"
            clearable
            style="max-width: 480px"
          />
        </ElDescriptionsItem>
        <ElDescriptionsItem label="ES Index es_index">
          <ElInput
            v-model="alertHistory.es_index"
            placeholder="如 eventide-alerts-*"
            clearable
            style="max-width: 480px"
          />
        </ElDescriptionsItem>
        <ElDescriptionsItem label="ES 用户名 es_username">
          <ElInput
            v-model="alertHistory.es_username"
            placeholder="选填"
            clearable
            style="max-width: 360px"
          />
        </ElDescriptionsItem>
        <ElDescriptionsItem label="ES 密码 es_password">
          <ElInput
            v-model="alertHistory.es_password"
            type="password"
            show-password
            placeholder="留空 = 保持现有密码"
            style="max-width: 360px"
          />
        </ElDescriptionsItem>
      </ElDescriptions>
      </div>
    </ElCard>

    <!-- b) Trap HTTP API Token -->
    <ElCard shadow="never" class="section-card" v-loading="trapTokenLoading">
      <template #header>
        <div class="card-header" @click="toggleSection('trapToken')">
          <div class="card-header-left">
            <span class="collapse-icon" :class="{ 'is-collapsed': sectionCollapsed.trapToken }">
              <el-icon :size="14"><ArrowDown v-if="!sectionCollapsed.trapToken" /><ArrowRight v-else /></el-icon>
            </span>
            <span class="card-title">b) Trap HTTP API Token</span>
          </div>
          <ElButton
            type="warning"
            :loading="trapTokenRegenerating"
            @click.stop="handleRegenerateTrapToken"
          >
            重新生成
          </ElButton>
        </div>
      </template>

      <div v-show="!sectionCollapsed.trapToken">
      <ElDescriptions :column="1" border>
        <ElDescriptionsItem label="Token 状态">
          <ElTag
            :type="trapToken.configured ? 'success' : 'danger'"
            size="large"
            effect="dark"
            style="font-weight: 600; padding: 6px 14px;"
          >
            {{ trapToken.configured ? '已设置' : '未设置' }}
          </ElTag>
        </ElDescriptionsItem>
        <ElDescriptionsItem label="Token 前缀">
          <span v-if="trapToken.token_preview" class="mono">
            {{ trapToken.token_preview }}
          </span>
          <span v-else style="color: var(--el-text-color-secondary)">
            （未返回）
          </span>
        </ElDescriptionsItem>
        <ElDescriptionsItem v-if="trapToken.source" label="配置来源">
          {{ trapToken.source === 'runtime' ? '运行时' : trapToken.source === 'toml' ? 'eventide.toml' : '未配置' }}
        </ElDescriptionsItem>
        <ElDescriptionsItem v-if="trapTokenCleartext" label="一次性明文（仅此次显示）">
          <div class="trap-cleartext">
            <code class="mono">{{ trapTokenCleartext }}</code>
            <ElButton size="small" style="margin-left: 12px" @click="copyText(trapTokenCleartext!, 'Token 已复制')">
              复制
            </ElButton>
          </div>
        </ElDescriptionsItem>
      </ElDescriptions>
      </div>
    </ElCard>

    <!-- b2) Lookup Sync Token -->
    <ElCard shadow="never" class="section-card" v-loading="lookupSyncLoading">
      <template #header>
        <div class="card-header" @click="toggleSection('lookupSync')">
          <div class="card-header-left">
            <span class="collapse-icon" :class="{ 'is-collapsed': sectionCollapsed.lookupSync }">
              <el-icon :size="14"><ArrowDown v-if="!sectionCollapsed.lookupSync" /><ArrowRight v-else /></el-icon>
            </span>
            <span class="card-title">b2) 外表同步 Token（MeridianOps）</span>
          </div>
          <ElButton
            type="warning"
            :loading="lookupSyncRegenerating"
            @click.stop="handleRegenerateLookupSync"
          >
            重新生成
          </ElButton>
        </div>
      </template>

      <div v-show="!sectionCollapsed.lookupSync">
        <ElAlert
          type="info"
          :closable="false"
          show-icon
          style="margin-bottom: 12px"
          title="MeridianOps 使用 Bearer sync token 调用 PUT /api/lookups/{id}/rows。allowlist 为空时仅允许 external_sync=true 的外表；非空则只允许名单内 id。"
        />
        <ElDescriptions :column="1" border>
          <ElDescriptionsItem label="Token 状态">
            <ElTag
              :type="lookupSync.configured ? 'success' : 'danger'"
              size="large"
              effect="dark"
              style="font-weight: 600; padding: 6px 14px;"
            >
              {{ lookupSync.configured ? '已设置' : '未设置' }}
            </ElTag>
          </ElDescriptionsItem>
          <ElDescriptionsItem label="Token 预览">
            <span v-if="lookupSync.token_preview" class="mono">{{ lookupSync.token_preview }}</span>
            <span v-else style="color: var(--el-text-color-secondary)">（未配置）</span>
          </ElDescriptionsItem>
          <ElDescriptionsItem v-if="lookupSync.source" label="配置来源">
            {{ lookupSync.source === 'runtime' ? '运行时' : lookupSync.source === 'toml' ? 'eventide.toml' : '未配置' }}
          </ElDescriptionsItem>
          <ElDescriptionsItem v-if="lookupSyncCleartext" label="一次性明文（仅此次显示）">
            <code class="mono">{{ lookupSyncCleartext }}</code>
            <ElButton size="small" style="margin-left: 12px" @click="copyText(lookupSyncCleartext!, 'Token 已复制')">
              复制
            </ElButton>
          </ElDescriptionsItem>
          <ElDescriptionsItem label="Allowlist（每行一个 lookup id，可空）">
            <ElInput
              v-model="lookupSyncAllowlistText"
              type="textarea"
              :rows="3"
              placeholder="留空 = 仅 external_sync 外表"
            />
            <ElButton type="primary" style="margin-top: 8px" @click="saveLookupSyncAllowlist">保存 allowlist</ElButton>
          </ElDescriptionsItem>
        </ElDescriptions>
      </div>
    </ElCard>

    <!-- c) 抗告警风暴 -->
    <ElCard shadow="never" class="section-card" v-loading="stormLoading">
      <template #header>
        <div class="card-header" @click="toggleSection('storm')">
          <div class="card-header-left">
            <span class="collapse-icon" :class="{ 'is-collapsed': sectionCollapsed.storm }">
              <el-icon :size="14"><ArrowDown v-if="!sectionCollapsed.storm" /><ArrowRight v-else /></el-icon>
            </span>
            <span class="card-title">c) 抗告警风暴</span>
          </div>
          <div class="card-actions">
            <ElButton :loading="stormSaving" @click.stop="resetStorm">恢复 toml 默认</ElButton>
            <ElButton type="primary" :icon="UploadFilled" :loading="stormSaving" @click.stop="saveStorm">保存</ElButton>
          </div>
        </div>
      </template>

      <div v-show="!sectionCollapsed.storm">
        <ElAlert
          type="info"
          :closable="false"
          show-icon
          style="margin-bottom: 14px"
          title="节流 / 聚合 / 接入削峰。保存后当前进程立即生效（写入 MySQL；多实例其他节点需重启或各自保存一次）。"
        >
          <template #default>
            来源 <code>{{ storm.source || 'toml' }}</code>。详解见 README §11.8。
          </template>
        </ElAlert>

        <ElForm label-position="top" class="storm-form">
          <ElDivider content-position="left">通知节流（P0）</ElDivider>
          <ElFormItem label="启用节流">
            <ElSwitch v-model="storm.throttle_enabled" />
          </ElFormItem>
          <ElRow :gutter="16">
            <ElCol :xs="24" :sm="12" :md="6">
              <ElFormItem label="最短间隔（秒）">
                <ElInputNumber
                  v-model="storm.min_interval_seconds"
                  :min="1"
                  controls-position="right"
                  style="width: 100%"
                />
              </ElFormItem>
            </ElCol>
            <ElCol :xs="24" :sm="12" :md="6">
              <ElFormItem label="窗口秒数">
                <ElInputNumber
                  v-model="storm.window_seconds"
                  :min="1"
                  controls-position="right"
                  style="width: 100%"
                />
              </ElFormItem>
            </ElCol>
            <ElCol :xs="24" :sm="12" :md="6">
              <ElFormItem label="窗口内最多发送">
                <ElInputNumber
                  v-model="storm.max_per_window"
                  :min="1"
                  controls-position="right"
                  style="width: 100%"
                />
              </ElFormItem>
            </ElCol>
            <ElCol :xs="24" :sm="12" :md="6">
              <ElFormItem label="恢复通知上限（可选）">
                <ElInputNumber
                  v-model="storm.resolve_max_per_window"
                  :min="1"
                  controls-position="right"
                  style="width: 100%"
                  placeholder="默认=最多发送"
                />
              </ElFormItem>
            </ElCol>
          </ElRow>
          <ElFormItem label="节流键 throttle_key">
            <ElInput
              v-model="storm.throttle_key"
              placeholder="fingerprint 或 labels:alertname"
              clearable
              style="max-width: 480px"
            />
            <div class="form-hint">常用：fingerprint；按告警名聚合节流可用 labels:alertname。</div>
          </ElFormItem>

          <ElDivider content-position="left">时间窗聚合（P1）</ElDivider>
          <ElFormItem label="启用聚合">
            <ElSwitch v-model="storm.aggregate_enabled" />
          </ElFormItem>
          <ElRow :gutter="16">
            <ElCol :xs="24" :sm="12">
              <ElFormItem label="聚合窗口（秒）">
                <ElInputNumber
                  v-model="storm.aggregate_window_seconds"
                  :min="1"
                  controls-position="right"
                  style="width: 100%"
                />
              </ElFormItem>
            </ElCol>
            <ElCol :xs="24" :sm="12">
              <ElFormItem label="采样条数上限">
                <ElInputNumber
                  v-model="storm.aggregate_sample_limit"
                  :min="1"
                  controls-position="right"
                  style="width: 100%"
                />
              </ElFormItem>
            </ElCol>
          </ElRow>
          <ElFormItem label="分组 group_by">
            <ElInput
              v-model="storm.group_by"
              placeholder="alertname 或 alertname,namespace"
              clearable
              style="max-width: 480px"
            />
          </ElFormItem>
          <ElFormItem label="模式">
            <ElSelect v-model="storm.aggregate_mode" style="max-width: 360px">
              <ElOption
                v-for="o in stormAggModeOptions"
                :key="o.value"
                :label="o.label"
                :value="o.value"
              />
            </ElSelect>
          </ElFormItem>
          <ElFormItem label="摘要采样标签">
            <ElInput
              v-model="storm.aggregate_sample_labels"
              placeholder="ip,instance,alertIp"
              clearable
              style="max-width: 480px"
            />
          </ElFormItem>

          <ElDivider content-position="left">接入削峰（P2）</ElDivider>
          <ElRow :gutter="16">
            <ElCol :xs="24" :sm="12">
              <ElFormItem label="最大并发 inflight（0=不限）">
                <ElInputNumber
                  v-model="storm.ingress_max_inflight"
                  :min="0"
                  controls-position="right"
                  style="width: 100%"
                />
              </ElFormItem>
            </ElCol>
            <ElCol :xs="24" :sm="12">
              <ElFormItem label="降级通知速率阈值 / 秒">
                <ElInputNumber
                  v-model="storm.degrade_notify_per_sec"
                  :min="1"
                  controls-position="right"
                  style="width: 100%"
                />
              </ElFormItem>
            </ElCol>
          </ElRow>
          <ElFormItem label="高压策略">
            <ElSwitch v-model="storm.degrade_skip_notify" active-text="高压时只落库不发通知" />
            <div class="form-hint">对应 degrade_skip_notify；适合 Webhook 被压测打爆时保入库。</div>
          </ElFormItem>
        </ElForm>
      </div>
    </ElCard>

    <!-- d) UI Beta 开关 -->
    <ElCard shadow="never" class="section-card" v-loading="uiBetaLoading">
      <template #header>
        <div class="card-header" @click="toggleSection('uiBeta')">
          <div class="card-header-left">
            <span class="collapse-icon" :class="{ 'is-collapsed': sectionCollapsed.uiBeta }">
              <el-icon :size="14"><ArrowDown v-if="!sectionCollapsed.uiBeta" /><ArrowRight v-else /></el-icon>
            </span>
            <span class="card-title">d) UI Beta 开关</span>
          </div>
          <ElButton type="primary" :icon="UploadFilled" :loading="uiBetaSaving" @click.stop="saveUiBeta">保存</ElButton>
        </div>
      </template>

      <div v-show="!sectionCollapsed.uiBeta">
      <ElDescriptions :column="1" border>
        <ElDescriptionsItem label="启用 v2 影子前端">
          <ElSwitch v-model="uiBeta.enabled" />
          <span v-if="uiBeta.v2_available === false" style="margin-left: 12px; color: var(--el-color-warning)">
            （当前后端未启用 v2 影子前端资源，v2_available=false）
          </span>
        </ElDescriptionsItem>
        <ElDescriptionsItem label="说明">
          <div class="beta-note">
            启用后后端会写 Cookie <code class="mono">eventide_use_v2=1</code>，下次访问自动进入 v2 影子前端。
          </div>
        </ElDescriptionsItem>
      </ElDescriptions>
      </div>
    </ElCard>

    <!-- e) 产品许可 License -->
    <ElCard id="license-section" shadow="never" class="section-card" v-loading="licenseLoading">
      <template #header>
        <div class="card-header" @click="toggleSection('license')">
          <div class="card-header-left">
            <span class="collapse-icon" :class="{ 'is-collapsed': sectionCollapsed.license }">
              <el-icon :size="14"><ArrowDown v-if="!sectionCollapsed.license" /><ArrowRight v-else /></el-icon>
            </span>
            <span class="card-title">e) 产品许可（License）</span>
          </div>
        </div>
      </template>

      <div v-show="!sectionCollapsed.license">
      <ElDescriptions :column="2" border>
        <ElDescriptionsItem label="授权类型">
          <ElTag
            :type="licenseKindTag.type"
            size="large"
            effect="dark"
            style="font-weight: 600; padding: 6px 14px;"
          >
            {{ licenseKindTag.text }}
          </ElTag>
        </ElDescriptionsItem>
        <ElDescriptionsItem label="可写状态">
          <ElTag
            :type="license.writable ? 'success' : 'danger'"
            size="large"
            effect="dark"
            style="font-weight: 600; padding: 6px 14px;"
          >
            {{ license.writable ? '可写' : '只读' }}
          </ElTag>
        </ElDescriptionsItem>
        <ElDescriptionsItem label="客户名称">
          {{ license.customer || '-' }}
        </ElDescriptionsItem>
        <ElDescriptionsItem label="版本/规格">
          {{ license.edition || '-' }}
        </ElDescriptionsItem>
        <ElDescriptionsItem label="有效期至">
          {{ (license.expires_at && license.expires_at.trim()) ? formatDate(license.expires_at) : '永久' }}
        </ElDescriptionsItem>
        <ElDescriptionsItem label="剩余天数">
          <span v-if="license.days_left !== undefined && license.days_left >= 0">
            {{ license.days_left }} 天
          </span>
          <span v-else>永久</span>
        </ElDescriptionsItem>
        <ElDescriptionsItem label="安装 ID" :span="2">
          <span class="mono">{{ license.install_id || '-' }}</span>
        </ElDescriptionsItem>
      </ElDescriptions>

      <div class="license-actions">
        <ElButton type="primary" @click="handleGenerateRequest" :loading="licenseRequesting">
          导出授权申请
        </ElButton>
        <ElTooltip
          :content="license.kind === 'trial' ? '试用许可证不可清除' : '清除当前许可证'"
          :disabled="license.kind !== 'trial'"
          placement="top"
        >
          <ElButton
            type="danger"
            @click="handleClearLicense"
            :loading="licenseClearing"
            :disabled="license.kind === 'trial'"
            style="font-weight: 500;"
          >
            清除许可证
          </ElButton>
        </ElTooltip>
      </div>

      <ElDivider content-position="left">导入授权</ElDivider>

      <ElForm label-position="top">
        <ElFormItem label="授权文件内容">
          <ElInput
            v-model="licenseImportText"
            type="textarea"
            :rows="4"
            placeholder="粘贴授权文件 JSON 内容"
          />
        </ElFormItem>
        <ElFormItem>
          <ElUpload
            :auto-upload="false"
            :show-file-list="false"
            :on-change="(file: any) => handleLicenseFileUpload(file.raw)"
            accept=".json,.txt"
          >
            <ElButton>选择文件</ElButton>
          </ElUpload>
          <ElButton
            type="primary"
            :icon="UploadFilled"
            :loading="licenseImporting"
            @click="handleImportLicense"
            style="margin-left: 12px"
          >
            导入
          </ElButton>
        </ElFormItem>
      </ElForm>
      </div>
    </ElCard>

    <!-- 授权申请对话框 -->
    <ElDialog
      v-model="licenseRequestDialogVisible"
      title="导出授权申请"
      width="600px"
    >
      <ElForm label-position="top">
        <ElFormItem label="客户名称（可选）">
          <ElInput v-model="licenseRequestCustomer" placeholder="填写客户名称，将写入授权文件" />
        </ElFormItem>
      </ElForm>
      <div v-if="licenseRequestResult" class="license-request-result">
        <ElDescriptions :column="1" border>
          <ElDescriptionsItem label="安装 ID">
            <span class="mono">{{ licenseRequestResult.install_id }}</span>
          </ElDescriptionsItem>
          <ElDescriptionsItem label="产品">
            {{ licenseRequestResult.product }}
          </ElDescriptionsItem>
          <ElDescriptionsItem label="版本">
            {{ licenseRequestResult.version }}
          </ElDescriptionsItem>
          <ElDescriptionsItem label="生成时间">
            {{ licenseRequestResult.generated_at }}
          </ElDescriptionsItem>
        </ElDescriptions>
        <div style="margin-top: 12px; text-align: right;">
          <ElButton type="primary" @click="copyText(JSON.stringify(licenseRequestResult, null, 2), '授权申请已复制')">
            复制全部
          </ElButton>
        </div>
      </div>
      <template #footer>
        <ElButton @click="licenseRequestDialogVisible = false">关闭</ElButton>
        <ElButton type="primary" :loading="licenseRequesting" @click="handleGenerateRequest">生成</ElButton>
      </template>
    </ElDialog>
  </div>
</template>

<style scoped>
.system-settings-page {
  padding: 8px 4px 24px;
}
.page-title {
  margin: 4px 0 4px;
}
.page-subtitle {
  color: var(--el-text-color-secondary);
  margin: 0 0 16px;
}
.toolbar-affix {
  z-index: 10;
}
.toolbar-bar {
  display: flex;
  gap: 8px;
  padding: 10px 12px;
  background: var(--el-bg-color-page);
  border-radius: 6px;
  margin-bottom: 16px;
}
.top-alert {
  margin-bottom: 16px;
}
.section-card {
  margin-bottom: 16px;
}
.card-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  cursor: pointer;
  user-select: none;
  padding: 0;
}
.card-header:hover .card-title {
  color: var(--el-color-primary);
}
.card-header:hover .collapse-icon {
  color: var(--el-color-primary);
}
.card-header-left {
  display: flex;
  align-items: center;
  gap: 8px;
}
.collapse-icon {
  font-size: 12px;
  color: var(--el-text-color-secondary);
  transition: transform 0.2s ease;
  display: inline-flex !important;
  align-items: center;
  width: 16px;
  height: 16px;
  line-height: 1;
}
.collapse-icon.is-collapsed {
  transform: rotate(-90deg);
}
.card-actions {
  display: flex;
  gap: 8px;
}
.card-title {
  font-weight: 600;
  font-size: 15px;
}
.storm-form {
  padding: 4px 4px 0;
}
.form-hint {
  margin-top: 6px;
  color: var(--el-text-color-secondary);
  font-size: 12px;
}
.mono {
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
}
.trap-cleartext {
  display: flex;
  align-items: center;
}
.trap-cleartext code {
  padding: 4px 10px;
  background: var(--el-fill-color-light);
  border-radius: 4px;
  word-break: break-all;
}
.beta-note {
  line-height: 1.7;
  color: var(--el-text-color-regular);
}
.beta-note code {
  padding: 2px 6px;
  background: var(--el-fill-color-light);
  border-radius: 4px;
}
.license-actions {
  display: flex;
  gap: 12px;
  margin-top: 16px;
}
.license-request-result {
  margin-top: 12px;
}
</style>
