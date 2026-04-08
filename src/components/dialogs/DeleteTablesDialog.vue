<script setup lang="ts">
import { ref } from 'vue'
import { CheckIcon, TableIcon, Trash2Icon, AlertTriangleIcon, XCircleIcon } from 'lucide-vue-next'
import { Button } from '@/components/ui/button'
import { ScrollArea } from '@/components/ui/scroll-area'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
} from '@/components/ui/dialog'

const props = defineProps<{
  open: boolean
  database: string
  tables: { name: string }[]
  loadingTables?: boolean
  isExecuting?: boolean
  error?: string | null
}>()

const emit = defineEmits<{
  'update:open': [val: boolean]
  'delete-tables': [tables: string[], disableFkChecks: boolean]
  'drop-database': []
}>()

const selectedTables = ref<string[]>([])
const lastClickedIndex = ref(-1)
const disableFkChecks = ref(false)
const confirmDropDb = ref(false)

function toggleAll() {
  if (selectedTables.value.length === props.tables.length) {
    selectedTables.value = []
  } else {
    selectedTables.value = props.tables.map(t => t.name)
  }
}

function handleTableClick(idx: number, event: MouseEvent) {
  const name = props.tables[idx].name
  if (event.shiftKey && lastClickedIndex.value >= 0) {
    const start = Math.min(lastClickedIndex.value, idx)
    const end = Math.max(lastClickedIndex.value, idx)
    const rangeNames = props.tables.slice(start, end + 1).map(t => t.name)
    const isSelecting = !selectedTables.value.includes(name)
    if (isSelecting) {
      selectedTables.value = [...new Set([...selectedTables.value, ...rangeNames])]
    } else {
      selectedTables.value = selectedTables.value.filter(n => !rangeNames.includes(n))
    }
  } else {
    lastClickedIndex.value = idx
    if (selectedTables.value.includes(name)) {
      selectedTables.value = selectedTables.value.filter(t => t !== name)
    } else {
      selectedTables.value = [...selectedTables.value, name]
    }
  }
  confirmDropDb.value = false
}

function handleClose() {
  if (props.isExecuting) return
  selectedTables.value = []
  lastClickedIndex.value = -1
  confirmDropDb.value = false
  disableFkChecks.value = false
  emit('update:open', false)
}

function handleDeleteTables() {
  emit('delete-tables', [...selectedTables.value], disableFkChecks.value)
}

function handleDropDatabase() {
  if (!confirmDropDb.value) {
    confirmDropDb.value = true
    return
  }
  emit('drop-database')
}
</script>

