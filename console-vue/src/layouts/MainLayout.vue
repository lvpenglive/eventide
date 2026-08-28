<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { RouterView, useRoute, useRouter } from 'vue-router'
import {
  ElAlert,
  ElButton,
  ElDropdown,
  ElSwitch,
  ElTooltip,
  ElMessageBox,
  ElMessage,
  type FormInstance,
  type FormRules,
} from 'element-plus'
import { Setting, SwitchButton, Sunny, Moon, Monitor, ArrowDown, ArrowRight } from '@element-plus/icons-vue'
import { useAuthStore } from '@/stores/auth'
import { useTheme, type ThemePref } from '@/composables/useTheme'
import { useUiStore } from '@/stores/ui'
import {
  PAGE_ORDER,
  PAGE_LABEL,
  PAGE_ICON,
  PAGE_PERM,
  PAGE_GROUP,
  NAV_GROUPS,
  isPageImplemented,
  type PageKey,
  type NavGroupKey,
} from '@/perms'
import { can as rawCan } from '@/perms'
import { getUiBetaToggle, putUiBetaToggle } from '@/api/overviewAuthLoginAndMore'
import { validatePasswordComplexity } from '@/utils/password'

const auth = useAuthStore()
const router = useRouter()
const route = useRoute()
const ui = useUiStore()
const { theme, setTheme, initTheme } = useTheme()

onMounted(() => initTheme())

// --- 侧栏收起 ---
const SIDEBAR_KEY = 'eventide_sidebar_collapsed'
const sidebarCollapsed = ref(localStorage.getItem(SIDEBAR_KEY) === '1')
function toggleSidebar() {
  sidebarCollapsed.value = !sidebarCollapsed.value
  localStorage.setItem(SIDEBAR_KEY, sidebarCollapsed.value ? '1' : '0')
}

// --- BetaToggle ---
const betaAvailable = ref(false)
const betaEnabled = ref(false)
const betaToggleLoading = ref(false)
async function loadBetaToggle() {
  try {
    const r = await getUiBetaToggle()
    betaAvailable.value = r.v2_available
    betaEnabled.value = r.enabled
  } catch {
    /* 非致命错误，静默展示默认关 */
  }
}
async function onBetaToggleChange(val: string | number | boolean) {
  const v = Boolean(val)
  betaToggleLoading.value = true
  try {
    const r = await putUiBetaToggle(v)
    betaEnabled.value = r.enabled
    ElMessage.success('UI Beta 开关已更新，即将刷新页面…')
    setTimeout(() => window.location.reload(), 600)
  } catch (e) {
    betaEnabled.value = !v // 回滚
    ElMessage.error('更新 Beta 开关失败')
  } finally {
    betaToggleLoading.value = false
  }
}
onMounted(loadBetaToggle)

// --- 许可证横幅 ---
import { getLicense, type LicenseInfo } from '@/api/license'
const licenseBanner = ref<{ show: boolean; cls: string; text: string }>({ show: false, cls: 'info', text: '' })

function fmtTime(s?: string): string {
  if (!s) return '—'
  try {
    const d = new Date(s)
    return d.toLocaleString('zh-CN', { hour12: false })
  } catch {
    return s
  }
}

async function loadLicenseBanner() {
  if (!auth.token) return
  try {
    const lic = await getLicense()
    updateLicenseBanner(lic)
  } catch {
    licenseBanner.value.show = false
  }
}

function updateLicenseBanner(lic: LicenseInfo) {
  const days = lic.days_left
  let show = false
  let cls = 'info'
  let text = ''
  if (lic.kind === 'expired' || lic.writable === false) {
    show = true
    cls = 'warn'
    text = lic.reason || '许可证无效或已过期，当前为只读宽限。可查看数据，配置变更已禁用。'
  } else if (lic.kind === 'trial') {
    show = true
    cls = days != null && days <= 7 ? 'warn' : 'info'
    text = `试用中，剩余约 ${days ?? '—'} 天（到期 ${fmtTime(lic.expires_at)}）。`
  } else if (lic.kind === 'licensed' && days != null && days <= 7) {
    show = true
    cls = 'warn'
    text = `许可证即将到期：剩余约 ${days} 天（${lic.customer || ''} · ${fmtTime(lic.expires_at)}）。`
  }
  licenseBanner.value = { show, cls, text }
}

