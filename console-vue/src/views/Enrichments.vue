<script setup lang="ts">
import { computed, onMounted, reactive, ref, markRaw, watch, nextTick } from 'vue'
import {
  ElAffix, ElButton, ElCollapse, ElCollapseItem, ElDialog, ElDrawer, ElEmpty,
  ElForm, ElFormItem, ElInput, ElInputNumber, ElMessage, ElMessageBox,
  ElOption, ElRow, ElCol, ElSelect, ElSwitch, ElTable, ElTableColumn,
  ElTag, ElTooltip, ElTabs, ElTabPane, ElSteps, ElStep, ElDivider,
  ElRadio, ElRadioGroup, ElAlert, ElCard, ElCheckbox, ElPagination,
} from 'element-plus'
import type { FormInstance, FormRules } from 'element-plus'
import {
  Delete, Edit, Plus, Refresh, VideoPlay, Search, QuestionFilled,
} from '@element-plus/icons-vue'
import { useRouter } from 'vue-router'
const router = useRouter()

export type EnrichKind = 'AnnotationTemplate' | 'LabelMap' | 'Lookup' | 'Composite'
export interface EnrichRow {
  id: string; name: string; kind: EnrichKind; matchers: Record<string, string>
  priority: number; enabled: boolean
  templates?: Record<string, string> | null; field_templates?: Record<string, string> | null
  label_extracts?: Record<string, string> | null; write_labels?: boolean | null
  match_key?: string | null; mappings?: Record<string, Record<string, string>> | null
  lookup_table_ids?: string[] | null; lookup_match_keys?: Record<string, string> | null
  description?: string | null; scope_note?: string | null
  created_at: string; updated_at: string
}
export interface EnrichInput {
  name: string; description?: string; kind?: EnrichKind
  enabled?: boolean; priority?: number; matchers?: Record<string, string>
  templates?: Record<string, string>; field_templates?: Record<string, string>
  label_extracts?: Record<string, string>; write_labels?: boolean; match_key?: string
  mappings?: Record<string, Record<string, string>>; lookup_table_ids?: string[]
  lookup_match_keys?: Record<string, string>
}
export interface LookupBrief {
  id: string; name: string; key_label?: string; enabled?: boolean
  rows?: Record<string, Record<string, string>> | null
}
export interface IngressBrief { id: string; name: string; kind?: string; endpoint?: string; enabled?: boolean; options?: Record<string, string> }
export interface PreviewAlertInput {
  payload?: unknown; ingress_id?: string; use_saved?: boolean
  rule_id?: string; rule_name?: string; rule?: EnrichInput
  matchers?: Record<string, string>; labels?: Record<string, string>
  annotations?: Record<string, string>; severity?: string; status?: string
  [k: string]: unknown
}
export interface PreviewEnrichResp {
  ok: boolean; original?: PreviewAlertInput; result?: Record<string, unknown>
  error?: string; detail?: unknown; labels?: Record<string, string>
  annotations?: Record<string, string>; severity?: string
  parsed_via?: string; before?: Record<string, unknown>
}

interface EnrichmentsApiShim {
  listEnrich(): Promise<EnrichRow[]>
  getEnrich(id: string): Promise<EnrichRow>
  createEnrich(body: EnrichInput): Promise<EnrichRow>
  updateEnrich(id: string, body: Partial<EnrichInput>): Promise<EnrichRow>
  deleteEnrich(id: string): Promise<{ ok: boolean }>
  previewEnrich(body: PreviewAlertInput): Promise<PreviewEnrichResp>
  listLookups(): Promise<LookupBrief[]>
  listIngresses(): Promise<IngressBrief[]>
}
function _unimpl(name: string): never {
  throw new Error(`[API shim] 模块未实现：${name}`)
}
const _shimEnrich: EnrichmentsApiShim = {
  listEnrich: () => Promise.reject(_unimpl('listEnrich')),
  getEnrich: () => Promise.reject(_unimpl('getEnrich')),
  createEnrich: () => Promise.reject(_unimpl('createEnrich')),
  updateEnrich: () => Promise.reject(_unimpl('updateEnrich')),
  deleteEnrich: () => Promise.reject(_unimpl('deleteEnrich')),
  previewEnrich: () => Promise.reject(_unimpl('previewEnrich')),
  listLookups: () => Promise.resolve([]),
  listIngresses: () => Promise.resolve([]),
}
// @ts-ignore
import * as _rawEnrich from '@/api/enrichments'
const _enrich = markRaw(_rawEnrich as unknown as EnrichmentsApiShim | Record<string, unknown>)
function _bindApi<A extends object, S>(api: A | Record<string, unknown>, shim: S): S {
  const out = { ...(shim as unknown as object) } as S
  for (const key of Object.keys(shim as unknown as object)) {
    if (key in api && typeof (api as Record<string, unknown>)[key] === 'function') {
      ;(out as unknown as Record<string, unknown>)[key] = (api as Record<string, unknown>)[key]
    }
  }
  return out
}
const _enrichApi = _bindApi<Record<string, unknown>, EnrichmentsApiShim>(
  _enrich as unknown as Record<string, unknown>, _shimEnrich)
const {
  listEnrich, getEnrich, createEnrich, updateEnrich, deleteEnrich,
  previewEnrich, listLookups, listIngresses,
} = _enrichApi

const KIND_NAMES: Record<EnrichKind, string> = {
  AnnotationTemplate: '模板丰富', LabelMap: '枚举映射',
  Lookup: '外部台账', Composite: '组合规则',
}
const STEP_GUIDE = [
  { title: '准备台账', desc: '导入主机 / 设备对照表' },
  { title: '建丰富规则', desc: '勾选要用的台账' },
  { title: '写到告警上', desc: '配置描述、IP、级别；保存前可试跑' },
]
const BUILTIN_CHIP_FIELDS = ['ip', 'instance', 'alertIp', 'alertname', 'severity']
function lookupNs(name: string): string {
  const s = String(name || '').split('').map((c) =>
    /[a-zA-Z0-9_\-\u4e00-\u9fff]/.test(c) ? c : '_').join('')
  return s || 'lookup'
}
function colsFromLookup(t: LookupBrief): string[] {
  const set = new Set<string>()
  Object.values(t.rows || {}).forEach((attrs) => {
    Object.keys(attrs || {}).forEach((k) => set.add(k))
  })
  return [...set].sort()
}

