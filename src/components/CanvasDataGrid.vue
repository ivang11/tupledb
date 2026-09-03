<template>
  <div
    ref="scrollContainer"
    class="flex-1 min-w-0 relative overflow-auto bg-background custom-scrollbar"
    tabindex="0"
    @scroll.passive="onScroll"
    @keydown.delete="onDeleteKeydown"
  >
    <div
      v-if="rows.length === 0 && pendingInserts.length === 0"
      class="absolute inset-0 flex flex-col items-center justify-center gap-3 text-center p-12"
    >
      <DatabaseIcon class="size-8 text-muted-foreground/15" />
      <div>
        <p class="text-sm font-semibold text-foreground/60">No records</p>
        <p class="text-xs text-muted-foreground/40 mt-1">This table is empty or your filters didn't match any rows.</p>
      </div>
    </div>

    <template v-else>
      <div class="sticky left-0 top-0 z-20 h-0 w-0 overflow-visible">
        <canvas
          ref="canvas"
          aria-hidden="true"
          class="absolute left-0 top-0 max-w-none select-none"
          :style="canvasStyle"
          @click="onCanvasClick"
          @dblclick="onCanvasDoubleClick"
          @contextmenu="onCanvasContextMenu"
          @mousemove="onCanvasMouseMove"
          @mouseleave="onCanvasMouseLeave"
          @mousedown="onCanvasMouseDown"
        />

        <input
          v-if="inlineEditor"
          :key="`${inlineEditor.rowIndex}:${inlineEditor.column.name}`"
          :data-grid-edit="inlineEditor.rowKey"
          :data-col="inlineEditor.column.name"
          :value="inlineEditor.value"
          :style="inlineEditor.style"
          class="absolute z-30 bg-background border border-primary/50 px-2 text-xs font-mono focus:outline-none focus:ring-1 focus:ring-primary/50"
          @input="emit('cell-input', inlineEditor.row, inlineEditor.column.name, ($event.target as HTMLInputElement).value)"
          @blur="emit('cell-blur')"
          @keydown.delete.stop
          @keydown.backspace.stop
          @click.stop
          autofocus
        />

        <template v-for="editor in pendingEditors" :key="editor.key">
          <span
            v-if="editor.autoIncrement"
            :style="editor.style"
            class="absolute z-30 px-2 flex items-center text-[10px] text-emerald-500/70 italic font-mono"
          >new</span>
          <input
            v-else
            :data-pending-insert-index="editor.insertIndex"
            :value="editor.value"
            :placeholder="editor.boolean ? '0 / 1' : ''"
            :style="editor.style"
            class="insert-row-input pending-insert-input absolute z-30 bg-background/95 px-2 text-xs font-mono focus:outline-none focus:bg-emerald-500/10 focus:ring-1 focus:ring-emerald-500/40"
            @input="emit('pending-insert-input', editor.insertIndex, editor.column.name, ($event.target as HTMLInputElement).value)"
            @keydown.delete.stop
            @keydown.backspace.stop
            @keydown.escape="emit('pending-insert-cancel', editor.insertIndex)"
          />
        </template>
      </div>

      <div data-grid-scroll-sizer aria-hidden="true" :style="sizerStyle" />

      <div
        class="sr-only"
        role="table"
        :aria-colcount="columns.length"
        :aria-rowcount="rows.length + pendingInserts.length"
        aria-label="Database results"
      >
        <div role="rowgroup">
          <div role="row">
            <div
              v-for="column in ariaColumns"
              :key="column.index"
              role="columnheader"
              :aria-colindex="column.index + 1"
            >{{ column.name }} {{ column.type_name }}</div>
          </div>
        </div>
        <div role="rowgroup">
          <div
            v-for="ariaRow in ariaRows"
            :key="ariaRow.index"
            role="row"
            :aria-rowindex="ariaRow.index + 2"
          >
            <div
              v-for="cell in ariaRow.cells"
              :key="cell.columnIndex"
              role="cell"
              :aria-colindex="cell.columnIndex + 1"
            >{{ cell.value }}</div>
          </div>
        </div>
      </div>

      <div
        v-if="rows.length > 0 && !primaryKey"
        class="sticky left-0 w-fit px-4 py-2.5 border-t border-dashed border-border/40 bg-muted/10"
      >
        <p class="text-[10px] text-muted-foreground/40 italic">Editing is disabled — this table has no primary key.</p>
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import {
  computed,
  nextTick,
  onBeforeUnmount,
  onMounted,
  onUpdated,
  ref,
  watch,
} from 'vue'
import { DatabaseIcon } from 'lucide-vue-next'
import { rowValue } from '@/lib/rowAccess'

