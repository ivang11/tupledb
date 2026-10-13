<template>
  <div class="flex flex-col flex-1 min-h-0 min-w-0">
    <div v-if="showColumns" class="max-h-48 overflow-auto border-b p-2 bg-muted/20 grid gap-1" aria-label="Column preferences">
      <div v-for="(column, index) in orderedColumns" :key="column.name" class="flex items-center gap-2 text-xs">
        <label class="flex items-center gap-2 min-w-0 flex-1"><input type="checkbox" :checked="!preferences.hidden.includes(column.name)" :disabled="displayColumns.length === 1 && !preferences.hidden.includes(column.name)" @change="toggleHidden(column.name)" /><span class="truncate">{{ column.name }}</span></label>
        <button class="px-2 py-1 hover:bg-muted rounded" @click="togglePinned(column.name)">{{ preferences.pinned.includes(column.name) ? 'Unpin' : 'Pin' }}</button>
        <button :disabled="index === 0" class="px-2 py-1 hover:bg-muted rounded disabled:opacity-30" :aria-label="`Move ${column.name} left`" @click="moveColumn(column.name, -1)">←</button>
        <button :disabled="index === orderedColumns.length - 1" class="px-2 py-1 hover:bg-muted rounded disabled:opacity-30" :aria-label="`Move ${column.name} right`" @click="moveColumn(column.name, 1)">→</button>
      </div>
      <button class="text-xs text-left p-1 hover:bg-muted rounded" @click="showColumns = false">Close column options</button>
      <button class="text-xs text-left p-1 hover:bg-muted rounded" @click="resetColumns">Reset columns</button>
    </div>
  <CanvasDataGrid
    v-if="canvasSupported"
    v-bind="renderProps"
    @row-click="gridRowClick"
    @cell-dblclick="(row, column) => emit('cell-dblclick', row, column)"
    @cell-blur="emit('cell-blur')"
    @cell-input="(row, column, value) => emit('cell-input', row, column, value)"
    @sort="column => sortColumnClick(column)"
    @start-col-resize="resizeColumn"
    @navigate-related="(table, column, value) => emit('navigate-related', table, column, value)"
    @insert-row-input="(column, value) => emit('insert-row-input', column, value)"
    @insert-row-submit="emit('insert-row-submit')"
    @insert-row-cancel="emit('insert-row-cancel')"
    @pending-insert-input="(index, column, value) => emit('pending-insert-input', index, column, value)"
    @pending-insert-cancel="index => emit('pending-insert-cancel', index)"
    @row-contextmenu="(row, x, y) => emit('row-contextmenu', row, x, y)"
    @cell-contextmenu="openCellMenu"
    @column-contextmenu="openColumnMenu"
    @inspect-cell="inspectCell"
    @delete-key-pressed="emit('delete-key-pressed')"
  />
  <div
    v-else
    ref="scrollContainer"
    class="flex-1 min-w-0 relative overflow-auto bg-background custom-scrollbar"
    tabindex="0"
    @scroll.passive="domScrollLeft = ($event.target as HTMLElement).scrollLeft"
    @keydown.delete="onDeleteKeydown"
  >
    <!-- Empty state -->
    <div
      v-if="rows && rows.length === 0 && pendingInserts.length === 0"
      class="absolute inset-0 flex flex-col items-center justify-center gap-2 text-center p-8"
    >
      <DatabaseIcon class="size-6 text-(--fg-5)" />
      <div>
        <p class="text-[12.5px] font-medium text-(--fg-2)">No records</p>
        <p class="text-[11.5px] text-(--fg-4) mt-0.5">
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
              effectiveWidths[virtualColumn.column.name],
              domScrollLeft,
              rowHeight,
              preferences.pinned,
              virtualColumn.start,
            ]"
            role="columnheader"
            class="absolute top-0 h-full bg-muted text-left whitespace-nowrap cursor-pointer select-none group/th transition-colors overflow-hidden"
            :class="sortColumn === virtualColumn.column.name ? 'bg-primary/12' : 'hover:bg-accent'"
            :style="columnPositionStyle(virtualColumn, HEADER_HEIGHT)"
            :aria-colindex="virtualColumn.index + 1"
            @click="sortColumnClick(virtualColumn.column.name)"
            @contextmenu.prevent.stop="openColumnMenu(virtualColumn.column.name, $event.clientX, $event.clientY)"
          >
            <div class="flex h-full items-center justify-between gap-2 px-2 leading-none" :title="virtualColumn.column.type_name">
              <!-- Name + PK badge -->
              <div class="flex items-center gap-1.5 min-w-0">
                <span
                  class="text-[11.5px] font-semibold font-mono truncate"
                  :class="sortColumn === virtualColumn.column.name ? 'text-primary' : 'text-foreground/95'"
                >
                  {{ virtualColumn.column.name }}
                </span>
                <span
                  v-if="isPrimaryKeyColumn(primaryKey, virtualColumn.column.name)"
                  class="text-[8px] font-bold text-(--pk) shrink-0 leading-none"
                >
                  PK
                </span>
              </div>

              <!-- Sort icon -->
              <ArrowDownIcon  v-if="sortColumn === virtualColumn.column.name && sortDesc"  class="size-3 text-primary shrink-0" />
              <ArrowUpIcon    v-else-if="sortColumn === virtualColumn.column.name"         class="size-3 text-primary shrink-0" />
              <ArrowUpDownIcon v-else                                      class="size-3 text-muted-foreground/25 shrink-0 opacity-0 group-hover/th:opacity-100 transition-opacity" />
            </div>

            <!-- Column resize handle -->
            <div
              class="group/resize absolute top-0 right-0 h-full w-2 cursor-col-resize z-30 flex items-stretch justify-end"
              @mousedown="resizeColumn($event, virtualColumn.column.name)"
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
          :key="virtualRowKey(virtualRow)"
          role="row"
          class="group/row absolute left-0 transition-colors"
          :class="rowClasses(displayRows[virtualRow.index], virtualRow.index)"
          :style="rowPositionStyle(virtualRow.start)"
          :aria-rowindex="virtualRow.index + 2"
          @click="gridRowClick(displayRows[virtualRow.index], $event, virtualRow.index)"
          @contextmenu="onRowContextMenu($event, displayRows[virtualRow.index])"
        >
          <div
            v-for="virtualColumn in virtualColumns"
            :key="virtualColumn.column.name"
            v-memo="[
              displayRows[virtualRow.index],
              virtualColumn.index,
              pendingChanges[rowKey(displayRows[virtualRow.index], primaryKey, columns)],
              pendingDeletions[rowKey(displayRows[virtualRow.index], primaryKey, columns)],
              pendingTruncate,
              pendingDrop,
              selectedRowPk,
              selectedRowPks,
              inlineEditColumn,
              effectiveWidths[virtualColumn.column.name],
              domScrollLeft,
              rowHeight,
              preferences.pinned,
              virtualColumn.start,
            ]"
            role="cell"
            class="absolute top-0 px-2 text-xs group/cell overflow-hidden"
            :style="columnPositionStyle(virtualColumn)"
            :aria-colindex="virtualColumn.index + 1"
            :class="[
              isPendingChange(displayRows[virtualRow.index], virtualColumn.column.name) ? 'bg-amber-500/10' : '',
              isPkRow(displayRows[virtualRow.index], virtualRow.index) ? 'border-r-primary/10' : '',
            ]"
            @contextmenu.prevent.stop="openCellMenu(displayRows[virtualRow.index], virtualColumn.column.name, $event.clientX, $event.clientY)"
            @dblclick.stop="
              primaryKey
              && !pendingTruncate
              && !pendingDrop
              && !isPendingDelete(displayRows[virtualRow.index])
              ? emit('cell-dblclick', displayRows[virtualRow.index], virtualColumn.column.name) : inspectCell(displayRows[virtualRow.index], virtualColumn.column.name)
            "
          >
            <!-- Pending change left border -->
            <div
              v-if="isPendingChange(displayRows[virtualRow.index], virtualColumn.column.name)"
              class="absolute left-0 top-0 bottom-0 w-0.5 bg-amber-400/70"
            />

            <!-- Multi-selected row indicator (first col) -->
            <div
              v-if="(isMultiSelected(displayRows[virtualRow.index]) || isPkRow(displayRows[virtualRow.index], virtualRow.index)) && virtualColumn.column.name === columns[0]?.name && selectedRowPks.length > 1"
              class="absolute left-0 top-0 bottom-0 w-1 bg-primary"
            />

            <!-- Selected row indicator (first col, single selection only) -->
            <div
              v-else-if="isPkRow(displayRows[virtualRow.index], virtualRow.index) && virtualColumn.column.name === columns[0]?.name && selectedRowPks.length <= 1"
              class="absolute left-0 top-0 bottom-0 w-0.5 bg-primary"
            />

            <!-- Inline edit -->
            <template v-if="primaryKey && inlineEditColumn === virtualColumn.column.name && selectedRowPk === rowKey(displayRows[virtualRow.index], primaryKey, columns)">
              <input
                :data-grid-edit="rowKey(displayRows[virtualRow.index], primaryKey, columns)"
                :data-col="virtualColumn.column.name"
                :value="getCellValue(displayRows[virtualRow.index], virtualColumn.column.name)"
                @input="(e) => emit('cell-input', displayRows[virtualRow.index], virtualColumn.column.name, (e.target as HTMLInputElement).value)"
                @blur="emit('cell-blur')"
                @keydown.delete.stop
                @keydown.backspace.stop
                class="w-full bg-background border border-primary/40 rounded px-2 py-0.5 text-xs font-mono focus:outline-none focus:ring-1 focus:ring-primary/50"
                @click.stop
                autofocus
              />
            </template>

