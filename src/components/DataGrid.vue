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
  return w ? { width: w + 'px', minWidth: w + 'px' } : { minWidth: '180px' }
}

function cellStyle(colName: string) {
  const w = props.columnWidths[colName]
  return w ? { width: w + 'px', maxWidth: w + 'px' } : { maxWidth: '300px' }
}

const scrollContainer = ref<HTMLElement | null>(null)

const virtualizer = useVirtualizer(computed(() => ({
  count: props.rows.length,
  getScrollElement: () => scrollContainer.value,
  estimateSize: () => 44,
  overscan: 8,
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
</script>

<template>
  <div
    ref="scrollContainer"
    class="flex-1 min-w-0 relative bg-muted/5 overflow-auto"
  >
    <!-- Empty state -->
    <div
      v-if="rows && rows.length === 0"
      class="absolute inset-0 flex flex-col items-center justify-center p-12 text-center"
    >
      <DatabaseIcon class="size-10 text-muted-foreground/20 mb-4" />
      <p class="text-lg font-bold text-foreground">No records</p>
      <p class="text-sm text-muted-foreground/60 max-w-62.5 mt-2">
        This table does not contain any data, or your filters didn't match any rows.
      </p>
    </div>

    <template v-else>
      <table class="data-grid w-max min-w-full border-collapse">
        <thead>
          <tr>
            <th
              v-for="col in columns"
              :key="col.name"
              class="sticky top-0 z-20 bg-background/95 backdrop-blur-md px-4 py-3 border-b border-r last:border-r-0 text-left whitespace-nowrap cursor-pointer hover:bg-muted/40 transition-colors select-none group/sortth"
              :style="colStyle(col.name)"
              title="Sort by this column"
              @click="emit('sort', col.name)"
            >
              <div class="flex items-center justify-between gap-2">
                <div class="flex items-center gap-1.5 min-w-0">
                  <span class="block text-xs font-semibold font-mono tracking-normal text-foreground truncate">{{ col.name }}</span>
                  <span v-if="primaryKey === col.name" class="text-[8px] font-black text-amber-500 border border-amber-500/30 px-1 rounded shrink-0">PK</span>
                </div>
                <span class="shrink-0 flex flex-col items-center justify-center opacity-60 group-hover/sortth:opacity-100">
                  <ArrowDownIcon v-if="sortColumn === col.name && sortDesc" class="size-3.5 text-primary" />
                  <ArrowUpIcon v-else-if="sortColumn === col.name" class="size-3.5 text-primary" />
                  <ArrowUpDownIcon v-else class="size-3.5 text-muted-foreground" />
                </span>
              </div>
              <span class="block text-[9px] font-medium font-mono tracking-normal text-muted-foreground opacity-70 mt-0.5">{{ col.type_name }}</span>
              <div
                class="absolute top-0 right-0 h-full w-1.5 cursor-col-resize z-30 opacity-0 group-hover/sortth:opacity-100 hover:bg-primary/40 transition-colors"
                @mousedown="emit('start-col-resize', $event, col.name)"
                @click.stop
                title=""
              />
            </th>
          </tr>
        </thead>
        <tbody>
          <!-- Top spacer -->
          <tr v-if="paddingTop > 0">
            <td :colspan="columns.length" :style="{ height: paddingTop + 'px', padding: 0, border: 'none' }" />
          </tr>

          <tr
            v-for="virtualRow in virtualRows"
            :key="virtualRow.index"
            class="hover:bg-primary/5 transition-colors group/row"
            :class="[
              virtualRow.index % 2 === 0 ? 'bg-background/30' : 'bg-transparent',
              pendingTruncate ? 'bg-destructive/20 opacity-70 grayscale' : '',
              pendingDeletions[String(rows[virtualRow.index][primaryKey || ''])] ? 'bg-destructive/20 text-destructive line-through' : '',
              primaryKey && selectedRowPk === String(rows[virtualRow.index][primaryKey]) ? 'bg-primary/10! ring-1 ring-inset ring-primary/25' : '',
              primaryKey ? 'cursor-pointer' : '',
            ]"
            @click="emit('row-click', rows[virtualRow.index], $event)"
          >
            <td
              v-for="col in columns"
              :key="col.name"
              class="px-4 py-3 text-sm font-medium border-b border-r last:border-r-0 relative group/cell overflow-hidden"
              :style="cellStyle(col.name)"
              :class="[
                pendingChanges[String(rows[virtualRow.index][primaryKey || ''])]?.[col.name] !== undefined ? 'bg-amber-500/10 border-amber-500/30' : '',
                pendingDeletions[String(rows[virtualRow.index][primaryKey || ''])] ? 'border-destructive/20' : '',
              ]"
              @dblclick.stop="primaryKey && !pendingDeletions[String(rows[virtualRow.index][primaryKey || ''])] && emit('cell-dblclick', rows[virtualRow.index], col.name)"
            >
              <!-- Inline edit input -->
              <template v-if="primaryKey && inlineEditColumn === col.name && selectedRowPk === String(rows[virtualRow.index][primaryKey])">
                <input
                  :data-grid-edit="String(rows[virtualRow.index][primaryKey])"
                  :data-col="col.name"
                  :value="getCellValue(rows[virtualRow.index], col.name)"
                  @input="(e) => emit('cell-input', rows[virtualRow.index], col.name, (e.target as HTMLInputElement).value)"
                  @blur="emit('cell-blur')"
                  class="bg-background/90 border border-primary/35 rounded px-2 py-1 text-sm font-medium focus:outline-none focus:ring-1 focus:ring-ring w-full min-w-0"
                  :class="pendingDeletions[String(rows[virtualRow.index][primaryKey || ''])] ? 'text-destructive' : 'text-foreground'"
                  @click.stop
                />
              </template>

              <!-- NULL cell -->
              <template v-else-if="rows[virtualRow.index][col.name] === null && pendingChanges[String(rows[virtualRow.index][primaryKey || ''])]?.[col.name] === undefined">
                <div class="flex items-center gap-1.5 min-w-0 h-full">
                  <span
                    class="text-[10px] italic font-normal tracking-wide shrink-0"
                    :class="pendingDeletions[String(rows[virtualRow.index][primaryKey || ''])] ? 'text-destructive/50' : 'text-muted-foreground/30'"
                  >NULL</span>
                  <button
                    v-if="fkMap[col.name] && rows[virtualRow.index][col.name] != null"
                    type="button"
                    @click.stop="emit('navigate-related', fkMap[col.name].table, fkMap[col.name].column, rows[virtualRow.index][col.name])"
                    class="shrink-0 text-white/60 hover:text-white transition-colors"
                    :title="`Go to ${fkMap[col.name].table}`"
                  >
                    <ArrowRightIcon class="size-3" />
                  </button>
                </div>
              </template>

              <!-- Regular cell -->
              <template v-else>
                <div class="flex items-center gap-1.5 min-w-0 h-full">
                  <span
                    class="truncate text-sm font-medium select-none min-w-0"
                    :class="pendingDeletions[String(rows[virtualRow.index][primaryKey || ''])] ? 'text-destructive font-bold' : 'text-foreground/80'"
                  >{{ getCellValue(rows[virtualRow.index], col.name) }}</span>
                  <button
                    v-if="fkMap[col.name] && rows[virtualRow.index][col.name] != null"
                    type="button"
                    @click.stop="emit('navigate-related', fkMap[col.name].table, fkMap[col.name].column, rows[virtualRow.index][col.name])"
                    class="shrink-0 text-white/60 hover:text-white transition-colors"
                    :title="`Go to ${fkMap[col.name].table} where ${fkMap[col.name].column} = ${rows[virtualRow.index][col.name]}`"
                  >
                    <ArrowRightIcon class="size-3" />
                  </button>
                </div>
              </template>

              <!-- Pending change indicator dot -->
              <div
                v-if="pendingChanges[String(rows[virtualRow.index][primaryKey || ''])]?.[col.name] !== undefined"
                class="absolute top-0 right-0 w-1.5 h-1.5 bg-amber-500 rounded-bl-full"
              />
            </td>
          </tr>

          <!-- Bottom spacer -->
          <tr v-if="paddingBottom > 0">
            <td :colspan="columns.length" :style="{ height: paddingBottom + 'px', padding: 0, border: 'none' }" />
          </tr>

          <!-- Insert row -->
          <tr v-if="insertingRow" class="bg-emerald-500/10 ring-1 ring-inset ring-emerald-500/20">
            <td
              v-for="col in columns"
              :key="col.name"
              class="px-1 py-1 border-b border-r last:border-r-0"
              :style="columnWidths[col.name] ? { width: columnWidths[col.name] + 'px', maxWidth: columnWidths[col.name] + 'px' } : { maxWidth: '300px' }"
            >
              <span v-if="isColAutoIncrement(col.name)" class="px-3 text-xs text-muted-foreground italic">auto</span>
              <input
                v-else
                :value="insertRowValues[col.name]"
                :placeholder="isBooleanCol(col.name) ? '0 / 1' : ''"
                class="insert-row-input w-full h-7 px-3 text-sm bg-transparent focus:outline-none focus:ring-1 focus:ring-emerald-500/50 rounded"
                @input="emit('insert-row-input', col.name, ($event.target as HTMLInputElement).value)"
                @keydown.enter="emit('insert-row-submit')"
                @keydown.escape="emit('insert-row-cancel')"
              />
            </td>
          </tr>
        </tbody>
      </table>

      <div v-if="!primaryKey" class="p-8 text-center bg-muted/20 border-t border-dashed mt-auto">
        <p class="text-sm text-muted-foreground italic">Edition is disabled because this table has no Primary Key.</p>
      </div>
    </template>
  </div>
</template>
