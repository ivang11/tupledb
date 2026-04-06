<script setup lang="ts">
import { TableIcon } from 'lucide-vue-next'

defineProps<{
  hasConnections: boolean
}>()
</script>

<template>
  <div class="flex-1 flex flex-col items-center justify-center p-12 text-center bg-muted/5">
    <div class="size-24 rounded-full bg-muted/20 flex items-center justify-center mb-8 border border-dashed border-muted-foreground/20">
      <TableIcon class="size-10 text-muted-foreground/30" />
    </div>
    <h2 class="text-2xl font-bold tracking-tight mb-3">Table Explorer</h2>
    <p class="text-muted-foreground max-w-sm mb-8 leading-relaxed">
      {{ hasConnections ? 'Expand a database in the sidebar and select a table to browse.' : 'Connect to a database from the sidebar to get started.' }}
    </p>
  </div>
</template>
