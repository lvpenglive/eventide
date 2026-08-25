export interface LoginReq {
  username: string
  password: string
}

export interface PasswordStatus {
  expired?: boolean
  change_required?: boolean
  days_until_expire?: number | null
}

export interface LoginResp {
  token: string
  username: string
  expires_at: string
  permissions: string[]
  password_status?: PasswordStatus
  display_name?: string
  id?: string
  token_ttl_hours?: number
}

export interface MeResp {
  id: string
  username: string
  display_name: string
  permissions: string[]
  password_status?: PasswordStatus
  token_ttl_hours?: number
  expires_at?: string
  /** 数据库信息（仅用于展示） */
  database?: {
    db_type?: string
    host?: string
    port?: number
    name?: string
    redis_url?: string
  }
  license?: unknown
}

export interface ChangePasswordResp {
  ok: boolean
}

export interface AlertRecentItem {
  fingerprint: string
  summary: string
  severity: 'critical' | 'warning' | 'info' | 'error' | 'ok' | string
  status: 'firing' | 'pending' | 'resolved' | string
  created_at: string
  datasource: string
}

export interface IngressBrief {
  id: string
  name: string
  kind: string
  enabled: boolean
}

export interface NotifySkipItem {
  error: string
  transition: string
  created_at: string | null
}

export interface OverviewResp {
  health: 'ok' | 'degraded' | 'error' | string
  datasources: number
  rules: number
  channels: number
  ingress_routes: number
  alerts_total: number
  alerts_firing: number
  alerts_pending: number
  alerts_resolved: number
  active_silences: number
  active_maintenance_windows: number
  recent_alerts: AlertRecentItem[]
  ingress: IngressBrief[]
  notify_skips: NotifySkipItem[]
  pressure_inflight: number
  ingress_max_inflight: number
  is_leader: boolean
  leader_holder_id: string
  cluster_enabled: boolean
}

export interface UiBetaToggleResp {
  v2_available: boolean
  enabled: boolean
}

// —— 通用枚举 / LabelMap ——
export type AlertStatus = 'firing' | 'pending' | 'resolved' | string
export type Severity = 'critical' | 'warning' | 'info' | 'error' | 'ok' | string
export type AlertTransition = 'became_firing' | 'became_resolved' | 'escalated' | 'unchanged' | string
export type IngressKind = 'alertmanager' | 'generic' | 'kafka' | string
export type ChannelKind = 'webhook' | 'dingtalk' | 'wecom' | 'feishu' | 'lark' | 'sms' | 'email' | string

// —— Alert 相关 ——
export interface AlertEvent {
  id: string
  rule_id: string
  fingerprint: string
  status: AlertStatus
  severity: Severity
  labels: Record<string, string>
  annotations: Record<string, string>
  value: number | null
  starts_at: string
  ends_at: string | null
  pending_since: string | null
  last_evaluated_at: string
  tally: number
  last_occurrence_at: string
  notified_firing: boolean
  notified_resolved: boolean
  acknowledged_at: string | null
  acknowledged_by: string | null
  assignee: string | null
  ack_comment: string | null
  closed_at: string | null
  closed_by: string | null
  close_comment: string | null
  escalated_at: string | null
}

export interface AlertStatusCounts { firing: number; pending: number; resolved: number }

export interface AlertListResp {
  items: AlertEvent[]
  total: number
  page: number
  limit: number
  status_counts: AlertStatusCounts
}

export interface AlertQuery {
  status?: '' | AlertStatus
  severity?: '' | Severity
  source?: '' | 'ingress' | 'rule'
  q?: string
  ip?: string
  store?: '' | 'mysql' | 'es'
  acked?: '' | 'true' | 'false'
  page?: number
  limit?: number
}

export interface AckInput { assignee?: string; comment?: string }
export interface CloseInput { comment: string; notify?: boolean }
export interface BatchOpError { id: string; error: string }
export interface BatchOpResp { ok: string[]; failed: BatchOpError[] }

// —— Notify / Maintenance / Channels ——
export interface NotifyLog {
  id: string
  alert_id: string
  channel_id: string
  transition: AlertTransition
  success: boolean
  error: string | null
  body: string
  created_at: string
  /** 接口不带，前端可自行按 channel 字典查表回填展示 */
  channel_name?: string
}

export interface MaintenanceWindow {
  id: string
  name: string
  comment: string
  rule_id: string | null
  matchers: Record<string, string>
  starts_at: string
  ends_at: string
  enabled: boolean
  created_at: string
  updated_at: string
}