interface GridColumn {
  name: string
  type_name?: string
  [key: string]: unknown
}

interface PendingInsert {
  values: Array<{ column: string; value: unknown }>
}

const props = defineProps<{
  columns: GridColumn[]
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
  pendingInserts: PendingInsert[]
  columnWidths: Record<string, number>
  fkMap: Record<string, { table: string; column: string }>
  bottomInset?: number
  isColAutoIncrement: (colName: string) => boolean
  isBooleanCol: (colName: string) => boolean
  getCellValue: (row: any, colName: string) => string
}>()

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

const HEADER_HEIGHT = 48
const ROW_HEIGHT = 34
const DEFAULT_COLUMN_WIDTH = 180
const COLUMN_OVERSCAN = 1
const ROW_OVERSCAN = 2
const RESIZE_HIT_WIDTH = 5
const MAX_NATIVE_SCROLL_WIDTH = 24_000

const scrollContainer = ref<HTMLElement | null>(null)
const canvas = ref<HTMLCanvasElement | null>(null)
const viewportWidth = ref(0)
const viewportHeight = ref(0)
const scrollLeft = ref(0)
const scrollTop = ref(0)
const hoveredRow = ref<number | null>(null)
const hoveredColumn = ref<number | null>(null)
const pointerX = ref(-10_000)
const ariaRange = ref({ rowStart: 0, rowEnd: 0, columnStart: 0, columnEnd: 0 })

let context: CanvasRenderingContext2D | null = null
let resizeObserver: ResizeObserver | null = null
let drawFrame: number | null = null
let ariaTimer: ReturnType<typeof setTimeout> | null = null
let cachedColors: CanvasGridColors | null = null

interface CanvasGridColors {
  background: string
  muted: string
  accent: string
  foreground: string
  secondary: string
  faint: string
  border: string
  primary: string
  primarySoft: string
  destructive: string
  amber: string
  mono: string
}

const columnLayout = computed(() => {
  let start = 0
  return props.columns.map((column, index) => {
    const size = props.columnWidths[column.name] ?? DEFAULT_COLUMN_WIDTH
    const item = { column, index, start, end: start + size, size }
    start += size
    return item
  })
})
const totalColumnSize = computed(() => columnLayout.value.at(-1)?.end ?? 0)
const nativeColumnSize = computed(() => Math.min(totalColumnSize.value, MAX_NATIVE_SCROLL_WIDTH))
const horizontalScrollScale = computed(() => {
  const logicalRange = Math.max(0, totalColumnSize.value - viewportWidth.value)
  const nativeRange = Math.max(0, nativeColumnSize.value - viewportWidth.value)
  return nativeRange > 0 ? logicalRange / nativeRange : 1
})
const totalRows = computed(() => props.rows.length + props.pendingInserts.length)
const totalHeight = computed(() =>
  HEADER_HEIGHT + totalRows.value * ROW_HEIGHT + (props.bottomInset ?? 0),
)
const sizerStyle = computed(() => ({
  width: `${Math.max(nativeColumnSize.value, viewportWidth.value)}px`,
  height: `${Math.max(totalHeight.value, viewportHeight.value)}px`,
}))
const canvasStyle = computed(() => ({
  width: `${viewportWidth.value}px`,
  height: `${viewportHeight.value}px`,
  cursor: hoveredResizeColumn() !== null ? 'col-resize' : 'default',
}))

function rawCellValue(row: any, column: string) {
  return rowValue(row, column, props.columns)
}

function lowerBoundColumn(offset: number) {
  const layout = columnLayout.value
  let low = 0
  let high = layout.length
  while (low < high) {
    const middle = (low + high) >>> 1
    if (layout[middle].end <= offset) low = middle + 1
    else high = middle
  }
  return Math.min(low, Math.max(0, layout.length - 1))
}

function visibleRange() {
  const columnStart = Math.max(0, lowerBoundColumn(scrollLeft.value) - COLUMN_OVERSCAN)
  const columnEnd = Math.min(
    props.columns.length,
    lowerBoundColumn(scrollLeft.value + viewportWidth.value) + 1 + COLUMN_OVERSCAN,
  )
  const rowStart = Math.max(0, Math.floor(scrollTop.value / ROW_HEIGHT) - ROW_OVERSCAN)
  const rowEnd = Math.min(
    totalRows.value,
    Math.ceil((scrollTop.value + Math.max(0, viewportHeight.value - HEADER_HEIGHT)) / ROW_HEIGHT) + ROW_OVERSCAN,
  )
  return { columnStart, columnEnd, rowStart, rowEnd }
}