const loading = ref(false)
const enrichments = ref<EnrichRow[]>([])
const lookups = ref<LookupBrief[]>([])
const ingresses = ref<IngressBrief[]>([])
const filterQ = ref('')
const filterEnabled = ref<'' | 'true' | 'false'>('')
const PAGE_SIZES = [10, 20, 50, 100] as const
const page = ref(1)
const pageSize = ref<number>(PAGE_SIZES[1])
const pagedEnrichments = computed(() => {
  const src = filteredEnrichments.value
  if (src.length <= pageSize.value) return src
  const start = (page.value - 1) * pageSize.value
  return src.slice(start, start + pageSize.value)
})
function onPage(p: number) { page.value = Math.max(1, p) }
function onSize(s: number) { pageSize.value = s; page.value = 1 }
watch([filterQ, filterEnabled, enrichments], () => { page.value = 1 })
const filteredEnrichments = computed(() => {
  return enrichments.value.filter((r) => {
    if (filterEnabled.value === 'true' && !r.enabled) return false
    if (filterEnabled.value === 'false' && r.enabled) return false
    if (filterQ.value.trim()) {
      const q = filterQ.value.trim().toLowerCase()
      return r.name.toLowerCase().includes(q)
    }
    return true
  })
})
const lookupMap = computed(() => {
  const m = new Map<string, LookupBrief>()
  for (const l of lookups.value) m.set(l.id, l)
  return m
})
function asEnrich(r: unknown): EnrichRow { return r as EnrichRow }
function formatTime(s: string | null | undefined): string {
  if (!s) return '—'
  return s.replace('T', ' ').slice(0, 19)
}
function prettyJson(v: unknown): string {
  try { return JSON.stringify(v, null, 2) } catch { return String(v) }
}
function enrichWhat(r: EnrichRow): string {
  const parts: string[] = []
  const ids = r.lookup_table_ids?.length ? r.lookup_table_ids
    : [] as string[]
  if (ids.length) {
    const names = ids.map((id) => {
      const t = lookupMap.value.get(id)
      return t ? t.name : id.slice(0, 8) + '...'
    })
    parts.push(`查 ${names.join('、')}`)
  }
  if (r.mappings && Object.keys(r.mappings).length)
    parts.push(`内联映射 ${Object.keys(r.mappings).length} 条`)
  const ft = r.field_templates || {}
  const tpl = r.templates || {}
  const writes: string[] = []
  if (ft.summary || tpl.summary) writes.push('描述')
  if (ft.ip || ft.alertIp) writes.push('IP')
  if (ft.severity) writes.push('级别')
  if (ft.alertname) writes.push('名称')
  if (writes.length) parts.push(`写${writes.join('/')}`)
  else if (tpl && Object.keys(tpl).length) parts.push('写注解模板')
  return parts.length ? parts.join(' · ') : '未配置动作'
}
function enrichScope(r: EnrichRow): string {
  const m = r.matchers || {}
  const keys = Object.keys(m)
  if (!keys.length) return '全部告警'
  return keys.map((k) => `${k}=${m[k]}`).join('，')
}
const allIngresses = computed(() => ingresses.value.filter((r) => r.enabled !== false))
const mappedIngresses = computed(() =>
  ingresses.value.filter((r) => r.enabled !== false && Object.keys(r.options || {}).some((k) => k.startsWith('map_')))
)
async function loadAll() {
  loading.value = true
  try {
    const [es, ls, igs] = await Promise.all([
      listEnrich().catch((e) => { ElMessage.error(String(e?.message || e)); return [] as EnrichRow[] }),
      listLookups().catch(() => [] as LookupBrief[]),
      listIngresses().catch(() => [] as IngressBrief[]),
    ])
    enrichments.value = es; lookups.value = ls; ingresses.value = igs
  } finally { loading.value = false }
}
onMounted(loadAll)
async function onToggleEnabled(row: EnrichRow) {
  const newVal = row.enabled
  try {
    await updateEnrich(row.id, { enabled: newVal })
    ElMessage.success(`已${newVal ? '启用' : '停用'}`)
    const fresh = await getEnrich(row.id).catch(() => null)
    if (fresh) {
      const idx = enrichments.value.findIndex((r) => r.id === row.id)
      if (idx >= 0) enrichments.value[idx] = fresh
    }
  } catch (e) {
    row.enabled = !newVal
    ElMessage.error(`更新失败：${String((e as Error)?.message || e)}`)
  }
}

interface KvRow { key: string; value: string }
function emptyKv(): KvRow[] { return [{ key: '', value: '' }] }
function mapToRows(m: Record<string, string> | undefined): KvRow[] {
  if (!m || Object.keys(m).length === 0) return emptyKv()
  return Object.entries(m).map(([k, v]) => ({ key: k, value: v }))
}
function rowsToMap(rows: KvRow[]): Record<string, string> {
  const out: Record<string, string> = {}
  for (const r of rows) if (r.key.trim()) out[r.key.trim()] = r.value
  return out
}
const drawerVisible = ref(false)
const previewLoading = ref(false)
const previewResult = ref<PreviewEnrichResp | null>(null)
const previewMode = ref<'payload' | 'kpi'>('payload')
const previewRuleId = ref<string>('__all__')
const previewIngressId = ref('')
const previewDraft = ref<EnrichInput | null>(null)
const previewFromEditor = ref(false)
const previewPayload = ref('')
const SAMPLE_PAYLOAD = `{
"severity": 4,
"summary": "麒麟主机当前系统磁盘[vdb] IO使用百分比为: 97.49 %, 已超过90%阈值",
"lastoccurrence": "2026-07-18 08:39:33",
"status": 2,
"sourceid": 1,
"sourceeventid": "71978",
"sourceciname": "82.12.161.32_kylin",
"sourcealertkey": "vfs.dev.util[vdb]",
"sourceseverity": "High",
"sourceidentifier": "82.12.161.32_kylin_vfs.dev.util[vdb]_Application:Disk vdb",
"ciinstance": "Application:Disk vdb",
"eventtypeid": "*UNKNOWN*"
}`
function previewOpen(row: EnrichRow | null) {
  previewDraft.value = null
  previewFromEditor.value = false
  previewRuleId.value = row?.id || '__all__'
  previewIngressId.value = (mappedIngresses.value.find((r) => /zabbix/i.test(r.name || '')) || mappedIngresses.value[0])?.id || ''
  previewPayload.value = getSavedPayload() || SAMPLE_PAYLOAD
  previewResult.value = null
  previewMode.value = 'payload'
  drawerVisible.value = true
}
function previewOpenDraft(draft: EnrichInput) {
  previewDraft.value = draft
  previewFromEditor.value = true
  previewIngressId.value = (mappedIngresses.value.find((r) => /zabbix/i.test(r.name || '')) || mappedIngresses.value[0])?.id || ''
  previewPayload.value = getSavedPayload() || SAMPLE_PAYLOAD
  previewResult.value = null
  previewMode.value = 'payload'
  drawerVisible.value = true
}

const PREVIEW_STORAGE_KEY = 'eventide_enrich_preview'
function getSavedPayload(): string { try { return JSON.parse(sessionStorage.getItem(PREVIEW_STORAGE_KEY) || '{}').payload || '' } catch { return '' } }
function persistPayload() { try { sessionStorage.setItem(PREVIEW_STORAGE_KEY, JSON.stringify({ payload: previewPayload.value })) } catch {} }

