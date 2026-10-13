<template>
  <ContextMenuSurface :show="show" :x="x" :y="y" @dismiss="emit('dismiss')">
    <div class="px-2 py-1 mb-1 border-b">
      <span class="text-[10px] font-semibold font-mono tracking-normal text-muted-foreground truncate block">
        {{ selectedCount > 1 ? `${selectedCount} rows selected` : '1 row selected' }}
      </span>
    </div>

    <button
      v-if="hasPrimaryKey"
      class="w-full flex items-center gap-2 px-3 py-2 text-xs font-medium rounded-md hover:bg-muted transition-colors text-left"
      @click="emit('duplicate')"
    >
      <CopyIcon class="size-3.5 text-muted-foreground" />
      {{ selectedCount > 1 ? `Duplicate ${selectedCount} rows` : 'Duplicate Row' }}
    </button>

    <button class="w-full flex items-center gap-2 px-3 py-2 text-xs font-medium rounded-md hover:bg-muted transition-colors text-left"
      @click="emit('copy-json')"><CopyIcon class="size-3.5 text-muted-foreground" />Copy row as JSON</button>
    <button class="w-full flex items-center gap-2 px-3 py-2 text-xs font-medium rounded-md hover:bg-muted transition-colors text-left"
      @click="emit('copy-tsv')"><CopyIcon class="size-3.5 text-muted-foreground" />Copy row as TSV</button>

    <button
      v-if="hasPrimaryKey"
      class="w-full flex items-center gap-2 px-3 py-2 text-xs font-medium rounded-md hover:bg-destructive/10 hover:text-destructive transition-colors text-left"
      @click="emit('delete')"
    >
      <Trash2Icon class="size-3.5 text-destructive/70" />
      Delete {{ selectedCount > 1 ? `${selectedCount} rows` : 'Row' }}
    </button>
  </ContextMenuSurface>
</template>

<script setup lang="ts">
import ContextMenuSurface from "./ContextMenuSurface.vue"
import { Trash2Icon, CopyIcon } from 'lucide-vue-next'

defineProps<{
  show: boolean
  x: number
  y: number
  hasPrimaryKey: boolean
  selectedCount: number
}>()

const emit = defineEmits<{
  'dismiss': []
  'copy-json': []
  'copy-tsv': []
  'delete': []
  'duplicate': []
}>()
</script>
