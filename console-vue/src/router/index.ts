import { createRouter, createWebHashHistory, type RouteRecordRaw, type NavigationGuardNext, type RouteLocationNormalized } from 'vue-router'
import { defineAsyncComponent } from 'vue'
import { ElMessage } from 'element-plus'
import { useAuthStore } from '@/stores/auth'

const LoginView = defineAsyncComponent(() => import('@/views/Login.vue'))
const OverviewView = defineAsyncComponent(() => import('@/views/Overview.vue'))
const AlertsView = defineAsyncComponent(() => import('@/views/Alerts.vue'))
const SilencesView = defineAsyncComponent(() => import('@/views/Silences.vue'))
const MaintenanceView = defineAsyncComponent(() => import('@/views/Maintenance.vue'))
const DatasourcesView = defineAsyncComponent(() => import('@/views/Datasources.vue'))
const RulesView = defineAsyncComponent(() => import('@/views/Rules.vue'))
const IngressView = defineAsyncComponent(() => import('@/views/Ingress.vue'))
const ChannelsView = defineAsyncComponent(() => import('@/views/Channels.vue'))
const NotifiesView = defineAsyncComponent(() => import('@/views/Notifies.vue'))
const EnrichmentsView = defineAsyncComponent(() => import('@/views/Enrichments.vue'))
const LookupsView = defineAsyncComponent(() => import('@/views/Lookups.vue'))
const UsersView = defineAsyncComponent(() => import('@/views/Users.vue'))
const AuditView = defineAsyncComponent(() => import('@/views/Audit.vue'))
const SystemSettingsView = defineAsyncComponent(() => import('@/views/SystemSettings.vue'))
const KafkaView = defineAsyncComponent(() => import('@/views/KafkaView.vue'))
const TrapView = defineAsyncComponent(() => import('@/views/TrapView.vue'))
const MIBView = defineAsyncComponent(() => import('@/views/MIBView.vue'))
const PoliciesView = defineAsyncComponent(() => import('@/views/PoliciesView.vue'))
const MainLayout = defineAsyncComponent(() => import('@/layouts/MainLayout.vue'))

declare module 'vue-router' {
  interface RouteMeta {
    /** 是否公开页面（无需登录） */
    public?: boolean
    /** 所需权限编码 */
    perm?: string
    /** 页面标题（浏览器标签/面包屑） */
    title?: string
  }
}