export interface NotifyChannel {
  id: string
  name: string
  kind: ChannelKind
  url: string
  secret: string | null
  options: Record<string, string>
  enabled: boolean
  created_at: string
  updated_at: string
}

// —— Ingress 相关 ——
export interface IngressRoute {
  id: string
  name: string
  kind: IngressKind
  token: string | null
  endpoint: string
  options: Record<string, string>
  channel_ids: string[]
  escalate_after_seconds: number
  escalate_severity: Severity | null
  escalate_channel_ids: string[]
  enabled: boolean
  created_at: string
  updated_at: string
}

export interface IngressInput {
  name: string
  kind: IngressKind
  token?: string | null
  endpoint?: string
  options?: Record<string, string>
  channel_ids?: string[]
  escalate_after_seconds?: number
  escalate_severity?: Severity | '' | null
  escalate_channel_ids?: string[]
  enabled?: boolean
}

// —— Kafka 工具 ——
export interface KafkaTopicRow { name: string; partitions: number }
export interface KafkaProbeResp {
  latency_ms: number
  topic_count: number
  topic_found?: boolean
  partitions?: number
  topics: KafkaTopicRow[]
}
export interface KafkaPartitionInfo { partition: number; earliest: string; latest: string; lag_approx?: string | number }
export interface KafkaDescribeResp { partitions: KafkaPartitionInfo[] }
export interface KafkaBrowseInput {
  brokers: string; topic: string; partition: number; from: 'latest'|'earliest'|'offset'; max: number; offset?: number
}
export interface KafkaMsg { partition: number; offset: string; timestamp: string; value_bytes?: number; key?: string|null; value: string }
export interface KafkaBrowseResp { messages: KafkaMsg[]; earliest: string; latest: string }
export interface KafkaGroupRow { group_id: string; protocol_type?: string; protocol?: string }
export interface KafkaGroupListResp { groups: KafkaGroupRow[] }
export interface KafkaGroupMember { member_id: string; client_id: string; client_host: string }
export interface KafkaGroupPartition { topic: string; partition: number; committed: string; latest: string; lag: number }
export interface KafkaGroupDescribeResp {
  group_id: string; state?: string; protocol_type?: string; protocol?: string
  total_lag: number; members: KafkaGroupMember[]; partitions: KafkaGroupPartition[]
}
export interface IngressTestResp { ok: boolean; parsed?: number; ingested?: number; [k: string]: unknown }

// ================================================================
// B2 批次：Silences / Maintenance / Datasources / Rules
// 仅追加导出；不修改 / 不覆盖上方既有 AlertEvent / MaintenanceWindow 等定义。
// ================================================================

/** 通用标签集合（Prometheus 风格） */
export type Labels = Record<string, string>

// —— Silence ——
export interface B2Silence {
  id: string
  rule_id: string | null
  matchers: Labels
  starts_at: string
  ends_at: string
  comment: string
  created_at: string
}
export interface SilenceInput {
  rule_id?: string | null
  matchers?: Labels
  starts_at: string
  ends_at: string
  comment?: string
}
/** 对外使用的 Silence 实体别名（B2 批次） */
export type Silence = B2Silence

// —— Datasource ——
export type DatasourceKind = 'prometheus' | 'victoriametrics' | 'kafka' | 'log' | string
export interface B2Datasource {
  id: string
  name: string
  kind: DatasourceKind
  url: string
  options: Record<string, string>
  enabled: boolean
  created_at: string
  updated_at: string
}
export interface DatasourceInput {
  name: string
  kind?: string
  url: string
  options?: Record<string, string>
  enabled?: boolean
}
/** 对外使用的 Datasource 实体别名（B2 批次） */
export type Datasource = B2Datasource

// —— Rule ——
export type RuleComparator =
  | 'gt' | 'gte' | 'lt' | 'lte' | 'eq' | 'neq'
  | '>' | '>=' | '<' | '<=' | '==' | '!='
  | string
export type RuleSeverity =
  | 'not_classified' | 'information' | 'warning' | 'average' | 'high' | 'disaster'
  | string
