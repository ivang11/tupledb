<template>
  <CanvasDataGrid
    v-if="canvasSupported"
    v-bind="props"
    @row-click="(row, event, index, column) => emit('row-click', row, event, index, column)"
    @cell-dblclick="(row, column) => emit('cell-dblclick', row, column)"
    @cell-blur="emit('cell-blur')"
    @cell-input="(row, column, value) => emit('cell-input', row, column, value)"
    @sort="column => emit('sort', column)"
    @start-col-resize="(event, column) => emit('start-col-resize', event, column)"
    @navigate-related="(table, column, value) => emit('navigate-related', table, column, value)"
    @insert-row-input="(column, value) => emit('insert-row-input', column, value)"
    @insert-row-submit="emit('insert-row-submit')"
    @insert-row-cancel="emit('insert-row-cancel')"
    @pending-insert-input="(index, column, value) => emit('pending-insert-input', index, column, value)"
    @pending-insert-cancel="index => emit('pending-insert-cancel', index)"
    @row-contextmenu="(row, x, y) => emit('row-contextmenu', row, x, y)"
    @delete-key-pressed="emit('delete-key-pressed')"
  />
  <div
    v-else
    ref="scrollContainer"
    class="flex-1 min-w-0 relative overflow-auto bg-background custom-scrollbar"
    tabindex="0"
    @keydown.delete="onDeleteKeydown"
  >
    <!-- Empty state -->
    <div
      v-if="rows && rows.length === 0 && pendingInserts.length === 0"
      class="absolute inset-0 flex flex-col items-center justify-center gap-3 text-center p-12"
    >
      <DatabaseIcon class="size-8 text-muted-foreground/15" />
      <div>
        <p class="text-sm font-semibold text-foreground/60">No records</p>
        <p class="text-xs text-muted-foreground/40 mt-1">
          This table is empty or your filters didn't match any rows.
        </p>
      </div>
    </div>

    <div
      v-else
      role="table"
      class="data-grid relative min-w-full"
      :style="{ width: totalColumnSize + 'px' }"
      :aria-colcount="columns.length"
      :aria-rowcount="rows.length + pendingInserts.length"
    >
      <!-- ── Header ─────────────────────────────────────────────────────────── -->
      <div role="rowgroup" class="sticky top-0 z-20 bg-muted" :style="{ height: HEADER_HEIGHT + 'px' }">
        <div role="row" class="relative" :style="{ width: totalColumnSize + 'px', height: HEADER_HEIGHT + 'px' }">
          <div
            v-for="virtualColumn in virtualColumns"
            :key="virtualColumn.column.name"
            v-memo="[
              virtualColumn.index,
              sortColumn,
              sortDesc,
              columnWidths[virtualColumn.column.name],
            ]"
            role="columnheader"
            class="absolute top-0 h-full bg-muted text-left whitespace-nowrap cursor-pointer select-none group/th transition-colors overflow-hidden"
            :class="sortColumn === virtualColumn.column.name ? 'bg-primary/12' : 'hover:bg-accent'"
            :style="columnPositionStyle(virtualColumn, HEADER_HEIGHT)"
            :aria-colindex="virtualColumn.index + 1"
            @click="emit('sort', virtualColumn.column.name)"
          >
            <div class="flex items-center justify-between gap-2 px-3 py-2">
              <!-- Name + PK badge -->
              <div class="flex items-center gap-1.5 min-w-0">
                <span
                  class="text-xs font-bold font-mono truncate"
                  :class="sortColumn === virtualColumn.column.name ? 'text-primary' : 'text-foreground/95'"
                >
                  {{ virtualColumn.column.name }}
                </span>
                <span
                  v-if="primaryKey === virtualColumn.column.name"
                  class="text-[8px] font-black text-amber-500/90 border border-amber-500/30 bg-amber-500/8 px-1 py-px rounded shrink-0 leading-none"
                >
                  PK
                </span>
              </div>

              <!-- Sort icon -->
              <ArrowDownIcon  v-if="sortColumn === virtualColumn.column.name && sortDesc"  class="size-3 text-primary shrink-0" />
              <ArrowUpIcon    v-else-if="sortColumn === virtualColumn.column.name"         class="size-3 text-primary shrink-0" />
              <ArrowUpDownIcon v-else                                      class="size-3 text-muted-foreground/25 shrink-0 opacity-0 group-hover/th:opacity-100 transition-opacity" />
            </div>

            <!-- Type label -->
            <div class="px-3 pb-1.5 -mt-1">
              <span class="text-[9px] font-semibold font-mono text-muted-foreground/82 leading-none">
                {{ virtualColumn.column.type_name }}
              </span>
            </div>

            <!-- Column resize handle -->
            <div
              class="group/resize absolute top-0 right-0 h-full w-2 cursor-col-resize z-30 flex items-stretch justify-end"
              @mousedown="emit('start-col-resize', $event, virtualColumn.column.name)"
              @click.stop
            >
              <div class="w-px group-hover/resize:bg-primary/50 transition-colors" />
            </div>
          </div>
        </div>
      </div>

      <!-- ── Body ──────────────────────────────────────────────────────────── -->
      <div role="rowgroup" class="relative" :style="{ height: bodyHeight + 'px' }">
        <div
          v-for="virtualRow in virtualRows"
          :key="rowKey(virtualRow)"
          role="row"
          class="group/row absolute left-0 transition-colors"
          :class="rowClasses(rows[virtualRow.index], virtualRow.index)"
          :style="rowPositionStyle(virtualRow.start)"
          :aria-rowindex="virtualRow.index + 2"
          @click="emit('row-click', rows[virtualRow.index], $event, virtualRow.index)"
          @contextmenu="onRowContextMenu($event, rows[virtualRow.index])"
        >
          <div
            v-for="virtualColumn in virtualColumns"
            :key="virtualColumn.column.name"
            v-memo="[
              rows[virtualRow.index],
              virtualColumn.index,
              pendingChanges[String(rawCellValue(rows[virtualRow.index], primaryKey || ''))],
              pendingDeletions[String(rawCellValue(rows[virtualRow.index], primaryKey || ''))],
              pendingTruncate,
              pendingDrop,
              selectedRowPk,
              selectedRowPks,
              inlineEditColumn,
              columnWidths[virtualColumn.column.name],
            ]"
            role="cell"
            class="absolute top-0 px-3 text-sm group/cell overflow-hidden"
            :style="columnPositionStyle(virtualColumn)"
            :aria-colindex="virtualColumn.index + 1"
            :class="[
              isPendingChange(rows[virtualRow.index], virtualColumn.column.name) ? 'bg-amber-500/10' : '',
              isPkRow(rows[virtualRow.index], virtualRow.index) ? 'border-r-primary/10' : '',
            ]"
            @dblclick.stop="
              primaryKey
              && !pendingTruncate
              && !pendingDrop
              && !isPendingDelete(rows[virtualRow.index])
              && emit('cell-dblclick', rows[virtualRow.index], virtualColumn.column.name)
            "
          >
            <!-- Pending change left border -->
            <div
              v-if="isPendingChange(rows[virtualRow.index], virtualColumn.column.name)"
              class="absolute left-0 top-0 bottom-0 w-0.5 bg-amber-400/70"
            />

            <!-- Multi-selected row indicator (first col) -->
            <div
              v-if="(isMultiSelected(rows[virtualRow.index]) || isPkRow(rows[virtualRow.index], virtualRow.index)) && virtualColumn.column.name === columns[0]?.name && selectedRowPks.length > 1"
              class="absolute left-0 top-0 bottom-0 w-1 bg-primary"
            />

            <!-- Selected row indicator (first col, single selection only) -->
            <div
              v-else-if="isPkRow(rows[virtualRow.index], virtualRow.index) && virtualColumn.column.name === columns[0]?.name && selectedRowPks.length <= 1"
              class="absolute left-0 top-0 bottom-0 w-0.5 bg-primary"
            />

            <!-- Inline edit -->
            <template v-if="primaryKey && inlineEditColumn === virtualColumn.column.name && selectedRowPk === String(rawCellValue(rows[virtualRow.index], primaryKey))">
              <input
                :data-grid-edit="String(rawCellValue(rows[virtualRow.index], primaryKey))"
                :data-col="virtualColumn.column.name"
                :value="getCellValue(rows[virtualRow.index], virtualColumn.column.name)"
                @input="(e) => emit('cell-input', rows[virtualRow.index], virtualColumn.column.name, (e.target as HTMLInputElement).value)"
                @blur="emit('cell-blur')"
                @keydown.delete.stop
                @keydown.backspace.stop
                class="w-full bg-background border border-primary/40 rounded px-2 py-0.5 text-xs font-mono focus:outline-none focus:ring-1 focus:ring-primary/50"
                @click.stop
                autofocus
              />
            </template>