function resolveColors(): CanvasGridColors {
  const element = scrollContainer.value
  if (!element) {
    return cachedColors ?? {
      background: '#202024',
      muted: '#27272c',
      accent: '#34343a',
      foreground: '#f1f1f2',
      secondary: '#8b8b92',
      faint: '#68686f',
      border: 'rgba(128,128,128,.22)',
      primary: '#2f9e68',
      primarySoft: 'rgba(47,158,104,.14)',
      destructive: '#e26d5c',
      amber: '#d6ad57',
      mono: 'ui-monospace, monospace',
    }
  }
  const style = getComputedStyle(element)
  const token = (name: string, fallback: string) => style.getPropertyValue(name).trim() || fallback
  return {
    background: token('--bg-0', '#202024'),
    muted: token('--bg-1', '#27272c'),
    accent: token('--bg-3', '#34343a'),
    foreground: token('--fg-1', '#f1f1f2'),
    secondary: token('--fg-3', '#8b8b92'),
    faint: token('--fg-4', '#68686f'),
    border: token('--line-2', 'rgba(128,128,128,.22)'),
    primary: token('--acc', '#2f9e68'),
    primarySoft: token('--acc-soft', 'rgba(47,158,104,.14)'),
    destructive: token('--destructive', '#e26d5c'),
    amber: token('--pk', '#d6ad57'),
    mono: token('--font-mono', 'ui-monospace, monospace'),
  }
}

function refreshColors() {
  cachedColors = resolveColors()
}

function rowSelectionKey(row: any, index: number) {
  return props.primaryKey ? String(rawCellValue(row, props.primaryKey)) : `__row_index:${index}`
}

function isSelected(row: any, index: number) {
  const key = rowSelectionKey(row, index)
  return props.selectedRowPk === key || props.selectedRowPks.includes(key)
}

function isDeleted(row: any) {
  return !!props.pendingDeletions[String(rawCellValue(row, props.primaryKey || ''))]
}

function isChanged(row: any, column: string) {
  return props.pendingChanges[String(rawCellValue(row, props.primaryKey || ''))]?.[column] !== undefined
}

function truncate(value: string, width: number) {
  const maxCharacters = Math.max(1, Math.floor((width - 24) / 7.7))
  return value.length > maxCharacters ? `${value.slice(0, Math.max(0, maxCharacters - 1))}…` : value
}

function drawHeader(ctx: CanvasRenderingContext2D, range: ReturnType<typeof visibleRange>, colors: CanvasGridColors) {
  ctx.fillStyle = colors.muted
  ctx.fillRect(0, 0, viewportWidth.value, HEADER_HEIGHT)
  ctx.textBaseline = 'middle'

  for (let index = range.columnStart; index < range.columnEnd; index += 1) {
    const item = columnLayout.value[index]
    const x = item.start - scrollLeft.value
    if (props.sortColumn === item.column.name) {
      ctx.fillStyle = colors.primarySoft
      ctx.fillRect(x, 0, item.size, HEADER_HEIGHT)
    }
    ctx.fillStyle = colors.foreground
    ctx.font = `700 12px ${colors.mono}`
    const suffix = props.primaryKey === item.column.name ? '  PK' : ''
    ctx.fillText(truncate(`${item.column.name}${suffix}`, item.size), x + 12, 17)
    ctx.fillStyle = colors.secondary
    ctx.font = `600 9px ${colors.mono}`
    const direction = props.sortColumn === item.column.name ? (props.sortDesc ? '  ↓' : '  ↑') : ''
    ctx.fillText(truncate(`${item.column.type_name ?? ''}${direction}`, item.size), x + 12, 36)
    ctx.strokeStyle = colors.border
    ctx.beginPath()
    ctx.moveTo(Math.round(x + item.size) + 0.5, 0)
    ctx.lineTo(Math.round(x + item.size) + 0.5, HEADER_HEIGHT)
    ctx.stroke()
  }
  ctx.strokeStyle = colors.border
  ctx.beginPath()
  ctx.moveTo(0, HEADER_HEIGHT - 0.5)
  ctx.lineTo(viewportWidth.value, HEADER_HEIGHT - 0.5)
  ctx.stroke()
}

