import { ref } from 'vue'

// Module-level singleton shared between TitleBar and HomeView
const _sidebarVisible = ref(true)

export function useSidebarState() {
  return { sidebarVisible: _sidebarVisible }
}