export interface B2Rule {
  id: string
  name: string
  datasource_id: string
  expr: string
  comparator: RuleComparator
  threshold: number
  for_seconds: number
  interval_seconds: number
  severity: RuleSeverity
  labels: Labels
  annotations: Labels
  channel_ids: string[]
  escalate_after_seconds: number
  escalate_severity: string | null
  escalate_channel_ids: string[]
  enabled: boolean
  created_at: string
  updated_at: string
}
export interface RuleInput {
  name: string
  datasource_id: string
  expr: string
  comparator: string
  threshold: number
  for_seconds?: number
  interval_seconds?: number
  severity?: string
  labels?: Labels
  annotations?: Labels
  channel_ids?: string[]
  escalate_after_seconds?: number
  escalate_severity?: string | null
  escalate_channel_ids?: string[]
  enabled?: boolean
}
export interface RuleEvaluateResp {
  ok: boolean
  alerts: unknown[]
  [k: string]: unknown
}
/** 对外使用的 Rule 实体别名（B2 批次） */
export type Rule = B2Rule

// —— Maintenance Window（与既有 MaintenanceWindow 保持兼容，仅补齐 Input） ——
export interface MaintenanceInput {
  name: string
  comment?: string
  rule_id?: string | null
  matchers?: Labels
  starts_at: string
  ends_at: string
  enabled?: boolean
}
/** B2 专属别名（与现有 MaintenanceWindow 等价，避免批次 1 类型引用被破坏） */
export type B2MaintenanceWindow = MaintenanceWindow

// ================================================================
// B3 批次：Channels / Notifies / Enrich / Lookup / IAM / Settings
// 仅追加导出；不修改 / 不覆盖上方既有 AlertEvent / MaintenanceWindow /
// NotifyChannel / IngressRoute / NotifyLog 等定义。
// 若与既有名称冲突，使用 B3* 别名。
// ================================================================

// —— Channels（NotifyChannel 已存在 → 仅提供别名 + Input/TestResp）——
/** B3 批次使用的 Channel 实体别名（与既有 NotifyChannel 等价） */
export type B3NotifyChannel = NotifyChannel

export interface ChannelInput {
  name: string
  kind: string
  url: string
  secret?: string | null
  options?: Record<string, string>
  enabled?: boolean
}

export interface ChannelTestResp {
  ok: boolean
  text?: string
  kind?: string
  request_url?: string
  request_body?: unknown
  http_status?: number
  response_body?: unknown
  error?: string
  [k: string]: unknown
}

// —— Notify Log（后端 list_notifies join 了 channel_name/channel_kind）——
export interface NotifyLogItem {
  id: string
  alert_id: string
  channel_id: string
  channel_name: string
  channel_kind: string
  transition: string
  success: boolean
  error: string | null
  body: string
  created_at: string
}

export type NotifyQuerySuccess =
  | '1' | 'true' | 'ok'
  | '0' | 'false' | 'fail'
  | ''
  | boolean
  | null

export interface NotifyQuery {
  channel_id?: string
  success?: NotifyQuerySuccess
  q?: string
  limit?: number
}

// —— Enrich & Lookup ——
export type EnrichKind =
  | 'AnnotationTemplate'
  | 'LabelMap'
  | 'Lookup'
  | 'Composite'
  | string

export interface EnrichRule {
  id: string
  name: string
  kind: EnrichKind
  matchers: Labels
  match_key: string
  templates: Labels
  mappings: Record<string, Labels>
  lookup_table_ids: string[]
  lookup_match_keys: Record<string, string>
  field_templates: Labels
  label_extracts: Labels
  write_labels: boolean
  enabled: boolean
  priority: number
  created_at: string
  updated_at: string
}

export interface EnrichInput {
  name: string
  kind?: string
  matchers?: Labels
  match_key?: string
  templates?: Labels
  mappings?: Record<string, Labels>
  lookup_table_ids?: string[]
  lookup_table_id?: string | null
  lookup_match_keys?: Record<string, string>
  field_templates?: Labels
  label_extracts?: Labels
  write_labels?: boolean
  enabled?: boolean
  priority?: number
}

export type EnrichPreviewInput = EnrichInput & {
  alert?: unknown
  rule_id?: string
}

export interface EnrichPreviewResp {
  ok?: boolean
  alert?: unknown
  [k: string]: unknown
}

export interface LookupTable {
  id: string
  name: string
  description?: string
  key_label: string
  rows: Record<string, Labels>
  enabled: boolean
  created_at: string
  updated_at: string
}

export interface LookupInput {
  name: string
  description?: string
  key_label?: string
  rows?: Record<string, Labels>
  text?: string
  key_label_from_header?: boolean
  enabled?: boolean
}

