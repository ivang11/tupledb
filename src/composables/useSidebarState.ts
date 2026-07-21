import { ref } from 'vue'

// Module-level singleton shared between TitleBar and WorkspaceView
const _sidebarVisible = ref(true)
const _sidebarToggleVisible = ref(true)

export function useSidebarState() {
  return {
    sidebarVisible: _sidebarVisible,
    sidebarToggleVisible: _sidebarToggleVisible,
  }
}
