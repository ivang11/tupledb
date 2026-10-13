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
import ConnectionStorageNotice from '@/components/ConnectionStorageNotice.vue'

const UpdaterDialog = defineAsyncComponent(() => import('@/components/dialogs/UpdaterDialog.vue'))

useAppearance()
const { checkForUpdates, dialogOpen } = useUpdater()
const AUTO_UPDATE_CHECK_DELAY_MS = 30_000

let stopUpdateRequests: UnlistenFn | undefined

onMounted(async () => {
  // The settings window asks for update checks here, where the update dialog lives.
  stopUpdateRequests = await listen('tupledb:check-updates', () => void checkForUpdates())
  if (!import.meta.env.PROD) return

  window.setTimeout(() => {
    void checkForUpdates({ silent: true })
  }, AUTO_UPDATE_CHECK_DELAY_MS)
})

onBeforeUnmount(() => stopUpdateRequests?.())
</script>

<style>
/* Reset global para asegurar que no haya márgenes extraños */
body {
  margin: 0;
  padding: 0;
  overflow: hidden;
}
</style>
