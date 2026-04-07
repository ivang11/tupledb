<script setup lang="ts">
import TitleBar from '@/components/TitleBar.vue'
import QueryLogPanel from '@/components/QueryLogPanel.vue'
</script>

<template>
  <div class="flex flex-col h-screen bg-background text-foreground overflow-hidden">
    <!-- El TitleBar global -->
    <TitleBar />

    <!-- El contenido de la ruta -->
    <main class="flex-1 overflow-hidden relative bg-background min-h-0">
      <router-view />
    </main>

    <!-- Query Log Panel -->
    <QueryLogPanel />
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