function goImportLicense() {
  router.push('/settings')
}

onMounted(loadLicenseBanner)

// --- 修改密码 ---
const changePwdVisible = ref(false)
const changePwdFormRef = ref<FormInstance>()
const changePwdSubmitting = ref(false)
const changePwdModel = ref({ oldPassword: '', newPassword: '', confirmPassword: '' })
const changePwdRules: FormRules = {
  oldPassword: [{ required: true, message: '请输入旧密码', trigger: 'blur' }],
  newPassword: [
    { required: true, message: '请输入新密码', trigger: 'blur' },
    {
      validator: (_rule, value, cb) => {
        const r = validatePasswordComplexity(value || '')
        r.ok ? cb() : cb(new Error(r.message))
      },
      trigger: 'blur',
    },
  ],
  confirmPassword: [
    { required: true, message: '请再输一次新密码', trigger: 'blur' },
    {
      validator: (_rule, value, cb) => {
        if (value !== changePwdModel.value.newPassword) cb(new Error('两次输入的新密码不一致'))
        else cb()
      },
      trigger: 'blur',
    },
  ],
}
function openChangePassword() {
  changePwdModel.value = { oldPassword: '', newPassword: '', confirmPassword: '' }
  changePwdVisible.value = true
}
async function submitChangePassword() {
  const form = changePwdFormRef.value
  if (!form) return
  try {
    await form.validate()
  } catch {
    return
  }
  changePwdSubmitting.value = true
  try {
    await auth.changePassword(changePwdModel.value.oldPassword, changePwdModel.value.newPassword)
    ElMessage.success('密码修改成功，请重新登录')
    changePwdVisible.value = false
    auth.logout()
  } catch (e) {
    const msg = e && typeof e === 'object' && 'message' in e ? String((e as { message: unknown }).message) : '修改密码失败'
    ElMessage.error(msg)
  } finally {
    changePwdSubmitting.value = false
  }
}
// 顶栏「注销」
function onLogout() {
  ElMessageBox.confirm('确定要注销登录吗？', '注销确认', {
    confirmButtonText: '注销',
    cancelButtonText: '取消',
    type: 'warning',
  })
    .then(() => auth.logout())
    .catch(() => undefined)
}

