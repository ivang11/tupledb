<script setup lang="ts">
import { RouterView } from 'vue-router'
</script>

<template>
  <div class="h-screen w-screen overflow-hidden bg-background">
    <RouterView />
  </div>
</template>

<style>
/* Estilo base global */
:root {
  color-scheme: dark;
}

html, body, #app {
  height: 100%;
  width: 100%;
  overflow: hidden;
  margin: 0;
  padding: 0;
}
</style>