<!-- NULL -->
            <template v-else-if="rawCellValue(displayRows[virtualRow.index], virtualColumn.column.name) === null && !isPendingChange(displayRows[virtualRow.index], virtualColumn.column.name)">
              <div class="flex items-center gap-1.5 h-full">
                <span class="text-[10px] italic font-mono text-muted-foreground/45">NULL</span>
              </div>
            </template>

            <!-- Empty string -->
            <template v-else-if="getCellValue(displayRows[virtualRow.index], virtualColumn.column.name) === '' && !isPendingChange(displayRows[virtualRow.index], virtualColumn.column.name)">
              <div class="flex items-center gap-1.5 h-full">
                <span class="text-[10px] italic font-mono text-muted-foreground/45">EMPTY</span>
              </div>
            </template>

            <!-- Value -->
            <template v-else>
              <div class="flex items-center h-full min-w-0">
                <span
                  class="truncate text-[13px] font-mono leading-none select-none min-w-0"
                  :class="cellValueClasses(displayRows[virtualRow.index], virtualColumn.column.name)"
                >
                  {{ getCellValue(displayRows[virtualRow.index], virtualColumn.column.name) }}
                </span>

                <!-- FK link button (shown on row hover) -->
                <button
                  v-if="fkMap[virtualColumn.column.name] && rawCellValue(displayRows[virtualRow.index], virtualColumn.column.name) != null"
                  type="button"
                  @click.stop="emit('navigate-related', fkMap[virtualColumn.column.name].table, fkMap[virtualColumn.column.name].column, rawCellValue(displayRows[virtualRow.index], virtualColumn.column.name))"
                  class="ml-1 shrink-0 text-foreground/80 hover:text-foreground transition-colors"
                  :title="`Go to ${tableLabel(fkMap[virtualColumn.column.name].table)}`"
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
    <ContextMenuSurface :show="gridMenu.show" :x="gridMenu.x" :y="gridMenu.y" @dismiss="gridMenu.show = false">
      <div class="px-3 py-1 text-[11px] font-mono text-muted-foreground border-b mb-1">{{ gridMenu.column }}</div>
      <template v-if="gridMenu.row !== null">
        <button class="grid-menu-item" @click="copyCell">Copy value</button>
        <button class="grid-menu-item" @click="inspectCell(gridMenu.row, gridMenu.column)">Inspect value</button>
        <button v-if="canFilter" class="grid-menu-item" @click="filterCell">Filter by this value</button>
        <button class="grid-menu-item" @click="emit('row-contextmenu', gridMenu.row, gridMenu.x, gridMenu.y)">Row actions…</button>
      </template>
      <button class="grid-menu-item" @click="void copyText(gridMenu.column)">Copy column name</button>
      <button class="grid-menu-item" @click="sortDirection(false)">Sort ascending</button>
      <button class="grid-menu-item" @click="sortDirection(true)">Sort descending</button>
      <div class="border-t my-1" />
      <button class="grid-menu-item" @click="autoSize(gridMenu.column)">Fit width to content</button>
      <button class="grid-menu-item" @click="togglePinned(gridMenu.column)">{{ preferences.pinned.includes(gridMenu.column) ? 'Unpin column' : 'Pin column' }}</button>
      <button :disabled="displayColumns.length <= 1" class="grid-menu-item" @click="toggleHidden(gridMenu.column)">Hide column</button>
      <button class="grid-menu-item" @click="showColumns = true">Choose columns…</button>
      <button class="grid-menu-item" @click="resetColumns">Reset columns</button>
      <div class="border-t my-1" />
      <button class="grid-menu-item" @click="openExport(false)">Export loaded rows…</button>
      <button v-if="clientSide && selectedIndexes.length" class="grid-menu-item" @click="openExport(true)">Export selected rows…</button>
    </ContextMenuSurface>
    <ResultExportDialog :open="exportOpen" :columns="displayColumns" :rows="exportRows" @update:open="exportOpen = $event" />
    <ValueInspector :open="inspector.open" :title="inspector.column" :value="inspector.value" @update:open="inspector.open = $event" />
  </div>