<!-- NULL -->
            <template v-else-if="rawCellValue(rows[virtualRow.index], virtualColumn.column.name) === null && !isPendingChange(rows[virtualRow.index], virtualColumn.column.name)">
              <div class="flex items-center gap-1.5 h-full">
                <span class="text-[10px] italic font-mono text-muted-foreground/45">NULL</span>
              </div>
            </template>

            <!-- Empty string -->
            <template v-else-if="getCellValue(rows[virtualRow.index], virtualColumn.column.name) === '' && !isPendingChange(rows[virtualRow.index], virtualColumn.column.name)">
              <div class="flex items-center gap-1.5 h-full">
                <span class="text-[10px] italic font-mono text-muted-foreground/45">EMPTY</span>
              </div>
            </template>

            <!-- Value -->
            <template v-else>
              <div class="flex items-center h-full min-w-0">
                <span
                  class="truncate text-[13px] font-mono leading-none select-none min-w-0"
                  :class="cellValueClasses(rows[virtualRow.index], virtualColumn.column.name)"
                >
                  {{ getCellValue(rows[virtualRow.index], virtualColumn.column.name) }}
                </span>

                <!-- FK link button (shown on row hover) -->
                <button
                  v-if="fkMap[virtualColumn.column.name] && rawCellValue(rows[virtualRow.index], virtualColumn.column.name) != null"
                  type="button"
                  @click.stop="emit('navigate-related', fkMap[virtualColumn.column.name].table, fkMap[virtualColumn.column.name].column, rawCellValue(rows[virtualRow.index], virtualColumn.column.name))"
                  class="ml-1 shrink-0 text-foreground/80 hover:text-foreground transition-colors"
                  :title="`Go to ${fkMap[virtualColumn.column.name].table}`"
                >
                  <ArrowRightIcon class="size-3" />
                </button>
              </div>
            </template>
          </div>
        </div>

        <!-- Pending insert rows -->
        <div
          v-for="(insert, insertIndex) in pendingInserts"
          :key="`pending-insert-${insertIndex}`"
          :data-pending-insert-index="insertIndex"
          role="row"
          class="absolute left-0 border-y border-emerald-500/25 bg-emerald-500/12"
          :style="rowPositionStyle(totalSize + insertIndex * ROW_HEIGHT)"
          :aria-rowindex="rows.length + insertIndex + 2"
        >
          <div
            v-for="virtualColumn in virtualColumns"
            :key="virtualColumn.column.name"
            role="cell"
            class="absolute top-0 px-1 border-r border-border/20"
            :style="columnPositionStyle(virtualColumn)"
            :aria-colindex="virtualColumn.index + 1"
          >
            <span v-if="isColAutoIncrement(virtualColumn.column.name)" class="px-2 text-[10px] text-emerald-500/70 italic font-mono">new</span>
            <input
              v-else
              :value="pendingInsertValue(insert, virtualColumn.column.name)"
              :placeholder="isBooleanCol(virtualColumn.column.name) ? '0 / 1' : ''"
              class="insert-row-input pending-insert-input w-full h-full px-2 text-xs font-mono focus:outline-none focus:bg-emerald-500/10 focus:ring-1 focus:ring-emerald-500/40 rounded"
              @input="emit('pending-insert-input', insertIndex, virtualColumn.column.name, ($event.target as HTMLInputElement).value)"
              @keydown.delete.stop
              @keydown.backspace.stop
              @keydown.escape="emit('pending-insert-cancel', insertIndex)"
            />
          </div>
        </div>
      </div>
    </div>

    <!-- No PK notice -->
    <div v-if="rows.length > 0 && !primaryKey" class="px-4 py-2.5 border-t border-dashed border-border/40 bg-muted/10">
      <p class="text-[10px] text-muted-foreground/40 italic">
        Editing is disabled — this table has no primary key.
      </p>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, nextTick, watch } from 'vue'
