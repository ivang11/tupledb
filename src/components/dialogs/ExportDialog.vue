<script setup lang="ts">
import { ref } from 'vue'
import { CheckIcon, TableIcon } from 'lucide-vue-next'
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
  selectedTables: string[]
  currentMode: string
}>()

const emit = defineEmits<{
  'update:open': [val: boolean]
  'update:selectedTables': [tables: string[]]
  'update:currentMode': [mode: string]
  'start': []
}>()

const exportOptions = [
  { mode: 'full', label: 'Full', desc: 'Schema + Data' },
  { mode: 'structure', label: 'Structure', desc: 'Schema only' },
  { mode: 'data', label: 'Data', desc: 'Data only' },
]

const lastClickedIndex = ref(-1)

function toggleAll() {
  if (props.selectedTables.length === props.tables.length) {
    emit('update:selectedTables', [])
  } else {
    emit('update:selectedTables', props.tables.map(t => t.name))
  }
}

function handleTableClick(idx: number, event: MouseEvent) {
  const name = props.tables[idx].name
  if (event.shiftKey && lastClickedIndex.value >= 0) {
    const start = Math.min(lastClickedIndex.value, idx)
    const end = Math.max(lastClickedIndex.value, idx)
    const rangeNames = props.tables.slice(start, end + 1).map(t => t.name)
    const isSelecting = !props.selectedTables.includes(name)
    if (isSelecting) {
      emit('update:selectedTables', [...new Set([...props.selectedTables, ...rangeNames])])
    } else {
      emit('update:selectedTables', props.selectedTables.filter(n => !rangeNames.includes(n)))
    }
  } else {
    lastClickedIndex.value = idx
    if (props.selectedTables.includes(name)) {
      emit('update:selectedTables', props.selectedTables.filter(t => t !== name))
    } else {
      emit('update:selectedTables', [...props.selectedTables, name])
    }
  }
}
</script>

<template>
  <Dialog :open="open" @update:open="(val) => emit('update:open', val)">
    <DialogContent class="sm:max-w-md max-h-[85vh] flex flex-col p-0 overflow-hidden shadow-2xl border-primary/10">
      <DialogHeader class="p-6 pb-4 bg-background border-b relative z-20">
        <DialogTitle>Database Export</DialogTitle>
        <DialogDescription>
          Configure your export for <span class="font-bold text-foreground">`{{ database }}`</span>
        </DialogDescription>
      </DialogHeader>

      <ScrollArea class="flex-1 bg-background">
        <div class="px-6 py-4 border-b bg-muted/20">
          <h3 class="text-[10px] font-black uppercase tracking-widest text-muted-foreground mb-3">Export Mode</h3>
          <div class="grid grid-cols-3 gap-2">
            <button
              v-for="opt in exportOptions"
              :key="opt.mode"
              @click="emit('update:currentMode', opt.mode)"
              :class="['flex flex-col items-center gap-1 p-2.5 rounded-xl border transition-all', currentMode === opt.mode ? 'bg-primary border-primary text-primary-foreground shadow-lg shadow-primary/20' : 'bg-background hover:border-primary/50 text-muted-foreground']"
            >
              <span class="text-xs font-bold">{{ opt.label }}</span>
              <span class="text-[9px] opacity-70 text-center">{{ opt.desc }}</span>
            </button>
          </div>
        </div>

        <div class="px-6 py-4">
          <div class="flex items-center justify-between mb-3 sticky top-0 bg-background py-1 z-10 border-b">
            <h3 class="text-[10px] font-black uppercase tracking-widest text-muted-foreground">Tables Selection</h3>
            <div class="flex items-center gap-3">
              <span class="text-[10px] font-bold text-muted-foreground">{{ selectedTables.length }} selected</span>
              <button v-if="!loadingTables" @click="toggleAll" class="text-[10px] font-black text-primary uppercase hover:underline">
                {{ selectedTables.length === tables.length ? 'None' : 'All' }}
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
                  ? 'bg-primary/12 border-primary/35 shadow-sm'
                  : 'border-transparent hover:bg-muted/50']"
            >
              <div :class="['size-4 rounded border-2 flex items-center justify-center flex-shrink-0 transition-all',
                selectedTables.includes(table.name)
                  ? 'bg-primary border-primary shadow-sm shadow-primary/30'
                  : 'border-muted-foreground/25']">
                <CheckIcon v-if="selectedTables.includes(table.name)" class="size-2.5 text-primary-foreground stroke-[3]" />
              </div>
              <TableIcon :class="['size-3.5 transition-colors flex-shrink-0',
                selectedTables.includes(table.name) ? 'text-primary' : 'text-muted-foreground']" />
              <span :class="['text-sm font-medium truncate transition-colors',
                selectedTables.includes(table.name) ? 'text-primary font-semibold' : '']">
                {{ table.name }}
              </span>
            </div>
          </div>
        </div>
      </ScrollArea>

      <div class="p-6 py-4 border-t bg-muted/10 flex flex-row items-center justify-between gap-4">
        <Button variant="ghost" class="text-xs font-bold uppercase tracking-wider h-9" @click="emit('update:open', false)">Cancel</Button>
        <Button
          class="font-bold px-8 shadow-lg shadow-primary/30 h-10"
          :disabled="loadingTables || selectedTables.length === 0"
          @click="emit('start')"
        >
          Start {{ currentMode.toUpperCase() }} Export
        </Button>
      </div>
    </DialogContent>
  </Dialog>
</template>
