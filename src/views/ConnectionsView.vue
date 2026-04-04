<script setup lang="ts">
import { ref, onMounted, computed } from 'vue'
import { useRouter } from 'vue-router'
import { useConnectionStore } from '@/stores/connections'
import type { Connection, Environment } from '@/types/connection'
import { v4 as uuidv4 } from 'uuid'
import { open as openDialog } from '@tauri-apps/plugin-dialog'
import {
  PlusIcon,
  Trash2Icon,
  DatabaseIcon,
  ShieldCheckIcon,
  HardDriveIcon,
  ServerIcon,
  SearchIcon,
  ArrowLeftIcon,
  PencilIcon,
  XIcon,
  FolderOpenIcon,
  CopyIcon
} from 'lucide-vue-next'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Badge } from '@/components/ui/badge'
import { Separator } from '@/components/ui/separator'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from '@/components/ui/dialog'

const store = useConnectionStore()
const router = useRouter()
const searchQuery = ref('')

// Modal state
const showDialog = ref(false)
const showDeleteDialog = ref(false)
const connectionToDelete = ref<string | null>(null)
const isSaving = ref(false)
const isTesting = ref(false)
const testResult = ref<{ ok: boolean; msg: string } | null>(null)

// SSH local state
const sshEnabled = ref(false)
const sshAuthType = ref<'password' | 'key'>('password')
const sshForm = ref({ host: '', port: 22, user: '', password: '', private_key_path: '', passphrase: '' })

const contextMenu = ref({
  show: false,
  x: 0,
  y: 0,
  connection: null as Connection | null
})

function blankConn(): Connection {
  return {
    id: uuidv4(),
    name: '',
    environment: 'LOCAL',
    mysql: { host: '127.0.0.1', port: 3306, user: 'root', password: '', database: '' }
  }
}

const newConn = ref<Connection>(blankConn())

function openNewConnDialog() {
  newConn.value = blankConn()
  sshEnabled.value = false
  sshAuthType.value = 'password'
  sshForm.value = { host: '', port: 22, user: '', password: '', private_key_path: '', passphrase: '' }
  testResult.value = null
  showDialog.value = true
}

function handleDuplicate(conn: Connection) {
  const duplicate = JSON.parse(JSON.stringify(conn))
  duplicate.id = uuidv4()
  duplicate.name = `${conn.name} (Copy)`
  newConn.value = duplicate
  
  sshEnabled.value = !!conn.ssh
  if (conn.ssh) {
    sshForm.value = {
      host: conn.ssh.host,
      port: conn.ssh.port,
      user: conn.ssh.user,
      password: conn.ssh.auth.type === 'password' ? conn.ssh.auth.password : '',
      private_key_path: conn.ssh.auth.type === 'key' ? conn.ssh.auth.private_key_path : '',
      passphrase: conn.ssh.auth.type === 'key' ? (conn.ssh.auth.passphrase || '') : ''
    }
    sshAuthType.value = conn.ssh.auth.type === 'password' ? 'password' : 'key'
  } else {
    sshForm.value = { host: '', port: 22, user: '', password: '', private_key_path: '', passphrase: '' }
    sshAuthType.value = 'password'
  }
  
  testResult.value = null
  showDialog.value = true
  contextMenu.value.show = false
}

function handleEdit(conn: Connection) {
  newConn.value = JSON.parse(JSON.stringify(conn))
  sshEnabled.value = !!conn.ssh
  if (conn.ssh) {
    sshForm.value = {
      host: conn.ssh.host,
      port: conn.ssh.port,
      user: conn.ssh.user,
      password: conn.ssh.auth.type === 'password' ? conn.ssh.auth.password : '',
      private_key_path: conn.ssh.auth.type === 'key' ? conn.ssh.auth.private_key_path : '',
      passphrase: conn.ssh.auth.type === 'key' ? (conn.ssh.auth.passphrase || '') : ''
    }
    sshAuthType.value = conn.ssh.auth.type === 'password' ? 'password' : 'key'
  } else {
    sshForm.value = { host: '', port: 22, user: '', password: '', private_key_path: '', passphrase: '' }
    sshAuthType.value = 'password'
  }
  testResult.value = null
  showDialog.value = true
  contextMenu.value.show = false
}

async function testConnection() {
  isTesting.value = true
  testResult.value = null
  const conn = buildConnWithSsh()
  try {
    const msg = await store.testConnection(conn)
    testResult.value = { ok: true, msg }
  } catch (e: any) {
    testResult.value = { ok: false, msg: String(e) }
  } finally {
    isTesting.value = false
  }
}

