<script setup lang="ts">
import { ref, computed } from 'vue'
import { useVirtualizer } from '@tanstack/vue-virtual'
import { ArrowUpIcon, ArrowDownIcon, ArrowUpDownIcon, ArrowRightIcon, DatabaseIcon } from 'lucide-vue-next'

const props = defineProps<{
  columns: any[]
  rows: any[]
  primaryKey: string | null
  totalCount: number
  pendingChanges: Record<string, Record<string, any>>
  pendingDeletions: Record<string, boolean>
  pendingTruncate: boolean
  selectedRowPk: string | null
  inlineEditColumn: string | null
  sortColumn: string | null
  sortDesc: boolean
  insertingRow: boolean
  insertRowValues: Record<string, string>
  columnWidths: Record<string, number>
  fkMap: Record<string, { table: string; column: string }>
  isColAutoIncrement: (colName: string) => boolean
  isBooleanCol: (colName: string) => boolean
  getCellValue: (row: any, colName: string) => string
}>()

const emit = defineEmits<{
  'row-click': [row: any, e: MouseEvent]
  'cell-dblclick': [row: any, colName: string]
  'cell-blur': []
  'cell-input': [row: any, colName: string, value: string]
  'sort': [colName: string]
  'start-col-resize': [e: MouseEvent, colName: string]
  'navigate-related': [table: string, column: string, value: any]
  'insert-row-input': [colName: string, value: string]
  'insert-row-submit': []
  'insert-row-cancel': []
}>()

function colStyle(colName: string) {
  const w = props.columnWidths[colName]
  return w ? { width: w + 'px', minWidth: w + 'px' } : { minWidth: '160px' }
}

function cellStyle(colName: string) {
  const w = props.columnWidths[colName]
  return w ? { width: w + 'px', maxWidth: w + 'px' } : { maxWidth: '280px' }
}

const scrollContainer = ref<HTMLElement | null>(null)

const ROW_HEIGHT = 34

const virtualizer = useVirtualizer(computed(() => ({
  count: props.rows.length,
  getScrollElement: () => scrollContainer.value,
  estimateSize: () => ROW_HEIGHT,
  overscan: 10,
})))

const virtualRows = computed(() => virtualizer.value.getVirtualItems())
const totalSize = computed(() => virtualizer.value.getTotalSize())

const paddingTop = computed(() =>
  virtualRows.value.length > 0 ? virtualRows.value[0].start : 0
)
const paddingBottom = computed(() =>
  virtualRows.value.length > 0
    ? totalSize.value - virtualRows.value[virtualRows.value.length - 1].end
    : 0
)

function rowKey(virtualRow: any) {
  const row = props.rows[virtualRow.index]
  return props.primaryKey ? String(row[props.primaryKey]) : virtualRow.index
}

function isPkRow(row: any) {
  return props.primaryKey && props.selectedRowPk === String(row[props.primaryKey])
}

function isPendingDelete(row: any) {
  return !!props.pendingDeletions[String(row[props.primaryKey || ''])]
}

function isPendingChange(row: any, col: string) {
  return props.pendingChanges[String(row[props.primaryKey || ''])]?.[col] !== undefined
}
</script>

