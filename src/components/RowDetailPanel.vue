<template>
  <aside
    ref="panel"
    data-row-detail-panel
    :data-density="settings.density"
    class="shrink-0 border-l border-border bg-card flex flex-col min-h-0 min-w-0 relative"
    :style="{ width: width + 'px', '--detail-font': fontCss(settings.tableFont), '--detail-size': settings.tableFontSize + 'px' }"
    :class="{ 'row-detail-compact': settings.density === 'compact' }"
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
          {{ primaryKey ? (Array.isArray(primaryKey) ? primaryKey : [primaryKey]).map(key => `${key} = ${rawValue(key)}`).join(', ') : 'No primary key' }}
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
      <div class="detail-fields p-3 pb-6" :class="settings.density === 'compact' ? 'space-y-1' : 'space-y-3.5'">
        <div
          v-for="col in filteredColumns()"
          :key="'detail-' + col.name"
          class="detail-field"
        >
          <div class="flex items-center justify-between gap-1">
            <Label class="text-xs font-medium text-foreground">{{ col.name }}</Label>
            <span v-if="settings.density === 'compact'" class="flex-1 truncate text-[10px] text-muted-foreground font-mono" :title="col.type_name">{{ col.type_name }}</span>
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
            class="detail-value rounded-md border border-input bg-background focus-within:ring-1 focus-within:ring-ring"
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
                resizeDetailField(t)
                emit('cell-input', col.name, t.value)
              }"
            />
            <button
              v-if="fkMap[col.name]"
              type="button"
              class="mt-0.5 ml-1 shrink-0 flex items-center justify-center rounded text-foreground/80 hover:text-foreground transition-colors"
              :title="`Go to ${tableLabel(fkMap[col.name].table)}`"
              @click="emit('navigate-related', fkMap[col.name].table, fkMap[col.name].column, rawValue(col.name))"
            >
              <ArrowRightIcon class="size-3" />
            </button>
          </div>
          <p v-if="settings.density !== 'compact'" class="text-[10px] text-foreground/60 font-mono">{{ col.type_name }}</p>
        </div>
      </div>
    </ScrollArea>
  </aside>
</template>

<script setup lang="ts">
import { resizeDetailField } from '@/lib/rowDetailLayout'
import { tableLabel } from '@/lib/tableReference'
import type { TableRef } from '@/types/database'

import { rowKey, type PrimaryKey } from '@/lib/tableIdentity'

import { useAppSettings, fontCss } from '@/composables/useAppSettings'
import { ref, nextTick, watch } from 'vue'
import { SearchIcon, XIcon, CopyIcon, CheckIcon, ArrowRightIcon } from 'lucide-vue-next'
import { Label } from '@/components/ui/label'
import { ScrollArea } from '@/components/ui/scroll-area'
import { rowValue } from '@/lib/rowAccess'

const props = defineProps<{
  paneId: string
  row: Record<string, any> | any[]
  columns: any[]
  primaryKey: PrimaryKey
  fkMap: Record<string, { table: string | TableRef; column: string }>
  pendingDeletions: Record<string, boolean>
  width: number
  getCellValue: (row: any, colName: string) => string
}>()

const emit = defineEmits<{
  'close': []
  'cell-input': [colName: string, value: string]
  'navigate-related': [table: string | TableRef, column: string, value: any]
  'start-resize': [e: MouseEvent]
}>()

const fieldSearch = ref('')
const { settings } = useAppSettings()
const panel = ref<HTMLElement | null>(null)
watch(() => [settings.value.density, settings.value.tableFontSize, settings.value.tableFont], () => nextTick(() => panel.value?.querySelectorAll('textarea').forEach(autoResize)))
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
  nextTick(() => resizeDetailField(el))
}

const isReadOnly = () => !props.primaryKey
const rawValue = (column: string) => rowValue(props.row, column, props.columns)
const pkVal = () => props.primaryKey ? rowKey(props.row, props.primaryKey, props.columns) : ''
const isPendingDelete = () => !!props.pendingDeletions[pkVal()]
const fieldWidth = (row: Record<string, any> | any[], colName: string) => {
  const length = Math.max(props.getCellValue(row, colName).length, 1)
  return `min(100%, ${length}ch)`
}
</script>

<style scoped>
[data-row-detail-panel] textarea { font-family: var(--detail-font); font-size: var(--detail-size); max-height: 180px; overflow-y: auto; }
.detail-field { display: grid; gap: 4px; }
.row-detail-compact .h-11 { height: 30px; }
.row-detail-compact .detail-fields { padding: 6px 8px; }
.row-detail-compact .detail-field { gap: 1px; }
.row-detail-compact .detail-value { display: flex; align-items: center; }
.row-detail-compact .detail-value.flex { padding: 0 6px; }
.row-detail-compact textarea { display: block; padding: 2px 6px; line-height: 1.2; max-height: 48px; }
.row-detail-compact .py-2 { padding-top: 4px; padding-bottom: 4px; }
</style>