import { useVirtualizer } from '@tanstack/vue-virtual'
import { ArrowUpIcon, ArrowDownIcon, ArrowUpDownIcon, ArrowRightIcon, DatabaseIcon } from 'lucide-vue-next'
import CanvasDataGrid from '@/components/CanvasDataGrid.vue'
import { rowValue } from '@/lib/rowAccess'

const props = defineProps<{
  columns: any[]
  rows: any[]
  primaryKey: string | null
  totalCount: number
  pendingChanges: Record<string, Record<string, any>>
  pendingDeletions: Record<string, boolean>
  pendingTruncate: boolean
  pendingDrop: boolean
  selectedRowPk: string | null
  selectedRowPks: string[]
  inlineEditColumn: string | null
  sortColumn: string | null
  sortDesc: boolean
  insertingRow: boolean
  insertRowValues: Record<string, string>
  pendingInserts: Array<{ values: Array<{ column: string; value: any }> }>
  columnWidths: Record<string, number>
  fkMap: Record<string, { table: string; column: string }>
  bottomInset?: number
  isColAutoIncrement: (colName: string) => boolean
  isBooleanCol: (colName: string) => boolean
  getCellValue: (row: any, colName: string) => string
}>()

const isTestEnvironment = typeof process !== 'undefined' && process.env.NODE_ENV === 'test'
const canvasSupported = !isTestEnvironment && typeof HTMLCanvasElement !== 'undefined'

