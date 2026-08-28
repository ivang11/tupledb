<template>
  <div class="flex flex-col h-screen bg-background text-foreground overflow-hidden">
    <WindowResizeHandles v-if="!usesNativeMacWindowControls" />
    <TitleBar @open-keybindings="showKeybindings = true" />

    <main class="flex-1 overflow-hidden relative bg-background min-h-0">
      <WorkspaceView />
    </main>

    <KeybindingsDialog v-if="showKeybindings" v-model:open="showKeybindings" />
    <UpdaterDialog v-if="dialogOpen" />
    <ToastContainer />
  </div>
</template>

<script setup lang="ts">
import { defineAsyncComponent, onMounted, ref } from 'vue'
import TitleBar from '@/components/TitleBar.vue'
import WindowResizeHandles from '@/components/WindowResizeHandles.vue'
import ToastContainer from '@/components/ToastContainer.vue'
import WorkspaceView from '@/views/WorkspaceView.vue'
import { useUpdater } from '@/composables/useUpdater'
import { usesNativeMacWindowControls } from '@/lib/platform'

const KeybindingsDialog = defineAsyncComponent(() => import('@/components/dialogs/KeybindingsDialog.vue'))
const UpdaterDialog = defineAsyncComponent(() => import('@/components/dialogs/UpdaterDialog.vue'))

const showKeybindings = ref(false)
const { checkForUpdates, dialogOpen } = useUpdater()
const AUTO_UPDATE_CHECK_DELAY_MS = 30_000

onMounted(() => {
  if (!import.meta.env.PROD) return

  window.setTimeout(() => {
    void checkForUpdates({ silent: true })
  }, AUTO_UPDATE_CHECK_DELAY_MS)
})
</script>

<style>
/* Reset global para asegurar que no haya márgenes extraños */
body {
  margin: 0;
  padding: 0;
  overflow: hidden;
}
</style>