<template>
  <div
    ref="scrollContainer"
    class="flex-1 min-w-0 relative overflow-auto bg-background custom-scrollbar"
  >
    <!-- Empty state -->
    <div
      v-if="rows && rows.length === 0"
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

    <table v-else class="data-grid w-max min-w-full border-collapse">
      <!-- ── Header ─────────────────────────────────────────────────────────── -->
      <thead>
        <tr>
          <th
            v-for="col in columns"
            :key="col.name"
            class="sticky top-0 z-20 bg-background border-b border-border text-left whitespace-nowrap cursor-pointer select-none group/th transition-colors"
            :class="sortColumn === col.name ? 'bg-primary/5' : 'hover:bg-muted/30'"
            :style="colStyle(col.name)"
            @click="emit('sort', col.name)"
          >
            <div class="flex items-center justify-between gap-2 px-3 py-2">
              <!-- Name + PK badge -->
              <div class="flex items-center gap-1.5 min-w-0">
                <span
                  class="text-xs font-bold font-mono truncate"
                  :class="sortColumn === col.name ? 'text-primary' : 'text-foreground/80'"
                >
                  {{ col.name }}
                </span>
                <span
                  v-if="primaryKey === col.name"
                  class="text-[8px] font-black text-amber-500/90 border border-amber-500/30 bg-amber-500/8 px-1 py-px rounded shrink-0 leading-none"
                >
                  PK
                </span>
              </div>

              <!-- Sort icon -->
              <ArrowDownIcon  v-if="sortColumn === col.name && sortDesc"  class="size-3 text-primary shrink-0" />
              <ArrowUpIcon    v-else-if="sortColumn === col.name"         class="size-3 text-primary shrink-0" />
              <ArrowUpDownIcon v-else                                      class="size-3 text-muted-foreground/25 shrink-0 opacity-0 group-hover/th:opacity-100 transition-opacity" />
            </div>

            <!-- Type label -->
            <div class="px-3 pb-1.5 -mt-1">
              <span class="text-[9px] font-medium font-mono text-muted-foreground/35 leading-none">
                {{ col.type_name }}
              </span>
            </div>

            <!-- Column resize handle -->
            <div
              class="group/resize absolute top-0 right-0 h-full w-2 cursor-col-resize z-30 flex items-stretch justify-end"
              @mousedown="emit('start-col-resize', $event, col.name)"
              @click.stop
            >
              <div class="w-px bg-border/40 group-hover/resize:bg-primary/60 transition-colors" />
            </div>
          </th>
        </tr>
      </thead>

      <!-- ── Body ──────────────────────────────────────────────────────────── -->
      <tbody>
        <!-- Top spacer -->
        <tr v-if="paddingTop > 0">
          <td :colspan="columns.length" :style="{ height: paddingTop + 'px', padding: 0, border: 'none' }" />
        </tr>

        <tr
          v-for="virtualRow in virtualRows"
          :key="rowKey(virtualRow)"
          class="group/row border-b border-border/50 transition-colors"
          :class="[
            pendingTruncate
              ? 'opacity-40 line-through bg-destructive/8'
              : isPendingDelete(rows[virtualRow.index])
                ? 'bg-destructive/10 text-destructive/70'
                : isPkRow(rows[virtualRow.index])
                  ? 'bg-primary/8'
                  : virtualRow.index % 2 === 1
                    ? 'bg-muted/20'
                    : 'bg-transparent',
            primaryKey ? 'cursor-pointer' : '',
            !pendingTruncate && !isPendingDelete(rows[virtualRow.index]) && !isPkRow(rows[virtualRow.index])
              ? 'hover:bg-primary/5'
              : '',
          ]"
          @click="emit('row-click', rows[virtualRow.index], $event)"
        >
          <td
            v-for="col in columns"
            :key="col.name"
            class="px-3 text-sm relative group/cell overflow-hidden border-r border-border/30 last:border-r-0"
            :style="[cellStyle(col.name), { height: ROW_HEIGHT + 'px' }]"
            :class="[
              isPendingChange(rows[virtualRow.index], col.name) ? 'bg-amber-500/10' : '',
              isPkRow(rows[virtualRow.index]) ? 'border-r-primary/10' : '',
            ]"
            @dblclick.stop="
              primaryKey
              && !isPendingDelete(rows[virtualRow.index])
              && emit('cell-dblclick', rows[virtualRow.index], col.name)
            "
          >
            <!-- Pending change left border -->
            <div
              v-if="isPendingChange(rows[virtualRow.index], col.name)"
              class="absolute left-0 top-0 bottom-0 w-0.5 bg-amber-400/70"
            />

            <!-- Selected row indicator (first col) -->
            <div
              v-if="isPkRow(rows[virtualRow.index]) && col.name === columns[0]?.name"
              class="absolute left-0 top-0 bottom-0 w-0.5 bg-primary"
            />

            <!-- Inline edit -->
            <template v-if="primaryKey && inlineEditColumn === col.name && selectedRowPk === String(rows[virtualRow.index][primaryKey])">
              <input
                :data-grid-edit="String(rows[virtualRow.index][primaryKey])"
                :data-col="col.name"
                :value="getCellValue(rows[virtualRow.index], col.name)"
                @input="(e) => emit('cell-input', rows[virtualRow.index], col.name, (e.target as HTMLInputElement).value)"
                @blur="emit('cell-blur')"
                class="w-full bg-background border border-primary/40 rounded px-2 py-0.5 text-xs font-mono focus:outline-none focus:ring-1 focus:ring-primary/50"
                @click.stop
                autofocus
              />
            </template>

            <!-- NULL -->
            <template v-else-if="rows[virtualRow.index][col.name] === null && !isPendingChange(rows[virtualRow.index], col.name)">
              <div class="flex items-center gap-1.5 h-full">
                <span class="text-[10px] italic font-mono text-muted-foreground/25">NULL</span>
              </div>
            </template>

            <!-- Value -->
            <template v-else>
              <div class="flex items-center gap-1.5 h-full min-w-0">
                <span
                  class="truncate text-xs font-mono leading-none select-none min-w-0 flex-1"
                  :class="
                    isPendingDelete(rows[virtualRow.index])
                      ? 'line-through text-destructive/50'
                      : isPendingChange(rows[virtualRow.index], col.name)
                        ? 'text-amber-300'
                        : 'text-foreground/80'
                  "
                >
                  {{ getCellValue(rows[virtualRow.index], col.name) }}
                </span>

                <!-- FK link button (shown on row hover) -->
                <button
                  v-if="fkMap[col.name] && rows[virtualRow.index][col.name] != null"
                  type="button"
                  @click.stop="emit('navigate-related', fkMap[col.name].table, fkMap[col.name].column, rows[virtualRow.index][col.name])"
                  class="shrink-0 text-muted-foreground/40 hover:text-primary transition-colors"
                  :title="`Go to ${fkMap[col.name].table}`"
                >
                  <ArrowRightIcon class="size-3" />
                </button>
              </div>
            </template>
          </td>
        </tr>

        <!-- Bottom spacer -->
        <tr v-if="paddingBottom > 0">
          <td :colspan="columns.length" :style="{ height: paddingBottom + 'px', padding: 0, border: 'none' }" />
        </tr>

        <!-- Insert row -->
        <tr v-if="insertingRow" class="border-b border-emerald-500/20 bg-emerald-500/8">
          <td
            v-for="col in columns"
            :key="col.name"
            class="px-1 border-r border-border/20 last:border-r-0"
            :style="[
              columnWidths[col.name]
                ? { width: columnWidths[col.name] + 'px', maxWidth: columnWidths[col.name] + 'px' }
                : { maxWidth: '280px' },
              { height: ROW_HEIGHT + 'px' }
            ]"
          >
            <span v-if="isColAutoIncrement(col.name)" class="px-2 text-[10px] text-muted-foreground/40 italic font-mono">auto</span>
            <input
              v-else
              :value="insertRowValues[col.name]"
              :placeholder="isBooleanCol(col.name) ? '0 / 1' : ''"
              class="insert-row-input w-full h-full px-2 text-xs font-mono bg-transparent focus:outline-none focus:bg-emerald-500/10 focus:ring-1 focus:ring-emerald-500/40 rounded"
              @input="emit('insert-row-input', col.name, ($event.target as HTMLInputElement).value)"
              @keydown.enter="emit('insert-row-submit')"
              @keydown.escape="emit('insert-row-cancel')"
            />
          </td>
        </tr>
      </tbody>
    </table>

    <!-- No PK notice -->
    <div v-if="rows.length > 0 && !primaryKey" class="px-4 py-2.5 border-t border-dashed border-border/40 bg-muted/10">
      <p class="text-[10px] text-muted-foreground/40 italic">
        Editing is disabled — this table has no primary key.
      </p>
    </div>
  </div>
</template>
