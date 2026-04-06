<script setup lang="ts">
defineProps<{
  paneIdx: number
  draggingPaneIdx: number | null
}>()

const emit = defineEmits<{
  (e: 'resize-start', event: MouseEvent, idx: number): void
}>()
</script>

<template>
  <div
    class="w-px bg-border hover:w-1 hover:bg-primary/40 cursor-col-resize shrink-0 transition-all z-10"
    :class="{ 'w-1 bg-primary/50': draggingPaneIdx === paneIdx - 1 }"
    @mousedown.prevent="emit('resize-start', $event, paneIdx - 1)"
  />
</template>