// --- 侧栏导航（分组 + 折叠） ---
const activePage = computed<PageKey>(() => {
  // 用 path 判断激活页（path 为小写，与 PageKey 一致）
  const seg = route.path.replace(/^\//, '').split('/')[0]
  if (PAGE_ORDER.includes(seg as PageKey)) return seg as PageKey
  return 'overview'
})
function pageHref(key: PageKey): string {
  if (isPageImplemented(key)) return `/${key}`
  // 未实现页：跳总览并带 todo 参数（按 TR-4.4 要求）
  return `/overview?todo=${key}`
}
function canView(key: PageKey): boolean {
  const perms = auth.permissions
  // 已登录但权限数组为空（me() 未完成或后端未返回）→ 默认展示所有菜单，路由守卫会拦截实际访问
  if (!perms || perms.length === 0) return auth.isLoggedIn
  return rawCan(PAGE_PERM[key], perms)
}

// 当前激活页所属的分组（用于首次加载自动展开）
const activeGroup = computed<NavGroupKey | null>(() => {
  const g = PAGE_GROUP[activePage.value]
  return g ?? null
})

// 每个分组下可见的页面（受权限过滤）
function pagesInGroup(g: NavGroupKey): PageKey[] {
  return PAGE_ORDER.filter((k) => PAGE_GROUP[k] === g && canView(k))
}

// 分组是否展开：未在 localStorage 设置过的默认展开；激活页所在分组强制展开
function isGroupOpen(g: NavGroupKey): boolean {
  const saved = ui.navGroupsOpen[g]
  if (saved === undefined || saved === null) return true
  return !!saved
}
// 当前分组是否含激活页（用于 has-active 标记）
function groupHasActive(g: NavGroupKey): boolean {
  return activeGroup.value === g
}
function toggleGroup(g: NavGroupKey) {
  ui.setNavGroupOpen(g, !isGroupOpen(g))
}

// --- 密码到期警告 ---
const passwordWarn = computed(() => {
  const s = auth.passwordStatus
  if (!s) return null
  if (s.expired) return { type: 'error' as const, text: '密码已过期，请立即修改以继续使用。' }
  if (s.change_required) return { type: 'warning' as const, text: '管理员要求您修改密码后才能继续。' }
  if (typeof s.days_until_expire === 'number' && s.days_until_expire <= 7) {
    return {
      type: 'warning' as const,
      text: `密码将在 ${s.days_until_expire} 天后过期，请尽快修改。`,
    }
  }
  return null
})

// 主题按钮：三态循环 light → dark → system
const themeOpts: { key: ThemePref; label: string; icon: typeof Sunny }[] = [
  { key: 'light', label: '浅色', icon: Sunny },
  { key: 'dark', label: '深色', icon: Moon },
  { key: 'system', label: '跟随系统', icon: Monitor },
]
</script>

<template>
  <div class="main-layout" :class="{ 'sidebar-collapsed': sidebarCollapsed }">
    <!-- 左侧固定侧栏（宽 220px，TR-4.4） -->
    <aside class="sidebar">
      <div class="sidebar-brand">
        <span class="brand-dot" />
        <span class="brand-title">Eventide</span>
        <el-tag size="small" type="success" effect="dark" class="brand-badge">v2 Beta</el-tag>
      </div>
      <nav class="sidebar-nav">
        <!-- overview 独立展示在分组之前 -->
        <router-link
          v-if="canView('overview')"
          :to="pageHref('overview')"
          class="nav-item"
          :class="{ active: activePage === 'overview' }"
        >
          <span class="nav-icon">{{ PAGE_ICON.overview }}</span>
          <span class="nav-label">{{ PAGE_LABEL.overview }}</span>
        </router-link>

        <!-- 分组：按 NAV_GROUPS 顺序渲染 -->
        <div
          v-for="g in NAV_GROUPS"
          :key="g.key"
          class="nav-group"
          :class="{
            open: isGroupOpen(g.key) || groupHasActive(g.key),
            'has-active': groupHasActive(g.key),
            hidden: pagesInGroup(g.key).length === 0,
          }"
          :data-group="g.key"
        >
          <button
            type="button"
            class="nav-group-btn"
            :title="g.label"
            @click="toggleGroup(g.key)"
          >
            <span class="nav-icon">{{ g.icon }}</span>
            <span class="nav-label">{{ g.label }}</span>
            <el-icon class="nav-caret" :size="12">
              <ArrowDown v-if="isGroupOpen(g.key) || groupHasActive(g.key)" />
              <ArrowRight v-else />
            </el-icon>
          </button>
          <div class="nav-sub">
            <router-link
              v-for="key in pagesInGroup(g.key)"
              :key="key"
              :to="pageHref(key)"
              class="nav-item"
              :class="{
                active: activePage === key,
                disabled: !isPageImplemented(key),
              }"
              :aria-disabled="!isPageImplemented(key)"
            >
              <span class="nav-icon">{{ PAGE_ICON[key] }}</span>
              <span class="nav-label">{{ PAGE_LABEL[key] }}</span>
              <span v-if="!isPageImplemented(key)" class="nav-soon">待实现</span>
            </router-link>
          </div>
        </div>
      </nav>
      <div class="sidebar-foot">
        <span class="foot-dot ok" /> 已连接 · eventide-server
      </div>
    </aside>

    <!-- 右侧主区 -->
    <div class="main-area">
      <!-- 顶栏 -->
      <header class="topbar">
        <div class="topbar-left">
          <el-button
            class="sidebar-toggle"
            :title="sidebarCollapsed ? '展开侧栏' : '收起侧栏'"
            @click="toggleSidebar"
          >
            <el-icon :size="16">
              <ArrowRight v-if="sidebarCollapsed" />
              <ArrowDown v-else />
            </el-icon>
          </el-button>
          <span class="crumb-title">{{ String(route.meta?.title || '总览') }}</span>
        </div>
        <div class="topbar-right">
          <!-- 主题切换：三按钮二选一群组（选中高亮） -->
          <div class="theme-switcher" role="group" aria-label="主题切换">
            <el-tooltip
              v-for="opt in themeOpts"
              :key="opt.key"
              :content="opt.label"
              placement="bottom"
            >
              <el-button
                :type="theme === opt.key ? 'primary' : 'default'"
                size="small"
                text
                round
                @click="setTheme(opt.key)"
              >
                <el-icon :size="16"><component :is="opt.icon" /></el-icon>
              </el-button>
            </el-tooltip>
          </div>

          <!-- BetaToggle 开关（TR-4.4 要求） -->
          <div v-if="betaAvailable" class="beta-toggle" title="Beta UI 开关">
            <el-icon :size="16" class="beta-icon"><SwitchButton /></el-icon>
            <span class="beta-label">Beta UI</span>
            <el-switch
              v-model="betaEnabled"
              :loading="betaToggleLoading"
              size="small"
              inline-prompt
              active-text="开"
              inactive-text="关"
              @change="onBetaToggleChange"
            />
          </div>

          <!-- 用户下拉 -->
          <el-dropdown trigger="click" @command="(c: string) => c === 'pwd' ? openChangePassword() : onLogout()">
            <div class="user-chip">
              <div class="avatar">{{ (auth.displayName || 'U').slice(0, 1) }}</div>
              <span class="user-name">{{ auth.displayName || auth.user?.username }}</span>
              <el-icon :size="14"><Setting /></el-icon>
            </div>
            <template #dropdown>
              <el-dropdown-menu>
                <el-dropdown-item command="pwd">修改密码</el-dropdown-item>
                <el-dropdown-item command="logout" divided>注销</el-dropdown-item>
              </el-dropdown-menu>
            </template>
          </el-dropdown>
        </div>
      </header>

      <!-- 密码到期警告 -->
      <div v-if="passwordWarn" class="password-warn">
        <el-alert
          :type="passwordWarn.type"
          :title="passwordWarn.text"
          show-icon
          :closable="false"
        >
          <template #default>
            <el-button size="small" type="primary" plain @click="openChangePassword">立即修改</el-button>
          </template>
        </el-alert>
      </div>

      <!-- 许可证横幅 -->
      <div v-if="licenseBanner.show" :class="['license-banner', licenseBanner.cls]">
        <span>{{ licenseBanner.text }}</span>
        <el-button size="small" type="primary" plain @click="goImportLicense">去导入许可证</el-button>
      </div>

      <!-- 页面内容区 -->
      <main class="page-area">
        <RouterView v-slot="{ Component }">
          <transition name="fade" mode="out-in">
            <component :is="Component" />
          </transition>
        </RouterView>
      </main>
    </div>

    <!-- 修改密码对话框 -->
    <el-dialog
      v-model="changePwdVisible"
      title="修改密码"
      width="480px"
      :close-on-click-modal="false"
      destroy-on-close
    >
      <el-form
        ref="changePwdFormRef"
        :model="changePwdModel"
        :rules="changePwdRules"
        label-position="top"
      >
        <el-form-item label="旧密码" prop="oldPassword">
          <el-input v-model="changePwdModel.oldPassword" type="password" show-password placeholder="请输入当前密码" />
        </el-form-item>
        <el-form-item label="新密码" prop="newPassword">
          <el-input v-model="changePwdModel.newPassword" type="password" show-password placeholder="至少 8 位，含大小写/数字/特殊字符" />
        </el-form-item>
        <el-form-item label="确认新密码" prop="confirmPassword">
          <el-input v-model="changePwdModel.confirmPassword" type="password" show-password placeholder="请再次输入新密码" />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="changePwdVisible = false">取消</el-button>
        <el-button type="primary" :loading="changePwdSubmitting" @click="submitChangePassword">确认修改</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<style scoped>