async function previewExecute() {
  previewLoading.value = true
  previewResult.value = null
  persistPayload()
  try {
    let payloadObj: Record<string, unknown> | undefined
    if (previewMode.value === 'payload') {
      try { payloadObj = JSON.parse(previewPayload.value || '{}') }
      catch (e) { throw new Error('JSON 无效：' + (e as Error).message) }
    }
    const body: PreviewAlertInput = {
      payload: payloadObj,
      annotations: {},
      value: 1,
      severity: 'information',
      rule_name: 'PreviewRule',
    }
    if (previewIngressId.value) body.ingress_id = previewIngressId.value
    if (previewDraft.value) {
      body.rule = previewDraft.value
    } else if (previewRuleId.value === '__all__') {
      body.use_saved = true
    } else {
      const r = enrichments.value.find((x) => x.id === previewRuleId.value)
      if (r) {
        body.rule = {
          name: r.name, kind: r.kind, matchers: r.matchers || {},
          match_key: r.match_key || '', templates: r.templates || {},
          mappings: r.mappings || {},
          lookup_table_ids: r.lookup_table_ids || [],
          lookup_match_keys: r.lookup_match_keys || {},
          field_templates: r.field_templates || {},
          label_extracts: r.label_extracts || {},
          write_labels: r.write_labels !== false,
          enabled: r.enabled !== false, priority: r.priority ?? 100,
        }
      }
    }
    const raw = (await previewEnrich(body)) as unknown as PreviewEnrichResp & Record<string, unknown>
    previewResult.value = {
      ok: true,
      labels: (raw.labels || {}) as Record<string, string>,
      annotations: (raw.annotations || {}) as Record<string, string>,
      severity: raw.severity,
      parsed_via: raw.parsed_via,
      before: raw.before as Record<string, unknown> | undefined,
    }
  } catch (e) {
    let msg = String((e as Error)?.message || e || '未知错误')
    try {
      const extra = e as { status?: number; data?: unknown }
      if (extra && typeof extra === 'object' && extra.status) {
        msg = `[${extra.status}] ${msg}`
      }
      if (extra && typeof extra === 'object' && extra.data) {
        const d = extra.data as Record<string, unknown> | null
        if (d && typeof d === 'object') {
          const de = d.detail
          const deStr = typeof de === 'string' ? de : (de ? JSON.stringify(de, null, 2) : '')
          const er = d.error
          const erStr = typeof er === 'string' ? er : (er && typeof er === 'object' ? JSON.stringify(er) : '')
          if (deStr && msg.indexOf(deStr) === -1) msg = `${msg} · ${deStr}`
          if (erStr && msg.indexOf(erStr) === -1) msg = `${msg} · ${erStr}`
          const rawFb = JSON.stringify(d)
          if (rawFb && rawFb !== '{}' && rawFb.length < 600 && msg.indexOf(rawFb) === -1) {
            msg = `${msg}\nraw: ${rawFb}`
          }
        }
      }
    } catch {}
    previewResult.value = { ok: false, error: msg }
  } finally { previewLoading.value = false }
}
function previewBackToEditor() {
  persistPayload()
  drawerVisible.value = false
  // 草稿模式下，编辑对话框本来就是打开的，不需要额外操作
  // 非草稿模式（从列表点试跑后点返回）：不用动作，已经关了抽屉
}

interface MappingRow { match_value: string; labels: KvRow[] }
interface DialogState {
  visible: boolean; mode: 'create' | 'edit'; id: string; name: string
  kind: EnrichKind; enabled: boolean; priority: number
  matchers: KvRow[]; label_extracts: KvRow[]
  write_labels: boolean; match_key: string; mappings: MappingRow[]
  lookup_table_ids: string[]; lookup_match_keys: Record<string, string>
  ft_summary: string; ft_ip: string; ft_severity: string; ft_alertname: string
  templates_extra: string
}
function emptyMappingRow(): MappingRow { return { match_value: '', labels: emptyKv() } }
function emptyDialog(): DialogState {
  return {
    visible: false, mode: 'create', id: '', name: '',
    kind: 'Composite', enabled: true, priority: 100,
    matchers: emptyKv(), label_extracts: emptyKv(),
    write_labels: true, match_key: '',
    mappings: [emptyMappingRow()], lookup_table_ids: [],
    lookup_match_keys: {},
    ft_summary: '', ft_ip: '', ft_severity: '', ft_alertname: '',
    templates_extra: '',
  }
}
const dlg = reactive<DialogState>(emptyDialog())
const dlgFormRef = ref<FormInstance | null>(null)
const helpPanel = ref<string[]>([])
const dlgFormRules: FormRules<DialogState> = {
  name: [{ required: true, message: '请输入规则名称', trigger: 'blur' }],
}
function kvRowAdd(arr: KvRow[]) { arr.push({ key: '', value: '' }) }
function kvRowRemove(arr: KvRow[], idx: number) {
  if (arr.length <= 1) arr[0] = { key: '', value: '' }
  else arr.splice(idx, 1)
}
function mappingAddRow() { dlg.mappings.push(emptyMappingRow()) }
function mappingRemoveRow(idx: number) {
  if (dlg.mappings.length <= 1) dlg.mappings[0] = emptyMappingRow()
  else dlg.mappings.splice(idx, 1)
}
function mappingsRecordToRows(m: Record<string, Record<string, string>> | undefined): MappingRow[] {
  if (!m || Object.keys(m).length === 0) return [emptyMappingRow()]
  return Object.entries(m).map(([mv, labels]) => ({ match_value: mv, labels: mapToRows(labels) }))
}
function mappingRowsToRecord(rows: MappingRow[]): Record<string, Record<string, string>> {
  const out: Record<string, Record<string, string>> = {}
  for (const r of rows) {
    if (!r.match_value.trim()) continue
    const lm = rowsToMap(r.labels)
    if (Object.keys(lm).length > 0) out[r.match_value.trim()] = lm
  }
  return out
}
function openCreate() {
  Object.assign(dlg, emptyDialog())
  dlg.visible = true
}
async function openEdit(id: string) {
  let e: EnrichRow | null = null
  try { e = await getEnrich(id) }
  catch (err) { ElMessage.error(`读取失败：${String((err as Error)?.message || err)}`); return }
  Object.assign(dlg, emptyDialog())
  dlg.visible = true; dlg.mode = 'edit'; dlg.id = e.id
  dlg.name = e.name; dlg.kind = e.kind || 'Composite'; dlg.enabled = e.enabled
  dlg.priority = typeof e.priority === 'number' ? e.priority : 100
  dlg.matchers = mapToRows(e.matchers || {})
  dlg.label_extracts = mapToRows(e.label_extracts || {})
  dlg.write_labels = e.write_labels !== false
  dlg.match_key = e.match_key || ''
  dlg.mappings = mappingsRecordToRows(e.mappings || {})
  dlg.lookup_table_ids = e.lookup_table_ids ? [...e.lookup_table_ids] : []
  dlg.lookup_match_keys = { ...(e.lookup_match_keys || {}) }
  const ft = e.field_templates || {}
  const tpl = { ...(e.templates || {}) }
  dlg.ft_summary = ft.summary || tpl.summary || ''
  delete tpl.summary
  dlg.ft_ip = ft.ip || ft.alertIp || ''
  dlg.ft_severity = ft.severity || ''
  dlg.ft_alertname = ft.alertname || ''
  dlg.templates_extra = Object.keys(tpl).length ? JSON.stringify(tpl, null, 2) : ''
}

