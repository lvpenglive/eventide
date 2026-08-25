<script setup lang="ts">
import { onMounted, reactive, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import {
  ElButton,
  ElCard,
  ElDialog,
  ElForm,
  ElInput,
  ElMessage,
  ElTag,
  ElTooltip,
  type FormInstance,
  type FormRules,
} from 'element-plus'
import { Lock, Sunny, Moon, Monitor, User } from '@element-plus/icons-vue'
import { useAuthStore } from '@/stores/auth'
import { useTheme, type ThemePref } from '@/composables/useTheme'
import { validatePasswordComplexity } from '@/utils/password'

const auth = useAuthStore()
const router = useRouter()
const route = useRoute()
const { theme, setTheme, initTheme } = useTheme()
onMounted(() => initTheme())

// ---- 登录表单 ----
const loginFormRef = ref<FormInstance>()
const loginSubmitting = ref(false)
const loginModel = reactive({ username: '', password: '' })
const loginRules: FormRules = {
  username: [{ required: true, message: '请输入用户名', trigger: 'blur' }],
  password: [{ required: true, message: '请输入密码', trigger: 'blur' }],
}

async function submitLogin() {
  const form = loginFormRef.value
  if (form) {
    try {
      await form.validate()
    } catch {
      return
    }
  }
  loginSubmitting.value = true
  try {
    const { must_change_password } = await auth.login(loginModel.username, loginModel.password)
    ElMessage.success('登录成功')

    if (must_change_password) {
      // 强制进入修改密码对话模式（TR-4.5）
      changePwdModel.oldPassword = loginModel.password
      changePwdForced.value = true
      changePwdVisible.value = true
      return
    }

    const redirect = typeof route.query.redirect === 'string' ? route.query.redirect : '/overview'
    router.replace(redirect)
  } catch (e) {
    const msg =
      e && typeof e === 'object' && 'message' in e
        ? String((e as { message: unknown }).message)
        : '登录失败'
    ElMessage.error(msg)
  } finally {
    loginSubmitting.value = false
  }
}

function onLoginKeydown(e: KeyboardEvent) {
  if (e.key === 'Enter') {
    void submitLogin()
  }
}

// ---- 修改密码对话框（首次登录/密码过期强制） ----
const changePwdVisible = ref(false)
const changePwdForced = ref(false)
const changePwdFormRef = ref<FormInstance>()
const changePwdSubmitting = ref(false)
const changePwdModel = reactive({ oldPassword: '', newPassword: '', confirmPassword: '' })
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
        if (value !== changePwdModel.newPassword) cb(new Error('两次输入的新密码不一致'))
        else cb()
      },
      trigger: 'blur',
    },
  ],
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
    const ok = await auth.changePassword(changePwdModel.oldPassword, changePwdModel.newPassword)
    if (!ok) throw new Error('服务器返回修改失败')
    ElMessage.success('密码修改成功，请使用新密码重新登录')
    changePwdVisible.value = false
    changePwdForced.value = false
    auth.logout()
  } catch (e) {
    const msg =
      e && typeof e === 'object' && 'message' in e
        ? String((e as { message: unknown }).message)
        : '修改密码失败'
    ElMessage.error(msg)
  } finally {
    changePwdSubmitting.value = false
  }
}

// ---- 主题按钮（右上角，TR-4.5） ----
const themeOpts: { key: ThemePref; label: string; icon: typeof Sunny }[] = [
  { key: 'light', label: '浅色模式', icon: Sunny },
  { key: 'dark', label: '深色模式', icon: Moon },
  { key: 'system', label: '跟随系统', icon: Monitor },
]
</script>

