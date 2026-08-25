import { defineStore } from 'pinia'

type ThemePref = 'light' | 'dark' | 'system'

interface UiState {
  theme: ThemePref
  sidebarCollapsed: boolean
  navGroupsOpen: Record<string, boolean>
}

function loadNavGroups(): Record<string, boolean> {
  try {
    const raw = localStorage.getItem('eventide_nav_groups')
    return raw ? (JSON.parse(raw) as Record<string, boolean>) : {}
  } catch {
    return {}
  }
}

export const useUiStore = defineStore('ui', {
  state: (): UiState => ({
    theme: (localStorage.getItem('eventide_theme') as ThemePref) || 'system',
    sidebarCollapsed: localStorage.getItem('eventide_sidebar_collapsed') === '1',
    navGroupsOpen: loadNavGroups(),
  }),
  actions: {
    setTheme(pref: ThemePref) {
      this.theme = pref
      localStorage.setItem('eventide_theme', pref)
    },
    toggleSidebar() {
      this.sidebarCollapsed = !this.sidebarCollapsed
      localStorage.setItem('eventide_sidebar_collapsed', this.sidebarCollapsed ? '1' : '0')
    },
    setNavGroupOpen(id: string, open: boolean) {
      this.navGroupsOpen[id] = open
      localStorage.setItem('eventide_nav_groups', JSON.stringify(this.navGroupsOpen))
    },
  },
})