</template>

<script setup lang="ts">
import { tableLabel } from '@/lib/tableReference'
import type { TableRef } from '@/types/database'

import { rowKey, isPrimaryKeyColumn, type PrimaryKey } from '@/lib/tableIdentity'

import { ref, computed, nextTick, watch, shallowRef } from 'vue'
import { useVirtualizer } from '@tanstack/vue-virtual'
import { ArrowUpIcon, ArrowDownIcon, ArrowUpDownIcon, ArrowRightIcon, DatabaseIcon } from 'lucide-vue-next'
import CanvasDataGrid from '@/components/CanvasDataGrid.vue'
import ResultExportDialog from './dialogs/ResultExportDialog.vue'
import ContextMenuSurface from './ContextMenuSurface.vue'
import ValueInspector from './ValueInspector.vue'
import { useAppSettings } from '@/composables/useAppSettings'
import { useGridPreferences } from '@/composables/useGridPreferences'
import { copyText } from '@/lib/clipboard'
import { rowValue } from '@/lib/rowAccess'

const props = defineProps<{
  preferenceKey?: string
  canFilter?: boolean
  clientSide?: boolean
  columns: any[]
  rows: any[]
  primaryKey: PrimaryKey
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
  fkMap: Record<string, { table: string | TableRef; column: string }>
  bottomInset?: number
  isColAutoIncrement: (colName: string) => boolean
  isBooleanCol: (colName: string) => boolean
  getCellValue: (row: any, colName: string) => string
}>()