function drawStoredRow(
  ctx: CanvasRenderingContext2D,
  row: any,
  rowIndex: number,
  y: number,
  range: ReturnType<typeof visibleRange>,
  colors: CanvasGridColors,
) {
  const selected = isSelected(row, rowIndex)
  const deleted = isDeleted(row)
  ctx.fillStyle = props.pendingDrop || deleted
    ? 'rgba(180,55,55,.13)'
    : props.pendingTruncate
      ? 'rgba(210,155,45,.13)'
      : selected
        ? colors.primarySoft
        : hoveredRow.value === rowIndex
          ? colors.accent
          : rowIndex % 2 === 1 ? colors.muted : colors.background
  ctx.fillRect(0, y, viewportWidth.value, ROW_HEIGHT)

  ctx.textBaseline = 'middle'
  ctx.font = `13px ${colors.mono}`
  for (let columnIndex = range.columnStart; columnIndex < range.columnEnd; columnIndex += 1) {
    const item = columnLayout.value[columnIndex]
    const x = item.start - scrollLeft.value
    const changed = isChanged(row, item.column.name)
    if (changed) {
      ctx.fillStyle = 'rgba(210,155,45,.10)'
      ctx.fillRect(x, y, item.size, ROW_HEIGHT)
    }
    const rawValue = rawCellValue(row, item.column.name)
    const value = props.getCellValue(row, item.column.name)
    ctx.fillStyle = deleted || props.pendingDrop
      ? colors.destructive
      : changed ? colors.amber : rawValue == null || value === '' ? colors.faint : colors.foreground
    ctx.fillText(truncate(rawValue == null ? 'NULL' : value === '' ? 'EMPTY' : value, item.size), x + 12, y + ROW_HEIGHT / 2)
    if (props.fkMap[item.column.name] && rawValue != null) {
      ctx.fillStyle = colors.secondary
      ctx.fillText('›', x + item.size - 17, y + ROW_HEIGHT / 2)
    }
    ctx.strokeStyle = colors.border
    ctx.beginPath()
    ctx.moveTo(Math.round(x + item.size) + 0.5, y)
    ctx.lineTo(Math.round(x + item.size) + 0.5, y + ROW_HEIGHT)
    ctx.stroke()
  }
  ctx.strokeStyle = colors.border
  ctx.beginPath()
  ctx.moveTo(0, Math.round(y + ROW_HEIGHT) + 0.5)
  ctx.lineTo(viewportWidth.value, Math.round(y + ROW_HEIGHT) + 0.5)
  ctx.stroke()
  if (selected) {
    ctx.fillStyle = colors.primary
    ctx.fillRect(0, y, 2, ROW_HEIGHT)
  }
}

function drawPendingRow(ctx: CanvasRenderingContext2D, y: number) {
  ctx.fillStyle = 'rgba(35,160,95,.12)'
  ctx.fillRect(0, y, viewportWidth.value, ROW_HEIGHT)
}

function draw() {
  drawFrame = null
  const element = canvas.value
  const container = scrollContainer.value
  if (!element || !container || !context || viewportWidth.value <= 0 || viewportHeight.value <= 0) return
  const ratio = window.devicePixelRatio || 1
  const pixelWidth = Math.max(1, Math.round(viewportWidth.value * ratio))
  const pixelHeight = Math.max(1, Math.round(viewportHeight.value * ratio))
  if (element.width !== pixelWidth || element.height !== pixelHeight) {
    element.width = pixelWidth
    element.height = pixelHeight
  }
  const ctx = context
  ctx.setTransform(ratio, 0, 0, ratio, 0, 0)
  const colors = cachedColors ?? resolveColors()
  ctx.fillStyle = colors.background
  ctx.fillRect(0, 0, viewportWidth.value, viewportHeight.value)
  const range = visibleRange()

  ctx.save()
  ctx.beginPath()
  ctx.rect(0, HEADER_HEIGHT, viewportWidth.value, Math.max(0, viewportHeight.value - HEADER_HEIGHT))
  ctx.clip()
  for (let index = range.rowStart; index < range.rowEnd; index += 1) {
    const y = HEADER_HEIGHT + index * ROW_HEIGHT - scrollTop.value
    if (index < props.rows.length) drawStoredRow(ctx, props.rows[index], index, y, range, colors)
    else drawPendingRow(ctx, y)
  }
  ctx.restore()
  drawHeader(ctx, range, colors)
}

function scheduleDraw() {
  if (drawFrame !== null) return
  drawFrame = requestAnimationFrame(draw)
}

