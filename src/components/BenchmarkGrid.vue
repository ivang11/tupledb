<template>
  <main class="h-screen w-screen flex flex-col overflow-hidden bg-background">
    <header class="h-10 shrink-0 flex items-center justify-between border-b border-border px-4">
      <strong class="text-sm">TupleDB Tauri grid benchmark</strong>
      <span class="text-xs text-muted-foreground">{{ rowCount }} rows × {{ columnCount }} columns</span>
    </header>
    <DataGrid
      :columns="columns"
      :rows="rows"
      primary-key="id"
      :total-count="rowCount"
      :pending-changes="{}"
      :pending-deletions="{}"
      :pending-truncate="false"
      :pending-drop="false"
      :selected-row-pk="selectedRowPk"
      :selected-row-pks="selectedRowPks"
      :inline-edit-column="inlineEditColumn"
      :sort-column="null"
      :sort-desc="false"
      :inserting-row="false"
      :insert-row-values="{}"
      :pending-inserts="[]"
      :column-widths="columnWidths"
      :fk-map="{}"
      :is-col-auto-increment="() => false"
      :is-boolean-col="column => column === 'enabled'"
      :get-cell-value="cellValue"
      @row-click="selectRow"
      @cell-dblclick="startEditing"
      @cell-blur="inlineEditColumn = null"
    />
  </main>
</template>

<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { nextTick, onMounted, ref } from 'vue'
import DataGrid from '@/components/DataGrid.vue'
import { rowValue } from '@/lib/rowAccess'

const props = defineProps<{ rowCount: number; columnCount: number; settleMs?: number }>()
const buildStarted = performance.now()

const columns = Array.from({ length: props.columnCount }, (_, index) => ({
  name: index === 0 ? 'id' : index === 1 ? 'name' : index === 2 ? 'enabled' : index === 3 ? 'nullable' : `extra_${index}`,
  type_name: index === 0 ? 'BIGINT' : index === 2 ? 'TINYINT' : 'VARCHAR',
}))
const rows = Array.from({ length: props.rowCount }, (_, rowIndex) =>
  columns.map((_column, columnIndex) =>
    columnIndex === 0
      ? rowIndex + 1
      : columnIndex === 1
        ? `Fixture row ${rowIndex + 1}`
        : columnIndex === 2
          ? rowIndex % 2
          : columnIndex === 3 && rowIndex % 3 === 0
            ? null
            : `r${rowIndex + 1}c${columnIndex}`,
  ),
)
const columnWidths = Object.fromEntries(columns.map(column => [column.name, 180]))
const fixtureBuildMs = performance.now() - buildStarted
const selectedRowPk = ref<string | null>(null)
const selectedRowPks = ref<string[]>([])
const inlineEditColumn = ref<string | null>(null)

function cellValue(row: Record<string, unknown> | unknown[], column: string) {
  const value = rowValue(row, column, columns)
  return value == null ? '' : typeof value === 'object' ? JSON.stringify(value) : String(value)
}

function selectRow(row: Record<string, unknown> | unknown[]) {
  const key = String(rowValue(row, 'id', columns))
  selectedRowPk.value = key
  selectedRowPks.value = [key]
}

function startEditing(row: Record<string, unknown> | unknown[], column: string) {
  selectRow(row)
  inlineEditColumn.value = column
}

function percentile(sorted: number[], fraction: number) {
  return sorted[Math.min(sorted.length - 1, Math.floor((sorted.length - 1) * fraction))] ?? 0
}

async function report(metrics: Array<[string, number]>) {
  await invoke('report_benchmark_metrics', { metrics })
}

onMounted(async () => {
  await nextTick()
  requestAnimationFrame(() => requestAnimationFrame(async () => {
    await report([
      ['first_paint_ms', performance.now()],
      ['fixture_build_ms', fixtureBuildMs],
    ])

    if (props.settleMs) {
      await new Promise(resolve => setTimeout(resolve, props.settleMs))
    }

    const scroller = document.querySelector<HTMLElement>('.custom-scrollbar')
    if (!scroller) return
    const deltas: number[] = []
    let previous = performance.now()
    let frame = 0
    const frameCount = 180

    const animate = async (timestamp: number) => {
      if (frame > 0) deltas.push(timestamp - previous)
      previous = timestamp
      const phase = frame / (frameCount - 1)
      const triangle = phase <= 0.5 ? phase * 2 : (1 - phase) * 2
      scroller.scrollLeft = scroller.scrollWidth * triangle
      frame += 1

      if (frame < frameCount) {
        requestAnimationFrame(animate)
        return
      }

      const sorted = deltas.slice().sort((left, right) => left - right)
      const maxFrameMs = sorted.at(-1) ?? 0
      const maxFrameIndex = deltas.indexOf(maxFrameMs)
      await report([
        ['frame_p50_ms', percentile(sorted, 0.50)],
        ['frame_p95_ms', percentile(sorted, 0.95)],
        ['frame_p99_ms', percentile(sorted, 0.99)],
        ['frame_max_ms', maxFrameMs],
        ['frame_max_index', maxFrameIndex],
        ['frame_max_phase', maxFrameIndex / (frameCount - 1)],
      ])
    }
    requestAnimationFrame(animate)
  }))
})
</script>