let _lastFocusedEl: HTMLInputElement | HTMLTextAreaElement | null = null
function rememberFocus(evt: FocusEvent) {
  const t = evt.target as HTMLElement
  if (t && (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA')) {
    _lastFocusedEl = t as HTMLInputElement | HTMLTextAreaElement
  }
}
function insertAtCursor(el: HTMLInputElement | HTMLTextAreaElement | null, text: string) {
  if (!el) { ElMessage.warning('请先点击一个输入框再插入芯片'); return }
  const start = el.selectionStart ?? el.value.length
  const end = el.selectionEnd ?? start
  const v = el.value
  el.value = v.slice(0, start) + text + v.slice(end)
  el.dispatchEvent(new Event('input', { bubbles: true }))
  void nextTick(() => {
    const pos = start + text.length
    el.setSelectionRange(pos, pos)
    el.focus()
  })
}
function onChipClick(chip: string) {
  insertAtCursor(_lastFocusedEl, chip)
}
function onChipDragStart(e: DragEvent, chip: string) {
  if (e.dataTransfer) {
    e.dataTransfer.effectAllowed = 'copy'
    e.dataTransfer.setData('text/plain', chip)
  }
}
function onDropInsert(e: DragEvent) {
  e.preventDefault()
  const text = e.dataTransfer?.getData('text/plain') || ''
  if (!text) return
  const t = e.currentTarget as HTMLElement
  const el = (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA')
    ? t as HTMLInputElement | HTMLTextAreaElement
    : (t.querySelector('input, textarea') as HTMLInputElement | HTMLTextAreaElement | null)
  insertAtCursor(el || _lastFocusedEl, text)
}
function syncLookupKey(id: string) {
  if (dlg.lookup_table_ids.includes(id)) {
    if (!dlg.lookup_match_keys[id]) {
      const t = lookupMap.value.get(id)
      dlg.lookup_match_keys[id] = t?.key_label || 'ip'
    }
  } else {
    delete dlg.lookup_match_keys[id]
  }
}
const dlgChips = computed(() => {
  const parts: { group: string; chips: { label: string; value: string }[] }[] = []
  parts.push({
    group: '告警自带标签',
    chips: BUILTIN_CHIP_FIELDS.map((k) => ({
      label: `{{labels.${k}}}`, value: `{{labels.${k}}}`,
    })),
  })
  for (const id of dlg.lookup_table_ids) {
    const t = lookupMap.value.get(id)
    if (!t) continue
    const ns = lookupNs(t.name)
    const cols = colsFromLookup(t)
    const keyLabel = dlg.lookup_match_keys[id] || t.key_label || 'ip'
    parts.push({
      group: `${t.name} · 匹配 labels.${keyLabel} → labels.${ns}.*`,
      chips: cols.map((col) => ({
        label: `{{labels.${ns}.${col}}}`,
        value: `{{labels.${ns}.${col}}}`,
      })),
    })
  }
  return parts
})

function buildDraft(): EnrichInput {
  let extra: Record<string, string> = {}
  const rawExtra = dlg.templates_extra.trim()
  if (rawExtra) extra = JSON.parse(rawExtra)
  const summary = dlg.ft_summary.trim()
  const templates = { ...extra }
  if (summary) templates.summary = summary
  const field_templates: Record<string, string> = {}
  if (summary) field_templates.summary = summary
  const ip = dlg.ft_ip.trim()
  const severity = dlg.ft_severity.trim()
  const alertname = dlg.ft_alertname.trim()
  if (ip) field_templates.ip = ip
  if (severity) field_templates.severity = severity
  if (alertname) field_templates.alertname = alertname
  return {
    name: dlg.name.trim() || 'preview',
    kind: dlg.kind, enabled: dlg.enabled,
    matchers: rowsToMap(dlg.matchers),
    match_key: dlg.match_key.trim(),
    templates, mappings: mappingRowsToRecord(dlg.mappings),
    lookup_table_ids: dlg.lookup_table_ids.filter(Boolean),
    lookup_match_keys: dlg.lookup_match_keys,
    field_templates, label_extracts: rowsToMap(dlg.label_extracts),
    write_labels: dlg.write_labels, priority: dlg.priority,
  }
}
function tryDraft() {
  let draft: EnrichInput
  try { draft = buildDraft() }
  catch (e) { ElMessage.error('高级 JSON 无效：' + (e as Error).message); return }
  previewOpenDraft(draft)
}
async function onDlgSave() {
  const valid = await dlgFormRef.value?.validate().catch(() => false)
  if (!valid) return
  let body: EnrichInput
  try { body = buildDraft() }
  catch (e) { ElMessage.error('高级 JSON 无效：' + (e as Error).message); return }
  body.name = dlg.name.trim()
  const hasAny = (body.lookup_table_ids || []).length
    || Object.keys(body.mappings || {}).length
    || Object.keys(body.templates || {}).length
    || Object.keys(body.field_templates || {}).length
    || Object.keys(body.label_extracts || {}).length
  if (!hasAny) { ElMessage.error('请至少勾选台账，或配置描述 / IP / 级别'); return }
  try {
    if (dlg.mode === 'create') { await createEnrich(body); ElMessage.success('已创建') }
    else { await updateEnrich(dlg.id, body); ElMessage.success('已更新') }
    dlg.visible = false; await loadAll()
  } catch (e) {
    ElMessage.error(`保存失败：${String((e as Error)?.message || e)}`)
  }
}
async function onDelete(row: EnrichRow) {
  try {
    await ElMessageBox.confirm(`确定删除「${row.name}」？`, '删除', { type: 'warning' })
  } catch { return }
  try { await deleteEnrich(row.id); ElMessage.success('已删除'); await loadAll() }
  catch (e) { ElMessage.error(`删除失败：${String((e as Error)?.message || e)}`) }
}
function goLookups() { router.push({ name: 'lookups' }).catch(() => router.push({ path: '/lookups' }).catch(() => {})) }
</script>
<template>
  <div class="enrichments-view" @focusin="rememberFocus">
    <div class="page-header">
      <h2 class="page-title">告警丰富</h2>
      <p class="page-sub">选台账补字段，再决定告警上显示的描述、IP 和级别。保存前可试跑。</p>
    </div>

    <div class="enrich-guide panel">
      <div class="enrich-guide-steps">
        <div class="enrich-guide-step" v-for="(s, i) in STEP_GUIDE" :key="i">
          <span class="step-n">{{ i + 1 }}</span>
          <div><strong>{{ s.title }}</strong><p class="step-desc">{{ s.desc }}</p></div>
        </div>
      </div>
    </div>


        <ElAffix :offset="0" class="affix-wrap">
          <div class="toolbar panel">
            <div class="tb-left">
              <ElButton :icon="VideoPlay" @click="previewOpen(null)">试跑预览</ElButton>
              <ElButton type="primary" :icon="Plus" @click="openCreate"
                :disabled="!lookups.length">新建丰富规则</ElButton>
              <ElButton :icon="Refresh" @click="loadAll" :disabled="loading">刷新</ElButton>
            </div>
            <div class="tb-right">
              <ElSelect v-model="filterEnabled" placeholder="状态" clearable style="width:120px">
                <ElOption label="已启用" value="true" />
                <ElOption label="已停用" value="false" />
              </ElSelect>
              <ElInput v-model="filterQ" placeholder="搜索名称" clearable style="width:220px" :prefix-icon="Search" />
            </div>
          </div>
        </ElAffix>

        <ElTable :data="pagedEnrichments" v-loading="loading" stripe class="enrich-table" style="width:100%;margin-top:12px"
          empty-text="暂无丰富规则">
          <ElTableColumn label="名称" min-width="180">
            <template #default="{ row }">
              <a class="name-link" @click="openEdit(asEnrich(row).id)">{{ asEnrich(row).name }}</a>
            </template>
          </ElTableColumn>
          <ElTableColumn label="做什么" min-width="280">
            <template #default="{ row }">{{ enrichWhat(asEnrich(row)) }}</template>
          </ElTableColumn>
          <ElTableColumn label="作用范围" min-width="200">
            <template #default="{ row }">{{ enrichScope(asEnrich(row)) }}</template>
          </ElTableColumn>
          <ElTableColumn label="状态" width="80" align="center">
            <template #default="{ row }">
              <ElSwitch class="en-switch" :model-value="asEnrich(row).enabled"
                @update:model-value="(v) => { asEnrich(row).enabled = Boolean(v); onToggleEnabled(asEnrich(row)) }" />
            </template>
          </ElTableColumn>
          <ElTableColumn label="最近更新" width="170">
            <template #default="{ row }">
              <span class="cell-updated" :title="asEnrich(row).updated_at">{{ formatTime(asEnrich(row).updated_at) }}</span>
            </template>
          </ElTableColumn>
          <ElTableColumn label="操作" width="170" fixed="right">
            <template #default="{ row }">
              <div class="row-actions">
                <ElButton size="small" class="act-btn act-edit" @click="openEdit(asEnrich(row).id)">编辑</ElButton>
                <ElButton size="small" type="danger" plain class="act-btn" @click="onDelete(asEnrich(row))">删除</ElButton>
              </div>
            </template>
          </ElTableColumn>
        </ElTable>

        <div class="panel enrich-pager">
          <div class="pager-tip">共 <span class="mono">{{ filteredEnrichments.length }}</span> 条</div>
          <ElPagination
            v-model:current-page="page"
            v-model:page-size="pageSize"
            :page-sizes="Array.from(PAGE_SIZES)"
            :total="filteredEnrichments.length"
            layout="sizes, prev, pager, next, jumper, ->, total"
            background
            @current-change="onPage"
            @size-change="onSize"
          />
        </div>

        <ElEmpty v-if="!loading && enrichments.length === 0" description="还没有丰富规则">
          <template #extra>
            <ElButton type="primary" :icon="Plus" @click="openCreate"
              :disabled="!lookups.length">{{ lookups.length ? '新建第一条规则' : '先去准备台账' }}</ElButton>
          </template>
        </ElEmpty>


    <ElDrawer v-model="drawerVisible"
      :title="previewFromEditor ? '试跑预览（编辑草稿）' : '试跑预览'"
      direction="rtl" size="1000px" destroy-on-close>
      <ElAlert v-if="previewFromEditor && previewDraft" type="info" :closable="false" show-icon style="margin-bottom:12px"
        :title="`使用当前编辑草稿「${previewDraft.name || '未命名'}」（未保存）`" />
      <ElRow :gutter="16">
        <ElCol :span="12">
          <h3 class="h3">输入</h3>
          <ElForm label-position="top">
            <ElFormItem v-if="!previewDraft" label="选择丰富规则">
              <ElSelect v-model="previewRuleId" filterable style="width:100%">
                <ElOption label="全部已启用规则" value="__all__" />
                <ElOption v-for="r in enrichments" :key="r.id" :label="r.name" :value="r.id" />
              </ElSelect>
            </ElFormItem>
            <ElFormItem v-else label="当前草稿规则">
              <ElInput :model-value="previewDraft.name" disabled />
            </ElFormItem>
            <ElFormItem label="接入源（字段映射）">
              <ElSelect v-model="previewIngressId" filterable clearable style="width:100%"
                placeholder="选择接入源">
                <ElOption label="自动 / 仅 labels 对象" value="" />
                <ElOption v-for="ig in allIngresses" :key="ig.id"
                  :label="ig.name + '（' + (ig.kind || 'unknown') + '）' + (mappedIngresses.some((m) => m.id === ig.id) ? ' · 已配字段映射' : '')"
                  :value="ig.id" />
              </ElSelect>
            </ElFormItem>
            <ElFormItem label="原始告警 JSON" style="margin-top:8px">
              <ElInput v-model="previewPayload" type="textarea" :rows="14"
                placeholder="粘贴原始告警 JSON" class="mono-font" />
              <div class="under-actions">
                <ElButton link type="primary" size="small" @click="previewPayload = SAMPLE_PAYLOAD">填入示例</ElButton>
                <ElButton link type="info" size="small" @click="previewPayload = ''">清空</ElButton>
              </div>
            </ElFormItem>
            <ElFormItem label="payload 持久化" style="margin-top:12px">
              <span style="font-size:12px;color:var(--el-text-color-secondary,#909399)">当前 payload 会存在 sessionStorage，下次打开试跑自动恢复。</span>
            </ElFormItem>
            <ElFormItem style="margin-top:4px">
              <ElButton type="primary" :icon="VideoPlay" :loading="previewLoading" @click="previewExecute">执行预览</ElButton>
              <ElButton v-if="previewFromEditor" @click="previewBackToEditor">返回编辑</ElButton>
            </ElFormItem>
          </ElForm>
        </ElCol>
        <ElCol :span="12">
          <h3 class="h3">结果</h3>
          <div v-loading="previewLoading" class="preview-area">
            <template v-if="previewResult && previewResult.ok === false">
              <div class="eval-head">
                <ElTag type="danger" size="large" effect="dark" class="tag-big">失败</ElTag>
                <span class="err-msg">{{ previewResult.error }}</span>
              </div>
            </template>
            <template v-else-if="previewResult && previewResult.ok">
              <div class="eval-head">
                <ElTag type="success" size="large" effect="dark" class="tag-big">成功</ElTag>
                <span class="parsed-tag">解析链路: {{ previewResult.parsed_via || '—' }}</span>
              </div>
              <div class="summary-box">
                <div class="summary-label">告警描述</div>
                <div class="summary-body">
                  {{ previewResult.annotations?.summary || previewResult.annotations?.description || '（未生成描述）' }}
                </div>
              </div>
              <div class="metric-cards">
                <div class="metric-card">
                  <span class="metric-label">告警 IP</span>
                  <span class="metric-value mono">{{ previewResult.labels?.alertIp
                    || previewResult.labels?.ip
                    || previewResult.labels?.instance || '—' }}</span>
                </div>
                <div class="metric-card">
                  <span class="metric-label">级别</span>
                  <span class="metric-value mono">{{ previewResult.labels?.severity || previewResult.severity || '—' }}</span>
                </div>
                <div class="metric-card">
                  <span class="metric-label">名称</span>
                  <span class="metric-value mono">{{ previewResult.labels?.alertname || '—' }}</span>
                </div>
              </div>
              <div class="diff-title">丰富前 Before</div>
              <pre class="json-pre">{{ prettyJson(previewResult.before || {}) }}</pre>
              <div class="diff-title">丰富后 After · labels / annotations / severity</div>
              <pre class="json-pre">{{ prettyJson({
                labels: previewResult.labels || {},
                annotations: previewResult.annotations || {},
                severity: previewResult.severity,
              }) }}</pre>
            </template>
            <template v-else-if="!previewLoading">
              <ElEmpty description="点击执行预览查看结果" />
            </template>
          </div>
        </ElCol>
      </ElRow>
    </ElDrawer>

    <ElDialog v-model="dlg.visible"
      :title="dlg.mode === 'create' ? '新建丰富规则' : '编辑丰富规则 · ' + dlg.name"
      width="1100px" :close-on-click-modal="false" destroy-on-close top="4vh">
      <ElCollapse v-model="helpPanel">
        <ElCollapseItem title="帮助说明" name="help">
          <div class="help-content" v-pre>
            <h4>执行顺序</h4>
            <p>matchers 过滤 → label_extracts 抽取 → lookup 台账补字段 → mappings 枚举映射 → field_templates 写卡片字段 → templates 写注解。priority 越小越早执行。</p>
            <h4>模板变量</h4>
            <table class="help-table">
              <tr><th>变量</th><th>含义</th></tr>
              <tr><td><code>{{labels.ip}}</code></td><td>告警原始标签 ip（或 alertIp / instance）</td></tr>
              <tr><td><code>{{labels.instance}}</code></td><td>原始 instance 标签</td></tr>
              <tr><td><code>{{labels.alertname}}</code></td><td>告警名称</td></tr>
              <tr><td><code>{{labels.severity}}</code></td><td>原始级别</td></tr>
              <tr><td><code v-pre>{{labels.台账名.列名}}</code></td><td>从台账查到的字段（台账需先勾选）</td></tr>
            </table>
            <h4>字符串截取语法</h4>
            <table class="help-table">
              <tr><th>语法</th><th>示例</th><th>效果</th></tr>
              <tr><td><code>|before:X</code></td><td><code>{{labels.instance}|before::}</code></td><td>取冒号前部分</td></tr>
              <tr><td><code>|after:X</code></td><td><code>{{labels.source}|after:-}</code></td><td>取 X 后部分</td></tr>
              <tr><td><code>|split:X|0</code></td><td><code>{{labels.instance}|split::|0}</code></td><td>按 X 分割取第 N 段</td></tr>
              <tr><td><code>|between:A:B</code></td><td><code>{{summary}|between:[:]</code></td><td>取 A 和 B 之间</td></tr>
            </table>
          </div>
        </ElCollapseItem>
      </ElCollapse>

      <ElForm ref="dlgFormRef" :model="dlg" :rules="dlgFormRules" label-position="top" style="margin-top:12px">
        <div class="step-section">
          <div class="step-title"><span class="step-badge">1</span> 基本信息与作用范围</div>
          <ElRow :gutter="16">
            <ElCol :span="12">
              <ElFormItem label="规则名称" prop="name">
                <ElInput v-model="dlg.name" placeholder="如：CMDB 主机信息补齐" />
              </ElFormItem>
            </ElCol>
            <ElCol :span="4">
              <ElFormItem label="启用">
                <ElSwitch v-model="dlg.enabled" />
              </ElFormItem>
            </ElCol>
            <ElCol :span="4">
              <ElFormItem label="优先级">
                <ElInputNumber v-model="dlg.priority" :min="0" :step="10" controls-position="right" style="width:100%" />
              </ElFormItem>
            </ElCol>
            <ElCol :span="4">
              <ElFormItem label="类型">
                <ElSelect v-model="dlg.kind" style="width:100%">
                  <ElOption v-for="(v, k) in KIND_NAMES" :key="k" :label="v" :value="k" />
                </ElSelect>
              </ElFormItem>
            </ElCol>
          </ElRow>
          <div class="kv-section">
            <div class="kv-title">
              <span>matchers（空 = 全部告警）</span>
              <ElButton size="small" :icon="Plus" link type="primary" @click="kvRowAdd(dlg.matchers)">新增</ElButton>
            </div>
            <div v-for="(r, idx) in dlg.matchers" :key="'dm-'+idx" class="kv-row">
              <ElInput v-model="r.key" placeholder="key 如 alertname" style="width:30%;margin-right:8px" />
              <ElInput v-model="r.value" placeholder="value（支持 ~regex）" style="width:50%;margin-right:8px" />
              <ElButton link type="danger" :icon="Delete" @click="kvRowRemove(dlg.matchers, idx)">删除</ElButton>
            </div>
          </div>
        </div>

        <div class="step-section">
          <div class="step-title"><span class="step-badge">2</span> 查表前抽取标签</div>
          <p class="step-hint">从原始标签中派生新标签备用（执行顺序先于台账查找）。</p>
          <div class="kv-section">
            <div class="kv-title">
              <span>label_extracts（新标签名 → 模板）</span>
              <ElButton size="small" :icon="Plus" link type="primary" @click="kvRowAdd(dlg.label_extracts)">新增</ElButton>
            </div>
            <div v-for="(r, idx) in dlg.label_extracts" :key="'dl-'+idx" class="kv-row">
              <ElInput v-model="r.key" placeholder="新标签名" style="width:25%;margin-right:8px" />
              <ElInput v-model="r.value" placeholder="模板如 {{labels.instance}|split::|0}"
                style="width:55%;margin-right:8px" @drop.prevent="onDropInsert($event)" @dragover.prevent />
              <ElButton link type="danger" :icon="Delete" @click="kvRowRemove(dlg.label_extracts, idx)">删除</ElButton>
            </div>
          </div>
        </div>

        <div class="step-section">
          <div class="step-title"><span class="step-badge">3</span> 从台账补字段</div>
          <p class="step-hint">勾选要关联的台账。每个台账用指定标签去匹配，查到的字段在后续模板中以
            <code v-pre>{{labels.台账名.列名}}</code> 引用。</p>
          <div v-if="!lookups.length" class="empty-lookups">
            <ElAlert type="warning" :closable="false" show-icon
              title="还没有台账数据，请先到「台账数据」Tab 创建。" />
            <ElButton type="primary" size="small" @click="goLookups">去创建台账</ElButton>
          </div>
          <table v-else class="lookup-check-table">
            <thead><tr>
              <th style="width:40px"></th>
              <th>台账名称</th>
              <th>用哪个标签匹配</th>
            </tr></thead>
            <tbody>
              <tr v-for="t in lookups" :key="t.id">
                <td style="text-align:center">
                  <ElCheckbox :model-value="dlg.lookup_table_ids.includes(t.id)"
                    @update:model-value="(v) => {
                      if (v && !dlg.lookup_table_ids.includes(t.id)) dlg.lookup_table_ids.push(t.id)
                      else if (!v) {
                        const i = dlg.lookup_table_ids.indexOf(t.id)
                        if (i >= 0) dlg.lookup_table_ids.splice(i, 1)
                      }
                      syncLookupKey(t.id)
                    }" />
                </td>
                <td>{{ t.name }}</td>
                <td>
                  <ElInput v-if="dlg.lookup_table_ids.includes(t.id)"
                    v-model="dlg.lookup_match_keys[t.id]"
                    placeholder="如 ip / instance / host"
                    style="width:200px" size="small" />
                  <span v-else class="text-muted">{{ t.key_label || 'ip' }}</span>
                </td>
              </tr>
            </tbody>
          </table>
        </div>

        <div class="step-section">
          <div class="step-title"><span class="step-badge">4</span> 写到告警卡片上</div>
          <p class="step-hint">下面芯片可点击或拖拽到输入框光标处插入。</p>

          <div class="chips-bar">
            <div class="chips-group" v-for="(grp, gi) in dlgChips" :key="gi">
              <span class="chips-group-label">{{ grp.group }}</span>
              <span v-for="(c, ci) in grp.chips" :key="gi+'-'+ci"
                draggable="true" class="chip"
                :title="'点击或拖拽插入: '+c.value"
                @click="onChipClick(c.value)"
                @dragstart="onChipDragStart($event, c.value)">{{ c.label }}</span>
            </div>
          </div>

          <ElRow :gutter="16">
            <ElCol :span="24">
              <ElFormItem label="告警描述">
                <ElInput v-model="dlg.ft_summary" type="textarea" :rows="3"
                  placeholder="如：{{labels.主机.hostname}} {{labels.instance}} 磁盘 IO 超阈值"
                  @drop.prevent="onDropInsert($event)" @dragover.prevent />
              </ElFormItem>
            </ElCol>
          </ElRow>
          <ElRow :gutter="16">
            <ElCol :span="8">
              <ElFormItem label="告警 IP">
                <ElInput v-model="dlg.ft_ip"
                  placeholder="{{labels.instance}|split::|0"
                  @drop.prevent="onDropInsert($event)" @dragover.prevent />
              </ElFormItem>
            </ElCol>
            <ElCol :span="8">
              <ElFormItem label="告警级别">
                <ElInput v-model="dlg.ft_severity"
                  placeholder="{{labels.severity}}"
                  @drop.prevent="onDropInsert($event)" @dragover.prevent />
              </ElFormItem>
            </ElCol>
            <ElCol :span="8">
              <ElFormItem label="告警名称">
                <ElInput v-model="dlg.ft_alertname"
                  placeholder="{{labels.alertname}}"
                  @drop.prevent="onDropInsert($event)" @dragover.prevent />
              </ElFormItem>
            </ElCol>
          </ElRow>
        </div>

        <ElCollapse>
          <ElCollapseItem title="高级选项（内联映射 / 其他注解 / write_labels）" name="advanced">
            <ElRow :gutter="16">
              <ElCol :span="8">
                <ElFormItem label="内联映射匹配键 (match_key)">
                  <ElInput v-model="dlg.match_key" placeholder="如 severity / job / source" />
                </ElFormItem>
              </ElCol>
              <ElCol :span="8">
                <ElFormItem label="write_labels">
                  <ElSwitch v-model="dlg.write_labels" />
                  <span class="form-tip" style="margin-left:8px">是否写回 labels</span>
                </ElFormItem>
              </ElCol>
            </ElRow>
            <div class="kv-section">
              <div class="kv-title">
                <span>内联映射（match_key 的值 → 一组 labels）</span>
                <ElButton size="small" :icon="Plus" link type="primary" @click="mappingAddRow()">新增匹配行</ElButton>
              </div>
              <div v-for="(row, idx) in dlg.mappings" :key="'mr-'+idx" class="mapping-block">
                <div class="mapping-head">
                  <span class="mapping-match-label">匹配值:</span>
                  <ElInput v-model="row.match_value" placeholder="如 P1 / critical" style="width:200px;margin-right:12px" />
                  <ElButton link type="danger" :icon="Delete" @click="mappingRemoveRow(idx)">删除</ElButton>
                </div>
                <div class="kv-title" style="margin-top:6px">
                  <span>命中后写的 labels</span>
                  <ElButton size="small" :icon="Plus" link type="primary" @click="kvRowAdd(row.labels)">新增</ElButton>
                </div>
                <div v-for="(r, i2) in row.labels" :key="'mrk-'+idx+'-'+i2" class="kv-row">
                  <ElInput v-model="r.key" placeholder="label key" style="width:25%;margin-right:8px" />
                  <ElInput v-model="r.value" placeholder="值或模板" style="width:55%;margin-right:8px"
                    @drop.prevent="onDropInsert($event)" @dragover.prevent />
                  <ElButton link type="danger" :icon="Delete" @click="kvRowRemove(row.labels, i2)">删除</ElButton>
                </div>
              </div>
            </div>
            <ElFormItem label="其他注解模板（JSON，如 description / runbook_url）" style="margin-top:10px">
              <ElInput v-model="dlg.templates_extra" type="textarea" :rows="4"
                placeholder='{"description":"{{labels.summary}}","runbook_url":"https://wiki/runbook/{{labels.alertname}}"}'
                class="mono-font" />
            </ElFormItem>
          </ElCollapseItem>
        </ElCollapse>
      </ElForm>

      <template #footer>
        <div class="dlg-footer">
          <div class="dlg-footer-left">
            <ElButton :icon="VideoPlay" type="success" plain @click="tryDraft">试跑预览</ElButton>
          </div>
          <div class="dlg-footer-right">
            <ElButton @click="dlg.visible = false">取消</ElButton>
            <ElButton type="primary" @click="onDlgSave">保存</ElButton>
          </div>
        </div>
      </template>
    </ElDialog>
  </div>
</template>
<style scoped>
.enrichments-view { padding: 16px 20px 32px; }
.page-header { margin-bottom: 10px; }
.page-title { margin: 0 0 4px; font-size: 22px; }
.page-sub { margin: 0; color: var(--el-text-color-secondary, #909399); font-size: 13px; }
.panel { background: var(--el-bg-color, #fff); border: 1px solid var(--el-border-color-lighter, #ebeef5); border-radius: 6px; }
.enrich-pager {
  display: flex; align-items: center; justify-content: space-between;
  flex-wrap: wrap; gap: 12px;
  padding: 12px 18px; margin: 14px 0;
  border: 1px solid var(--el-border-color-lighter, #ebeef5);
  background: var(--el-bg-color, #fff);
  border-radius: 8px;
  box-shadow: 0 1px 2px rgba(0,0,0,0.03);
}
.enrich-pager .pager-tip { font-size: 13px; color: var(--el-text-color-secondary, #909399); }
.enrich-pager .mono {
  font-family: Consolas, Menlo, monospace;
  font-weight: 600; color: var(--el-text-color-primary, #303133); padding: 0 2px;
}
.enrich-guide { padding: 14px 16px; margin-bottom: 12px; display: flex; align-items: center; justify-content: space-between; gap: 16px; }
.enrich-guide-steps { display: flex; gap: 24px; }
.enrich-guide-step { display: flex; align-items: flex-start; gap: 8px; }
.step-n { display: inline-flex; align-items: center; justify-content: center; width: 26px; height: 26px; border-radius: 50%; background: var(--el-color-primary, #409eff); color: #fff; font-size: 14px; font-weight: 700; flex-shrink: 0; }
.step-desc { margin: 2px 0 0; font-size: 12px; color: var(--el-text-color-secondary, #909399); }
.main-tabs :deep(.el-tabs__item) { font-weight: 600; font-size: 14px; }
.affix-wrap { z-index: 10; }
.toolbar { display: flex; align-items: center; justify-content: space-between; padding: 10px 12px; gap: 8px; flex-wrap: wrap; }
.tb-left, .tb-right { display: flex; gap: 8px; align-items: center; }
.name-link { color: var(--el-color-primary, #409eff); cursor: pointer; font-weight: 600; }
.pill-sm { height: 22px; padding: 0 8px; font-size: 11.5px; border-radius: 4px; display: inline-flex; align-items: center; }
.mono { font-family: 'SFMono-Regular', Consolas, Menlo, monospace; font-size: 12.5px; }
.h3 { margin: 0 0 10px; font-size: 15px; }
.preview-area { min-height: 400px; }
.preview-mode { padding: 2px 0; }
.under-actions { display: flex; gap: 8px; margin-top: 4px; }
.mono-font :deep(textarea) { font-family: 'SFMono-Regular', Consolas, Menlo, monospace; font-size: 12.5px; line-height: 1.5; }
.summary-box {
  border: 1px solid var(--el-border-color, #dcdfe6);
  border-radius: 6px;
  padding: 8px 12px;
  margin: 12px 0 4px;
  background: var(--el-fill-color-lighter, #fafafa);
}
.summary-label { font-size: 11px; color: var(--el-text-color-secondary, #909399); margin-bottom: 3px; }
.summary-body { font-size: 13.5px; color: var(--el-text-color-primary, #303133); line-height: 1.55; word-break: break-all; }
.parsed-tag { font-size: 12px; color: var(--el-text-color-regular, #606266); }
.eval-head { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
.tag-big { font-weight: 600; padding: 6px 14px; }
.err-msg { color: #f56c6c; margin-left: 8px; font-size: 13px; }
.metric-cards { display: flex; gap: 10px; margin: 12px 0; }
.metric-card { flex: 1; border: 1px solid var(--el-border-color, #dcdfe6); border-radius: 6px; padding: 8px 12px; text-align: center; }
.metric-label { display: block; font-size: 11px; color: var(--el-text-color-secondary, #909399); }
.metric-value { display: block; font-size: 14px; font-weight: 600; margin-top: 2px; word-break: break-all; }
.diff-title { font-size: 12px; color: var(--el-text-color-regular, #606266); margin: 8px 0 4px; padding: 2px 6px; background: var(--el-fill-color-light, #f5f7fa); border-radius: 3px; }
.json-pre { background: #1e1e1e; color: #d4d4d4; padding: 10px; border-radius: 4px; max-height: 360px; overflow: auto; font-size: 12px; line-height: 1.45; white-space: pre-wrap; word-break: break-all; margin: 0; }
.parsed-via { margin-top: 8px; font-size: 12px; color: var(--el-text-color-secondary, #909399); }
.help-content h4 { margin: 10px 0 4px; font-size: 13px; }
.help-content p { margin: 0 0 8px; font-size: 12.5px; line-height: 1.6; color: var(--el-text-color-regular, #606266); }
.help-content code { background: #eef; padding: 1px 4px; border-radius: 3px; font-size: 12px; }
.help-table { width: 100%; border-collapse: collapse; margin-bottom: 10px; font-size: 12.5px; }
.help-table th { background: var(--el-fill-color-light, #f5f7fa); padding: 4px 8px; text-align: left; border: 1px solid var(--el-border-color-lighter, #ebeef5); }
.help-table td { padding: 4px 8px; border: 1px solid var(--el-border-color-lighter, #ebeef5); }
.help-table code { font-size: 11.5px; }
.step-section { border: 1px solid var(--el-border-color-lighter, #ebeef5); border-radius: 6px; padding: 12px 16px; margin-bottom: 12px; }
.step-title { font-size: 14px; font-weight: 600; margin-bottom: 8px; display: flex; align-items: center; gap: 6px; }
.step-badge { display: inline-flex; align-items: center; justify-content: center; width: 22px; height: 22px; border-radius: 50%; background: var(--el-color-primary, #409eff); color: #fff; font-size: 12px; font-weight: 700; }
.step-hint { margin: 0 0 8px; font-size: 12.5px; color: var(--el-text-color-secondary, #909399); }
.step-hint code { background: #eef; padding: 1px 4px; border-radius: 3px; }
.kv-section { margin-top: 4px; }
.kv-title { display: flex; justify-content: space-between; align-items: center; margin-bottom: 6px; font-size: 13px; font-weight: 600; }
.kv-row { display: flex; align-items: center; margin-bottom: 6px; }
.form-tip { font-size: 12px; color: var(--el-text-color-secondary, #909399); }
.empty-lookups { display: flex; flex-direction: column; gap: 8px; align-items: flex-start; }
.text-muted { color: var(--el-text-color-secondary, #909399); font-size: 12.5px; }
.lookup-check-table { width: 100%; border-collapse: collapse; font-size: 13px; }
.lookup-check-table th { background: var(--el-fill-color-light, #f5f7fa); padding: 6px 8px; text-align: left; border: 1px solid var(--el-border-color-lighter, #ebeef5); font-weight: 600; }
.lookup-check-table td { padding: 6px 8px; border: 1px solid var(--el-border-color-lighter, #ebeef5); }
.chips-bar { border: 1px dashed var(--el-border-color, #dcdfe6); border-radius: 4px; padding: 8px 10px; background: var(--el-fill-color-light, #f5f7fa); margin-bottom: 10px; display: flex; flex-direction: column; gap: 6px; }
.chips-group { display: flex; flex-wrap: wrap; align-items: center; gap: 6px; }
.chips-group-label { font-size: 11.5px; font-weight: 600; color: var(--el-text-color-secondary, #909399); margin-right: 4px; }
.chip { display: inline-flex; align-items: center; height: 24px; padding: 0 8px; border: 1px solid var(--el-border-color, #dcdfe6); background: var(--el-bg-color, #fff); border-radius: 4px; font-size: 12px; color: var(--el-text-color-regular, #606266); cursor: grab; user-select: none; transition: all .12s; font-family: 'SFMono-Regular', Consolas, Menlo, monospace; }
.chip:hover { color: var(--el-color-primary, #409eff); border-color: var(--el-color-primary, #409eff); background: var(--el-color-primary-light-9, #ecf5ff); }
.chip:active { cursor: grabbing; }
.mapping-block { border: 1px dashed #dcdfe6; border-radius: 4px; padding: 10px; margin-bottom: 8px; background: var(--el-fill-color-light, #f5f7fa); }
.mapping-head { display: flex; align-items: center; margin-bottom: 8px; flex-wrap: wrap; gap: 6px; }
.mapping-match-label { font-size: 13px; font-weight: 600; }
.dlg-footer { display: flex; align-items: center; justify-content: space-between; width: 100%; }
.dlg-footer-left, .dlg-footer-right { display: flex; gap: 8px; align-items: center; }
/* === 列表紧凑样式（与通知渠道一致） === */
.enrich-table {
  font-size: 13px; overflow: hidden;
}
.enrich-table :deep(.el-table__inner-wrapper::before) { display: none; }
.enrich-table :deep(.el-table th.el-table__cell) { background: transparent; }
.enrich-table :deep(.el-table td.el-table__cell),
.enrich-table :deep(.el-table th.el-table__cell.is-leaf) {
  border-bottom: 1px solid var(--el-border-color-lighter, #f2f3f5);
}
.enrich-table :deep(.el-table th.el-table__cell .cell) { padding-top: 4px; padding-bottom: 4px; }
.enrich-table :deep(.el-table td.el-table__cell .cell) { padding-top: 8px; padding-bottom: 8px; }
.cell-updated { font-family: Consolas, Menlo, monospace; font-size: 12.5px; color: var(--el-text-color-secondary, #909399); }
/* 开关：紧凑 40x20 */
.en-switch {
  --el-switch-on-color: var(--el-color-primary, #409eff);
  --el-switch-off-color: var(--el-border-color-darker, #c0c4cc);
  --el-switch-height: 20px;
  --el-switch-width: 40px;
}
.en-switch :deep(.el-switch__action) { height: 18px; width: 18px; }
/* 操作按钮：小胶囊 4px 圆角 */
.row-actions { display: inline-flex; align-items: center; gap: 6px; }
.act-btn {
  height: 26px; padding: 0 12px !important;
  font-size: 12px !important; border-radius: 4px;
}
.act-edit {
  --el-button-border-color: #3f8bff;
  --el-button-text-color: #3f8bff;
  --el-button-hover-bg-color: rgba(63, 139, 255, 0.08);
  --el-button-hover-border-color: #3077ef;
  --el-button-hover-text-color: #3077ef;
}
:deep(.el-switch) { --el-switch-on-color: var(--el-color-primary, #409eff); --el-switch-off-color: #c0c4cc; width: 40px !important; height: 20px !important; }
:deep(.el-switch .el-switch__core) { width: 40px !important; height: 20px !important; border-radius: 20px; }
:deep(.el-switch .el-switch__action) { width: 18px !important; height: 18px !important; }
</style>