const emit = defineEmits<{
  'row-click': [row: any, e: MouseEvent, index: number, colName?: string]
  'cell-dblclick': [row: any, colName: string]
  'cell-blur': []
  'cell-input': [row: any, colName: string, value: string]
  'sort': [colName: string]
  'start-col-resize': [e: MouseEvent, colName: string]
  'navigate-related': [table: string, column: string, value: any]
  'insert-row-input': [colName: string, value: string]
  'insert-row-submit': []
  'insert-row-cancel': []
  'pending-insert-input': [index: number, colName: string, value: string]
  'pending-insert-cancel': [index: number]
  'row-contextmenu': [row: any, x: number, y: number]
  'delete-key-pressed': []
}>()

const DEFAULT_COLUMN_WIDTH = 180

function rawCellValue(row: any, column: string) {
  return rowValue(row, column, props.columns)
}

function columnWidth(colName: string) {
  return props.columnWidths[colName] ?? DEFAULT_COLUMN_WIDTH
}

function columnStyle(colName: string) {
  const width = columnWidth(colName) + 'px'
  return { width, minWidth: width, maxWidth: width }
}

function pendingInsertValue(
  insert: { values: Array<{ column: string; value: any }> },
  colName: string,
) {
  const value = insert.values.find(item => item.column === colName)?.value
  return value === null || value === undefined ? '' : String(value)
}

const scrollContainer = ref<HTMLElement | null>(null)

watch(
  () => props.pendingInserts.length,
  (nextCount, previousCount) => {
    if (nextCount <= previousCount) return
    nextTick(() => {
      const container = scrollContainer.value
      if (!container) return
      container.scrollTop = container.scrollHeight
      container
        .querySelector<HTMLInputElement>(`[data-pending-insert-index="${previousCount}"] input`)
        ?.focus()
    })
  },
)

const HEADER_HEIGHT = 48
const ROW_HEIGHT = 34

const virtualizer = useVirtualizer(computed(() => ({
  count: props.rows.length,
  getScrollElement: () => scrollContainer.value,
  estimateSize: () => ROW_HEIGHT,
  overscan: 4,
})))

const virtualRows = computed(() => virtualizer.value.getVirtualItems())
const totalSize = computed(() => virtualizer.value.getTotalSize())

