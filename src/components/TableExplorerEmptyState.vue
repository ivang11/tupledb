<template>
  <div class="flex-1 flex flex-col items-center justify-center p-12 text-center select-none">
    <Table2Icon class="size-10 text-muted-foreground/20 mb-5" />
    <h2 class="text-base font-semibold tracking-tight text-foreground mb-1.5">Table Explorer</h2>
    <p v-if="hasConnections && hasSelectedDatabase" class="text-sm text-muted-foreground max-w-xs leading-relaxed">
      Select a table in the sidebar to browse it.
    </p>
    <p v-else-if="hasConnections" class="text-sm text-muted-foreground max-w-xs leading-relaxed">
      Select a database in the sidebar to browse its tables.
    </p>
  </div>
</template>

<script setup lang="ts">
import { Table2Icon } from 'lucide-vue-next'

defineProps<{
  hasConnections: boolean
  hasSelectedDatabase?: boolean
}>()
</script>

