<template>
  <div class="flex flex-col h-screen bg-background text-foreground overflow-hidden">
    <WindowResizeHandles v-if="!usesNativeMacWindowControls" />
    <TitleBar />
    <ConnectionStorageNotice />

    <main class="flex-1 overflow-hidden relative bg-background min-h-0">
      <WorkspaceView />
    </main>

    <UpdaterDialog v-if="dialogOpen" />
    <ToastContainer />
  </div>
</template>

<script setup lang="ts">
import { defineAsyncComponent, onBeforeUnmount, onMounted } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { useAppearance } from '@/composables/useAppearance'
import TitleBar from '@/components/TitleBar.vue'
import WindowResizeHandles from '@/components/WindowResizeHandles.vue'
import ToastContainer from '@/components/ToastContainer.vue'
import WorkspaceView from '@/views/WorkspaceView.vue'
import { useUpdater } from '@/composables/useUpdater'
import { usesNativeMacWindowControls } from '@/lib/platform'
import { useSidebarState } from '@/composables/useSidebarState'
import { openSettingsWindow } from '@/lib/settingsWindow'
import { useToast } from '@/composables/useToast'
import ConnectionStorageNotice from '@/components/ConnectionStorageNotice.vue'

const UpdaterDialog = defineAsyncComponent(() => import('@/components/dialogs/UpdaterDialog.vue'))

useAppearance()
const { checkForUpdates, dialogOpen } = useUpdater()
const AUTO_UPDATE_CHECK_DELAY_MS = 30_000

let stopUpdateRequests: UnlistenFn | undefined
let stopMenuEvents: UnlistenFn | undefined
const { sidebarVisible } = useSidebarState()

// The native menu bar (built in Rust) reports its picks here; workspace-level
// actions are re-dispatched as DOM events that WorkspaceView handles.
function onMenuAction(action: string) {
  switch (action) {
    case 'settings': return void openSettingsWindow().catch(error => useToast().error('Could not open settings', String(error)))
    case 'settings-keyboard': return void openSettingsWindow('keyboard').catch(error => useToast().error('Could not open settings', String(error)))
    case 'check-updates': return void checkForUpdates()
    case 'toggle-sidebar': sidebarVisible.value = !sidebarVisible.value; return
    case 'quick-search': return window.dispatchEvent(new Event('tupledb:quick-search'))
    default: return window.dispatchEvent(new Event(`tupledb:menu:${action}`))
  }
}

onMounted(async () => {
  stopMenuEvents = await listen<string>('tupledb:menu', event => onMenuAction(String(event.payload)))
  // The settings window asks for update checks here, where the update dialog lives.
  stopUpdateRequests = await listen('tupledb:check-updates', () => void checkForUpdates())
  if (!import.meta.env.PROD) return

  window.setTimeout(() => {
    void checkForUpdates({ silent: true })
  }, AUTO_UPDATE_CHECK_DELAY_MS)
})

onBeforeUnmount(() => { stopUpdateRequests?.(); stopMenuEvents?.() })
</script>

<style>
/* Reset global para asegurar que no haya márgenes extraños */
body {
  margin: 0;
  padding: 0;
  overflow: hidden;
}
</style>
