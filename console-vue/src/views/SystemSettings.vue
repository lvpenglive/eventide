<script setup lang="ts">
import { computed, onMounted, reactive, ref, markRaw } from 'vue'
import {
  ElAffix,
  ElAlert,
  ElButton,
  ElCard,
  ElDescriptions,
  ElDescriptionsItem,
  ElForm,
  ElFormItem,
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
} from 'element-plus'
import {
  Refresh,
  UploadFilled,
} from '@element-plus/icons-vue'

// ============================================================
// 本地设置类型（不触碰外部文件）
// ============================================================
interface AlertHistorySettings {
  write_to_es: boolean
  search_store: 'mysql' | 'es'
  es_configured: boolean
  es_url: string
  es_index: string
  es_username: string
  // 密码一般不以明文回传；前端用空串表示「不改」。
  es_password?: string
}
interface AlertHistorySettingsUpdate {
  write_to_es: boolean
  search_store: 'mysql' | 'es'
  es_url?: string
  es_index?: string
  es_username?: string
  es_password?: string
}

interface TrapTokenSettings {
  token_set: boolean
  token_prefix?: string
  // 仅在 regenerate=true 时一次性返回
  regenerated_token?: string
}

type StormAction = 'silence_only' | 'notify_once' | 'both' | 'disable'
interface StormSettings {
  enabled: boolean
  window_seconds: number
  threshold: number
  group_by: string[]
  action: StormAction
  silence_seconds: number
}

interface UiBetaSettings {
  enabled: boolean
  v2_available?: boolean
}

interface SettingsApiShim {
  getAlertHistorySettings(): Promise<AlertHistorySettings>
  putAlertHistorySettings(body: AlertHistorySettingsUpdate): Promise<AlertHistorySettings>

  getTrapToken(): Promise<TrapTokenSettings>
  regenerateTrapToken(): Promise<TrapTokenSettings>

  getStormSettings(): Promise<StormSettings>
  putStormSettings(body: StormSettings): Promise<StormSettings>

  getUiBetaSettings(): Promise<UiBetaSettings>
  putUiBetaSettings(body: { enabled: boolean }): Promise<UiBetaSettings>
}

// ============================================================
// Shim / 真实模块双模式
// ============================================================
function _unimpl(name: string): never {
  throw new Error(`[API shim] 模块未实现：${name}，请等待并行任务创建对应 api/*.ts`)
}
const _emptyAlertHistory: AlertHistorySettings = {
  write_to_es: false,
  search_store: 'mysql',
  es_configured: false,
  es_url: '',
  es_index: '',
  es_username: '',
  es_password: '',
}
const _emptyTrapToken: TrapTokenSettings = { token_set: false, token_prefix: '' }
const _emptyStorm: StormSettings = {
  enabled: false,
  window_seconds: 300,
  threshold: 10,
  group_by: ['alertname', 'severity'],
  action: 'notify_once',
  silence_seconds: 900,
}
const _emptyUiBeta: UiBetaSettings = { enabled: false }