const isTestEnvironment = typeof process !== 'undefined' && process.env.NODE_ENV === 'test'
const canvasSupported = !isTestEnvironment && typeof HTMLCanvasElement !== 'undefined' && (() => { try { return !!document.createElement('canvas').getContext('2d') } catch { return false } })()

const emit = defineEmits<{
  'row-click': [row: any, e: MouseEvent, index: number, colName?: string]
  'cell-dblclick': [row: any, colName: string]
  'cell-blur': []
  'cell-input': [row: any, colName: string, value: string]
  'sort': [colName: string]
  'sort-direction': [column: string, desc: boolean]
  'filter-value': [column: string, value: unknown]
  'start-col-resize': [e: MouseEvent, colName: string]
  'navigate-related': [table: string | TableRef, column: string, value: any]
  'insert-row-input': [colName: string, value: string]
  'insert-row-submit': []
  'insert-row-cancel': []
  'pending-insert-input': [index: number, colName: string, value: string]
  'pending-insert-cancel': [index: number]
  'row-contextmenu': [row: any, x: number, y: number]
  'delete-key-pressed': []
}>()

const DEFAULT_COLUMN_WIDTH = 180

const { settings } = useAppSettings()
const { preferences, rowHeight } = useGridPreferences(() => props.preferenceKey)
const exportOpen = ref(false), exportRows = shallowRef<any[]>([]), selectedIndexes = ref<number[]>([])
let selectionAnchor = 0
function gridRowClick(row: any, event: MouseEvent, index: number, column?: string) {
  if (props.clientSide) {
    if (event.shiftKey) selectedIndexes.value = Array.from({ length: Math.abs(index - selectionAnchor) + 1 }, (_, offset) => Math.min(index, selectionAnchor) + offset)
    else if (event.metaKey || event.ctrlKey) { selectedIndexes.value = selectedIndexes.value.includes(index) ? selectedIndexes.value.filter(item => item !== index) : [...selectedIndexes.value, index]; selectionAnchor = index }
    else { selectedIndexes.value = [index]; selectionAnchor = index }
  }
  emit('row-click', row, event, index, column)
}
function openExport(selected: boolean) {
  const rows = selected ? selectedIndexes.value.map(index => displayRows.value[index]).filter(Boolean) : displayRows.value
  exportRows.value = rows.map(row => Object.fromEntries(displayColumns.value.map(column => [column.name, cellValue(row, column.name)])))
  exportOpen.value = true
}
const displaySelectedKey = computed(() => props.clientSide && selectedIndexes.value.length ? `__row_index:${selectedIndexes.value.at(-1)}` : props.selectedRowPk)
const displaySelectedKeys = computed(() => props.clientSide ? selectedIndexes.value.map(index => `__row_index:${index}`) : props.selectedRowPks)
const showColumns = ref(false)
const gridMenu = ref<{ show: boolean; row: any; column: string; x: number; y: number }>({ show: false, row: null, column: '', x: 0, y: 0 })
const inspector = ref({ open: false, column: '', value: null as unknown })
const clientSort = ref<{ column: string; desc: boolean } | null>(null)
const orderedColumns = computed(() => {
  const order = preferences.value.order
  return [...props.columns].sort((a, b) => {
    const ai = order.indexOf(a.name), bi = order.indexOf(b.name)
    return (ai < 0 ? order.length + props.columns.indexOf(a) : ai) - (bi < 0 ? order.length + props.columns.indexOf(b) : bi)
  })
})
const displayColumns = computed(() => {
  const visible = orderedColumns.value.filter(column => !preferences.value.hidden.includes(column.name))
  const columns = visible.length ? visible : orderedColumns.value.slice(0, 1)
  return [...columns.filter(column => preferences.value.pinned.includes(column.name)), ...columns.filter(column => !preferences.value.pinned.includes(column.name))]
})
watch(clientSort, () => { selectedIndexes.value = [] })
const displayRows = computed(() => {
  if (!props.clientSide || !clientSort.value) return props.rows
  const { column, desc } = clientSort.value
  return [...props.rows].sort((a, b) => {
    const av = rowValue(a, column, props.columns), bv = rowValue(b, column, props.columns)
    const comparison = av == null ? (bv == null ? 0 : -1) : bv == null ? 1 : typeof av === 'number' && typeof bv === 'number' ? av - bv : String(av).localeCompare(String(bv), undefined, { numeric: true })
    return desc ? -comparison : comparison
  })
})
function fittedWidth(column: string) {
  const values = props.rows.slice(0, 500).map(row => props.getCellValue(row, column))
  return Math.max(90, Math.min(600, Math.max(column.length, ...values.map(value => String(value).split('\n')[0].length)) * 8 + 32))
}
const effectiveWidths = computed(() => ({
  ...Object.fromEntries(props.columns.map(column => [column.name, settings.value.columnWidth === 'fit' ? fittedWidth(column.name) : { narrow: 100, normal: 180, wide: 260 }[settings.value.columnWidth]])),
  ...props.columnWidths, ...preferences.value.widths,
}))
const renderProps = computed(() => ({ ...props, columns: displayColumns.value, sourceColumns: props.columns, rows: displayRows.value, selectedRowPk: displaySelectedKey.value, selectedRowPks: displaySelectedKeys.value, columnWidths: effectiveWidths.value, rowHeight: rowHeight.value, pinnedColumns: preferences.value.pinned, sortColumn: clientSort.value?.column ?? props.sortColumn, sortDesc: clientSort.value?.desc ?? props.sortDesc }))
function toggleHidden(name: string) {
  const hidden = preferences.value.hidden
  if (hidden.includes(name)) preferences.value.hidden = hidden.filter(item => item !== name)
  else if (displayColumns.value.length > 1) hidden.push(name)
}
function togglePinned(name: string) {
  const pinned = preferences.value.pinned
  preferences.value.pinned = pinned.includes(name) ? pinned.filter(item => item !== name) : [...pinned, name]
}
function moveColumn(name: string, delta: number) {
  const order = orderedColumns.value.map(column => column.name)
  const index = order.indexOf(name), next = index + delta
  if (next < 0 || next >= order.length) return
  ;[order[index], order[next]] = [order[next], order[index]]
  preferences.value.order = order
}
function resetColumns() { preferences.value.hidden = []; preferences.value.pinned = []; preferences.value.order = []; preferences.value.widths = {} }
function cellValue(row: any, column: string) {
  const pending = props.pendingChanges[rowKey(row, props.primaryKey, props.columns)]
  return pending && Object.hasOwn(pending, column) ? pending[column] : rowValue(row, column, props.columns)
}
function openCellMenu(row: any, column: string, x: number, y: number) { gridMenu.value = { show: true, row, column, x, y } }
function openColumnMenu(column: string, x: number, y: number) { gridMenu.value = { show: true, row: null, column, x, y } }
function inspectCell(row: any, column: string) { inspector.value = { open: true, column, value: cellValue(row, column) } }
function copyCell() {
  const value = cellValue(gridMenu.value.row, gridMenu.value.column)
  void copyText(value === null ? 'NULL' : typeof value === 'object' ? JSON.stringify(value) : String(value ?? ''))
}
function filterCell() { emit('filter-value', gridMenu.value.column, cellValue(gridMenu.value.row, gridMenu.value.column)) }
function sortDirection(desc: boolean) {
  if (props.clientSide) clientSort.value = { column: gridMenu.value.column, desc }
  else emit('sort-direction', gridMenu.value.column, desc)
}
function sortColumnClick(column: string) {
  if (props.clientSide) clientSort.value = { column, desc: clientSort.value?.column === column ? !clientSort.value.desc : false }
  else emit('sort', column)
}
function resizeColumn(event: MouseEvent, column: string) {
  event.preventDefault(); event.stopPropagation()
  const startX = event.clientX, width = effectiveWidths.value[column] ?? DEFAULT_COLUMN_WIDTH
  const move = (next: MouseEvent) => { preferences.value.widths[column] = Math.max(60, width + next.clientX - startX) }
  const up = () => { window.removeEventListener('mousemove', move); window.removeEventListener('mouseup', up) }
  window.addEventListener('mousemove', move); window.addEventListener('mouseup', up)
}
function autoSize(column: string) { preferences.value.widths[column] = fittedWidth(column) }