function updateViewport() {
  const container = scrollContainer.value
  if (!container) return
  refreshColors()
  viewportWidth.value = container.clientWidth
  viewportHeight.value = container.clientHeight
  scrollLeft.value = container.scrollLeft * horizontalScrollScale.value
  scrollTop.value = container.scrollTop
  scheduleDraw()
  scheduleAriaUpdate(0)
}

function scheduleAriaUpdate(delay = 160) {
  if (ariaTimer !== null) clearTimeout(ariaTimer)
  ariaTimer = setTimeout(() => {
    ariaTimer = null
    ariaRange.value = visibleRange()
  }, delay)
}

function onScroll() {
  const container = scrollContainer.value
  if (!container) return
  scrollLeft.value = container.scrollLeft * horizontalScrollScale.value
  scrollTop.value = container.scrollTop
  scheduleDraw()
  scheduleAriaUpdate()
}

function localPoint(event: MouseEvent) {
  const rect = canvas.value!.getBoundingClientRect()
  return { x: event.clientX - rect.left, y: event.clientY - rect.top }
}

function columnAtViewportX(x: number) {
  if (!props.columns.length) return null
  const index = lowerBoundColumn(x + scrollLeft.value)
  return columnLayout.value[index] ?? null
}

function rowAtViewportY(y: number) {
  if (y < HEADER_HEIGHT) return null
  const index = Math.floor((y + scrollTop.value - HEADER_HEIGHT) / ROW_HEIGHT)
  return index >= 0 && index < props.rows.length ? { index, row: props.rows[index] } : null
}

function hoveredResizeColumn() {
  if (!canvas.value) return null
  const logicalPointer = pointerX.value + scrollLeft.value
  const index = lowerBoundColumn(logicalPointer)
  for (const candidate of [columnLayout.value[index], columnLayout.value[index - 1]]) {
    if (candidate && Math.abs(logicalPointer - candidate.end) <= RESIZE_HIT_WIDTH) return candidate
  }
  return null
}

function onCanvasMouseMove(event: MouseEvent) {
  const point = localPoint(event)
  pointerX.value = point.x
  const column = columnAtViewportX(point.x)
  const row = rowAtViewportY(point.y)
  const nextColumn = column?.index ?? null
  const nextRow = row?.index ?? null
  if (hoveredColumn.value !== nextColumn || hoveredRow.value !== nextRow) {
    hoveredColumn.value = nextColumn
    hoveredRow.value = nextRow
    scheduleDraw()
  }
}

function onCanvasMouseLeave() {
  hoveredColumn.value = null
  hoveredRow.value = null
  pointerX.value = -10_000
  scheduleDraw()
}

function onCanvasMouseDown(event: MouseEvent) {
  const point = localPoint(event)
  if (point.y >= HEADER_HEIGHT) return
  const resizeColumn = hoveredResizeColumn()
  if (!resizeColumn) return
  event.preventDefault()
  event.stopPropagation()
  emit('start-col-resize', event, resizeColumn.column.name)
}

function onCanvasClick(event: MouseEvent) {
  const point = localPoint(event)
  const column = columnAtViewportX(point.x)
  if (!column) return
  if (point.y < HEADER_HEIGHT) {
    if (!hoveredResizeColumn()) emit('sort', column.column.name)
    return
  }
  const hit = rowAtViewportY(point.y)
  if (!hit) return
  const fk = props.fkMap[column.column.name]
  const xWithinColumn = point.x + scrollLeft.value - column.start
  const value = rawCellValue(hit.row, column.column.name)
  if (fk && value != null && xWithinColumn >= column.size - 28) {
    emit('navigate-related', fk.table, fk.column, value)
    return
  }
  emit('row-click', hit.row, event, hit.index, column.column.name)
}

function onCanvasDoubleClick(event: MouseEvent) {
  const point = localPoint(event)
  const column = columnAtViewportX(point.x)
  const hit = rowAtViewportY(point.y)
  if (!column || !hit || !props.primaryKey || props.pendingTruncate || props.pendingDrop || isDeleted(hit.row)) return
  emit('cell-dblclick', hit.row, column.column.name)
}

function onCanvasContextMenu(event: MouseEvent) {
  const point = localPoint(event)
  const hit = rowAtViewportY(point.y)
  if (!hit) return
  event.preventDefault()
  event.stopPropagation()
  emit('row-contextmenu', hit.row, event.clientX, event.clientY)
}

function isEditableTarget(target: EventTarget | null) {
  if (!(target instanceof HTMLElement)) return false
  return target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.isContentEditable
}

