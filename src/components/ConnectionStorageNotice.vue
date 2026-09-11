<template>
  <div v-if="development" class="px-4 py-2 text-xs border-b border-border text-muted-foreground">
    Development uses separate saved connections. Import a connections export to copy them from the installed app.
  </div>
  <div v-if="store.storageError" role="alert" class="px-4 py-3 text-sm border-b border-destructive/40 bg-destructive/10">
    <p class="font-medium">Saved connections need attention</p>
    <p class="break-words select-text">{{ store.storageError }}</p>
    <button type="button" class="mt-2 underline underline-offset-2 focus-visible:outline focus-visible:outline-2" @click="store.fetchConnections()">
      Reload connections
    </button>
  </div>
</template>

<script setup lang="ts">
import { useConnectionStore } from '@/stores/connections'

defineProps<{ development: boolean }>()
const store = useConnectionStore()
</script>