<template>
  <Dialog :open="open" @update:open="(val) => !val && handleClose()">
    <DialogContent class="sm:max-w-md max-h-[85vh] flex flex-col p-0 overflow-hidden shadow-2xl border-destructive/10">
      <DialogHeader class="p-6 pb-4 bg-background border-b relative z-20">
        <DialogTitle class="flex items-center gap-2">
          <Trash2Icon class="size-4 text-destructive" />
          Delete Tables
        </DialogTitle>
        <DialogDescription>
          Select tables to drop from <span class="font-bold text-foreground">`{{ database }}`</span>
        </DialogDescription>
      </DialogHeader>

      <ScrollArea class="flex-1 bg-background">
        <div class="px-6 py-4">
          <div class="flex items-center justify-between mb-3 sticky top-0 bg-background py-1 z-10 border-b">
            <h3 class="text-[10px] font-black uppercase tracking-widest text-muted-foreground">Tables</h3>
            <div class="flex items-center gap-3">
              <span class="text-[10px] font-bold text-muted-foreground">{{ selectedTables.length }} / {{ tables.length }}</span>
              <button v-if="!loadingTables && tables.length > 0" @click="toggleAll"
                class="text-[10px] font-black text-destructive uppercase hover:underline">
                {{ selectedTables.length === tables.length ? 'None' : `All (${tables.length})` }}
              </button>
            </div>
          </div>

          <!-- Loading skeleton -->
          <div v-if="loadingTables" class="grid grid-cols-1 gap-1">
            <div v-for="i in 6" :key="i" class="flex items-center gap-3 p-2">
              <div class="size-4 rounded bg-muted animate-pulse" />
              <div class="size-3.5 rounded bg-muted animate-pulse" />
              <div class="h-3.5 rounded bg-muted animate-pulse" :style="`width: ${50 + (i * 17) % 40}%`" />
            </div>
          </div>

          <div v-else class="grid grid-cols-1 gap-0.5">
            <div
              v-for="(table, idx) in tables"
              :key="table.name"
              @click="handleTableClick(idx, $event)"
              :class="['flex items-center gap-3 px-2.5 py-2 rounded-lg cursor-pointer select-none transition-all border',
                selectedTables.includes(table.name)
                  ? 'bg-destructive/10 border-destructive/30 shadow-sm'
                  : 'border-transparent hover:bg-muted/50']"
            >
              <div :class="['size-4 rounded border-2 flex items-center justify-center flex-shrink-0 transition-all',
                selectedTables.includes(table.name)
                  ? 'bg-destructive border-destructive shadow-sm shadow-destructive/20'
                  : 'border-muted-foreground/25']">
                <CheckIcon v-if="selectedTables.includes(table.name)" class="size-2.5 text-destructive-foreground stroke-[3]" />
              </div>
              <TableIcon :class="['size-3.5 transition-colors flex-shrink-0',
                selectedTables.includes(table.name) ? 'text-destructive' : 'text-muted-foreground']" />
              <span :class="['text-sm truncate transition-colors',
                selectedTables.includes(table.name) ? 'text-destructive font-semibold' : 'font-medium']">
                {{ table.name }}
              </span>
            </div>
          </div>
        </div>

        <!-- FK checks option -->
        <div class="px-6 pb-4" v-if="!loadingTables && tables.length > 0">
          <button
            @click="disableFkChecks = !disableFkChecks"
            class="flex items-start gap-3 cursor-pointer w-full bg-muted/20 p-3 rounded-lg border border-border hover:bg-muted/40 transition-colors text-left"
          >
            <div :class="['mt-0.5 shrink-0 flex items-center justify-center size-4 rounded border-2 border-border transition-colors',
              disableFkChecks ? 'bg-destructive border-destructive' : '']">
              <CheckIcon v-if="disableFkChecks" class="size-3 text-destructive-foreground stroke-[3]" />
            </div>
            <div class="flex flex-col min-w-0">
              <span class="text-sm font-bold text-foreground">Disable Foreign Key Checks</span>
              <span class="text-xs text-muted-foreground leading-relaxed mt-0.5">Allows dropping tables referenced by others.</span>
            </div>
          </button>
        </div>
      </ScrollArea>

      <!-- Error banner -->
      <div
        v-if="error"
        class="px-6 py-3 bg-destructive/10 border-t border-destructive/20 flex items-start gap-3"
      >
        <XCircleIcon class="size-4 text-destructive shrink-0 mt-0.5" />
        <span class="text-xs text-destructive font-medium leading-relaxed whitespace-pre-line">{{ error }}</span>
      </div>

      <!-- Drop DB confirm banner -->
      <div
        v-if="confirmDropDb"
        class="px-6 py-3 bg-destructive/10 border-t border-destructive/20 flex items-center gap-3"
      >
        <AlertTriangleIcon class="size-4 text-destructive flex-shrink-0" />
        <span class="text-xs text-destructive font-semibold flex-1">
          Drop <code class="font-black">`{{ database }}`</code>? This permanently deletes the database and all its data.
        </span>
        <button @click="confirmDropDb = false" class="text-xs text-muted-foreground hover:text-foreground font-medium">Cancel</button>
      </div>

      <div class="p-6 py-4 border-t bg-muted/10 flex flex-row items-center justify-between gap-2">
        <Button variant="ghost" class="text-xs font-bold uppercase tracking-wider h-9" @click="handleClose" :disabled="isExecuting">
          Cancel
        </Button>
        <div class="flex items-center gap-2">
          <Button
            variant="outline"
            class="text-xs font-bold h-9 border-destructive/40 text-destructive hover:bg-destructive/10 hover:border-destructive"
            :disabled="isExecuting || loadingTables"
            @click="handleDropDatabase"
          >
            <span v-if="!confirmDropDb">Drop Database</span>
            <span v-else class="flex items-center gap-1.5"><AlertTriangleIcon class="size-3.5" /> Confirm Drop</span>
          </Button>
          <Button
            variant="destructive"
            class="font-bold h-9 px-5 shadow-lg shadow-destructive/20"
            :disabled="isExecuting || loadingTables || selectedTables.length === 0"
            @click="handleDeleteTables"
          >
            <span v-if="!isExecuting">Delete {{ selectedTables.length > 0 ? selectedTables.length : '' }} Table{{ selectedTables.length !== 1 ? 's' : '' }}</span>
            <span v-else>Deleting…</span>
          </Button>
        </div>
      </div>
    </DialogContent>
  </Dialog>
</template>
