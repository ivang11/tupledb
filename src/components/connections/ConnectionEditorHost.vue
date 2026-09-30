<template>
  <ConnectionDialog v-if="fallback" :open="open" :connection="connection" :is-saving="isSaving"
    :show-connect-button="showConnectButton" @update:open="emit('update:open', $event)" @save="saveConnection" />
</template>

<script setup lang="ts">
import { onUnmounted, ref, toRaw, watch } from 'vue'
import ConnectionDialog from '@/components/dialogs/ConnectionDialog.vue'
import { useConnectionStore } from '@/stores/connections'
import { openConnectionEditorWindow } from '@/lib/connectionEditorWindow'
import type { Connection } from '@/types/connection'

const props = withDefaults(defineProps<{
  open: boolean
  connection: Connection
  isSaving?: boolean
  showConnectButton?: boolean
  saveConnection: (connection: Connection, andConnect: boolean) => Promise<boolean>
}>(), { isSaving: false, showConnectButton: false })
const emit = defineEmits<{ 'update:open': [open: boolean] }>()
const store = useConnectionStore()
const fallback = ref(!('__TAURI_INTERNALS__' in window))
let session: ReturnType<typeof openConnectionEditorWindow> | undefined

watch(() => [props.open, props.connection.id] as const, ([open]) => {
  session?.dispose()
  session = undefined
  if (!open || fallback.value) return
  session = openConnectionEditorWindow({
    draft: {
      connection: JSON.parse(JSON.stringify(toRaw(props.connection))),
      editing: store.connections.some(connection => connection.id === props.connection.id),
      drivers: JSON.parse(JSON.stringify(toRaw(store.availableDrivers))),
      showConnectButton: props.showConnectButton,
    },
    save: props.saveConnection,
    saveError: () => store.storageError,
    closed: () => emit('update:open', false),
    failed: () => { fallback.value = true },
  })
}, { immediate: true })

onUnmounted(() => session?.dispose())
</script>