const _shimSettings: SettingsApiShim = {
  getAlertHistorySettings: () => Promise.resolve(_emptyAlertHistory),
  putAlertHistorySettings: () => Promise.reject(_unimpl('putAlertHistorySettings')),
  getTrapToken: () => Promise.resolve(_emptyTrapToken),
  regenerateTrapToken: () => Promise.reject(_unimpl('regenerateTrapToken')),
  getStormSettings: () => Promise.resolve(_emptyStorm),
  putStormSettings: () => Promise.reject(_unimpl('putStormSettings')),
  getUiBetaSettings: () => Promise.resolve(_emptyUiBeta),
  putUiBetaSettings: () => Promise.reject(_unimpl('putUiBetaSettings')),
}
// @ts-ignore 若 @/api/settings 模块尚未创建则忽略解析错误
import * as _rawSettings from '@/api/settings'
const _settings = markRaw(_rawSettings as unknown as SettingsApiShim | Record<string, unknown>)
function _bindApi<A extends object, S>(api: A | Record<string, unknown>, shim: S): S {
  const out = { ...(shim as unknown as object) } as S
  for (const key of Object.keys(shim as unknown as object)) {
    if (key in api && typeof (api as Record<string, unknown>)[key] === 'function') {
      ;(out as unknown as Record<string, unknown>)[key] = (api as Record<string, unknown>)[key]
    }
  }
  return out
}
const _api = _bindApi<Record<string, unknown>, SettingsApiShim>(
  _settings as unknown as Record<string, unknown>,
  _shimSettings
)
const {
  getAlertHistorySettings,
  putAlertHistorySettings,
  getTrapToken,
  regenerateTrapToken,
  getStormSettings,
  putStormSettings,
  getUiBetaSettings,
  putUiBetaSettings,
} = _api

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
// 分组 a) 历史事件 Alert History
// ============================================================
const alertHistory = reactive<AlertHistorySettings>({ ..._emptyAlertHistory })
const alertHistorySaving = ref(false)
const alertHistoryLoading = ref(false)
async function loadAlertHistory(force = false): Promise<void> {
  if (alertHistoryLoading.value && !force) return
  alertHistoryLoading.value = true
  try {
    const resp = await getAlertHistorySettings()
    Object.assign(alertHistory, {
      write_to_es: !!resp.write_to_es,
      search_store: resp.search_store === 'es' ? 'es' : 'mysql',
      es_configured: !!resp.es_configured,
      es_url: resp.es_url ?? '',
      es_index: resp.es_index ?? '',
      es_username: resp.es_username ?? '',
      es_password: '', // 服务器一般不回传明文，始终由用户主动填写
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
    }
    if (alertHistory.es_url !== undefined) body.es_url = alertHistory.es_url
    if (alertHistory.es_index !== undefined) body.es_index = alertHistory.es_index
    if (alertHistory.es_username !== undefined) body.es_username = alertHistory.es_username
    if (alertHistory.es_password && alertHistory.es_password.length > 0) {
      body.es_password = alertHistory.es_password
    }
    const resp = await putAlertHistorySettings(body)
    // 保存后将状态刷新（如 es_configured 可能变化）
    alertHistory.es_configured = !!resp.es_configured
    // 清空已提交的密码字段占位，避免用户误以为再次「覆盖」
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
const trapToken = reactive<TrapTokenSettings>({ token_set: false, token_prefix: '' })
const trapTokenLoading = ref(false)
const trapTokenRegenerating = ref(false)
const trapTokenCleartext = ref<string | null>(null)
async function loadTrapToken(force = false): Promise<void> {
  if (trapTokenLoading.value && !force) return
  trapTokenLoading.value = true
  try {
    const resp = await getTrapToken()
    trapToken.token_set = !!resp.token_set
    trapToken.token_prefix = resp.token_prefix ?? ''
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
    const resp = await regenerateTrapToken()
    trapToken.token_set = true
    trapToken.token_prefix = resp.token_prefix ?? trapToken.token_prefix
    const cleartext = resp.regenerated_token
    if (!cleartext) {
      ElMessage.warning('Token 已重新生成，但服务端未返回一次性明文，请联系管理员')
      return false
    }
    trapTokenCleartext.value = cleartext
    await ElMessageBox.alert(
      `新的 Trap HTTP API Token 已生成：\n\n${cleartext}\n\n请立即复制保存，此窗口关闭后将不再显示明文。`,
      'Trap Token 已重新生成',
      {
        confirmButtonText: '我已复制并关闭',
        dangerouslyUseHTMLString: false,
      }
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
// 分组 c) 告警风暴抑制 Storm
// ============================================================
const stormGroupByCandidates: string[] = ['severity', 'alertname', 'instance', 'ip', 'team']
const stormActionOptions: { label: string; value: StormAction }[] = [
  { label: '仅静默 silence_only', value: 'silence_only' },
  { label: '仅一次通知 notify_once', value: 'notify_once' },
  { label: '静默 + 一次通知 both', value: 'both' },
  { label: '关闭抑制 disable', value: 'disable' },
]
const storm = reactive<StormSettings>({ ..._emptyStorm })
const stormSaving = ref(false)
const stormLoading = ref(false)
async function loadStorm(force = false): Promise<void> {
  if (stormLoading.value && !force) return
  stormLoading.value = true
  try {
    const resp = await getStormSettings()
    storm.enabled = !!resp.enabled
    storm.window_seconds = typeof resp.window_seconds === 'number' ? resp.window_seconds : 300
    storm.threshold = typeof resp.threshold === 'number' ? resp.threshold : 10
    storm.group_by = Array.isArray(resp.group_by) ? [...resp.group_by] : []
    storm.action = (['silence_only', 'notify_once', 'both', 'disable'] as StormAction[]).includes(resp.action)
      ? resp.action
      : 'notify_once'
    storm.silence_seconds = typeof resp.silence_seconds === 'number' ? resp.silence_seconds : 900
  } catch (e) {
    ElMessage.error(errMsgOf(e, '加载告警风暴设置失败'))
  } finally {
    stormLoading.value = false
  }
}
async function saveStorm(): Promise<boolean> {
  stormSaving.value = true
  try {
    const body: StormSettings = {
      enabled: !!storm.enabled,
      window_seconds: Number(storm.window_seconds) || 0,
      threshold: Number(storm.threshold) || 0,
      group_by: [...(storm.group_by ?? [])],
      action: storm.action,
      silence_seconds: Number(storm.silence_seconds) || 0,
    }
    await putStormSettings(body)
    ElMessage.success('告警风暴抑制设置已保存')
    return true
  } catch (e) {
    ElMessage.error(errMsgOf(e, '保存告警风暴设置失败'))
    return false
  } finally {
    stormSaving.value = false
  }
}

// ============================================================
// 分组 d) UI Beta 开关
// ============================================================
const uiBeta = reactive<UiBetaSettings>({ enabled: false })
const uiBetaSaving = ref(false)
const uiBetaLoading = ref(false)
async function loadUiBeta(force = false): Promise<void> {
  if (uiBetaLoading.value && !force) return
  uiBetaLoading.value = true
  try {
    const resp = await getUiBetaSettings()
    uiBeta.enabled = !!resp.enabled
    uiBeta.v2_available = resp.v2_available
  } catch (e) {
    ElMessage.error(errMsgOf(e, '加载 UI Beta 开关失败'))
  } finally {
    uiBetaLoading.value = false
  }
}
async function saveUiBeta(): Promise<boolean> {
  uiBetaSaving.value = true
  try {
    const resp = await putUiBetaSettings({ enabled: !!uiBeta.enabled })
    uiBeta.enabled = !!resp.enabled
    uiBeta.v2_available = resp.v2_available
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
      // Trap Token 不参与自动保存（需主动 regenerate，无 put body 语义）
      Promise.resolve(true),
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
    loadStorm(true),
    loadUiBeta(true),
  ])
  ElMessage.success('已刷新')
}

const anyLoading = computed(
  () =>
    alertHistoryLoading.value ||
    trapTokenLoading.value ||
    stormLoading.value ||
    uiBetaLoading.value
)

onMounted(() => {
  void Promise.all([loadAlertHistory(), loadTrapToken(), loadStorm(), loadUiBeta()])
})
</script>

<template>
  <div class="system-settings-page">
    <h2 class="page-title">系统设置</h2>
    <p class="page-subtitle">历史事件 / Trap Token / 告警风暴 / UI Beta 开关</p>

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
        <div class="card-header">
          <span class="card-title">a) 历史事件（Alert History）</span>
          <ElButton type="primary" :icon="UploadFilled" :loading="alertHistorySaving" @click="saveAlertHistory">保存</ElButton>
        </div>
      </template>

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
          <ElTag :type="alertHistory.es_configured ? 'success' : 'info'">
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
    </ElCard>

    <!-- b) Trap HTTP API Token -->
    <ElCard shadow="never" class="section-card" v-loading="trapTokenLoading">
      <template #header>
        <div class="card-header">
          <span class="card-title">b) Trap HTTP API Token</span>
          <ElButton
            type="warning"
            :loading="trapTokenRegenerating"
            @click="handleRegenerateTrapToken"
          >
            重新生成
          </ElButton>
        </div>
      </template>

      <ElDescriptions :column="1" border>
        <ElDescriptionsItem label="Token 状态">
          <ElTag :type="trapToken.token_set ? 'success' : 'danger'" effect="light">
            {{ trapToken.token_set ? '已设置' : '未设置' }}
          </ElTag>
        </ElDescriptionsItem>
        <ElDescriptionsItem label="Token 前缀 token_prefix">
          <span v-if="trapToken.token_prefix" class="mono">
            {{ trapToken.token_prefix }}****
          </span>
          <span v-else style="color: var(--el-text-color-secondary)">
            （未返回前缀）
          </span>
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
    </ElCard>

    <!-- c) 告警风暴抑制 -->
    <ElCard shadow="never" class="section-card" v-loading="stormLoading">
      <template #header>
        <div class="card-header">
          <span class="card-title">c) 告警风暴抑制（Storm）</span>
          <ElButton type="primary" :icon="UploadFilled" :loading="stormSaving" @click="saveStorm">保存</ElButton>
        </div>
      </template>

      <ElForm label-width="180px" class="storm-form">
        <ElFormItem label="启用风暴抑制 enabled">
          <ElSwitch v-model="storm.enabled" />
        </ElFormItem>
        <ElRow :gutter="20">
          <ElCol :xs="24" :sm="12">
            <ElFormItem label="窗口（秒）window_seconds">
              <ElInputNumber
                v-model="storm.window_seconds"
                :min="1"
                :max="86400"
                controls-position="right"
                style="width: 100%"
              />
            </ElFormItem>
          </ElCol>
          <ElCol :xs="24" :sm="12">
            <ElFormItem label="阈值 threshold">
              <ElInputNumber
                v-model="storm.threshold"
                :min="1"
                :max="100000"
                controls-position="right"
                style="width: 100%"
              />
            </ElFormItem>
          </ElCol>
        </ElRow>
        <ElFormItem label="分组字段 group_by">
          <ElSelect
            v-model="storm.group_by"
            multiple
            filterable
            allow-create
            default-first-option
            :reserve-keyword="false"
            placeholder="选择或输入分组字段"
            style="width: 100%"
          >
            <ElOption
              v-for="c in stormGroupByCandidates"
              :key="c"
              :label="c"
              :value="c"
            />
          </ElSelect>
          <div class="form-hint">内置候选：severity、alertname、instance、ip、team；支持自由输入自定义标签名。</div>
        </ElFormItem>
        <ElFormItem label="动作 action">
          <ElSelect v-model="storm.action" placeholder="选择动作" style="max-width: 360px">
            <ElOption
              v-for="o in stormActionOptions"
              :key="o.value"
              :label="o.label"
              :value="o.value"
            />
          </ElSelect>
        </ElFormItem>
        <ElFormItem label="静默时长（秒）silence_seconds">
          <ElInputNumber
            v-model="storm.silence_seconds"
            :min="0"
            :max="86400 * 30"
            controls-position="right"
          />
          <span class="form-hint" style="margin-left: 12px">仅 silence_only / both 生效。</span>
        </ElFormItem>
      </ElForm>
    </ElCard>

    <!-- d) UI Beta 开关 -->
    <ElCard shadow="never" class="section-card" v-loading="uiBetaLoading">
      <template #header>
        <div class="card-header">
          <span class="card-title">d) UI Beta 开关</span>
          <ElButton type="primary" :icon="UploadFilled" :loading="uiBetaSaving" @click="saveUiBeta">保存</ElButton>
        </div>
      </template>

      <ElDescriptions :column="1" border>
        <ElDescriptionsItem label="启用 v2 影子前端">
          <ElSwitch v-model="uiBeta.enabled" />
          <span v-if="uiBeta.v2_available === false" style="margin-left: 12px; color: var(--el-color-warning)">
            （当前后端未启用 v2 影子前端资源，v2_available=false）
          </span>
        </ElDescriptionsItem>
        <ElDescriptionsItem label="说明">
          <div class="beta-note">
            启用后后端会写 Cookie <code class="mono">ui_beta=v2</code>，下次访问自动进入 v2 影子前端。
          </div>
        </ElDescriptionsItem>
      </ElDescriptions>
    </ElCard>
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
</style>