function buildConnWithSsh(): Connection {
  const conn = { ...newConn.value }
  if (sshEnabled.value) {
    conn.ssh = {
      host: sshForm.value.host,
      port: sshForm.value.port,
      user: sshForm.value.user,
      auth: sshAuthType.value === 'password'
        ? { type: 'password' as const, password: sshForm.value.password }
        : { type: 'key' as const, private_key_path: sshForm.value.private_key_path, passphrase: sshForm.value.passphrase || undefined }
    }
  } else {
    conn.ssh = undefined
  }
  return conn
}

async function saveConnection() {
  if (!newConn.value.name) return
  isSaving.value = true
  const conn = buildConnWithSsh()
  try {
    await store.addConnection(conn)
    showDialog.value = false
  } catch (e: any) {
    alert(`Error: ${e}`)
  } finally {
    isSaving.value = false
  }
}

function openContextMenu(e: MouseEvent, conn: Connection) {
  e.preventDefault()
  contextMenu.value = {
    show: true,
    x: e.clientX,
    y: e.clientY,
    connection: conn
  }
  
  const close = () => {
    contextMenu.value.show = false
    window.removeEventListener('click', close)
  }
  window.addEventListener('click', close)
}

async function pickSshKey() {
  try {
    const selected = await openDialog({
      multiple: false,
      filters: [{ name: 'SSH Key', extensions: ['*', 'pem', 'pub'] }]
    })
    if (selected && typeof selected === 'string') {
      sshForm.value.private_key_path = selected
    }
  } catch (e) {
    console.error('Error picking SSH key:', e)
  }
}

onMounted(() => {
  store.fetchConnections()
})

async function connect(conn: Connection) {
  try {
    await store.connect(conn)
    router.push('/')
  } catch (error) {
    alert(`Failed to connect: ${error}`)
  }
}

function confirmDeleteDialog(id: string) {
  connectionToDelete.value = id
  showDeleteDialog.value = true
  contextMenu.value.show = false
}

async function handleDelete() {
  if (!connectionToDelete.value) return
  try {
    await store.removeConnection(connectionToDelete.value)
  } catch (error) {
    alert(`Failed to delete: ${error}`)
  } finally {
    showDeleteDialog.value = false
    connectionToDelete.value = null
  }
}

const getEnvColor = (env: Environment) => {
  switch (env) {
    case 'PRODUCTION': return 'bg-red-500/10 text-red-500 border-red-500/20'
    case 'STAGING': return 'bg-orange-500/10 text-orange-500 border-orange-500/20'
    case 'DEV': return 'bg-blue-500/10 text-blue-500 border-blue-500/20'
    default: return 'bg-green-500/10 text-green-500 border-green-500/20'
  }
}

const filteredConnections = computed(() => {
  if (!searchQuery.value) return store.connections
  const query = searchQuery.value.toLowerCase()
  return store.connections.filter(c => 
    c.name.toLowerCase().includes(query) || 
    c.mysql.host.toLowerCase().includes(query)
  )
})
</script>

