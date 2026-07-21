<template>
  <div
    v-if="show && connection"
    class="fixed z-100 min-w-40 bg-background/95 backdrop-blur-md border rounded-lg shadow-xl p-1 animate-in fade-in zoom-in-95 duration-100"
    :style="{ left: x + 'px', top: y + 'px' }"
  >
    <button
      class="w-full flex items-center gap-2 px-3 py-2 text-xs font-medium rounded-md hover:bg-muted transition-colors text-left"
      @click="emit('edit', connection)"
    >
      <PencilIcon class="size-3.5 text-muted-foreground" /> Edit Connection
    </button>
    <button
      class="w-full flex items-center gap-2 px-3 py-2 text-xs font-medium rounded-md hover:bg-muted transition-colors text-left"
      @click="emit('duplicate', connection)"
    >
      <CopyIcon class="size-3.5 text-muted-foreground" /> Duplicate Connection
    </button>
    <template v-if="isConnected">
      <button
        class="w-full flex items-center gap-2 px-3 py-2 text-xs font-medium rounded-md hover:bg-muted transition-colors text-left"
        @click="emit('new-database', connection.id)"
      >
        <PlusIcon class="size-3.5 text-muted-foreground" /> New Database
      </button>
      <button
        class="w-full flex items-center gap-2 px-3 py-2 text-xs font-medium rounded-md hover:bg-destructive/10 text-destructive transition-colors text-left"
        @click="emit('disconnect', connection.id)"
      >
        <XIcon class="size-3.5" /> Disconnect
      </button>
    </template>

<div class="h-px bg-border my-1"></div>
    <button
      class="w-full flex items-center gap-2 px-3 py-2 text-xs font-medium rounded-md text-destructive hover:bg-destructive/10 transition-colors text-left"
      @click="emit('delete', connection.id)"
    >
      <Trash2Icon class="size-3.5" /> Delete Connection
    </button>
  </div>
</template>

<script setup lang="ts">
import { PencilIcon, CopyIcon, XIcon, Trash2Icon, PlusIcon } from 'lucide-vue-next'
import type { Connection } from '@/types/connection'

defineProps<{
  show: boolean
  x: number
  y: number
  connection: Connection | null
  isConnected: boolean
}>()

const emit = defineEmits<{
  'edit': [conn: Connection]
  'duplicate': [conn: Connection]
  'disconnect': [id: string]
  'delete': [id: string]
  'new-database': [id: string]
}>()
</script>