export const routes: RouteRecordRaw[] = [
  {
    path: '/login',
    name: 'Login',
    component: LoginView,
    meta: { public: true, title: '登录 Eventide' },
  },
  {
    path: '/',
    redirect: '/overview',
  },
  {
    path: '/',
    component: MainLayout,
    children: [
      {
        path: 'overview',
        name: 'Overview',
        component: OverviewView,
        meta: { perm: 'overview:read', title: '总览' },
      },
      {
        path: 'alerts',
        name: 'Alerts',
        component: AlertsView,
        meta: { perm: 'alerts:read', title: '告警事件' },
      },
      {
        path: 'silences',
        name: 'Silences',
        component: SilencesView,
        meta: { perm: 'silences:read', title: '静默策略' },
      },
      {
        path: 'maintenance',
        name: 'Maintenance',
        component: MaintenanceView,
        meta: { perm: 'maintenance:read', title: '维护窗' },
      },
      {
        path: 'datasources',
        name: 'Datasources',
        component: DatasourcesView,
        meta: { perm: 'datasources:read', title: '数据源' },
      },
      {
        path: 'rules',
        name: 'Rules',
        component: RulesView,
        meta: { perm: 'rules:read', title: '告警规则' },
      },
      {
        path: 'ingress',
        name: 'Ingress',
        component: IngressView,
        meta: { perm: 'ingress:read', title: '告警接入' },
      },
      {
        path: 'channels',
        name: 'Channels',
        component: ChannelsView,
        meta: { perm: 'channels:read', title: '通知渠道' },
      },
      {
        path: 'notifies',
        name: 'Notifies',
        component: NotifiesView,
        meta: { perm: 'notifies:read', title: '通知记录' },
      },
      {
        path: 'enrich',
        name: 'Enrichments',
        component: EnrichmentsView,
        meta: { perm: 'enrich:read', title: '告警丰富' },
      },
      {
        path: 'lookups',
        name: 'Lookups',
        component: LookupsView,
        meta: { perm: 'lookups:read', title: '查询字典' },
      },
      {
        path: 'users',
        name: 'Users',
        component: UsersView,
        meta: { perm: 'iam:read', title: '用户管理' },
      },
      {
        path: 'roles',
        name: 'Roles',
        component: UsersView,
        meta: { perm: 'iam:read', title: '权限管理' },
      },
      {
        path: 'departments',
        name: 'Departments',
        component: UsersView,
        meta: { perm: 'iam:read', title: '部门管理' },
      },
      {
        path: 'audit',
        name: 'Audit',
        component: AuditView,
        meta: { perm: 'audit:read', title: '操作审计' },
      },
      {
        path: 'settings',
        name: 'SystemSettings',
        component: SystemSettingsView,
        meta: { perm: 'settings:read', title: '系统设置' },
      },
      {
        path: 'kafka',
        name: 'Kafka',
        component: KafkaView,
        meta: { perm: 'ingress:read', title: 'Kafka 接入' },
      },
      {
        path: 'trap',
        name: 'Trap',
        component: TrapView,
        meta: { perm: 'trap:read', title: 'SNMP Trap' },
      },
      {
        path: 'mib',
        name: 'MIB',
        component: MIBView,
        meta: { perm: 'trap:read', title: 'MIB 库' },
      },
      {
        path: 'policies',
        name: 'Policies',
        component: PoliciesView,
        meta: { perm: 'trap:read', title: 'Trap 策略' },
      },
    ],
  },
  // 兜底：未匹配路径回到总览（后续批次可接 404）
  {
    path: '/:pathMatch(.*)*',
    redirect: '/overview',
  },
]

export const router = createRouter({
  history: createWebHashHistory(),
  routes,
})

/**
 * 全局路由守卫（TR-4.3）
 * - 无 token && 非 public → 登录页
 * - 有 token 访问 login → 跳总览
 * - meta.perm：若 me 未加载完，则延后；me 完成后若仍无权限 → 警告 + 跳 /overview
 */
router.beforeEach(
  async (
    to: RouteLocationNormalized,
    _from: RouteLocationNormalized,
    next: NavigationGuardNext,
  ) => {
    const auth = useAuthStore()

    // 1) 公开页面一律放行（但登录页对已登录用户做重定向）
    if (to.meta.public === true) {
      if (to.path === '/login' && auth.isLoggedIn) {
        return next('/overview')
      }
      return next()
    }

    // 2) 未登录 → 登录
    if (!auth.isLoggedIn) {
      return next({ path: '/login', query: { redirect: to.fullPath } })
    }

    // 3) 若还未拉过 me，先拉一次再做权限校验。
    //    拉取失败时若已缓存 user/permissions（例如后端暂时不可达，但本地已登录过），
    //    走缓存兜底避免被踢出登录页；只有缓存也空才强跳登录。
    if (!auth.meLoaded) {
      try {
        await auth.me()
      } catch {
        if (!auth.isLoggedIn || auth.permissions.length === 0) {
          auth.logout()
          return next({ path: '/login', query: { redirect: to.fullPath } })
        }
        // 缓存兜底：meLoaded 置 true，后续权限校验用缓存 permissions。
        // （auth:unauthorized 事件仅在 request 判定鉴权失败时才会被 dispatch，
        //  网络/服务不可达的 GET 404/500 类错误不会触发）。
      }
    }

    // 4) 权限校验
    const needPerm = to.meta.perm
    if (needPerm && !auth.can(needPerm)) {
      ElMessage.warning(`您没有访问此页面的权限（${needPerm}），已为您跳转到总览。`)
      return next('/overview')
    }

    return next()
  },
)

/** 可选：设置 document.title */
router.afterEach((to) => {
  const t = to.meta?.title
  if (t) {
    document.title = `${t} · Eventide`
  } else {
    document.title = 'Eventide Console'
  }
})

export default router
