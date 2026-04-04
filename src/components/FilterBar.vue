<script setup lang="ts">
import { ref, watch } from 'vue'
import { PlusIcon, XIcon, PlayIcon, Trash2Icon } from 'lucide-vue-next'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Badge } from '@/components/ui/badge'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import type { FilterSet, FilterRow, Operator } from '@/types/filters'
import { useKeyboardShortcut } from '@/composables/useKeyboardShortcut'

const props = defineProps<{
  columns: { name: string, type_name: string }[]
}>()

const emit = defineEmits<{
  (e: 'apply', filters: FilterSet): void
  (e: 'clear'): void
}>()

useKeyboardShortcut('enter', () => {
  applyFilters()
})

const operators: { label: string, value: Operator }[] = [
  { label: '=', value: 'equals' },
  { label: '!=', value: 'not_equals' },
  { label: 'contains', value: 'contains' },
  { label: 'starts with', value: 'starts_with' },
  { label: 'ends with', value: 'ends_with' },
  { label: 'in', value: 'not_in' },
  { label: 'is null', value: 'is_null' },
  { label: 'is not null', value: 'is_not_null' },
  { label: 'is true', value: 'true' },
  { label: 'is false', value: 'false' },
  { label: '>', value: 'greater_than' },
  { label: '>=', value: 'greater_or_equal' },
  { label: '<', value: 'less_than' },
  { label: '<=', value: 'less_or_equal' },
]

const filterSet = ref<FilterSet>({
  match_all: true,
  rows: []
})

function addRow() {
  filterSet.value.rows.push({
    active: true,
    column: props.columns[0]?.name || '',
    operator: 'equals',
    value: ''
  })
}

function removeRow(index: number) {
  filterSet.value.rows.splice(index, 1)
}

function clearFilters() {
  filterSet.value.rows = []
  emit('clear')
}

function applyFilters() {
  emit('apply', JSON.parse(JSON.stringify(filterSet.value)))
}

function applySingleRow(row: FilterRow) {
  const singleSet: FilterSet = {
    match_all: filterSet.value.match_all,
    rows: [row]
  }
  emit('apply', singleSet)
}

// Add initial row if empty
watch(() => props.columns, (cols) => {
  if (cols.length > 0 && filterSet.value.rows.length === 0) {
    // addRow() // Don't auto-add, let user click [+]
  }
}, { immediate: true })
</script>

<template>
  <div class="bg-sidebar/40 border-b border-border p-2 space-y-2">
    <!-- Global Actions -->
    <div class="flex items-center justify-between px-1">
      <div class="flex items-center gap-2">
        <span class="text-[10px] font-bold uppercase text-muted-foreground">Filters</span>
        <Badge variant="outline" class="text-[9px] py-0 h-4 px-1.5 cursor-pointer hover:bg-accent" @click="filterSet.match_all = !filterSet.match_all">
          Match: {{ filterSet.match_all ? 'AND' : 'OR' }}
        </Badge>
      </div>
      <div class="flex items-center gap-1">
        <Button variant="ghost" size="sm" class="h-6 px-2 text-[10px] font-bold uppercase tracking-tight" @click="clearFilters">
          <Trash2Icon class="size-3 mr-1.5" /> Clear
        </Button>
        <Button variant="secondary" size="sm" class="h-6 px-2 text-[10px] font-bold uppercase tracking-tight" @click="applyFilters">
          <PlayIcon class="size-3 mr-1.5" /> Apply All
        </Button>
        <Button size="icon" class="h-6 w-6" @click="addRow">
          <PlusIcon class="size-3" />
        </Button>
      </div>
    </div>

    <!-- Filter Rows -->
    <div v-if="filterSet.rows.length > 0" class="space-y-1.5 pb-1">
      <div v-for="(row, i) in filterSet.rows" :key="i" class="flex items-center gap-2 px-1 group">
        <input type="checkbox" v-model="row.active" class="size-3.5 rounded border-input accent-primary" />
        
        <Select v-model="row.column">
          <SelectTrigger class="h-7 text-[11px] w-[140px] bg-background/50 border-dashed">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem v-for="col in columns" :key="col.name" :value="col.name" class="text-xs">
              {{ col.name }}
            </SelectItem>
          </SelectContent>
        </Select>

        <Select v-model="row.operator">
          <SelectTrigger class="h-7 text-[11px] w-[110px] bg-background/50 border-dashed">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem v-for="op in operators" :key="op.value" :value="op.value" class="text-xs">
              {{ op.label }}
            </SelectItem>
          </SelectContent>
        </Select>

        <Input 
          v-model="row.value" 
          class="h-7 text-[11px] flex-1 bg-background/50 border-dashed focus-visible:ring-1" 
          placeholder="Value..."
          @keyup.enter="applyFilters"
          v-if="!['is_null', 'is_not_null', 'true', 'false'].includes(row.operator)"
        />
        <div v-else class="flex-1"></div>

        <div class="flex items-center gap-0.5 shrink-0">
          <Button variant="ghost" size="icon" class="h-7 w-7 text-muted-foreground hover:text-foreground" title="Apply this filter" @click="applySingleRow(row)">
            <PlayIcon class="size-3" />
          </Button>
          <Button variant="ghost" size="icon" class="h-7 w-7 text-muted-foreground hover:text-destructive" @click="removeRow(i)">
            <XIcon class="size-3" />
          </Button>
        </div>
      </div>
    </div>
    <div v-else class="px-2 py-1 text-[10px] text-muted-foreground italic">
      No active filters. Click [+] to add one.
    </div>
  </div>
</template>

<style scoped>
@reference "../assets/main.css";

:deep(button[role="combobox"]) {
  @apply px-2 py-0;
}
</style>
