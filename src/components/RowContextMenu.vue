<script setup lang="ts">
import { Trash2Icon, CopyIcon } from 'lucide-vue-next'

defineProps<{
  show: boolean
  x: number
  y: number
  hasPrimaryKey: boolean
  selectedCount: number
}>()

const emit = defineEmits<{
  'delete': []
  'duplicate': []
}>()
</script>

<template>
  <div
    v-if="show"
    class="fixed z-[100] min-w-[170px] bg-background/95 backdrop-blur-md border rounded-lg shadow-xl p-1 animate-in fade-in zoom-in-95 duration-100"
    :style="{ left: x + 'px', top: y + 'px' }"
  >
    <div class="px-2 py-1 mb-1 border-b">
      <span class="text-[10px] font-semibold font-mono tracking-normal text-muted-foreground truncate block">
        {{ selectedCount > 1 ? `${selectedCount} rows selected` : '1 row selected' }}
      </span>
    </div>

    <button
      v-if="selectedCount === 1"
      class="w-full flex items-center gap-2 px-3 py-2 text-xs font-medium rounded-md hover:bg-muted transition-colors text-left"
      @click="emit('duplicate')"
    >
      <CopyIcon class="size-3.5 text-muted-foreground" /> Duplicate Row
    </button>

    <button
      v-if="hasPrimaryKey"
      class="w-full flex items-center gap-2 px-3 py-2 text-xs font-medium rounded-md hover:bg-destructive/10 hover:text-destructive transition-colors text-left"
      @click="emit('delete')"
    >
      <Trash2Icon class="size-3.5 text-destructive/70" />
      Delete {{ selectedCount > 1 ? `${selectedCount} rows` : 'Row' }}
    </button>
  </div>
</template>
