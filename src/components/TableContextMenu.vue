<script setup lang="ts">
import { Trash2Icon, XIcon } from 'lucide-vue-next'

defineProps<{
  show: boolean
  x: number
  y: number
  tableName: string
  selectedCount?: number
}>()

const emit = defineEmits<{
  'truncate': []
  'drop': []
  'truncate-selected': []
  'drop-selected': []
}>()
</script>

<template>
  <div
    v-if="show"
    class="fixed z-[100] min-w-[160px] bg-background/95 backdrop-blur-md border rounded-lg shadow-xl p-1 animate-in fade-in zoom-in-95 duration-100"
    :style="{ left: x + 'px', top: y + 'px' }"
  >
    <div class="px-2 py-1 mb-1 border-b">
      <span class="text-[10px] font-semibold font-mono tracking-normal text-muted-foreground truncate block">
        {{ selectedCount && selectedCount > 1 ? `${selectedCount} tables selected` : tableName }}
      </span>
    </div>

    <!-- Single table options -->
    <template v-if="!selectedCount || selectedCount <= 1">
      <button
        class="w-full flex items-center gap-2 px-3 py-2 text-xs font-medium rounded-md hover:bg-muted transition-colors text-left"
        @click="emit('truncate')"
      >
        <Trash2Icon class="size-3.5 text-muted-foreground" /> Truncate Table
      </button>
      <button
        class="w-full flex items-center gap-2 px-3 py-2 text-xs font-medium rounded-md hover:bg-muted transition-colors text-left"
        @click="emit('drop')"
      >
        <XIcon class="size-3.5 text-muted-foreground" /> Drop Table
      </button>
    </template>

    <!-- Multiple tables options -->
    <template v-else>
      <button
        class="w-full flex items-center gap-2 px-3 py-2 text-xs font-medium rounded-md hover:bg-muted transition-colors text-left"
        @click="emit('truncate-selected')"
      >
        <Trash2Icon class="size-3.5 text-muted-foreground" /> Truncate {{ selectedCount }} Tables
      </button>
      <button
        class="w-full flex items-center gap-2 px-3 py-2 text-xs font-medium rounded-md hover:bg-muted transition-colors text-left"
        @click="emit('drop-selected')"
      >
        <XIcon class="size-3.5 text-muted-foreground" /> Drop {{ selectedCount }} Tables
      </button>
    </template>
  </div>
</template>