// —— IAM：Department / Role / User / Audit ——
export interface Department {
  id: string
  name: string
  parent_id: string | null
  sort_order: number
  enabled: boolean
  created_at: string
  updated_at: string
}

export interface DepartmentInput {
  name: string
  parent_id?: string | null
  sort_order?: number
  enabled?: boolean
}

export interface Role {
  id: string
  name: string
  description: string
  permissions: string[]
  is_system: boolean
  created_at: string
  updated_at: string
}

export interface RoleInput {
  name: string
  description?: string
  permissions?: string[]
}

export interface UserAccount {
  id: string
  username: string
  display_name: string
  department_id: string | null
  role_ids: string[]
  enabled: boolean
  created_at: string
  updated_at: string
  password_changed_at: string
}

export interface UserInput {
  username: string
  display_name: string
  password?: string
  department_id?: string | null
  role_ids?: string[]
  enabled?: boolean
}

/** 与 UserInput 等价；空字符串 password 表示不修改密码 */
export type UserUpdateInput = UserInput

export interface ResetPasswordResp {
  ok: boolean
  password?: string
  [k: string]: unknown
}

export interface AuditLog {
  id: string
  created_at: string
  actor_username: string
  actor_uid: string | null
  action: string
  resource_type: string
  resource_id: string | null
  method: string
  path: string
  status_code: number
  detail_json: string | null
  client_ip: string | null
}

export interface AuditQuery {
  actor?: string
  action?: string
  resource_type?: string
  q?: string
  limit?: number
}

// —— Settings（4 组 GET/PUT）——
export interface AlertHistorySettingsView {
  write_to_es: boolean
  search_store: string
  es_configured: boolean
  es_url: string
  es_index: string
  es_username: string
  es_password_set: boolean
}

export interface AlertHistorySettingsUpdate {
  write_to_es?: boolean
  search_store?: string
  es_url?: string
  es_index?: string
  es_username?: string
  es_password?: string
}

export interface TrapTokenSettingsView {
  token?: string
  token_set: boolean
  token_prefix?: string
  regenerate?: boolean
  [k: string]: unknown
}

export interface TrapTokenSettingsUpdate {
  regenerate?: boolean
}

export interface StormSettingsView {
  enabled: boolean
  window_seconds?: number
  threshold?: number
  group_by?: string[]
  action?: string
  silence_seconds?: number
  [k: string]: unknown
}

export type StormSettingsUpdate = Partial<StormSettingsView>

/**
 * UI Beta Toggle Settings View
 * 与 settings/ui-betatoggle 现有 { enabled: boolean } 保持一致。
 * 若其他批次已定义同名，此处再次导出别名避免报错。
 */
export interface UiBetaSettingsView {
  enabled: boolean
}
/** 别名导出（同名 = 兼容） */
export type B3UiBetaSettingsView = UiBetaSettingsView

// ================================================================
// B4 批次：Kafka / Trap BFF / MIB / Trap Policies
// 为 100% 不破坏 B1–B3，所有 B4 新类型统一使用 B4 前缀；若与既有
// 名称冲突（例如 KafkaPartitionInfo）不会触发 TypeScript 报错。
// api/kafka.ts / api/trap.ts / api/mibs.ts / api/policies.ts 会
// 从本文件 import 相应 B4* 类型。
// ================================================================

/** AlertSeverity：与既有 Severity 等价，Policies 系列直接复用 */
export type AlertSeverity = Severity

// ————————————— B4：Kafka 系列 —————————————

export interface B4KafkaTopicInfo {
  name: string
  partitions: number
  is_internal?: boolean
  compacted?: boolean
}

export interface B4KafkaClusterProbe {
  ok: true
  brokers: string
  latency_ms: number
  topic_count: number
  topics: B4KafkaTopicInfo[]
  topic_found?: boolean
  partitions?: number
}

/**
 * B4 版分区信息（注意：B1 已存在同名 KafkaPartitionInfo interface，
 * 字段类型不兼容，此处使用 B4 前缀避免 declaration merge 冲突）。
 */
export interface B4KafkaPartitionInfo {
  partition: number | string
  earliest: number
  latest: number
  lag_approx?: number
  leader?: number | string
  replicas?: number[]
  isr?: number[]
}

export interface B4KafkaTopicDescribe {
  topic: string
  partitions: B4KafkaPartitionInfo[]
}