.main-layout {
  min-height: 100vh;
  display: flex;
  background:
    radial-gradient(900px 420px at 12% -10%, var(--body-glow-1), transparent 55%),
    radial-gradient(700px 380px at 100% 0%, var(--body-glow-2), transparent 50%),
    var(--bg);
  color: var(--text);
}

/* ===== 侧栏（220px 宽，TR-4.4） ===== */
.sidebar {
  flex: 0 0 220px;
  width: 220px;
  min-height: 100vh;
  background: var(--sidebar-bg-grad);
  border-right: 1px solid var(--sidebar-border);
  display: flex;
  flex-direction: column;
  position: sticky;
  top: 0;
  box-shadow: inset -1px 0 0 var(--line);
}

/* ===== 侧栏收起态 ===== */
.main-layout.sidebar-collapsed .sidebar {
  flex: 0 0 68px;
  width: 68px;
}
.main-layout.sidebar-collapsed .sidebar-brand {
  padding: 18px 8px 14px;
  justify-content: center;
}
.main-layout.sidebar-collapsed .brand-title,
.main-layout.sidebar-collapsed .brand-badge {
  display: none;
}
.main-layout.sidebar-collapsed .sidebar-nav {
  padding: 10px 6px 12px;
}
.main-layout.sidebar-collapsed .nav-item {
  width: calc(100% - 12px);
  margin: 3px 6px;
  padding: 10px 0;
  justify-content: center;
  gap: 0;
}
.main-layout.sidebar-collapsed .nav-label,
.main-layout.sidebar-collapsed .nav-soon {
  display: none;
}
.main-layout.sidebar-collapsed .nav-group-btn {
  display: none;
}
.main-layout.sidebar-collapsed .nav-sub {
  display: flex;
  padding: 0;
  margin-left: 0;
  border-left: none;
}
.main-layout.sidebar-collapsed .nav-sub .nav-item {
  margin: 3px 6px;
}
.main-layout.sidebar-collapsed .sidebar-foot {
  padding: 10px 8px 16px;
  justify-content: center;
}
.main-layout.sidebar-collapsed .sidebar-foot span:not(.foot-dot) {
  display: none;
}
.sidebar-toggle {
  margin-right: 8px;
}
.sidebar-brand {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 18px 18px 14px;
  border-bottom: 1px solid var(--line);
}
.brand-dot {
  width: 10px;
  height: 10px;
  border-radius: 50%;
  background: linear-gradient(135deg, var(--primary-grad-from), var(--accent));
  box-shadow: 0 0 0 3px var(--primary-bg);
}
.brand-title {
  font-weight: 700;
  letter-spacing: 0.02em;
  background: var(--brand-grad);
  -webkit-background-clip: text;
  background-clip: text;
  color: transparent;
  font-size: 17px;
}
.brand-badge {
  margin-left: auto;
}
.sidebar-nav {
  padding: 10px 10px 12px;
  display: flex;
  flex-direction: column;
  gap: 2px;
  overflow-y: auto;
  flex: 1;
}
.nav-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 9px 12px;
  border-radius: 8px;
  color: var(--text-secondary);
  font-size: 13.5px;
  text-decoration: none;
  transition: background 0.15s ease, color 0.15s ease, transform 0.15s ease;
}
.nav-item:hover:not(.disabled) {
  background: var(--overlay-soft);
  color: var(--text);
}
.nav-item.active {
  background: var(--primary-bg);
  color: var(--primary);
  font-weight: 600;
  box-shadow: inset 0 0 0 1px var(--primary-border);
}
.nav-item.disabled {
  color: var(--muted);
  cursor: not-allowed;
  opacity: 0.75;
}
.nav-item.hidden {
  display: none;
}

