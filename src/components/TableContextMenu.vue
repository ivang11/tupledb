<template>
  <ContextMenuSurface :show="show" :x="x" :y="y" @dismiss="emit('dismiss')">
    <div class="px-2 py-1 mb-1 border-b">
      <span class="text-[10px] font-semibold font-mono tracking-normal text-muted-foreground truncate block">
        {{ selectedCount && selectedCount > 1 ? `${selectedCount} tables selected` : tableName }}
      </span>
    </div>

    <button class="w-full px-3 py-2 text-xs rounded-md hover:bg-muted text-left" @click="emit('open')">Open table</button>
    <button class="w-full px-3 py-2 text-xs rounded-md hover:bg-muted text-left" @click="emit('copy-name')">Copy table name</button>
    <div class="border-t my-1" />
    <!-- Single table options -->
    <template v-if="!selectedCount || selectedCount <= 1">
      <button :disabled="canWrite === false"
        class="w-full flex items-center gap-2 px-3 py-2 text-xs font-medium rounded-md hover:bg-muted transition-colors text-left"
        @click="emit('truncate')"
      >
        <Trash2Icon class="size-3.5 text-muted-foreground" /> Truncate Table
      </button>
      <button :disabled="canWrite === false"
        class="w-full flex items-center gap-2 px-3 py-2 text-xs font-medium rounded-md hover:bg-muted transition-colors text-left"
        @click="emit('drop')"
      >
        <XIcon class="size-3.5 text-muted-foreground" /> Drop Table
      </button>
    </template>

<!-- Multiple tables options -->
    <template v-else>
      <button :disabled="canWrite === false"
        class="w-full flex items-center gap-2 px-3 py-2 text-xs font-medium rounded-md hover:bg-muted transition-colors text-left"
        @click="emit('truncate-selected')"
      >
        <Trash2Icon class="size-3.5 text-muted-foreground" /> Truncate {{ selectedCount }} Tables
      </button>
      <button :disabled="canWrite === false"
        class="w-full flex items-center gap-2 px-3 py-2 text-xs font-medium rounded-md hover:bg-muted transition-colors text-left"
        @click="emit('drop-selected')"
      >
        <XIcon class="size-3.5 text-muted-foreground" /> Drop {{ selectedCount }} Tables
      </button>
    </template>
  </ContextMenuSurface>
</template>

<script setup lang="ts">
import ContextMenuSurface from "./ContextMenuSurface.vue"
import { Trash2Icon, XIcon } from 'lucide-vue-next'

defineProps<{
  show: boolean
  x: number
  y: number
  tableName: string
  selectedCount?: number
  canWrite?: boolean
}>()

const emit = defineEmits<{
  'dismiss': []
  'open': []
  'copy-name': []
  'truncate': []
  'drop': []
  'truncate-selected': []
  'drop-selected': []
}>()
</script>
