<script setup lang="ts">
import { TableIcon } from 'lucide-vue-next'
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

function toggleAll() {
  if (props.selectedTables.length === props.tables.length) {
    emit('update:selectedTables', [])
  } else {
    emit('update:selectedTables', props.tables.map(t => t.name))
  }
}

function toggleTable(name: string) {
  if (props.selectedTables.includes(name)) {
    emit('update:selectedTables', props.selectedTables.filter(t => t !== name))
  } else {
    emit('update:selectedTables', [...props.selectedTables, name])
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
              <button @click="toggleAll" class="text-[10px] font-black text-primary uppercase hover:underline">
                {{ selectedTables.length === tables.length ? 'None' : 'All' }}
              </button>
            </div>
          </div>
          <div class="grid grid-cols-1 gap-1">
            <label
              v-for="table in tables"
              :key="table.name"
              class="flex items-center gap-3 p-2 rounded-lg hover:bg-muted/50 cursor-pointer transition-colors group border border-transparent"
            >
              <input
                type="checkbox"
                :value="table.name"
                :checked="selectedTables.includes(table.name)"
                @change="toggleTable(table.name)"
                class="size-4 rounded border-muted accent-primary cursor-pointer"
              />
              <TableIcon class="size-3.5 text-muted-foreground group-hover:text-primary transition-colors" />
              <span class="text-sm font-medium truncate">{{ table.name }}</span>
            </label>
          </div>
        </div>
      </ScrollArea>

      <div class="p-6 py-4 border-t bg-muted/10 flex flex-row items-center justify-between gap-4">
        <Button variant="ghost" class="text-xs font-bold uppercase tracking-wider h-9" @click="emit('update:open', false)">Cancel</Button>
        <Button
          class="font-bold px-8 shadow-lg shadow-primary/30 h-10"
          :disabled="selectedTables.length === 0"
          @click="emit('start')"
        >
          Start {{ currentMode.toUpperCase() }} Export
        </Button>
      </div>
    </DialogContent>
  </Dialog>
</template>