/* ===== 二级菜单分组 ===== */
.nav-group {
  margin-top: 4px;
}
.nav-group.hidden {
  display: none;
}
.nav-group-btn {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  padding: 8px 12px;
  border: none;
  background: transparent;
  color: var(--text-secondary);
  font-size: 12.5px;
  font-weight: 600;
  letter-spacing: 0.02em;
  text-transform: uppercase;
  cursor: pointer;
  border-radius: 8px;
  transition: background 0.15s ease, color 0.15s ease;
}
.nav-group-btn:hover {
  background: var(--overlay-soft);
  color: var(--text);
}
.nav-group-btn .nav-label {
  flex: 1;
  text-align: left;
}
.nav-caret {
  color: var(--muted);
  transition: transform 0.18s ease;
}
.nav-sub {
  display: none;
  flex-direction: column;
  gap: 2px;
  padding: 2px 0 4px 10px;
  margin-left: 6px;
  border-left: 1px dashed var(--line-soft);
}
.nav-group.open .nav-sub {
  display: flex;
}
.nav-group.open .nav-caret {
  transform: rotate(0deg);
}
.nav-group.has-active .nav-group-btn {
  color: var(--text);
}
.nav-icon {
  width: 20px;
  text-align: center;
  font-size: 16px;
}
.nav-label {
  flex: 1;
}
.nav-soon {
  font-size: 11px;
  color: var(--muted);
  padding: 1px 6px;
  border-radius: 999px;
  border: 1px dashed var(--line-soft);
}
.sidebar-foot {
  padding: 10px 14px 16px;
  font-size: 12px;
  color: var(--muted);
  border-top: 1px solid var(--line);
  display: flex;
  align-items: center;
  gap: 6px;
}
.foot-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--ok);
  box-shadow: 0 0 0 3px rgba(92, 184, 122, 0.18);
}

