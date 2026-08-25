<script setup lang="ts">
import { computed, markRaw, onMounted, reactive, ref } from 'vue'
import {
  ElButton,
  ElCard,
  ElCol,
  ElDialog,
  ElDivider,
  ElDrawer,
  ElEmpty,
  ElForm,
  ElFormItem,
  ElIcon,
  ElInput,
  ElInputNumber,
  ElMessage,
  ElMessageBox,
  ElOption,
  ElRow,
  ElSelect,
  ElTable,
  ElTableColumn,
  ElTag,
  ElTree,
  ElUpload,
} from 'element-plus'
import type { FormInstance, UploadInstance, UploadProps, UploadRawFile } from 'element-plus'
import {
  ArrowDown,
  CaretBottom,
  CaretRight,
  Download,
  Refresh,
  Search,
  UploadFilled,
} from '@element-plus/icons-vue'
import { useAuthStore } from '@/stores/auth'

// ============================================================
// 本地类型
// ============================================================
interface MibModuleItem {
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

interface MibModuleList {
  items: MibModuleItem[]
  mib_dir: string
  backend: string
  load_error?: string
}

interface MibTreeNode {
  oid: string
  name: string
  has_children: boolean
  is_notification?: boolean
  status?: string
  kind?: string
}

interface MibTreeNodeList {
  module: string
  parent_oid: string
  children: MibTreeNode[]
}

interface MibNodeDetail {
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

interface MibNotifRow {
  trap_oid: string
  name: string
  severity?: string
  objects?: string[]
  module?: string
  description?: string
}

interface MibNotificationList {
  module: string
  items: MibNotifRow[]
}

interface ApplyPolicyResult {
  inserted?: number
  updated?: number
  skipped?: number
  deleted?: number
  errors?: string[]
}

interface SnmpGetResult {
  oid: string
  name?: string
  value: string
  type?: string
  host: string
}

interface MibsApiShim {
  reloadMibs(): Promise<MibModuleList>
  listMibModules(): Promise<MibModuleList>
  listMibChildren(moduleId: string, parentOid: string): Promise<MibTreeNodeList>
  getMibNodeDetail(moduleId: string, oid: string): Promise<MibNodeDetail>
  listMibNotifications(moduleId: string): Promise<MibNotificationList>
  uploadMib(file: File): Promise<{ ok: true; module_name?: string }>
  exportModulePolicies(moduleId: string, mode?: 'module' | 'all'): Promise<Blob>
  exportAllPolicies(): Promise<Blob>
  applyModulePolicies(moduleId: string, mode: 'merge' | 'replace'): Promise<ApplyPolicyResult>
  applyAllPolicies(mode: 'merge' | 'replace'): Promise<ApplyPolicyResult>
  snmpGetByOid(host: string, port: number, community: string, oid: string): Promise<SnmpGetResult>
}

function _unimpl(name: string): never {
  throw new Error(`[API shim] 模块未实现：${name}，请等待并行任务创建 @/api/mibs`)
}
const _shimMibs: MibsApiShim = {
  reloadMibs: () => Promise.reject(_unimpl('reloadMibs')),
  listMibModules: () => Promise.reject(_unimpl('listMibModules')),
  listMibChildren: () => Promise.reject(_unimpl('listMibChildren')),
  getMibNodeDetail: () => Promise.reject(_unimpl('getMibNodeDetail')),
  listMibNotifications: () => Promise.reject(_unimpl('listMibNotifications')),
  uploadMib: () => Promise.reject(_unimpl('uploadMib')),
  exportModulePolicies: () => Promise.reject(_unimpl('exportModulePolicies')),
  exportAllPolicies: () => Promise.reject(_unimpl('exportAllPolicies')),
  applyModulePolicies: () => Promise.reject(_unimpl('applyModulePolicies')),
  applyAllPolicies: () => Promise.reject(_unimpl('applyAllPolicies')),
  snmpGetByOid: () => Promise.reject(_unimpl('snmpGetByOid')),
}

// @ts-ignore 模块未创建时忽略
import * as _rawMibs from '@/api/mibs'
const _mibs = markRaw(_rawMibs as unknown as Record<string, unknown>)

function _bind<S extends object>(shim: S, src: Record<string, unknown>): S {
  const out = { ...(shim as unknown as object) } as S
  for (const key of Object.keys(shim as unknown as object)) {
    if (key in src && typeof (src as Record<string, unknown>)[key] === 'function') {
      ;(out as unknown as Record<string, unknown>)[key] = (src as Record<string, unknown>)[key]
    }
  }
  return out
}
const mibsApi = _bind<MibsApiShim>(_shimMibs, {
  ..._mibs,
  listMibModules: _mibs.listMibs,
  listMibChildren: _mibs.listChildren,
  getMibNodeDetail: _mibs.getNodeDetail,
  listMibNotifications: _mibs.listNotifications,
  snmpGetByOid: _mibs.snmpGet,
})

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

function asMod(r: unknown): MibModuleItem { return r as MibModuleItem }
function asNotif(r: unknown): MibNotifRow { return r as MibNotifRow }

type TagType = 'success' | 'warning' | 'info' | 'danger' | 'primary'
function sevMeta(s: string | undefined): { text: string; type: TagType } {
  switch (s) {
    case 'critical': return { text: '严重', type: 'danger' }
    case 'error': return { text: '错误', type: 'danger' }
    case 'warning': return { text: '警告', type: 'warning' }
    case 'info': return { text: '信息', type: 'info' }
    case 'ok': return { text: '正常', type: 'success' }
    default: return { text: String(s || '-'), type: 'primary' }
  }
}

// ============================================================
// a) 左：模块列表 & 搜索
// ============================================================
const loadingModules = ref(false)
const modules = ref<MibModuleItem[]>([])
const mibListMeta = reactive({ mib_dir: '', backend: '', load_error: '' })
const moduleFilter = ref('')
const selectedModuleId = ref('')

const filteredModules = computed<MibModuleItem[]>(() => {
  const q = moduleFilter.value.trim().toLowerCase()
  if (!q) return modules.value
  return modules.value.filter((m) => {
    const hay = `${m.module_name} ${m.id} ${m.filename}`.toLowerCase()
    return hay.includes(q)
  })
})

async function loadModules(force = false) {
  loadingModules.value = true
  try {
    const res = force ? await mibsApi.reloadMibs() : await mibsApi.listMibModules()
    modules.value = res.items || []
    mibListMeta.mib_dir = res.mib_dir || ''
    mibListMeta.backend = res.backend || ''
    mibListMeta.load_error = res.load_error || ''
    if (mibListMeta.load_error) ElMessage.warning(`MIB 加载存在错误：${mibListMeta.load_error}`)
    // 默认选中第一个
    if (!selectedModuleId.value && modules.value.length > 0) {
      selectedModuleId.value = modules.value[0].id
      void onSelectModule(modules.value[0])
    }
  } catch (e) {
    ElMessage.error(errMsg(e, '加载 MIB 模块失败'))
    modules.value = []
  } finally {
    loadingModules.value = false
  }
}

// ============================================================
// b) 中：OID 树 + 节点详情（懒加载）
// ============================================================
const treeSearch = ref('')
const loadingTree = ref(false)

/** ElTree 节点数据（扁平） */
interface TreeRow {
  id: string // `${moduleId}|${oid}`
  label: string
  oid: string
  moduleId: string
  hasChildren: boolean
  isNotification?: boolean
  children?: TreeRow[]
  loaded?: boolean
}

const treeRows = ref<TreeRow[]>([])
const childrenCache = new Map<string, TreeRow[]>() // key: `${moduleId}|${parentOid}`

function mkTreeRow(m: MibModuleItem | string, oid: string, name: string, hasChildren: boolean, isNotif = false): TreeRow {
  const moduleId = typeof m === 'string' ? m : m.id
  return {
    id: `${moduleId}|${oid}`,
    label: name,
    oid,
    moduleId,
    hasChildren,
    isNotification: isNotif,
    children: hasChildren ? undefined : [],
    loaded: !hasChildren,
  }
}

function resetTreeForModule(mod: MibModuleItem) {
  treeRows.value = []
  childrenCache.clear()
  const rootOid = mod.module_oid || '1'
  const root: TreeRow = mkTreeRow(mod, rootOid, mod.module_name, true)
  treeRows.value = [root]
}

async function loadNodeChildren(node: TreeRow, cb?: (children: TreeRow[]) => void) {
  const cacheKey = `${node.moduleId}|${node.oid}`
  if (childrenCache.has(cacheKey)) {
    const cached = childrenCache.get(cacheKey)!
    if (cb) cb(cached)
    return cached
  }
  loadingTree.value = true
  try {
    const res = await mibsApi.listMibChildren(node.moduleId, node.oid)
    const rows: TreeRow[] = (res.children || []).map((c) =>
      mkTreeRow(node.moduleId, c.oid, c.name, c.has_children, !!c.is_notification)
    )
    childrenCache.set(cacheKey, rows)
    node.loaded = true
    if (cb) cb(rows)
    return rows
  } catch (e) {
    ElMessage.error(errMsg(e, '加载 OID 子节点失败'))
    if (cb) cb([])
    return [] as TreeRow[]
  } finally {
    loadingTree.value = false
  }
}

/** ElTree lazy 加载回调 (在模板中去掉签名) */
async function treeLoadNode(row: unknown, _node: unknown, cb: (children: TreeRow[]) => void) {
  const r = row as TreeRow
  if (r.loaded && childrenCache.has(`${r.moduleId}|${r.oid}`)) {
    cb(childrenCache.get(`${r.moduleId}|${r.oid}`)!)
    return
  }
  await loadNodeChildren(r, cb)
}

function treeFilterNode(_value: unknown, data: unknown): boolean {
  const d = data as TreeRow
  const q = treeSearch.value.trim().toLowerCase()
  if (!q) return true
  return d.label.toLowerCase().includes(q) || d.oid.toLowerCase().includes(q)
}

function treeIsLeaf(data: unknown): boolean {
  return !(data as TreeRow).hasChildren
}

const selectedNodeKey = ref<string | ''>('')
const loadingNodeDetail = ref(false)
const nodeDetail = ref<MibNodeDetail | null>(null)

async function onTreeSelect(node: TreeRow) {
  selectedNodeKey.value = node.id
  loadingNodeDetail.value = true
  try {
    nodeDetail.value = await mibsApi.getMibNodeDetail(node.moduleId, node.oid)
  } catch (e) {
    ElMessage.error(errMsg(e, '加载节点详情失败'))
    nodeDetail.value = null
  } finally {
    loadingNodeDetail.value = false
  }
}

async function onSelectModule(mod: MibModuleItem) {
  selectedModuleId.value = mod.id
  resetTreeForModule(mod)
  selectedNodeKey.value = ''
  nodeDetail.value = null
  void loadNotifications()
}

// ============================================================
// c) 下：Notifications 表格
// ============================================================
const loadingNotifs = ref(false)
const notifs = ref<MibNotifRow[]>([])
const notifFilter = ref('')

const filteredNotifs = computed<MibNotifRow[]>(() => {
  const q = notifFilter.value.trim().toLowerCase()
  if (!q) return notifs.value
  return notifs.value.filter((n) =>
    `${n.trap_oid} ${n.name} ${n.severity || ''} ${(n.objects || []).join(' ')}`.toLowerCase().includes(q)
  )
})

async function loadNotifications() {
  if (!selectedModuleId.value) {
    notifs.value = []
    return
  }
  loadingNotifs.value = true
  try {
    const res = await mibsApi.listMibNotifications(selectedModuleId.value)
    notifs.value = res.items || []
  } catch (e) {
    ElMessage.error(errMsg(e, '加载 Notifications 失败'))
    notifs.value = []
  } finally {
    loadingNotifs.value = false
  }
}

// ============================================================
// SNMP Get
// ============================================================
const LS_HOST = 'eventide_snmp_host'
const LS_PORT = 'eventide_snmp_port'
const LS_COMM = 'eventide_snmp_community'

const snmpForm = reactive({
  host: localStorage.getItem(LS_HOST) || '127.0.0.1',
  port: Number(localStorage.getItem(LS_PORT) || '161'),
  community: localStorage.getItem(LS_COMM) || 'public',
})
const snmpLoading = ref(false)
const snmpResult = ref<SnmpGetResult | null>(null)

async function handleSnmpGet(oid?: string) {
  const targetOid = oid || nodeDetail.value?.oid
  if (!targetOid) {
    ElMessage.warning('请先选择一个 OID 节点')
    return
  }
  localStorage.setItem(LS_HOST, snmpForm.host)
  localStorage.setItem(LS_PORT, String(snmpForm.port))
  localStorage.setItem(LS_COMM, snmpForm.community)
  snmpLoading.value = true
  try {
    snmpResult.value = await mibsApi.snmpGetByOid(snmpForm.host, snmpForm.port, snmpForm.community, targetOid)
    ElMessage.success('SNMP Get 成功')
  } catch (e) {
    ElMessage.error(errMsg(e, 'SNMP Get 失败'))
    snmpResult.value = null
  } finally {
    snmpLoading.value = false
  }
}

// ============================================================
// 策略动作：导出 / 应用（模块 / 全部）
// ============================================================
const applyDlgVisible = ref(false)
const applyMode = ref<'merge' | 'replace'>('merge')
const applyScope = ref<'module' | 'all'>('module')
const applyLoading = ref(false)
const applyResult = ref<ApplyPolicyResult | null>(null)

function openApplyDlg(scope: 'module' | 'all') {
  if (scope === 'module' && !selectedModuleId.value) {
    ElMessage.warning('请先选择一个 MIB 模块')
    return
  }
  applyScope.value = scope
  applyMode.value = 'merge'
  applyResult.value = null
  applyDlgVisible.value = true
}

async function handleApplySubmit() {
  try {
    const mode = applyMode.value
    await ElMessageBox.confirm(
      `确认${applyScope.value === 'all' ? '应用全部' : '应用本模块'}策略？模式：${mode === 'merge' ? '合并' : '替换'}。替换模式会删除未覆盖的策略。`,
      '二次确认',
      { type: 'warning', confirmButtonText: '确认应用', cancelButtonText: '取消' }
    )
  } catch { return }
  applyLoading.value = true
  applyResult.value = null
  try {
    applyResult.value = applyScope.value === 'all'
      ? await mibsApi.applyAllPolicies(applyMode.value)
      : await mibsApi.applyModulePolicies(selectedModuleId.value, applyMode.value)
    ElMessage.success('策略已应用')
  } catch (e) {
    ElMessage.error(errMsg(e, '应用策略失败'))
  } finally {
    applyLoading.value = false
  }
}

async function handleExport(scope: 'module' | 'all') {
  try {
    let blob: Blob
    if (scope === 'all') {
      blob = await mibsApi.exportAllPolicies()
    } else {
      if (!selectedModuleId.value) {
        ElMessage.warning('请先选择一个 MIB 模块')
        return
      }
      blob = await mibsApi.exportModulePolicies(selectedModuleId.value, 'module')
    }
    const a = document.createElement('a')
    const url = URL.createObjectURL(blob)
    a.href = url
    const ts = new Date().toISOString().replace(/[:.]/g, '-').slice(0, 19)
    a.download = scope === 'all' ? `trap-policies-all-${ts}.json` : `trap-policies-module-${selectedModuleId.value}-${ts}.json`
    document.body.appendChild(a)
    a.click()
    document.body.removeChild(a)
    URL.revokeObjectURL(url)
    ElMessage.success('已导出策略')
  } catch (e) {
    ElMessage.error(errMsg(e, '导出失败'))
  }
}

// ============================================================
// 上传 MIB
// ============================================================
const uploadRef = ref<UploadInstance>()
const uploadAccept = '.mib,.txt,.my,.smi'
const uploadLoading = ref(false)

const beforeMibUpload: UploadProps['beforeUpload'] = (rawFile: UploadRawFile) => {
  if (!canWrite.value) {
    ElMessage.warning('需要 trap:write 权限上传 MIB')
    return false
  }
  return true
}

const customMibRequest: UploadProps['httpRequest'] = async (options) => {
  const f = options.file as File
  uploadLoading.value = true
  try {
    const res = await mibsApi.uploadMib(f)
    ElMessage.success(`${f.name} 上传成功${res.module_name ? `（解析为 ${res.module_name}）` : ''}`)
    options.onSuccess?.(res)
    void loadModules(true)
  } catch (e) {
    ElMessage.error(`${f.name} 上传失败：${errMsg(e, '未知错误')}`)
    const msg = errMsg(e, '上传失败')
    // 使用 class-free 方式匹配 UploadAjaxError extends Error 的形状
    const faux = Object.assign(new Error(msg), {
      name: 'UploadAjaxError',
      status: 0,
      method: 'POST',
      url: '/api/mibs/upload',
    })
    options.onError?.(faux)
  } finally {
    uploadLoading.value = false
  }
}

onMounted(() => { void loadModules() })
</script>

<template>
  <div class="page-wrap" style="padding: 16px 20px;">
    <!-- 顶部标题 & 动作 -->
    <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 14px; flex-wrap: wrap; gap: 8px;">
      <h2 style="font-size: 20px; margin: 0; color: var(--heading);">
        MIB 库
        <span style="font-size:13px;color:var(--el-text-color-secondary);margin-left:8px">OID 树浏览器 · Trap 策略导出与应用 · SNMP Get</span>
      </h2>
      <div style="display: flex; gap: 8px; flex-wrap: wrap;">
        <el-button :icon="Refresh" @click="loadModules(true)">重新加载</el-button>
        <el-button :icon="Download" type="primary" plain @click="handleExport('all')">导出全部策略</el-button>
        <el-upload
          v-if="canWrite"
          ref="uploadRef"
          :show-file-list="false"
          :accept="uploadAccept"
          :multiple="true"
          :before-upload="beforeMibUpload"
          :http-request="customMibRequest"
          drag
          style="display: inline-block;"
        >
          <el-button type="success" :icon="UploadFilled" :loading="uploadLoading">上传 MIB</el-button>
        </el-upload>
      </div>
    </div>

