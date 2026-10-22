import { defineStore } from 'pinia'
import { login as apiLogin, me as apiMe, changePassword as apiChangePassword } from '@/api/overviewAuthLoginAndMore'
import type { MeResp, PasswordStatus } from '@/api/types'
import { can as rawCan } from '@/perms'

/** 从 mysql://… 解析展示用信息（不展示密码） */
function parseDatabaseFromMe(
  mysqlUrl?: string,
  redisUrl?: string,
): AuthState['database'] | undefined {
  if (!mysqlUrl && !redisUrl) return undefined
  const out: NonNullable<AuthState['database']> = {
    redis_url: redisUrl || undefined,
  }
  if (mysqlUrl) {
    out.db_type = 'mysql'
    try {
      const u = new URL(mysqlUrl.replace(/^mysql:\/\//i, 'http://'))
      out.host = u.hostname || undefined
      out.port = u.port ? Number(u.port) : 3306
      out.name = u.pathname.replace(/^\//, '') || undefined
    } catch {
      out.host = '(mysql)'
    }
  }
  return out
}

export interface AuthUser {
  id?: string
  username: string
  display_name: string
  permissions: string[]
  password_status?: PasswordStatus
  token_ttl_hours?: number
}

interface AuthState {
  token: string
  user: AuthUser | null
  /** me() 已完成加载（用于路由守卫延迟校验） */
  meLoaded: boolean
  /** 最近一次 me() 返回的数据库信息，供 Overview 展示 */
  database?: {
    db_type?: string
    host?: string
    port?: number
    name?: string
    redis_url?: string
  }
}

const TOKEN_KEY = 'eventide_token'
const USER_KEY = 'eventide_user'

function readUserFromStorage(): AuthUser | null {
  try {
    const raw = localStorage.getItem(USER_KEY)
    if (!raw) return null
    const obj = JSON.parse(raw) as AuthUser
    if (!obj || typeof obj.username !== 'string') return null
    return obj
  } catch {
    return null
  }
}

export const useAuthStore = defineStore('auth', {
  state: (): AuthState => ({
    token: localStorage.getItem(TOKEN_KEY) || '',
    user: readUserFromStorage(),
    meLoaded: false,
    database: undefined,
  }),
  getters: {
    isLoggedIn: (state): boolean => !!state.token,
    displayName: (state): string => state.user?.display_name || state.user?.username || '',
    permissions: (state): string[] => state.user?.permissions || [],
    passwordStatus: (state): PasswordStatus | undefined => state.user?.password_status,
  },
  actions: {
    can(perm: string): boolean {
      return rawCan(perm, this.permissions)
    },

    /** 调用登录接口并写入本地 */
    async login(username: string, password: string): Promise<{ must_change_password: boolean }> {
      const resp = await apiLogin({ username, password })
      this.token = resp.token
      localStorage.setItem(TOKEN_KEY, resp.token)

      const user: AuthUser = {
        id: resp.id,
        username: resp.username,
        display_name: resp.display_name || resp.username,
        permissions: resp.permissions || [],
        password_status: resp.password_status,
        token_ttl_hours: resp.token_ttl_hours,
      }
      this.user = user
      this.meLoaded = false
      localStorage.setItem(USER_KEY, JSON.stringify(user))

      const must_change_password =
        !!resp.password_status?.expired || !!resp.password_status?.change_required
      return { must_change_password }
    },

    /** 拉取当前用户信息并刷新 state/permissions/password_status */
    async me(): Promise<void> {
      if (!this.token) return
      try {
        const resp = await apiMe() as MeResp & {
          mysql_url?: string
          redis_url?: string
          uid?: string
        }
        const user: AuthUser = {
          id: resp.id || resp.uid,
          username: resp.username,
          display_name: resp.display_name || resp.username,
          permissions: resp.permissions || [],
          password_status: resp.password_status,
          token_ttl_hours: resp.token_ttl_hours,
        }
        this.user = user
        // 后端 /api/auth/me 返回扁平 mysql_url/redis_url；兼容可选 database 对象
        this.database = resp.database || parseDatabaseFromMe(resp.mysql_url, resp.redis_url)
        this.meLoaded = true
        localStorage.setItem(USER_KEY, JSON.stringify(user))
      } catch (e) {
        this.meLoaded = true
        throw e
      }
    },

    async changePassword(oldPassword: string, newPassword: string): Promise<boolean> {
      const resp = await apiChangePassword(oldPassword, newPassword)
      return !!resp?.ok
    },

    logout(): void {
      this.token = ''
      this.user = null
      this.meLoaded = false
      this.database = undefined
      localStorage.removeItem(TOKEN_KEY)
      localStorage.removeItem(USER_KEY)
      // 通知 App / request.ts 处理跳转
      window.dispatchEvent(new CustomEvent('auth:unauthorized', { detail: { from: 'logout' } }))
    },
  },
})

export default useAuthStore