function onDeleteKeydown(event: KeyboardEvent) {
  if (isEditableTarget(event.target)) return
  event.stopPropagation()
  emit('delete-key-pressed')
}

function pendingValue(insert: PendingInsert, columnName: string) {
  const value = insert.values.find(item => item.column === columnName)?.value
  return value == null ? '' : String(value)
}

const inlineEditor = computed(() => {
  if (!props.primaryKey || !props.inlineEditColumn || props.selectedRowPk === null) return null
  const rowIndex = props.rows.findIndex(row => String(rawCellValue(row, props.primaryKey!)) === props.selectedRowPk)
  const column = columnLayout.value.find(item => item.column.name === props.inlineEditColumn)
  if (rowIndex < 0 || !column) return null
  const left = column.start - scrollLeft.value
  const top = HEADER_HEIGHT + rowIndex * ROW_HEIGHT - scrollTop.value
  if (left + column.size < 0 || left > viewportWidth.value || top + ROW_HEIGHT < HEADER_HEIGHT || top > viewportHeight.value) return null
  const row = props.rows[rowIndex]
  return {
    row,
    rowIndex,
    rowKey: String(rawCellValue(row, props.primaryKey)),
    column: column.column,
    value: props.getCellValue(row, column.column.name),
    style: {
      left: `${left}px`,
      top: `${top}px`,
      width: `${column.size}px`,
      height: `${ROW_HEIGHT}px`,
    },
  }
})

const pendingEditors = computed(() => {
  if (!props.pendingInserts.length) return []
  const range = visibleRange()
  return props.pendingInserts.flatMap((insert, insertIndex) => {
    const rowIndex = props.rows.length + insertIndex
    const top = HEADER_HEIGHT + rowIndex * ROW_HEIGHT - scrollTop.value
    if (top + ROW_HEIGHT < HEADER_HEIGHT || top > viewportHeight.value) return []
    return columnLayout.value.slice(range.columnStart, range.columnEnd).map(item => ({
      key: `${insertIndex}:${item.column.name}`,
      insertIndex,
      column: item.column,
      value: pendingValue(insert, item.column.name),
      autoIncrement: props.isColAutoIncrement(item.column.name),
      boolean: props.isBooleanCol(item.column.name),
      style: {
        left: `${item.start - scrollLeft.value}px`,
        top: `${top}px`,
        width: `${item.size}px`,
        height: `${ROW_HEIGHT}px`,
      },
    }))
  })
})

const ariaColumns = computed(() =>
  props.columns.slice(ariaRange.value.columnStart, ariaRange.value.columnEnd).map((column, offset) => ({
    ...column,
    index: ariaRange.value.columnStart + offset,
  })),
)
const ariaRows = computed(() => {
  const end = Math.min(props.rows.length, ariaRange.value.rowEnd)
  return props.rows.slice(ariaRange.value.rowStart, end).map((row, offset) => {
    const index = ariaRange.value.rowStart + offset
    return {
      index,
      cells: ariaColumns.value.map(column => ({
        columnIndex: column.index,
        value: props.getCellValue(row, column.name),
      })),
    }
  })
})

watch(
  () => props.pendingInserts.length,
  (nextCount, previousCount) => {
    if (nextCount <= previousCount) return
    nextTick(() => {
      const container = scrollContainer.value
      if (!container) return
      container.scrollTop = container.scrollHeight
      scheduleDraw()
      nextTick(() => container.querySelector<HTMLInputElement>(`[data-pending-insert-index="${previousCount}"]`)?.focus())
    })
  },
)

watch(
  () => [props.rows, props.columns, props.columnWidths, props.selectedRowPk, props.selectedRowPks, props.sortColumn, props.sortDesc],
  () => {
    scheduleDraw()
    scheduleAriaUpdate(0)
  },
)

onMounted(() => {
  const element = canvas.value
  if (!element) return
  context = element.getContext('2d', { alpha: false })
  if (typeof ResizeObserver !== 'undefined') {
    resizeObserver = new ResizeObserver(updateViewport)
    resizeObserver.observe(scrollContainer.value!)
  } else {
    window.addEventListener('resize', updateViewport)
  }
  updateViewport()
})

onUpdated(scheduleDraw)

onBeforeUnmount(() => {
  resizeObserver?.disconnect()
  window.removeEventListener('resize', updateViewport)
  if (drawFrame !== null) cancelAnimationFrame(drawFrame)
  if (ariaTimer !== null) clearTimeout(ariaTimer)
})
</script>
