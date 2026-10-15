<template>
  <div class="bg-(--bg-1) border-b border-(--line-2) px-2 py-1.5 space-y-1.5">
    <!-- Global Actions -->
    <div class="flex items-center justify-between px-0.5">
      <div class="flex items-center gap-2">
        <span class="text-[11px] font-medium text-(--fg-2)">Filters</span>
        <Badge variant="outline" class="text-[10px] font-medium py-0 h-[18px] px-1.5 rounded-[4px] cursor-pointer hover:bg-accent" @click="filterSet.match_all = !filterSet.match_all">
          Match {{ filterSet.match_all ? 'all (AND)' : 'any (OR)' }}
        </Badge>
      </div>
      <div class="flex items-center gap-1">
        <Button variant="ghost" size="sm" class="h-6 px-2 text-[11px] font-medium" @click="clearFilters">
          <Trash2Icon class="size-3 mr-1" /> Clear
        </Button>
        <Button variant="secondary" size="sm" class="h-6 px-2 text-[11px] font-medium" @click="applyFilters">
          <PlayIcon class="size-3 mr-1" /> Apply
        </Button>
        <Button size="icon" class="h-6 w-6" title="Add filter" @click="addRow">
          <PlusIcon class="size-3" />
        </Button>
      </div>
    </div>

    <!-- Filter Rows -->
    <div v-if="filterSet.rows.length > 0" class="space-y-1 pb-0.5">
      <div v-for="(row, i) in filterSet.rows" :key="i" class="flex items-center gap-1.5 px-0.5 group">
        <input type="checkbox" v-model="row.active" class="size-3.5 rounded border-input accent-primary" />

        <Select v-model="row.column">
          <SelectTrigger class="h-6 text-[11px] w-35 bg-(--bg-0) px-2 py-0 rounded-[4px]">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem v-for="col in columns" :key="col.name" :value="col.name" class="text-xs">
              {{ col.name }}
            </SelectItem>
          </SelectContent>
        </Select>

        <Select v-model="row.operator">
          <SelectTrigger class="h-6 text-[11px] w-27.5 bg-(--bg-0) px-2 py-0 rounded-[4px]">
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
          class="h-6 text-[11px] md:text-[11px] flex-1 bg-(--bg-0) rounded-[4px] shadow-none px-2 focus-visible:ring-1"
          placeholder="Value"
          @keyup.enter="applyFilters"
          v-if="!['is_null', 'is_not_null', 'true', 'false'].includes(row.operator)"
        />
        <div v-else class="flex-1"></div>

        <div class="flex items-center gap-0.5 shrink-0">
          <Button variant="ghost" size="icon" class="h-6 w-6 text-muted-foreground hover:text-foreground" title="Apply this filter" @click="applySingleRow(row)">
            <PlayIcon class="size-3" />
          </Button>
          <Button variant="ghost" size="icon" class="h-6 w-6 text-muted-foreground hover:text-destructive" title="Remove filter" @click="removeRow(i)">
            <XIcon class="size-3" />
          </Button>
        </div>
      </div>
    </div>
    <div v-else class="px-0.5 text-[11px] text-(--fg-4)">
      No filters yet. Add one with the + button.
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue'
import { PlusIcon, XIcon, PlayIcon, Trash2Icon } from 'lucide-vue-next'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Badge } from '@/components/ui/badge'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import type { FilterSet, FilterRow, Operator } from '@/types/filters'
import { useActionShortcut } from '@/composables/useKeyboardShortcut'

const props = defineProps<{
  visible: boolean
  columns: { name: string, type_name: string }[]
  initialFilter?: { match_all: boolean; rows: FilterRow[] } | null
}>()

const emit = defineEmits<{
  (e: 'apply', filters: FilterSet, configuredFilters?: FilterSet): void
  (e: 'clear'): void
}>()

useActionShortcut('applyFilters', () => {
  applyFilters()
})

const operators: { label: string, value: Operator }[] = [
  { label: '=', value: 'equals' },
  { label: '!=', value: 'not_equals' },
  { label: 'contains', value: 'contains' },
  { label: 'starts with', value: 'starts_with' },
  { label: 'ends with', value: 'ends_with' },
  { label: 'in', value: 'in' },
  { label: 'not in', value: 'not_in' },
  { label: 'is null', value: 'is_null' },
  { label: 'is not null', value: 'is_not_null' },
  { label: 'is true', value: 'true' },
  { label: 'is false', value: 'false' },
  { label: '>', value: 'greater_than' },
  { label: '>=', value: 'greater_or_equal' },
  { label: '<', value: 'less_than' },
  { label: '<=', value: 'less_or_equal' },
]

const filterSet = ref<FilterSet>(
  props.initialFilter
    ? JSON.parse(JSON.stringify(props.initialFilter))
    : { match_all: true, rows: [] }
)

watch(() => props.initialFilter, value => {
  filterSet.value = value ? JSON.parse(JSON.stringify(value)) : { match_all: true, rows: [] }
}, { deep: true })

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
    rows: [{ ...row, active: true }]
  }
  emit('apply', JSON.parse(JSON.stringify(singleSet)), JSON.parse(JSON.stringify(filterSet.value)))
}

watch(() => [props.visible, props.columns.length], () => {
  if (props.visible && props.columns.length > 0 && filterSet.value.rows.length === 0) addRow()
}, { immediate: true })
</script>
