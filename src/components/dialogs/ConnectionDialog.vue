<template>
  <Dialog :open="open" @update:open="emit('update:open', $event)">
    <DialogContent class="flex max-h-[90vh] flex-col gap-0 overflow-hidden p-0 sm:max-w-[640px]"
      :style="{ height: `${formHeight + 66}px` }">
      <DialogHeader class="shrink-0 px-5 py-4">
        <DialogTitle>{{ isEdit ? 'Edit Connection' : 'New Connection' }}</DialogTitle>
        <DialogDescription class="sr-only">Configure the connection, SSH tunnel and advanced settings.</DialogDescription>
      </DialogHeader>
      <ConnectionForm class="min-h-0 flex-1" :open="open" :connection="connection" :is-saving="isSaving"
        :show-connect-button="showConnectButton" @update:open="emit('update:open', $event)" @save="(conn, connect) => emit('save', conn, connect)"
        @height-change="formHeight = $event" />
    </DialogContent>
  </Dialog>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { useConnectionStore } from '@/stores/connections'
import type { Connection } from '@/types/connection'
import ConnectionForm from '@/components/connections/ConnectionForm.vue'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription } from '@/components/ui/dialog'

const props = withDefaults(defineProps<{ open: boolean; connection: Connection; isSaving?: boolean; showConnectButton?: boolean }>(), {
  isSaving: false, showConnectButton: false,
})
const emit = defineEmits<{
  'update:open': [open: boolean];
  save: [connection: Connection, andConnect: boolean];
}>()
const store = useConnectionStore()
const formHeight = ref(494)
const isEdit = computed(() => store.connections.some(conn => conn.id === props.connection.id))
</script>
