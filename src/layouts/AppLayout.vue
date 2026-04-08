<script setup lang="ts">
import { ref } from 'vue'
import TitleBar from '@/components/TitleBar.vue'
import QueryLogPanel from '@/components/QueryLogPanel.vue'
import KeybindingsDialog from '@/components/dialogs/KeybindingsDialog.vue'

const showKeybindings = ref(false)
</script>

<template>
  <div class="flex flex-col h-screen bg-background text-foreground overflow-hidden">
    <!-- El TitleBar global -->
    <TitleBar @open-keybindings="showKeybindings = true" />

    <!-- El contenido de la ruta -->
    <main class="flex-1 overflow-hidden relative bg-background min-h-0">
      <router-view />
    </main>

    <!-- Query Log Panel -->
    <QueryLogPanel />

    <!-- Keybindings dialog -->
    <KeybindingsDialog v-model:open="showKeybindings" />
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
