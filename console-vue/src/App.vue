<script setup lang="ts">
import { onMounted, onUnmounted } from 'vue'
import { useRouter } from 'vue-router'
import { ElMessage } from 'element-plus'
import { useAuthStore } from '@/stores/auth'
import { useTheme } from '@/composables/useTheme'

const router = useRouter()
const auth = useAuthStore()
const { initTheme } = useTheme()

// 全局：启动时应用一次主题，避免首屏闪烁
onMounted(() => {
  initTheme()
  window.addEventListener('auth:unauthorized', handleUnauthorized)
})

onUnmounted(() => {
  window.removeEventListener('auth:unauthorized', handleUnauthorized)
})

function handleUnauthorized() {
  // 先清 authStore（含 localStorage 两个键）
  auth.token = ''
  auth.user = null
  localStorage.removeItem('eventide_token')
  localStorage.removeItem('eventide_user')

  ElMessage.warning('登录已过期，请重新登录')
  // 避免在登录页重复进入
  if (router.currentRoute.value.path !== '/login') {
    void router.push('/login')
  }
}
</script>

<template>
  <RouterView />
</template>