/* ===== 主区 ===== */
.main-area {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-width: 0;
}
.topbar {
  height: 56px;
  padding: 0 20px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  background: var(--bg-elev);
  border-bottom: 1px solid var(--line);
  position: sticky;
  top: 0;
  z-index: 20;
  backdrop-filter: saturate(180%) blur(6px);
}
.topbar-left .crumb-title {
  font-weight: 600;
  color: var(--heading);
  font-size: 15px;
}
.topbar-right {
  display: flex;
  align-items: center;
  gap: 14px;
}
.theme-switcher {
  display: inline-flex;
  gap: 4px;
  padding: 2px;
  border-radius: 999px;
  background: var(--overlay-soft);
  border: 1px solid var(--line);
}
.beta-toggle {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 4px 10px;
  border-radius: 999px;
  background: var(--overlay-soft);
  border: 1px solid var(--line);
  font-size: 12.5px;
  color: var(--text-secondary);
}
.beta-icon {
  color: var(--accent);
}
.beta-label {
  margin-right: 4px;
}
.user-chip {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  padding: 4px 10px 4px 4px;
  border-radius: 999px;
  background: var(--overlay-soft);
  border: 1px solid var(--line);
  cursor: pointer;
  transition: background 0.15s ease;
}
.user-chip:hover {
  background: var(--overlay-mid);
}
.avatar {
  width: 28px;
  height: 28px;
  border-radius: 50%;
  display: grid;
  place-items: center;
  font-weight: 700;
  color: var(--on-primary);
  background: linear-gradient(135deg, var(--primary-grad-from), var(--primary-grad-to));
  font-size: 13px;
}
.user-name {
  font-size: 13px;
  color: var(--text);
  max-width: 140px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.password-warn {
  padding: 10px 24px 0;
}
.license-banner {
  margin: 0 24px 12px;
}
.page-area {
  flex: 1;
  padding: 20px 24px 32px;
  min-width: 0;
}

.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.18s ease, transform 0.18s ease;
}
.fade-enter-from {
  opacity: 0;
  transform: translateY(6px);
}
.fade-leave-to {
  opacity: 0;
  transform: translateY(-4px);
}
</style>