function rawCellValue(row: any, column: string) {
  return rowValue(row, column, props.columns)
}

function columnWidth(colName: string) {
  return effectiveWidths.value[colName] ?? DEFAULT_COLUMN_WIDTH
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

const HEADER_HEIGHT = 28
const ROW_HEIGHT = rowHeight

const virtualizer = useVirtualizer(computed(() => ({
  count: displayRows.value.length,
  getScrollElement: () => scrollContainer.value,
  estimateSize: () => ROW_HEIGHT.value,
  overscan: 4,
})))

const virtualRows = computed(() => virtualizer.value.getVirtualItems())
const totalSize = computed(() => virtualizer.value.getTotalSize())

const columnVirtualizer = useVirtualizer(computed(() => ({
  horizontal: true,
  count: displayColumns.value.length,
  getScrollElement: () => scrollContainer.value,
  estimateSize: (index: number) => columnWidth(displayColumns.value[index]?.name ?? ''),
  overscan: 1,
})))

const domScrollLeft = ref(0)
const virtualColumns = computed(() => {
  const items = columnVirtualizer.value.getVirtualItems().map(item => ({ ...item, column: displayColumns.value[item.index] })).filter(item => item.column)
  let start = 0
  const pinned = displayColumns.value.flatMap((column, index) => {
    const size = columnWidth(column.name), item = { index, key: index, start, end: start + size, size, lane: 0, column }
    start += size
    return preferences.value.pinned.includes(column.name) ? [item] : []
  })
  return [...items.filter(item => !preferences.value.pinned.includes(item.column.name)), ...pinned]
})
watch([effectiveWidths, displayColumns], () => columnVirtualizer.value.measure())
watch(rowHeight, () => virtualizer.value.measure())
const totalColumnSize = computed(() => columnVirtualizer.value.getTotalSize())
const paddingBottom = computed(() => props.bottomInset ?? 0)
const bodyHeight = computed(() =>
  totalSize.value + props.pendingInserts.length * ROW_HEIGHT.value + paddingBottom.value
)

function columnPositionStyle(
  virtualColumn: { start: number; column: { name: string } },
  height = ROW_HEIGHT.value,
) {
  return {
    ...columnStyle(virtualColumn.column.name),
    left: (virtualColumn.start + (preferences.value.pinned.includes(virtualColumn.column.name) ? domScrollLeft.value : 0)) + 'px',
    zIndex: preferences.value.pinned.includes(virtualColumn.column.name) ? 2 : 0,
    background: preferences.value.pinned.includes(virtualColumn.column.name) ? 'var(--bg-1)' : undefined,
    height: height + 'px',
  }
}

function rowPositionStyle(start: number) {
  return {
    transform: `translateY(${start}px)`,
    width: totalColumnSize.value + 'px',
    height: ROW_HEIGHT.value + 'px',
  }
}

function virtualRowKey(virtualRow: any) {
  const row = displayRows.value[virtualRow.index]
  return props.primaryKey ? rowKey(row, props.primaryKey, props.columns) : virtualRow.index
}

function rowSelectionKey(row: any, index: number) {
  return props.primaryKey ? rowKey(row, props.primaryKey, props.columns) : `__row_index:${index}`
}

function isPkRow(row: any, index: number) {
  return displaySelectedKey.value === rowSelectionKey(row, index)
}

function isMultiSelected(row: any) {
  if (!props.primaryKey) return false
  return props.selectedRowPks.includes(rowKey(row, props.primaryKey, props.columns))
}

function isPendingDelete(row: any) {
  return !!props.pendingDeletions[rowKey(row, props.primaryKey, props.columns)]
}

function isPendingChange(row: any, col: string) {
  return props.pendingChanges[rowKey(row, props.primaryKey, props.columns)]?.[col] !== undefined
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
  if (settings.value.alternatingRows && index % 2 === 1) return 'cursor-pointer bg-muted/50 hover:bg-accent/70'
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

<style scoped>
.grid-menu-item { display: block; width: 100%; padding: 7px 12px; text-align: left; font-size: 12px; border-radius: 4px; }
.grid-menu-item:hover:not(:disabled) { background: var(--muted); }
</style>