    <!-- 三面板布局：上左/上右 两栏，下一栏 -->
    <el-row :gutter="14">
      <!-- a) 左：MIB 模块列表 -->
      <el-col :span="8">
        <el-card shadow="never" style="border: 1px solid var(--line); background: var(--panel-bg, var(--panel)); margin-bottom: 14px; height: 560px; display: flex; flex-direction: column;">
          <template #header>
            <div style="display: flex; justify-content: space-between; align-items: center;">
              <strong>MIB 模块</strong>
              <el-tag size="small" type="info">{{ modules.length }}</el-tag>
            </div>
          </template>
          <el-input
            v-model="moduleFilter"
            placeholder="搜索 名称 / ID / 文件名"
            size="small"
            clearable
            style="margin-bottom: 8px;"
          >
            <template #prefix><el-icon><Search /></el-icon></template>
          </el-input>
          <div v-if="mibListMeta.load_error" style="margin-bottom: 8px;">
            <el-tag type="danger" effect="plain" size="small">加载错误: {{ mibListMeta.load_error }}</el-tag>
          </div>
          <el-table
            :data="filteredModules as unknown as MibModuleItem[]"
            size="small"
            stripe
            height="450"
            highlight-current-row
            v-loading="loadingModules"
            @current-change="(row: unknown) => { if (row) onSelectModule(asMod(row)) }"
            empty-text="无 MIB 模块"
          >
            <el-table-column label="名称" min-width="130">
              <template #default="{ row }">
                <div style="display: flex; align-items: center; gap: 6px;">
                  <el-tag
                    v-if="asMod(row).error"
                    type="danger"
                    effect="dark"
                    size="small"
                  >错误</el-tag>
                  <span>{{ asMod(row).module_name }}</span>
                </div>
              </template>
            </el-table-column>
            <el-table-column label="ID" width="90">
              <template #default="{ row }">
                <span style="font-family: var(--font-mono); font-size: 12.5px;">{{ asMod(row).id }}</span>
              </template>
            </el-table-column>
            <el-table-column label="OID 根" min-width="120">
              <template #default="{ row }">
                <span style="font-family: var(--font-mono); font-size: 12.5px;">{{ asMod(row).module_oid || '—' }}</span>
              </template>
            </el-table-column>
            <el-table-column label="错误" width="90">
              <template #default="{ row }">
                <el-tooltip v-if="asMod(row).error" :content="asMod(row).error" placement="top">
                  <el-tag type="danger" effect="plain" size="small">错误</el-tag>
                </el-tooltip>
                <span v-else style="color: var(--muted);">-</span>
              </template>
            </el-table-column>
          </el-table>
        </el-card>
      </el-col>