export interface B4KafkaMessage {
  partition: number
  offset: number
  timestamp: string
  key?: string
  value?: string
  value_bytes: number
}

export interface B4KafkaBrowseResult {
  partition: number
  earliest: number
  latest: number
  messages: B4KafkaMessage[]
}

export interface B4KafkaGroupState {
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

export interface B4KafkaGroupListed {
  group_id: string
  state?: string
  total_lag?: number
  source?: 'brokers' | 'routes'
}

export interface B4KafkaProduceResult {
  ok: true
  offset?: number
  partition?: number
  key?: string
}

// ————————————— B4：Trap BFF 系列 —————————————

export interface B4TrapHealth {
  ok?: true
  instance_id: string
  listen_udp: string
  ha_vip?: string
  kafka_enabled: boolean
  kafka_topic?: string
  started_at?: string
  version?: string
}

export interface B4TrapStats {
  received: number
  parsed_ok: number
  parse_err: number
  kafka_ok: number
  kafka_err: number
  simulated: number
  dropped?: number
}

export interface B4TrapRecentItem {
  at: string
  peer: string
  trap_oid: string
  alertname?: string
  severity?: string
  kafka?: boolean
  varbinds?: Array<{
    oid: string
    name?: string
    value?: string
    type?: string
  }>
}

export interface B4TrapRecentList {
  items: B4TrapRecentItem[]
}

export interface B4TrapPeerItem {
  instance_id: string
  listen_udp: string
  ha_vip?: string
  received?: number
  updated_at?: string
  kafka_topic?: string
  redis_connected?: boolean
}

export interface B4TrapClusterResp {
  items: B4TrapPeerItem[]
  redis?: boolean
  hint?: string
}

export interface B4TrapSimulateReq {
  ip: string
  trap_oid: string
  alertname?: string | null
  severity?: string
  dry_run?: boolean
}

export interface B4TrapSimulateResp {
  ok?: true
  kafka?: boolean
  alert?: Record<string, unknown>
}

// ————————————— B4：MIB 系列 —————————————

export interface B4MibModuleItem {
  id: string
  module_name: string
  filename: string
  module_oid?: string
  status?: string
  updated_at?: string
  error?: string
  notifications_count?: number
  objects_count?: number
}

export interface B4MibModuleList {
  items: B4MibModuleItem[]
  mib_dir: string
  backend: string
  load_error?: string
}

export interface B4MibTreeNode {
  oid: string
  name: string
  has_children: boolean
  is_notification?: boolean
  status?: string
  kind?: string
}

export interface B4MibTreeNodeList {
  module: string
  parent_oid: string
  children: B4MibTreeNode[]
}

export interface B4MibNodeDetail {
  oid: string
  name: string
  kind?: string
  status?: string
  description?: string
  objects?: string[]
  is_notification?: boolean
  type?: string
  values?: Record<string, string>
}

export interface B4MibNotifRow {
  trap_oid: string
  name: string
  severity?: string
  objects?: string[]
  module?: string
  description?: string
}

export interface B4MibNotificationList {
  module: string
  items: B4MibNotifRow[]
}

export interface B4ApplyPolicyResult {
  inserted?: number
  updated?: number
  skipped?: number
  deleted?: number
  errors?: string[]
}

export interface B4SnmpGetResult {
  oid: string
  name?: string
  value: string
  type?: string
  host: string
}

// ————————————— B4：Policies 系列 —————————————

export type B4TrapPolicyMatchMode = 'exact' | 'prefix' | 'oid_prefix'

export interface B4TrapPolicy {
  id: string
  name: string
  trap_oid: string
  match_mode: B4TrapPolicyMatchMode
  severity: AlertSeverity
  enabled: boolean
  summary_template?: string
  description?: string
  module?: string
  status?: string
  objects?: string[]
  object_oids?: Record<string, string>
  keywords?: string[]
  resolve_oid?: string
  resolve_values?: string[]
  fingerprint_oids?: string[]
  severity_oid?: string
  severity_map?: Record<string, AlertSeverity>
  created_at?: string
  updated_at?: string
}

export interface B4TrapPolicyList {
  items: B4TrapPolicy[]
  path: string
  count: number
}

export interface B4TrapPolicyImportResult {
  ok: true
  result: B4ApplyPolicyResult
}

export type B4PolicyImportMode = 'merge' | 'replace' | 'keep'
