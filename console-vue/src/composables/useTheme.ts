import { computed } from 'vue'
import { useUiStore } from '@/stores/ui'

export type ThemePref = 'light' | 'dark' | 'system'

function applyThemeToDom(pref: ThemePref) {
  const resolved: 'light' | 'dark' =
    pref === 'light' || pref === 'dark'
      ? pref
      : window.matchMedia('(prefers-color-scheme: dark)').matches
        ? 'dark'
        : 'light'
  document.documentElement.setAttribute('data-theme', resolved)
  document.documentElement.setAttribute('data-theme-pref', pref === 'light' || pref === 'dark' ? pref : 'system')
}

export function useTheme() {
  const ui = useUiStore()

  const theme = computed<ThemePref>(() => ui.theme)

  const resolvedTheme = computed<'light' | 'dark'>(() => {
    if (ui.theme === 'light' || ui.theme === 'dark') return ui.theme
    return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light'
  })

  function setTheme(pref: ThemePref) {
    ui.setTheme(pref)
    applyThemeToDom(pref)
  }

  function initTheme() {
    applyThemeToDom(ui.theme)
  }

  return {
    theme,
    resolvedTheme,
    setTheme,
    initTheme,
  }
}

export default useTheme
