<template>
  <Dialog :open="open" @update:open="(v: boolean) => !isSaving && emit('update:open', v)">
    <DialogContent class="sm:max-w-110">
      <DialogHeader>
        <DialogTitle class="flex items-center gap-2">
          <BookmarkIcon class="size-4 text-primary" />
          {{ editing ? 'Editar query guardada' : 'Guardar query' }}
        </DialogTitle>
      </DialogHeader>

      <div class="py-2 flex flex-col gap-4">
        <!-- SQL preview -->
        <div class="bg-muted/30 border rounded-md px-3 py-2 text-xs font-mono text-foreground/60 max-h-20 overflow-hidden relative">
          <span class="line-clamp-3 whitespace-pre-wrap break-all">{{ sql }}</span>
          <div class="absolute inset-x-0 bottom-0 h-6 bg-linear-to-t from-muted/30 to-transparent pointer-events-none" />
        </div>

        <!-- Name -->
        <div class="flex flex-col gap-1.5">
          <Label for="sq-name" class="text-xs font-semibold">Nombre <span class="text-destructive">*</span></Label>
          <Input
            id="sq-name"
            v-model="name"
            placeholder="Ej: Usuarios activos último mes"
            @keydown.enter="save"
            :aria-invalid="!!error && !name.trim()"
          />
        </div>

        <!-- Description -->
        <div class="flex flex-col gap-1.5">
          <Label for="sq-desc" class="text-xs font-semibold">Descripción <span class="text-muted-foreground font-normal">(opcional)</span></Label>
          <textarea
            id="sq-desc"
            v-model="description"
            rows="2"
            placeholder="Para qué sirve esta query..."
            class="w-full rounded-md border border-input px-3 py-2 text-sm placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50 focus-visible:border-ring resize-none"
          />
        </div>

        <!-- Connection -->
        <div class="flex flex-col gap-1.5">
          <Label for="sq-conn" class="text-xs font-semibold">Conexión <span class="text-muted-foreground font-normal">(opcional)</span></Label>
          <select
            id="sq-conn"
            v-model="selectedConnectionId"
            class="h-9 w-full rounded-md border border-input px-3 py-1 text-sm focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50 focus-visible:border-ring"
          >
            <option value="">— Ninguna —</option>
            <option v-for="conn in connStore.connections" :key="conn.id" :value="conn.id">
              {{ conn.name }}
            </option>
          </select>
        </div>

        <!-- Database -->
        <div class="flex flex-col gap-1.5">
          <Label for="sq-db" class="text-xs font-semibold">Base de datos <span class="text-muted-foreground font-normal">(opcional)</span></Label>
          <Input
            id="sq-db"
            v-model="selectedDatabase"
            placeholder="Ej: my_database"
          />
        </div>

        <!-- Error -->
        <p v-if="error" class="text-xs text-destructive">{{ error }}</p>
      </div>

      <DialogFooter>
        <Button variant="ghost" @click="emit('update:open', false)" :disabled="isSaving">Cancelar</Button>
        <Button @click="save" :disabled="isSaving || !name.trim()" class="min-w-25 font-bold">
          {{ isSaving ? 'Guardando…' : (editing ? 'Actualizar' : 'Guardar') }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue'
import { BookmarkIcon } from 'lucide-vue-next'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from '@/components/ui/dialog'
import { useConnectionStore } from '@/stores/connections'
import { useSavedQueriesStore } from '@/stores/savedQueries'
import type { SavedQuery } from '@/types/savedQuery'

const props = defineProps<{
  open: boolean
  sql: string
  database?: string | null
  connectionId?: string
  /** Pass an existing SavedQuery to edit it */
  editing?: SavedQuery | null
}>()

const emit = defineEmits<{
  'update:open': [val: boolean]
  saved: [id: string]
}>()

const connStore = useConnectionStore()
const savedStore = useSavedQueriesStore()

const name = ref('')
const description = ref('')
const selectedConnectionId = ref<string>('')
const selectedDatabase = ref('')
const isSaving = ref(false)
const error = ref<string | null>(null)

watch(
  () => props.open,
  (open) => {
    if (!open) return
    error.value = null
    if (props.editing) {
      name.value = props.editing.name
      description.value = props.editing.description ?? ''
      selectedConnectionId.value = props.editing.connection_id ?? ''
      selectedDatabase.value = props.editing.database ?? ''
    } else {
      name.value = ''
      description.value = ''
      selectedConnectionId.value = props.connectionId ?? ''
      selectedDatabase.value = props.database ?? ''
    }
  },
)

async function save() {
  if (!name.value.trim()) {
    error.value = 'El nombre es obligatorio.'
    return
  }
  isSaving.value = true
  error.value = null
  try {
    const id = await savedStore.upsert({
      id: props.editing?.id,
      created_at: props.editing?.created_at,
      name: name.value.trim(),
      description: description.value.trim() || undefined,
      sql: props.sql,
      connection_id: selectedConnectionId.value || undefined,
      database: selectedDatabase.value.trim() || undefined,
    })
    emit('saved', id)
    emit('update:open', false)
  } catch (e: any) {
    error.value = String(e)
  } finally {
    isSaving.value = false
  }
}
</script>
