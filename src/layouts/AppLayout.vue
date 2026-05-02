<script setup lang="ts">
import { onMounted, ref } from 'vue'
import TitleBar from '@/components/TitleBar.vue'
import StatusBar from '@/components/StatusBar.vue'
import ToastContainer from '@/components/ToastContainer.vue'
import KeybindingsDialog from '@/components/dialogs/KeybindingsDialog.vue'
import UpdaterDialog from '@/components/dialogs/UpdaterDialog.vue'
import { useUpdater } from '@/composables/useUpdater'

const showKeybindings = ref(false)
const { checkForUpdates } = useUpdater()

onMounted(() => {
  void checkForUpdates({ silent: true })
})
</script>

<template>
  <div class="flex flex-col h-screen bg-background text-foreground overflow-hidden">
    <TitleBar @open-keybindings="showKeybindings = true" />

    <main class="flex-1 overflow-hidden relative bg-background min-h-0">
      <router-view />
    </main>

    <StatusBar />
    <KeybindingsDialog v-model:open="showKeybindings" />
    <UpdaterDialog />
    <ToastContainer />
  </div>
</template>

<style>
/* Reset global para asegurar que no haya márgenes extraños */
body {
  margin: 0;
  padding: 0;
  overflow: hidden;
}
</style>