<template>
  <div class="login-page">
    <!-- 背景装饰 -->
    <div class="bg-orb bg-orb-a" aria-hidden="true" />
    <div class="bg-orb bg-orb-b" aria-hidden="true" />

    <!-- 右上角主题切换 -->
    <div class="theme-corner">
      <el-tooltip v-for="opt in themeOpts" :key="opt.key" :content="opt.label" placement="bottom">
        <el-button
          :type="theme === opt.key ? 'primary' : 'default'"
          size="small"
          text
          round
          class="theme-btn"
          @click="setTheme(opt.key)"
        >
          <el-icon :size="16"><component :is="opt.icon" /></el-icon>
        </el-button>
      </el-tooltip>
    </div>

    <!-- 居中卡片（1:1 复刻旧版，TR-4.5） -->
    <el-card class="login-card" shadow="always">
      <div class="login-head">
        <div class="logo-block">
          <span class="logo-dot" />
          <h1 class="title">Eventide</h1>
          <el-tag size="small" effect="dark" type="success" class="ver-badge">v2 Beta</el-tag>
        </div>
        <p class="subtitle">事件 / 告警 统一控制台</p>
      </div>

      <el-form
        ref="loginFormRef"
        :model="loginModel"
        :rules="loginRules"
        label-position="top"
        class="login-form"
        @keydown="onLoginKeydown"
      >
        <el-form-item label="用户名" prop="username">
          <el-input
            v-model="loginModel.username"
            placeholder="请输入用户名"
            size="large"
            autocomplete="username"
          >
            <template #prefix>
              <el-icon><User /></el-icon>
            </template>
          </el-input>
        </el-form-item>
        <el-form-item label="密码" prop="password">
          <el-input
            v-model="loginModel.password"
            placeholder="请输入密码"
            size="large"
            show-password
            type="password"
            autocomplete="current-password"
          >
            <template #prefix>
              <el-icon><Lock /></el-icon>
            </template>
          </el-input>
        </el-form-item>

        <el-button
          type="primary"
          size="large"
          class="submit-btn"
          :loading="loginSubmitting"
          @click="submitLogin"
        >
          登 录
        </el-button>
        <p class="foot-tip">按 Enter 直接登录 · 建议使用最新版 Chrome / Edge</p>
      </el-form>
    </el-card>

    <!-- 底部版权 -->
    <div class="copyright">© Eventide Console — eventide-server v2</div>

    <!-- 强制修改密码对话框（TR-4.5 支持 expired/change_required） -->
    <el-dialog
      v-model="changePwdVisible"
      title="必须修改密码后才可继续"
      width="460px"
      :close-on-click-modal="false"
      :close-on-press-escape="!changePwdForced"
      destroy-on-close
    >
      <p v-if="changePwdForced" class="force-note">
        由于密码已过期或管理员要求首次登录改密，您必须完成以下修改：
      </p>
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
        <el-button v-if="!changePwdForced" @click="changePwdVisible = false">取消</el-button>
        <el-button type="primary" :loading="changePwdSubmitting" @click="submitChangePassword">
          确认修改并重新登录
        </el-button>
      </template>
    </el-dialog>
  </div>
</template>

<style scoped>
.login-page {
  min-height: 100vh;
  width: 100%;
  position: relative;
  display: grid;
  place-items: center;
  background: var(--login-bg);
  color: var(--text);
  overflow: hidden;
}

.bg-orb {
  position: absolute;
  border-radius: 50%;
  filter: blur(80px);
  opacity: 0.7;
  pointer-events: none;
  z-index: 0;
}
.bg-orb-a {
  width: 520px;
  height: 520px;
  top: -180px;
  left: -120px;
  background: radial-gradient(circle, var(--body-glow-1), transparent 60%);
}
.bg-orb-b {
  width: 560px;
  height: 560px;
  bottom: -200px;
  right: -160px;
  background: radial-gradient(circle, var(--body-glow-2), transparent 60%);
}

.theme-corner {
  position: absolute;
  top: 18px;
  right: 22px;
  z-index: 5;
  display: inline-flex;
  gap: 4px;
  padding: 3px;
  background: var(--overlay-soft);
  border: 1px solid var(--line);
  border-radius: 999px;
}
.theme-btn {
  color: var(--text-secondary);
}

.login-card {
  width: 420px;
  max-width: calc(100% - 32px);
  z-index: 2;
  background: var(--login-panel-bg);
  border: 1px solid var(--line);
  border-radius: 16px;
  box-shadow: var(--shadow);
  backdrop-filter: blur(10px) saturate(180%);
}
.login-card :deep(.el-card__body) {
  padding: 28px 28px 22px;
}

.login-head {
  text-align: center;
  margin-bottom: 22px;
}
.logo-block {
  display: inline-flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 6px;
}
.logo-dot {
  width: 14px;
  height: 14px;
  border-radius: 50%;
  background: linear-gradient(135deg, var(--primary-grad-from), var(--accent));
  box-shadow: 0 0 0 4px var(--primary-bg);
}
.title {
  margin: 0;
  font-size: 30px;
  font-weight: 800;
  letter-spacing: 0.02em;
  background: var(--brand-grad);
  -webkit-background-clip: text;
  background-clip: text;
  color: transparent;
}
.ver-badge {
  margin-left: 4px;
  transform: translateY(-2px);
}
.subtitle {
  margin: 4px 0 0;
  color: var(--text-secondary);
  font-size: 13px;
}

.login-form {
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.submit-btn {
  width: 100%;
  margin-top: 10px;
  height: 42px;
  font-weight: 600;
  letter-spacing: 0.06em;
  background: linear-gradient(135deg, var(--primary-grad-from), var(--primary-grad-to));
  border-color: transparent;
  box-shadow: 0 8px 20px -8px var(--primary);
}
.foot-tip {
  text-align: center;
  color: var(--muted);
  font-size: 12px;
  margin-top: 14px;
}

.copyright {
  position: absolute;
  bottom: 14px;
  left: 0;
  right: 0;
  text-align: center;
  font-size: 12px;
  color: var(--muted);
  z-index: 1;
}

.force-note {
  padding: 10px 12px;
  background: var(--primary-bg);
  border: 1px solid var(--primary-border);
  border-radius: 8px;
  font-size: 13px;
  color: var(--text);
}
</style>
