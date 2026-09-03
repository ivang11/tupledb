<template>
  <aside
    data-row-detail-panel
    class="shrink-0 border-l border-border bg-card flex flex-col min-h-0 min-w-0 relative"
    :style="{ width: width + 'px' }"
  >
    <!-- Resize handle -->
    <div
      class="absolute left-0 top-0 h-full w-1 cursor-col-resize hover:bg-primary/40 transition-colors z-10"
      @mousedown="emit('start-resize', $event)"
    />

    <!-- Header -->
    <div class="h-11 shrink-0 border-b flex items-center justify-between gap-2 px-3 bg-muted/25">
      <div class="min-w-0">
        <p class="text-[10px] font-bold text-muted-foreground uppercase tracking-wider truncate">
          {{ isReadOnly() ? 'Read-only row' : 'Selected row' }}
        </p>
        <p class="text-[11px] font-mono font-semibold text-foreground truncate">
          {{ primaryKey ? `${primaryKey} = ${rawValue(primaryKey)}` : 'No primary key' }}
        </p>
      </div>
      <button
        type="button"
        class="size-8 shrink-0 flex items-center justify-center rounded-md text-muted-foreground hover:text-foreground hover:bg-muted/60 transition-colors"
        title="Close panel"
        @click="emit('close')"
      >
        <XIcon class="size-4" />
      </button>
    </div>

    <!-- Field search -->
    <div class="px-3 py-2 border-b shrink-0">
      <div class="relative">
        <SearchIcon class="absolute left-2.5 top-1/2 -translate-y-1/2 size-3 text-muted-foreground/50" />
        <input
          v-model="fieldSearch"
          type="text"
          placeholder="Filter fields..."
          class="w-full bg-muted/30 border border-input rounded-md pl-7 pr-3 py-1.5 text-xs focus:outline-none focus:ring-1 focus:ring-ring"
        />
      </div>
    </div>

    <ScrollArea class="flex-1 min-h-0">
      <div class="p-3 space-y-3.5 pb-6">
        <div
          v-for="col in filteredColumns()"
          :key="'detail-' + col.name"
          class="space-y-1"
        >
          <div class="flex items-center justify-between gap-1">
            <Label class="text-xs font-bold text-foreground">{{ col.name }}</Label>
            <button
              type="button"
              class="size-5 shrink-0 flex items-center justify-center rounded transition-colors"
              :class="copiedField === col.name ? 'text-green-400' : 'text-muted-foreground/30 hover:text-foreground'"
              title="Copy value"
              @click="copyValue(col.name, getCellValue(row, col.name))"
            >
              <CheckIcon v-if="copiedField === col.name" class="size-3" />
              <CopyIcon v-else class="size-3" />
            </button>
          </div>
          <div
            class="rounded-md border border-input bg-background focus-within:ring-1 focus-within:ring-ring"
            :class="[
              fkMap[col.name] ? 'flex items-start px-3 py-1.5' : '',
              isPendingDelete() ? 'opacity-50 cursor-not-allowed' : '',
            ]"
          >
            <textarea
              :ref="(el) => autoResize(el as HTMLTextAreaElement)"
              rows="1"
              class="min-w-0 text-xs font-mono resize-none overflow-hidden leading-relaxed focus:outline-none read-only:text-foreground/80 disabled:cursor-not-allowed"
              :class="fkMap[col.name]
                ? 'shrink p-0'
                : 'w-full rounded-md border-0 bg-background px-3 py-1.5 focus:ring-1 focus:ring-ring read-only:bg-muted/20 disabled:opacity-50'"
              :style="fkMap[col.name] ? { width: fieldWidth(row, col.name) } : undefined"
              :readonly="isReadOnly()"
              :disabled="isPendingDelete()"
              :value="getCellValue(row, col.name)"
              :placeholder="rawValue(col.name) === null ? 'NULL' : 'EMPTY'"
              @input="(e) => {
                if (isReadOnly()) return
                const t = e.target as HTMLTextAreaElement
                t.style.height = 'auto'
                t.style.height = t.scrollHeight + 'px'
                emit('cell-input', col.name, t.value)
              }"
            />
            <button
              v-if="fkMap[col.name]"
              type="button"
              class="mt-0.5 ml-1 shrink-0 flex items-center justify-center rounded text-foreground/80 hover:text-foreground transition-colors"
              :title="`Go to ${fkMap[col.name].table}`"
              @click="emit('navigate-related', fkMap[col.name].table, fkMap[col.name].column, rawValue(col.name))"
            >
              <ArrowRightIcon class="size-3" />
            </button>
          </div>
          <p class="text-[10px] text-foreground/60 font-mono">{{ col.type_name }}</p>
        </div>
      </div>
    </ScrollArea>
  </aside>
</template>

<script setup lang="ts">
import { ref, nextTick } from 'vue'
import { SearchIcon, XIcon, CopyIcon, CheckIcon, ArrowRightIcon } from 'lucide-vue-next'
import { Label } from '@/components/ui/label'
import { ScrollArea } from '@/components/ui/scroll-area'
import { rowValue } from '@/lib/rowAccess'

const props = defineProps<{
  paneId: string
  row: Record<string, any> | any[]
  columns: any[]
  primaryKey: string | null
  fkMap: Record<string, { table: string; column: string }>
  pendingDeletions: Record<string, boolean>
  width: number
  getCellValue: (row: any, colName: string) => string
}>()

const emit = defineEmits<{
  'close': []
  'cell-input': [colName: string, value: string]
  'navigate-related': [table: string, column: string, value: any]
  'start-resize': [e: MouseEvent]
}>()

const fieldSearch = ref('')
const copiedField = ref<string | null>(null)

function filteredColumns() {
  if (!fieldSearch.value) return props.columns
  return props.columns.filter((c: any) => c.name.toLowerCase().includes(fieldSearch.value.toLowerCase()))
}

async function copyValue(colName: string, value: any) {
  const text = value === null || value === undefined ? '' : String(value)
  await navigator.clipboard.writeText(text)
  copiedField.value = colName
  setTimeout(() => { copiedField.value = null }, 1500)
}

function autoResize(el: HTMLTextAreaElement | null) {
  if (!el) return
  nextTick(() => {
    el.style.height = 'auto'
    el.style.height = el.scrollHeight + 'px'
  })
}

const isReadOnly = () => !props.primaryKey
const rawValue = (column: string) => rowValue(props.row, column, props.columns)
const pkVal = () => props.primaryKey ? String(rawValue(props.primaryKey)) : ''
const isPendingDelete = () => !!props.pendingDeletions[pkVal()]
const fieldWidth = (row: Record<string, any> | any[], colName: string) => {
  const length = Math.max(props.getCellValue(row, colName).length, 1)
  return `min(100%, ${length}ch)`
}
</script>
