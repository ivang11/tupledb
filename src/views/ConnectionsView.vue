<script setup lang="ts">
import { ref, onMounted, computed } from 'vue'
import { useRouter } from 'vue-router'
import { useConnectionStore } from '@/stores/connections'
import { useToast } from '@/composables/useToast'
import type { Connection } from '@/types/connection'
import { v4 as uuidv4 } from 'uuid'
import {
  PlusIcon,
  SearchIcon,
  ArrowLeftIcon,
} from 'lucide-vue-next'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { ScrollArea } from '@/components/ui/scroll-area'
import ConnectionGrid from '@/components/ConnectionGrid.vue'
import ConnectionContextMenu from '@/components/ConnectionContextMenu.vue'
import ConnectionDialog from '@/components/dialogs/ConnectionDialog.vue'
import DeleteConfirmDialog from '@/components/dialogs/DeleteConfirmDialog.vue'

const store = useConnectionStore()
const router = useRouter()
const { error: toastError } = useToast()
const searchQuery = ref('')

const showDialog = ref(false)
const showDeleteDialog = ref(false)
const connectionToDelete = ref<string | null>(null)
const isSaving = ref(false)

const contextMenu = ref({
  show: false,
  x: 0,
  y: 0,
  connection: null as Connection | null,
})

function blankConn(): Connection {
  return {
    id: uuidv4(),
    name: '',
    environment: 'LOCAL',
    mysql: { host: '127.0.0.1', port: 3306, user: 'root', password: '', database: '' },
  }
}

const editConn = ref<Connection>(blankConn())

function openNewConnDialog() {
  editConn.value = blankConn()
  showDialog.value = true
}

function openContextMenu(e: MouseEvent, conn: Connection) {
  e.preventDefault()
  contextMenu.value = { show: true, x: e.clientX, y: e.clientY, connection: conn }
  const close = () => { contextMenu.value.show = false; window.removeEventListener('click', close) }
  window.addEventListener('click', close)
}

function handleEdit(conn: Connection) {
  editConn.value = JSON.parse(JSON.stringify(conn))
  showDialog.value = true
  contextMenu.value.show = false
}

function handleDuplicate(conn: Connection) {
  const dup = JSON.parse(JSON.stringify(conn))
  dup.id = uuidv4()
  dup.name = `${conn.name} (Copy)`
  editConn.value = dup
  showDialog.value = true
  contextMenu.value.show = false
}

async function handleSave(conn: Connection) {
  if (!conn.name) return
  isSaving.value = true
  try {
    await store.addConnection(conn)
    showDialog.value = false
  } catch (e: any) {
    toastError('Error', String(e))
  } finally {
    isSaving.value = false
  }
}

function confirmDelete(id: string) {
  connectionToDelete.value = id
  showDeleteDialog.value = true
  contextMenu.value.show = false
}

async function handleDelete() {
  if (!connectionToDelete.value) return
  try {
    await store.removeConnection(connectionToDelete.value)
  } catch (e) {
    toastError('Failed to delete', String(e))
  } finally {
    showDeleteDialog.value = false
    connectionToDelete.value = null
  }
}

async function connect(conn: Connection) {
  try {
    await store.connect(conn)
    router.push('/')
  } catch (e) {
    toastError('Failed to connect', String(e))
  }
}

onMounted(() => store.fetchConnections())

const filteredConnections = computed(() => {
  if (!searchQuery.value) return store.connections
  const q = searchQuery.value.toLowerCase()
  return store.connections.filter(c => c.name.toLowerCase().includes(q) || c.mysql.host.toLowerCase().includes(q))
})
</script>

<template>
  <div class="h-full flex flex-col bg-muted/30">
    <ScrollArea class="flex-1 flex flex-col items-center p-8">
      <div class="max-w-4xl w-full flex flex-col gap-6">

        <!-- Header -->
        <div class="flex items-center justify-between">
          <div>
            <button class="flex items-center gap-1.5 text-xs text-muted-foreground hover:text-foreground transition-colors mb-1" @click="router.push('/')">
              <ArrowLeftIcon class="size-3" /> Back to Explorer
            </button>
            <h1 class="text-3xl font-bold tracking-tight">Connections</h1>
            <p class="text-sm text-muted-foreground">Manage and organize your database access</p>
          </div>
          <Button class="gap-2" @click="openNewConnDialog">
            <PlusIcon class="size-4" /> New Connection
          </Button>
        </div>

        <!-- Search -->
        <div class="relative">
          <SearchIcon class="absolute left-3 top-1/2 -translate-y-1/2 size-4 text-muted-foreground" />
          <Input v-model="searchQuery" placeholder="Search connections by name or host..." class="pl-10 h-11 bg-background border-none shadow-sm text-base" />
        </div>

        <!-- Grid -->
        <ConnectionGrid
          :connections="filteredConnections"
          @connect="connect"
          @context-menu="openContextMenu"
        />
      </div>
    </ScrollArea>

    <!-- Connection Dialog -->
    <ConnectionDialog
      :open="showDialog"
      :connection="editConn"
      :is-saving="isSaving"
      @update:open="(val) => { if (!val) showDialog = false }"
      @save="(conn) => handleSave(conn)"
    />

    <!-- Delete Dialog -->
    <DeleteConfirmDialog
      :open="showDeleteDialog"
      title="Delete Connection"
      description="Are you sure you want to delete this connection? This action cannot be undone."
      @update:open="(val) => { if (!val) showDeleteDialog = false }"
      @confirm="handleDelete"
    />

    <!-- Context Menu -->
    <ConnectionContextMenu
      :show="contextMenu.show"
      :x="contextMenu.x"
      :y="contextMenu.y"
      :connection="contextMenu.connection"
      :is-connected="false"
      @edit="handleEdit"
      @duplicate="handleDuplicate"
      @delete="confirmDelete"
    />
  </div>
</template>
