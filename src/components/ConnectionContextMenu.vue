<template>
  <ContextMenuSurface v-if="connection" :show="show" :x="x" :y="y" @dismiss="emit('dismiss')">
    <!-- Rail and sidebar: only session actions. Editing and deleting live in the connections list. -->
    <template v-if="variant === 'rail'">
      <button
        class="w-full flex items-center gap-2 px-3 py-2 text-xs font-medium rounded-md hover:bg-muted transition-colors text-left"
        @click="emit('disconnect', connection.id)"
      >
        <XIcon class="size-3.5 text-muted-foreground" /> Close connection
      </button>
      <button
        class="w-full flex items-center gap-2 px-3 py-2 text-xs font-medium rounded-md hover:bg-muted transition-colors text-left"
        :disabled="(openConnectionCount ?? 0) <= 1"
        @click="emit('disconnect-others', connection.id)"
      >
        <CopyXIcon class="size-3.5 text-muted-foreground" /> Close other connections
      </button>
    </template>

    <template v-else>
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
          v-if="canCreateDatabase !== false"
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
    </template>
  </ContextMenuSurface>
</template>

<script setup lang="ts">
import ContextMenuSurface from "./ContextMenuSurface.vue"
import { PencilIcon, CopyIcon, CopyXIcon, XIcon, Trash2Icon, PlusIcon } from 'lucide-vue-next'
import type { Connection } from '@/types/connection'

defineProps<{
  show: boolean
  x: number
  y: number
  connection: Connection | null
  isConnected: boolean
  canCreateDatabase?: boolean
  /** 'rail' shows only session actions; 'list' shows the full management menu. */
  variant?: 'rail' | 'list'
  openConnectionCount?: number
}>()

const emit = defineEmits<{
  'dismiss': []
  'edit': [conn: Connection]
  'duplicate': [conn: Connection]
  'disconnect': [id: string]
  'disconnect-others': [id: string]
  'delete': [id: string]
  'new-database': [id: string]
}>()
</script>