      <!-- b) 中：OID 树 + 节点详情 -->
      <el-col :span="16">
        <el-row :gutter="14">
          <!-- OID 树 -->
          <el-col :span="10">
            <el-card shadow="never" style="border: 1px solid var(--line); background: var(--panel-bg, var(--panel)); margin-bottom: 14px; height: 560px; display: flex; flex-direction: column;">
              <template #header>
                <div style="display: flex; justify-content: space-between; align-items: center;">
                  <strong>OID 树</strong>
                  <el-tag size="small" type="info" effect="plain">{{ selectedModuleId || '未选模块' }}</el-tag>
                </div>
              </template>
              <el-input
                v-model="treeSearch"
                placeholder="搜索 OID / 名称（前端过滤可见节点）"
                size="small"
                clearable
                style="margin-bottom: 8px;"
              >
                <template #prefix><el-icon><Search /></el-icon></template>
              </el-input>
              <el-empty
                v-if="!selectedModuleId"
                description="请先选择左侧 MIB 模块"
                :image-size="60"
                style="flex: 1; display: flex; align-items: center; justify-content: center;"
              />
              <el-tree
                v-else
                :data="treeRows"
                :props="{ label: 'label', children: 'children', isLeaf: treeIsLeaf }"
                node-key="id"
                lazy
                :load="treeLoadNode"
                :expand-on-click-node="false"
                :current-node-key="selectedNodeKey"
                highlight-current
                :filter-node-method="treeFilterNode"
                v-loading="loadingTree"
                @node-click="(d) => onTreeSelect(d as TreeRow)"
                style="flex: 1; overflow: auto;"
                height="450"
              >
                <template #default="{ node, data }">
                  <div style="display: flex; align-items: center; gap: 6px; width: 100%;">
                    <el-icon size="12" style="color: var(--muted);">
                      <component :is="node.expanded ? CaretBottom : CaretRight" v-if="data.hasChildren" />
                    </el-icon>
                    <span style="font-family: var(--font-mono); font-size: 12.5px;">{{ data.label }}</span>
                    <el-tag v-if="data.isNotification" type="warning" size="small" effect="plain">Trap</el-tag>
                  </div>
                </template>
              </el-tree>
            </el-card>
          </el-col>

          <!-- 节点详情 + SNMP Get -->
          <el-col :span="14">
            <el-card shadow="never" style="border: 1px solid var(--line); background: var(--panel-bg, var(--panel)); margin-bottom: 14px; height: 560px; display: flex; flex-direction: column;">
              <template #header>
                <strong>节点详情</strong>
              </template>
              <el-empty
                v-if="!nodeDetail && !loadingNodeDetail"
                description="请在左侧点击 OID 树节点"
                :image-size="60"
              />
              <div v-else style="flex: 1; overflow: auto;" v-loading="loadingNodeDetail">
                <template v-if="nodeDetail">
                  <h4 style="margin: 0 0 8px; color: var(--heading); font-family: var(--font-mono);">
                    {{ nodeDetail.name }}
                    <el-tag v-if="nodeDetail.is_notification" type="warning" size="small" style="margin-left: 6px;">Trap</el-tag>
                  </h4>
                  <el-descriptions :column="1" size="small" border style="margin-bottom: 10px;">
                    <el-descriptions-item label="OID">
                      <span style="font-family: var(--font-mono); font-size: 12.5px;">{{ nodeDetail.oid }}</span>
                    </el-descriptions-item>
                    <el-descriptions-item label="类型">{{ nodeDetail.kind || nodeDetail.type || '—' }}</el-descriptions-item>
                    <el-descriptions-item label="状态">{{ nodeDetail.status || '—' }}</el-descriptions-item>
                    <el-descriptions-item label="描述">
                      <span style="white-space: pre-wrap;">{{ nodeDetail.description || '—' }}</span>
                    </el-descriptions-item>
                    <el-descriptions-item label="OBJECTS">
                      <template v-if="nodeDetail.objects && nodeDetail.objects.length">
                        <el-tag v-for="o in nodeDetail.objects" :key="o" size="small" effect="plain" style="margin: 2px;">{{ o }}</el-tag>
                      </template>
                      <span v-else style="color: var(--muted);">—</span>
                    </el-descriptions-item>
                    <el-descriptions-item v-if="nodeDetail.values && Object.keys(nodeDetail.values).length" label="值">
                      <pre style="margin: 0; font-family: var(--font-mono); font-size: 12.5px;">{{ JSON.stringify(nodeDetail.values, null, 2) }}</pre>
                    </el-descriptions-item>
                  </el-descriptions>
                </template>

                <el-divider content-position="left">SNMP Get</el-divider>
                <el-form :model="snmpForm" size="small" label-width="90px" inline style="flex-wrap: wrap;">
                  <el-form-item label="目标 IP">
                    <el-input v-model="snmpForm.host" style="width: 150px;" />
                  </el-form-item>
                  <el-form-item label="Port">
                    <el-input-number v-model="snmpForm.port" :min="1" :max="65535" style="width: 110px;" />
                  </el-form-item>
                  <el-form-item label="Community">
                    <el-input v-model="snmpForm.community" style="width: 140px;" />
                  </el-form-item>
                  <el-form-item>
                    <el-button type="primary" :loading="snmpLoading" @click="handleSnmpGet()">
                      Get
                    </el-button>
                  </el-form-item>
                </el-form>
                <pre
                  v-if="snmpResult"
                  style="margin-top: 4px; background: var(--inset-bg); padding: 10px; border-radius: 6px; font-family: var(--font-mono); font-size: 12.5px; max-height: 200px; overflow: auto;"
                >{{ JSON.stringify(snmpResult, null, 2) }}</pre>
              </div>
            </el-card>
          </el-col>
        </el-row>
      </el-col>
    </el-row>

    <!-- c) 下：Trap Notifications + 模块策略动作 -->
    <el-card shadow="never" style="border: 1px solid var(--line); background: var(--panel-bg, var(--panel));">
      <template #header>
        <div style="display: flex; justify-content: space-between; align-items: center; flex-wrap: wrap; gap: 8px;">
          <div>
            <strong>Trap Notifications</strong>
            <el-tag size="small" type="info" effect="plain" style="margin-left: 8px;">模块：{{ selectedModuleId || '—' }}</el-tag>
          </div>
          <div style="display: flex; gap: 8px; align-items: center;">
            <el-input
              v-model="notifFilter"
              placeholder="搜索 oid / 名称 / objects"
              size="small"
              clearable
              style="width: 260px;"
            >
              <template #prefix><el-icon><Search /></el-icon></template>
            </el-input>
            <el-button :icon="Download" size="small" plain @click="handleExport('module')">导出本模块策略</el-button>
            <el-button
              v-if="canWrite"
              type="primary"
              size="small"
              plain
              :icon="ArrowDown"
              @click="openApplyDlg('module')"
            >应用本模块策略</el-button>
            <el-button
              v-if="canWrite"
              type="warning"
              size="small"
              plain
              :icon="ArrowDown"
              @click="openApplyDlg('all')"
            >应用全部</el-button>
          </div>
        </div>
      </template>
      <el-table
        :data="filteredNotifs as unknown as MibNotifRow[]"
        size="small"
        stripe
        v-loading="loadingNotifs"
        max-height="360"
        empty-text="当前模块无 Notifications"
      >
        <el-table-column label="Trap OID" min-width="220">
          <template #default="{ row }">
            <span style="font-family: var(--font-mono); font-size: 12.5px;">{{ asNotif(row).trap_oid }}</span>
          </template>
        </el-table-column>
        <el-table-column label="名称" min-width="180">
          <template #default="{ row }">{{ asNotif(row).name || '—' }}</template>
        </el-table-column>
        <el-table-column label="Severity" width="100" align="center">
          <template #default="{ row }">
            <el-tag v-if="asNotif(row).severity" size="small" :type="sevMeta(asNotif(row).severity).type" effect="dark">
              {{ sevMeta(asNotif(row).severity).text }}
            </el-tag>
            <span v-else style="color: var(--muted);">-</span>
          </template>
        </el-table-column>
        <el-table-column label="OBJECTS" min-width="300">
          <template #default="{ row }">
            <template v-if="asNotif(row).objects && asNotif(row).objects!.length">
              <el-tag v-for="o in asNotif(row).objects" :key="o" size="small" effect="plain" style="margin: 2px;">{{ o }}</el-tag>
            </template>
            <span v-else style="color: var(--muted);">—</span>
          </template>
        </el-table-column>
        <el-table-column label="描述" min-width="240">
          <template #default="{ row }">
            <span style="color: var(--el-text-color-secondary);">{{ asNotif(row).description || '—' }}</span>
          </template>
        </el-table-column>
      </el-table>
    </el-card>

    <!-- 应用策略 Dialog -->
    <el-dialog v-model="applyDlgVisible" :title="applyScope === 'all' ? '应用全部策略' : '应用本模块策略'" width="520px">
      <el-form label-width="100px">
        <el-form-item label="作用范围">
          <el-tag :type="applyScope === 'all' ? 'warning' : 'primary'" effect="plain">
            {{ applyScope === 'all' ? '所有 MIB 模块' : '当前模块：' + selectedModuleId }}
          </el-tag>
        </el-form-item>
        <el-form-item label="应用模式">
          <el-radio-group v-model="applyMode">
            <el-radio label="merge">合并（保留不在策略文件中的旧策略）</el-radio>
            <el-radio label="replace">替换（删除不在策略文件中的旧策略）</el-radio>
          </el-radio-group>
        </el-form-item>
      </el-form>
      <el-alert
        v-if="applyResult"
        :type="(applyResult.errors?.length ?? 0) > 0 ? 'warning' : 'success'"
        :closable="false"
        style="margin: 10px 0;"
      >
        <template #title>应用结果</template>
        <template #default>
          <pre style="margin: 0; white-space: pre-wrap; font-family: var(--font-mono); font-size: 12.5px;">{{ JSON.stringify(applyResult, null, 2) }}</pre>
        </template>
      </el-alert>
      <template #footer>
        <el-button @click="applyDlgVisible = false">关闭</el-button>
        <el-button type="primary" :loading="applyLoading" @click="handleApplySubmit">确认应用</el-button>
      </template>
    </el-dialog>
  </div>
</template>