const columnVirtualizer = useVirtualizer(computed(() => ({
  horizontal: true,
  count: props.columns.length,
  getScrollElement: () => scrollContainer.value,
  estimateSize: (index: number) => columnWidth(props.columns[index]?.name ?? ''),
  overscan: 1,
})))

const virtualColumns = computed(() =>
  columnVirtualizer.value.getVirtualItems().map(item => ({
    ...item,
    column: props.columns[item.index],
  })).filter(item => item.column),
)
const totalColumnSize = computed(() => columnVirtualizer.value.getTotalSize())
const paddingBottom = computed(() => props.bottomInset ?? 0)
const bodyHeight = computed(() =>
  totalSize.value + props.pendingInserts.length * ROW_HEIGHT + paddingBottom.value
)

function columnPositionStyle(
  virtualColumn: { start: number; column: { name: string } },
  height = ROW_HEIGHT,
) {
  return {
    ...columnStyle(virtualColumn.column.name),
    left: virtualColumn.start + 'px',
    height: height + 'px',
  }
}

function rowPositionStyle(start: number) {
  return {
    transform: `translateY(${start}px)`,
    width: totalColumnSize.value + 'px',
    height: ROW_HEIGHT + 'px',
  }
}

function rowKey(virtualRow: any) {
  const row = props.rows[virtualRow.index]
  return props.primaryKey ? String(rawCellValue(row, props.primaryKey)) : virtualRow.index
}

function rowSelectionKey(row: any, index: number) {
  return props.primaryKey ? String(rawCellValue(row, props.primaryKey)) : `__row_index:${index}`
}

function isPkRow(row: any, index: number) {
  return props.selectedRowPk === rowSelectionKey(row, index)
}

function isMultiSelected(row: any) {
  if (!props.primaryKey) return false
  return props.selectedRowPks.includes(String(rawCellValue(row, props.primaryKey)))
}

function isPendingDelete(row: any) {
  return !!props.pendingDeletions[String(rawCellValue(row, props.primaryKey || ''))]
}

function isPendingChange(row: any, col: string) {
  return props.pendingChanges[String(rawCellValue(row, props.primaryKey || ''))]?.[col] !== undefined
}

function rowClasses(row: any, index: number) {
  if (props.pendingTruncate) return 'cursor-pointer opacity-60 bg-amber-500/12 text-amber-200/75'
  if (props.pendingDrop) return 'cursor-pointer opacity-55 bg-destructive/10 text-destructive/75'
  if (isPendingDelete(row)) return 'cursor-pointer bg-destructive/10 text-destructive/70'

  const isSelected = props.selectedRowPks.length > 1
    ? isPkRow(row, index) || isMultiSelected(row)
    : isPkRow(row, index)

  if (props.selectedRowPks.length > 1 && isSelected) return 'cursor-pointer bg-primary/25 hover:bg-accent/70'
  if (isSelected) return 'cursor-pointer bg-primary/10'
  if (index % 2 === 1) return 'cursor-pointer bg-muted/50 hover:bg-accent/70'
  return 'cursor-pointer bg-background hover:bg-accent/70'
}

function cellValueClasses(row: any, col: string) {
  if (props.pendingTruncate) return 'line-through text-amber-200/70'
  if (props.pendingDrop) return 'line-through text-destructive/55'
  if (isPendingDelete(row)) return 'line-through text-destructive/50'
  if (isPendingChange(row, col)) return 'text-amber-300'
  return 'text-foreground/96'
}

function onRowContextMenu(e: MouseEvent, row: any) {
  e.preventDefault()
  e.stopPropagation()
  emit('row-contextmenu', row, e.clientX, e.clientY)
}

function isEditableTarget(target: EventTarget | null) {
  if (!(target instanceof HTMLElement)) return false
  const tagName = target.tagName
  return tagName === 'INPUT' || tagName === 'TEXTAREA' || target.isContentEditable
}

function onDeleteKeydown(e: KeyboardEvent) {
  if (isEditableTarget(e.target)) return
  e.stopPropagation()
  emit('delete-key-pressed')
}
</script>
