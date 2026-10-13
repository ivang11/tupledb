<template>
  <div class="flex-1 flex flex-col items-center justify-center p-12 text-center select-none">
    <Table2Icon class="size-7 text-(--fg-5) mb-3" />
    <h2 class="text-[13px] font-semibold tracking-tight text-(--fg-2) mb-1">No table open</h2>
    <p v-if="hasConnections && hasSelectedDatabase" class="text-[12px] text-(--fg-4) max-w-xs leading-relaxed">
      Select a table in the sidebar to browse it.
    </p>
    <p v-else-if="hasConnections" class="text-[12px] text-(--fg-4) max-w-xs leading-relaxed">
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

