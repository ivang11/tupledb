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
  <!-- Wider grab zone (8px) with a thin visual line in the center -->
  <div
    class="w-2 shrink-0 cursor-col-resize z-10 group relative flex items-stretch justify-center"
    :class="{ 'select-none': draggingPaneIdx === paneIdx - 1 }"
    @mousedown.prevent="emit('resize-start', $event, paneIdx - 1)"
  >
    <div
      class="w-px transition-colors"
      :class="
        draggingPaneIdx === paneIdx - 1
          ? 'bg-primary/60'
          : 'bg-border group-hover:bg-primary/40'
      "
    />
  </div>
</template>