<template>
  <div class="h-full flex flex-col bg-muted/30">
    <div class="flex-1 flex flex-col items-center p-8 overflow-auto">
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

        <!-- Connection List -->
        <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
          <div 
            v-for="conn in filteredConnections" 
            :key="conn.id"
            class="group relative flex items-center gap-4 p-5 rounded-2xl border bg-card hover:border-primary/50 transition-all cursor-pointer shadow-sm hover:shadow-md select-none"
            @dblclick="connect(conn)"
            @contextmenu="openContextMenu($event, conn)"
          >
            <div class="size-14 rounded-xl bg-primary/10 flex items-center justify-center text-primary shrink-0 transition-transform group-hover:scale-105">
              <DatabaseIcon class="size-7" />
            </div>
            <div class="flex-1 min-w-0">
              <div class="flex items-center gap-2 mb-1">
                <span class="font-bold text-lg truncate">{{ conn.name }}</span>
                <Badge variant="outline" :class="[getEnvColor(conn.environment), 'text-[10px] uppercase py-0 px-2 h-4.5 font-bold tracking-wider']">
                  {{ conn.environment }}
                </Badge>
              </div>
              <div class="text-sm text-muted-foreground truncate flex items-center gap-2">
                <ServerIcon class="size-3.5" />
                {{ conn.mysql.user }}@{{ conn.mysql.host }}
                <template v-if="conn.ssh">
                  <div class="h-3 w-px bg-border mx-1"></div>
                  <ShieldCheckIcon class="size-3.5 text-primary" />
                  <span class="text-xs font-medium">SSH</span>
                </template>
              </div>
            </div>
          </div>

          <!-- Empty State -->
          <div v-if="filteredConnections.length === 0" class="col-span-full py-20 flex flex-col items-center justify-center text-center text-muted-foreground">
            <DatabaseIcon class="size-12 opacity-20 mb-4" />
            <p class="text-lg font-medium">No connections found</p>
            <p class="text-sm">Try a different search or create a new connection</p>
          </div>
        </div>
      </div>
    </div>

    <!-- Connection Dialog (New/Edit) -->
    <Dialog :open="showDialog" @update:open="(val: boolean) => !val && (showDialog = false)">
      <DialogContent class="sm:max-w-lg overflow-y-auto max-h-[90vh]">
        <DialogHeader>
          <DialogTitle>{{ store.connections.some(c => c.id === newConn.id) ? 'Edit Connection' : 'New Connection' }}</DialogTitle>
          <DialogDescription>
            {{ store.connections.some(c => c.id === newConn.id) ? 'Update your connection settings' : 'Configure your MySQL connection settings' }}
          </DialogDescription>
        </DialogHeader>

        <div class="space-y-5 py-2">
          <div class="grid grid-cols-2 gap-4">
            <div class="space-y-2">
              <Label>Connection Name</Label>
              <Input v-model="newConn.name" placeholder="Local Development" />
            </div>
            <div class="space-y-2">
              <Label>Environment</Label>
              <select v-model="newConn.environment" class="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring">
                <option value="LOCAL">Local</option>
                <option value="DEV">Development</option>
                <option value="STAGING">Staging</option>
                <option value="PRODUCTION">Production</option>
              </select>
            </div>
          </div>

          <Separator />

          <div class="space-y-4">
            <div class="flex items-center gap-2 text-xs font-bold text-muted-foreground uppercase tracking-wider">
              <HardDriveIcon class="size-3.5" /> MySQL Settings
            </div>
            <div class="grid grid-cols-12 gap-3">
              <div class="col-span-8 space-y-2">
                <Label>Host</Label>
                <Input v-model="newConn.mysql.host" placeholder="127.0.0.1" />
              </div>
              <div class="col-span-4 space-y-2">
                <Label>Port</Label>
                <Input v-model.number="newConn.mysql.port" type="number" />
              </div>
            </div>
            <div class="grid grid-cols-2 gap-3">
              <div class="space-y-2">
                <Label>User</Label>
                <Input v-model="newConn.mysql.user" placeholder="root" />
              </div>
              <div class="space-y-2">
                <Label>Password</Label>
                <Input v-model="newConn.mysql.password" type="password" placeholder="••••••••" />
              </div>
            </div>
            <div class="space-y-2">
              <Label>Database <span class="text-muted-foreground font-normal">(optional)</span></Label>
              <Input v-model="newConn.mysql.database" placeholder="Leave blank to pick after connecting" />
            </div>
          </div>

          <Separator />

          <!-- SSH Tunnel -->
          <div class="space-y-4">
            <label class="flex items-center gap-3 cursor-pointer select-none">
              <div
                @click="sshEnabled = !sshEnabled"
                :class="[
                  'relative w-9 h-5 rounded-full transition-colors shrink-0',
                  sshEnabled ? 'bg-primary' : 'bg-muted'
                ]"
              >
                <div :class="[
                  'absolute top-0.5 left-0.5 size-4 rounded-full bg-white shadow transition-transform',
                  sshEnabled ? 'translate-x-4' : 'translate-x-0'
                ]" />
              </div>
              <div class="flex items-center gap-2 text-xs font-bold text-muted-foreground uppercase tracking-wider">
                <ShieldCheckIcon class="size-3.5" /> SSH Tunnel
              </div>
            </label>

            <div v-if="sshEnabled" class="space-y-3 pl-1">
              <div class="grid grid-cols-12 gap-3">
                <div class="col-span-8 space-y-2">
                  <Label>SSH Host</Label>
                  <Input v-model="sshForm.host" placeholder="bastion.example.com" />
                </div>
                <div class="col-span-4 space-y-2">
                  <Label>Port</Label>
                  <Input v-model.number="sshForm.port" type="number" />
                </div>
              </div>
              <div class="space-y-2">
                <Label>SSH User</Label>
                <Input v-model="sshForm.user" placeholder="ubuntu" />
              </div>

              <!-- Auth type selector -->
              <div class="flex gap-2 pt-1">
                <button
                  @click="sshAuthType = 'password'"
                  :class="[
                    'flex-1 h-8 rounded-md text-xs font-bold border transition-all',
                    sshAuthType === 'password' ? 'bg-primary text-primary-foreground border-primary' : 'bg-transparent text-muted-foreground border-input hover:border-primary/50'
                  ]"
                >Password</button>
                <button
                  @click="sshAuthType = 'key'"
                  :class="[
                    'flex-1 h-8 rounded-md text-xs font-bold border transition-all',
                    sshAuthType === 'key' ? 'bg-primary text-primary-foreground border-primary' : 'bg-transparent text-muted-foreground border-input hover:border-primary/50'
                  ]"
                >SSH Key</button>
              </div>

              <div v-if="sshAuthType === 'password'" class="space-y-2">
                <Label>SSH Password</Label>
                <Input v-model="sshForm.password" type="password" placeholder="••••••••" />
              </div>

              <div v-if="sshAuthType === 'key'" class="space-y-3">
                <div class="space-y-2">
                  <Label>Private Key</Label>
                  <button
                    type="button"
                    @click="pickSshKey"
                    class="w-full flex items-center gap-2.5 h-9 px-3 rounded-md border border-input bg-background text-sm transition-colors hover:bg-accent hover:border-ring/50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/40"
                  >
                    <FolderOpenIcon class="size-4 shrink-0 text-muted-foreground" />
                    <span :class="sshForm.private_key_path ? 'text-foreground' : 'text-muted-foreground'" class="flex-1 text-left truncate">
                      {{ sshForm.private_key_path || 'Select SSH key file...' }}
                    </span>
                    <span v-if="sshForm.private_key_path" @click.stop="sshForm.private_key_path = ''" class="text-muted-foreground hover:text-foreground">
                      <XIcon class="size-3.5" />
                    </span>
                  </button>
                </div>
                <div class="space-y-2">
                  <Label>Passphrase <span class="text-muted-foreground font-normal">(optional)</span></Label>
                  <Input v-model="sshForm.passphrase" type="password" placeholder="••••••••" />
                </div>
              </div>
            </div>
          </div>
        </div>

        <!-- Test result -->
        <div v-if="testResult" :class="[
          'text-xs px-3 py-2 rounded-md font-medium',
          testResult.ok ? 'bg-green-500/10 text-green-500' : 'bg-destructive/10 text-destructive'
        ]">
          {{ testResult.msg }}
        </div>

        <div class="flex items-center justify-between pt-4 border-t">
          <Button variant="ghost" @click="showDialog = false">Cancel</Button>
          <div class="flex gap-2">
            <Button variant="outline" :disabled="isTesting || isSaving" @click="testConnection">
              {{ isTesting ? 'Testing...' : 'Test' }}
            </Button>
            <Button :disabled="isSaving || !newConn.name" @click="saveConnection">
              {{ store.connections.some(c => c.id === newConn.id) ? 'Update' : 'Save Connection' }}
            </Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>

    <!-- Delete Confirmation Dialog -->
    <Dialog :open="showDeleteDialog" @update:open="(val: boolean) => !val && (showDeleteDialog = false)">
      <DialogContent class="sm:max-w-[400px]">
        <DialogHeader>
          <DialogTitle>Delete Connection</DialogTitle>
          <DialogDescription>
            Are you sure you want to delete this connection? This action cannot be undone.
          </DialogDescription>
        </DialogHeader>
        <DialogFooter class="gap-2 sm:gap-0">
          <Button variant="ghost" @click="showDeleteDialog = false">Cancel</Button>
          <Button variant="destructive" @click="handleDelete">Delete</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>

    <!-- Context Menu -->
    <div
      v-if="contextMenu.show"
      class="fixed z-[100] min-w-[160px] bg-background/95 backdrop-blur-md border rounded-lg shadow-xl p-1 animate-in fade-in zoom-in-95 duration-100"
      :style="{ left: contextMenu.x + 'px', top: contextMenu.y + 'px' }"
    >
      <button
        class="w-full flex items-center gap-2 px-3 py-2 text-xs font-medium rounded-md hover:bg-muted transition-colors text-left"
        @click="handleEdit(contextMenu.connection!)"
      >
        <PencilIcon class="size-3.5 text-muted-foreground" /> Edit Connection
      </button>
      <button
        class="w-full flex items-center gap-2 px-3 py-2 text-xs font-medium rounded-md hover:bg-muted transition-colors text-left"
        @click="handleDuplicate(contextMenu.connection!)"
      >
        <CopyIcon class="size-3.5 text-muted-foreground" /> Duplicate Connection
      </button>
      <div class="h-px bg-border my-1"></div>
      <button
        class="w-full flex items-center gap-2 px-3 py-2 text-xs font-medium rounded-md text-destructive hover:bg-destructive/10 transition-colors text-left"
        @click="confirmDeleteDialog(contextMenu.connection!.id)"
      >
        <Trash2Icon class="size-3.5" /> Delete Connection
      </button>
    </div>
  </div>
</template>

<style scoped>
.custom-scrollbar::-webkit-scrollbar {
  width: 4px;
}
.custom-scrollbar::-webkit-scrollbar-track {
  background: transparent;
}
.custom-scrollbar::-webkit-scrollbar-thumb {
  background: hsl(var(--muted-foreground) / 0.2);
  border-radius: 10px;
}
.custom-scrollbar::-webkit-scrollbar-thumb:hover {
  background: hsl(var(--muted-foreground) / 0.4);
}
</style>
