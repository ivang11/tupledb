<template>
  <div class="flex h-full flex-1 flex-col min-h-0 overflow-hidden">
    <!-- Toolbar -->
    <div class="h-10 border-b flex items-center gap-1.5 px-3 bg-background/50 backdrop-blur-sm shrink-0 relative z-10">
      <div class="flex-1" />

      <!-- Execution time badge -->
      <div v-if="executionTime !== null" class="flex items-center gap-1 text-[10px] font-medium text-muted-foreground">
        <ClockIcon class="size-3" />
        {{ formatDuration(executionTime) }}
      </div>

      <!-- Beautify button -->
      <button
        @click="beautify"
        :disabled="!sql.trim()"
        class="flex items-center gap-1.5 h-7 px-2.5 rounded-md text-xs font-medium text-muted-foreground hover:text-foreground hover:bg-muted/60 transition-colors disabled:opacity-30 disabled:cursor-not-allowed"
        title="Beautify SQL (⌘⇧F)"
      >
        <WandSparklesIcon class="size-3.5" />
        <span class="hidden sm:inline">Format</span>
      </button>

      <!-- Save query button -->
      <button
        @click="openSaveDialog(null)"
        :disabled="!sql.trim()"
        class="flex items-center gap-1.5 h-7 px-2.5 rounded-md text-xs font-medium text-muted-foreground hover:text-foreground hover:bg-muted/60 transition-colors disabled:opacity-30 disabled:cursor-not-allowed"
        title="Guardar query"
      >
        <BookmarkPlusIcon class="size-3.5" />
        <span class="hidden sm:inline">Guardar</span>
      </button>

      <!-- Saved queries toggle -->
      <button
        @click="toggleSaved"
        :class="[
          'flex items-center gap-1.5 h-7 px-2.5 rounded-md text-xs font-medium transition-colors',
          showSaved
            ? 'bg-primary/10 text-primary'
            : 'text-muted-foreground hover:text-foreground hover:bg-muted/60'
        ]"
        title="Queries guardadas"
      >
        <BookmarkIcon class="size-3.5" />
        <span class="hidden sm:inline">Guardadas</span>
        <span v-if="savedStore.queries.length > 0" class="text-[9px] bg-muted text-muted-foreground rounded-full px-1.5 py-0.5 font-semibold">{{ savedStore.queries.length }}</span>
      </button>

      <!-- History toggle -->
      <button
        @click="toggleHistory"
        :class="[
          'flex items-center gap-1.5 h-7 px-2.5 rounded-md text-xs font-medium transition-colors',
          showHistory
            ? 'bg-primary/10 text-primary'
            : 'text-muted-foreground hover:text-foreground hover:bg-muted/60'
        ]"
        title="Query History"
      >
        <HistoryIcon class="size-3.5" />
        <span class="hidden sm:inline">History</span>
        <span v-if="history.length > 0" class="text-[9px] bg-muted text-muted-foreground rounded-full px-1.5 py-0.5 font-semibold">{{ history.length }}</span>
      </button>

      <!-- Live row counter (while running and rows are coming in) -->
      <span
        v-if="isRunning && rowsFetched !== null"
        class="text-[10px] font-medium text-muted-foreground tabular-nums"
      >
        {{ rowsFetched.toLocaleString() }} rows…
        <span v-if="resultRowsLimited" class="text-amber-500">
          showing {{ retainedResultRows.toLocaleString() }}
        </span>
      </span>

      <!-- Cancel button (only while running) -->
      <button
        v-if="isRunning"
        @click="cancelQuery"
        :disabled="cancelButtonState.disabled || !canCancelQuery"
        class="flex items-center gap-1.5 h-7 px-3 rounded-md text-xs font-medium bg-destructive/10 text-destructive hover:bg-destructive/20 transition-colors disabled:opacity-50 disabled:cursor-not-allowed border border-destructive/20"
        title="Cancel query"
      >
        <Square class="size-3" />
        {{ cancelButtonState.label }}
      </button>

      <button v-if="engine !== 'sqlite'" class="flex items-center gap-1.5 h-7 px-2.5 rounded-md text-xs font-semibold text-muted-foreground hover:bg-muted disabled:opacity-30"
        :disabled="!sql.trim() || isRunning" title="Show estimated plan without executing the query" @click="runQuery(true)">Explain</button>
      <button v-if="!isRunning" class="h-7 px-2 rounded text-xs text-muted-foreground hover:bg-muted disabled:opacity-30" :disabled="!sql.trim()" title="Execute all statements on one connection" @click="runAll">Run all</button>
      <!-- Run button -->
      <button
        v-if="!isRunning"
        @click="runQuery()"
        :disabled="!sql.trim()"
        class="flex items-center gap-1.5 h-7 px-3 rounded-md text-xs font-medium bg-primary text-primary-foreground hover:bg-primary/90 transition-colors disabled:opacity-50 disabled:cursor-not-allowed shadow-sm"
      >
        <PlayIcon class="size-3.5" />
        Run
        <span class="text-[9px] opacity-60 hidden sm:inline">Ctrl/⌘ ↵</span>
      </button>
    </div>

    <!-- Content area: editor + results + optional history panel -->
    <div class="flex flex-1 h-0 min-h-0 items-stretch overflow-hidden">
      <!-- Left: SQL editor + results -->
      <div class="flex-1 flex flex-col min-w-0 min-h-0">
        <!-- SQL editor (CodeMirror) -->
        <div class="h-44 shrink-0 border-b bg-background overflow-hidden">
          <div ref="editorEl" class="h-full overflow-auto" />
        </div>

        <div v-if="batchResults.length > 1" class="flex shrink-0 border-b overflow-x-auto px-2 gap-1 bg-muted/20">
          <button v-for="(item, index) in batchResults" :key="index" class="px-3 py-2 text-xs whitespace-nowrap border-b-2" :class="index === activeResult ? 'border-primary text-foreground' : 'border-transparent text-muted-foreground'" @click="selectBatchResult(index)">
            {{ index + 1 }} · {{ item.error ? 'Error' : item.result?.is_select ? `${item.result.rows_affected} rows` : `${item.result?.rows_affected ?? 0} affected` }}
          </button>
        </div>
        <!-- Results -->
        <div class="flex flex-1 min-h-0 min-w-0 flex-col bg-muted/5">

          <!-- Error state -->
          <div v-if="queryError" class="m-4 p-4 rounded-lg bg-destructive/10 border border-destructive/20 flex items-start gap-3">
            <AlertCircleIcon class="size-4 text-destructive shrink-0 mt-0.5" />
            <div>
              <p class="text-xs font-medium text-destructive mb-1 uppercase tracking-widest">Query Error</p>
              <p class="text-xs font-mono text-destructive/80 whitespace-pre-wrap break-all">{{ queryError }}</p>
            </div>
          </div>

          <!-- DML result -->
          <div v-else-if="result && !result.is_select" class="flex flex-col items-center justify-center min-h-full gap-3 text-center p-8">
            <div class="size-12 rounded-full bg-green-500/10 flex items-center justify-center">
              <CheckCircleIcon class="size-6 text-green-500" />
            </div>
            <p class="text-sm font-medium text-foreground">Query executed successfully</p>
            <p class="text-xs text-muted-foreground">
              <span class="font-semibold text-primary">{{ result.rows_affected }}</span>
              {{ result.rows_affected === 1 ? 'row' : 'rows' }} affected
              <span v-if="executionTime !== null" class="ml-2 opacity-60">· {{ formatDuration(executionTime) }}</span>
            </p>
          </div>

          <!-- SELECT results table (virtualized) -->
          <template v-else-if="result && result.is_select">
            <div v-if="result.rows.length === 0" class="flex flex-col items-center justify-center min-h-full gap-2 text-center p-8">
              <p class="text-sm font-medium text-muted-foreground">No rows returned</p>
              <p class="text-xs text-muted-foreground/50">Query completed in {{ formatDuration(executionTime!) }}</p>
            </div>
            <div v-else class="flex flex-1 min-h-0 min-w-0 flex-col">
              <div
                v-if="resultRowsLimited"
                class="shrink-0 border-b border-amber-500/20 bg-amber-500/10 px-4 py-2 text-xs font-semibold text-amber-200"
              >
                Showing first {{ result.rows.length.toLocaleString() }} rows
                <span v-if="resultTotalRows !== null">
                  of {{ resultTotalRows.toLocaleString() }}
                </span>. Add a LIMIT/OFFSET or export the source table for the full dataset.
              </div>
              <ExplainPlanView v-if="isExplainResult" :result="result" />
              <DataGrid v-else
                :columns="result.columns"
                :preference-key="JSON.stringify([connectionId, database, 'query', result.columns.map(column => column.name)])"
                :client-side="true"
                :rows="result.rows"
                :primary-key="null"
                :total-count="result.rows_affected"
                :pending-changes="{}"
                :pending-deletions="{}"
                :pending-truncate="false"
                :pending-drop="false"
                :selected-row-pk="null"
                :selected-row-pks="[]"
                :inline-edit-column="null"
                :sort-column="null"
                :sort-desc="false"
                :inserting-row="false"
                :insert-row-values="{}"
                :pending-inserts="[]"
                :column-widths="queryColumnWidths"
                :fk-map="{}"
                :is-col-auto-increment="() => false"
                :is-boolean-col="() => false"
                :get-cell-value="queryCellValue"
                @row-click="() => {}"
                @cell-dblclick="() => {}"
                @cell-blur="() => {}"
                @cell-input="() => {}"
                @sort="() => {}"
                @start-col-resize="startQueryColResize"
                @navigate-related="() => {}"
                @insert-row-input="() => {}"
                @insert-row-submit="() => {}"
                @insert-row-cancel="() => {}"
                @row-contextmenu="(row, x, y) => resultContextMenu = { show: true, row, x, y }"
                @delete-key-pressed="() => {}"
              />
            </div>
          </template>

<!-- Empty state -->
          <div v-else class="flex flex-col items-center justify-center h-full gap-3 text-center p-8 text-muted-foreground/40">
            <p class="text-xs font-medium uppercase tracking-widest">Results will appear here</p>
          </div>

        </div>

        <!-- Result count footer -->
        <div v-if="result && result.is_select && result.rows_affected > 0" class="h-9 border-t flex items-center justify-between px-4 bg-background shrink-0">
          <span class="text-[11px] font-medium text-muted-foreground uppercase tracking-wider">
            <template v-if="resultRowsLimited">
              Showing {{ result.rows.length.toLocaleString() }} of {{ result.rows_affected.toLocaleString() }} rows
            </template>
            <template v-else>
              {{ result.rows_affected }} {{ result.rows_affected === 1 ? 'row' : 'rows' }}
            </template>
          </span>
          <span v-if="executionTime !== null" class="text-[11px] font-medium text-muted-foreground">
            {{ formatDuration(executionTime) }}
          </span>
        </div>
      </div>

      <!-- Saved queries panel -->
      <div
        v-if="showSaved"
        class="border-l flex h-full min-h-0 flex-col self-stretch bg-muted/5 shrink-0 overflow-hidden relative"
        :style="{ width: savedPanelWidth + 'px' }"
      >
        <!-- Resize handle -->
        <div
          class="group absolute left-0 top-0 bottom-0 w-2 cursor-col-resize z-10 flex items-stretch"
          @mousedown.prevent="startPanelResize($event, 'saved')"
        >
          <div class="w-px bg-border group-hover:bg-primary/50 transition-colors" />
        </div>

        <div class="h-10 border-b flex items-center justify-between px-3 shrink-0">
          <span class="text-[10px] font-semibold uppercase tracking-widest text-muted-foreground">Queries Guardadas</span>
          <button
            @click="showSaved = false"
            class="size-6 flex items-center justify-center rounded text-muted-foreground/50 hover:text-foreground hover:bg-muted/60 transition-colors"
          >
            <XIcon class="size-3.5" />
          </button>
        </div>
        <div class="px-2 pt-2 pb-1 shrink-0">
          <input
            v-model="savedSearch"
            type="text"
            placeholder="Buscar..."
            class="w-full h-7 text-xs bg-muted/40 rounded-md px-2 border border-border/50 focus:outline-none focus:ring-1 focus:ring-ring"
          />
        </div>
        <ScrollArea class="flex-1 min-h-0">
          <div v-if="filteredSaved.length === 0" class="flex items-center justify-center p-4 text-muted-foreground/30 text-xs text-center">
            {{ savedStore.queries.length === 0 ? 'Aún no hay queries guardadas' : 'Sin resultados' }}
          </div>
          <div
            v-for="sq in filteredSaved"
            :key="sq.id"
            class="group border-b border-muted/40 last:border-0 hover:bg-muted/30 transition-colors"
          >
            <div class="flex items-start gap-2 p-3 pr-2">
              <button @click="loadFromSaved(sq)" class="min-w-0 flex-1 text-left">
                <div class="flex items-center gap-1.5 mb-1">
                  <BookmarkIcon class="size-3 text-primary/70 shrink-0" />
                  <span class="text-[11px] font-semibold text-foreground truncate flex-1">{{ sq.name }}</span>
                </div>
                <p v-if="sq.description" class="text-[10px] text-muted-foreground/60 mb-1 truncate">{{ sq.description }}</p>
                <p class="text-[11px] font-mono text-foreground/50 truncate">{{ sq.sql }}</p>
                <div class="flex items-center gap-2 mt-1.5">
                  <span v-if="sq.database" class="text-[9px] text-muted-foreground/40 font-mono truncate">{{ sq.database }}</span>
                  <span v-if="sq.connection_id" class="text-[9px] text-muted-foreground/40 truncate ml-auto">
                    {{ connStore.connections.find(c => c.id === sq.connection_id)?.name ?? sq.connection_id }}
                  </span>
                </div>
              </button>
              <div class="flex items-center gap-1 shrink-0 pt-0.5 opacity-0 group-hover:opacity-100 transition-all">
                <button
                  @click.stop="openSaveDialog(sq)"
                  class="size-5 flex items-center justify-center rounded text-muted-foreground/30 hover:text-foreground hover:bg-muted/60"
                  title="Editar"
                >
                  <PencilIcon class="size-3" />
                </button>
                <button
                  @click.stop="deleteSaved(sq.id)"
                  class="size-5 flex items-center justify-center rounded text-muted-foreground/30 hover:text-destructive hover:bg-destructive/10"
                  title="Eliminar"
                >
                  <XIcon class="size-3" />
                </button>
              </div>
            </div>
          </div>
        </ScrollArea>
      </div>

      <!-- History panel -->
      <div
        v-if="showHistory"
        class="border-l flex h-full min-h-0 flex-col self-stretch bg-muted/5 shrink-0 overflow-hidden relative"
        :style="{ width: historyPanelWidth + 'px' }"
      >
        <!-- Resize handle -->
        <div
          class="group absolute left-0 top-0 bottom-0 w-2 cursor-col-resize z-10 flex items-stretch"
          @mousedown.prevent="startPanelResize($event, 'history')"
        >
          <div class="w-px bg-border group-hover:bg-primary/50 transition-colors" />
        </div>

        <div class="h-10 border-b flex items-center justify-between px-3 shrink-0">
          <span class="text-[10px] font-semibold uppercase tracking-widest text-muted-foreground">Query History</span>
          <div class="flex items-center gap-1">
            <button
              v-if="history.length > 0"
              @click="clearHistory"
              class="size-6 flex items-center justify-center rounded text-muted-foreground/50 hover:text-destructive hover:bg-destructive/10 transition-colors"
              title="Clear history"
            >
              <TrashIcon class="size-3.5" />
            </button>
            <button
              @click="showHistory = false"
              class="size-6 flex items-center justify-center rounded text-muted-foreground/50 hover:text-foreground hover:bg-muted/60 transition-colors"
            >
              <XIcon class="size-3.5" />
            </button>
          </div>
        </div>

        <ScrollArea class="flex-1 min-h-0">
          <div v-if="history.length === 0" class="flex items-center justify-center h-full text-muted-foreground/30 text-xs p-4 text-center">
            No history yet
          </div>
          <div
            v-for="entry in history"
            :key="entry.id"
            class="group border-b border-muted/40 last:border-0 hover:bg-muted/30 transition-colors"
          >
            <button
              @click="loadFromHistory(entry)"
              class="w-full text-left p-3 pr-2"
            >
              <div class="flex items-center gap-2 mb-1.5">
                <span :class="[
                  'text-[9px] font-semibold uppercase tracking-wider px-1.5 py-0.5 rounded',
                  entry.error
                    ? 'bg-destructive/10 text-destructive'
                    : 'bg-green-500/10 text-green-500'
                ]">
                  {{ entry.error ? 'ERR' : (entry.isSelect ? 'SEL' : 'DML') }}
                </span>
                <span class="text-[9px] text-muted-foreground/50 ml-auto">{{ formatTimeAgo(entry.executedAt) }}</span>
                <button
                  @click.stop="() => { sql = entry.sql; openSaveDialog(null) }"
                  class="size-4 flex items-center justify-center rounded text-muted-foreground/30 hover:text-primary opacity-0 group-hover:opacity-100 transition-all"
                  title="Guardar como favorita"
                >
                  <BookmarkPlusIcon class="size-3" />
                </button>
                <button
                  @click.stop="removeHistoryEntry(entry.id)"
                  class="size-4 flex items-center justify-center rounded text-muted-foreground/30 hover:text-destructive opacity-0 group-hover:opacity-100 transition-all"
                >
                  <XIcon class="size-3" />
                </button>
              </div>
              <p class="text-[11px] font-mono text-foreground/70 truncate leading-relaxed">{{ entry.sql }}</p>
              <div class="flex items-center gap-2 mt-1.5">
                <span v-if="entry.database" class="text-[9px] text-muted-foreground/40 font-mono truncate">{{ entry.database }}</span>
                <span class="text-[9px] text-muted-foreground/40 ml-auto shrink-0">{{ formatDuration(entry.durationMs) }}</span>
                <span v-if="!entry.error" class="text-[9px] text-muted-foreground/40 shrink-0">{{ entry.rowCount }} rows</span>
              </div>
            </button>
          </div>
        </ScrollArea>
      </div>
    </div>
    <RowContextMenu :show="resultContextMenu.show" :x="resultContextMenu.x" :y="resultContextMenu.y"
      :has-primary-key="false" :selected-count="1" @dismiss="resultContextMenu.show = false"
      @copy-json="copyResultRow('json')" @copy-tsv="copyResultRow('tsv')" />
  </div>

  <!-- Save Query Dialog -->
  <QueryParametersDialog :open="parameterDialogOpen" :parameters="parameters" :error="parameterError" @cancel="parameterDialogOpen = false" @execute="confirmParameters" />
  <SaveQueryDialog
    v-model:open="saveDialogOpen"
    :sql="sql"
    :database="selectedDb"
    :connection-id="connectionId"
    :editing="editingQuery"
    @saved="savedStore.fetch()"
  />
</template>

<script setup lang="ts">
import { tableSqlName } from "@/lib/tableReference"
import { ref, shallowRef, triggerRef, computed, markRaw, onMounted, onBeforeUnmount, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { format as formatSql } from 'sql-formatter'
import {
  PlayIcon,
  HistoryIcon,
  XIcon,
  ClockIcon,
  CheckCircleIcon,
  AlertCircleIcon,
  TrashIcon,
  WandSparklesIcon,
  Square,
  BookmarkIcon,
  BookmarkPlusIcon,
  PencilIcon,
} from 'lucide-vue-next'
import { ScrollArea } from '@/components/ui/scroll-area'
import { useConnectionStore } from '@/stores/connections'
import { useSavedQueriesStore } from '@/stores/savedQueries'
import SaveQueryDialog from '@/components/dialogs/SaveQueryDialog.vue'
import DataGrid from '@/components/DataGrid.vue'
import type { SavedQuery } from '@/types/savedQuery'
import { EditorView, minimalSetup } from 'codemirror'
import { placeholder, keymap, lineNumbers, highlightActiveLine, highlightActiveLineGutter } from '@codemirror/view'
import { MySQL, PostgreSQL, SQLite } from '@codemirror/lang-sql'
import { databaseEngines } from '@/lib/databaseEngines'
import { EditorState, Compartment } from '@codemirror/state'
import { useKeybindings } from '@/composables/useKeybindings'
import { syntaxHighlighting, HighlightStyle, indentUnit, bracketMatching, foldGutter } from '@codemirror/language'
import { closeBrackets, closeBracketsKeymap, autocompletion, completionKeymap, acceptCompletion, type CompletionContext, type CompletionResult, type Completion } from '@codemirror/autocomplete'
import { tags } from '@lezer/highlight'
import {
  QUERY_RESULT_CELL_LIMIT,
  QUERY_RESULT_ROW_LIMIT,
  applyQueryChunk,
  finalizeStreamedQueryResult,
  limitBufferedQueryResult,
  type ColumnInfo,
  type RawQueryResult,
} from '@/lib/queryStreaming'
import { getQueryCancelButtonState, shouldSurfaceQueryError } from '@/lib/queryExecutionUi'
import QueryParametersDialog from './dialogs/QueryParametersDialog.vue'
import { useAppSettings, fontCss } from '@/composables/useAppSettings'
import { splitSqlStatements, executionSql, parameterNames, substituteParameters, type QueryParameter } from '@/lib/sqlExecution'
import ExplainPlanView from './ExplainPlanView.vue'
import { explainSql } from '@/lib/explainPlan'
import RowContextMenu from "./RowContextMenu.vue"
import { copyText, rowTsv } from "@/lib/clipboard"
import { rowRecord, rowValue, type DataRow } from '@/lib/rowAccess'

const props = defineProps<{
  connectionId: string
  database: string | null
  initialBatchResults?: BatchResult[]
  initialIsExplain?: boolean
  initialSql?: string
  initialResult?: RawQueryResult | null
  initialError?: string | null
  initialExecutionTime?: number | null
  initialRowsLimited?: boolean
  initialTotalRows?: number | null
  // columns per table for databases already loaded in open tabs
  openTabsSchema?: Record<string, string[]>
}>()

const emit = defineEmits<{
  'update:batch-results': [value: BatchResult[]]
  'update:is-explain': [value: boolean]
  'update:sql': [string]
  'update:result': [RawQueryResult | null]
  'update:error': [string | null]
  'update:execution-time': [number | null]
  'update:rows-limited': [boolean]
  'update:total-rows': [number | null]
}>()

interface HistoryEntry {
  id: string
  sql: string
  database: string | null
  executedAt: string
  durationMs: number
  rowCount: number
  isSelect: boolean
  error?: string
}

const HISTORY_KEY = 'tupledb:query-history'
const SAVED_PANEL_WIDTH_KEY = 'tupledb:saved-panel-width'
const HISTORY_PANEL_WIDTH_KEY = 'tupledb:history-panel-width'
const MAX_HISTORY = 100

const { settings } = useAppSettings()
const connStore = useConnectionStore()
const canCancelQuery = computed(() => connStore.openConnections[props.connectionId]?.capabilities.cancelQuery ?? false)
const engine = computed(() => connStore.openConnections[props.connectionId]?.connection.database.engine ?? 'mysql')
const sqlDialect = computed(() => ({ mysql: MySQL, postgresql: PostgreSQL, sqlite: SQLite })[engine.value])
const dialectCompartment = new Compartment()
const savedStore = useSavedQueriesStore()

// Cache of fetched column names keyed by table name
const columnCache = ref<Record<string, string[]>>({})

const sql = ref(props.initialSql ?? '')
const selectedDb = computed(() => props.database)
// shallowRef + markRaw: Vue tracks the reference but never wraps row objects in Proxies.
// triggerRef() forces the virtualizer to re-read count after chunk appends.
const resultContextMenu = ref<{ show: boolean; row: DataRow | null; x: number; y: number }>({ show: false, row: null, x: 0, y: 0 })
function copyResultRow(format: 'json' | 'tsv') {
  if (!resultContextMenu.value.row || !result.value) return
  const record = rowRecord(resultContextMenu.value.row, result.value.columns)
  void copyText(format === 'json' ? JSON.stringify(record, null, 2) : rowTsv(record))
}
const result = shallowRef<RawQueryResult | null>(props.initialResult ?? null)
const queryError = ref<string | null>(props.initialError ?? null)
const isRunning = ref(false)
const isCancelling = ref(false)
const executionTime = ref<number | null>(props.initialExecutionTime ?? null)
const rowsFetched = ref<number | null>(null)
const resultRowsLimited = ref(props.initialRowsLimited ?? false)
const resultTotalRows = ref<number | null>(props.initialTotalRows ?? null)
const showHistory = ref(false)
const showSaved = ref(false)
const history = ref<HistoryEntry[]>([])
const activeQueryId = ref<string | null>(null)
const cancelButtonState = computed(() => getQueryCancelButtonState(
  isRunning.value,
  isCancelling.value,
  activeQueryId.value,
))
const retainedResultRows = computed(() => result.value?.rows.length ?? QUERY_RESULT_ROW_LIMIT)
const queryResultCellLimit = QUERY_RESULT_CELL_LIMIT

function syncResultState() {
  emit('update:result', result.value)
  emit('update:error', queryError.value)
  emit('update:execution-time', executionTime.value)
  emit('update:rows-limited', resultRowsLimited.value)
  emit('update:total-rows', resultTotalRows.value)
}

// ── Side panel resize ─────────────────────────────────────────────────────────

function loadPanelWidth(key: string, fallback = 280) {
  try {
    const raw = localStorage.getItem(key)
    const parsed = raw ? parseInt(raw, 10) : fallback
    return Number.isFinite(parsed) ? parsed : fallback
  } catch {
    return fallback
  }
}

const savedPanelWidth = ref(loadPanelWidth(SAVED_PANEL_WIDTH_KEY))
const historyPanelWidth = ref(loadPanelWidth(HISTORY_PANEL_WIDTH_KEY))

function startPanelResize(e: MouseEvent, panel: 'saved' | 'history') {
  e.preventDefault()
  const startX = e.clientX
  const widthRef = panel === 'saved' ? savedPanelWidth : historyPanelWidth
  const storageKey = panel === 'saved' ? SAVED_PANEL_WIDTH_KEY : HISTORY_PANEL_WIDTH_KEY
  const startW = widthRef.value
  const onMove = (ev: MouseEvent) => {
    widthRef.value = Math.max(200, Math.min(560, startW + (startX - ev.clientX)))
  }
  const onUp = () => {
    window.removeEventListener('mousemove', onMove)
    window.removeEventListener('mouseup', onUp)
    try {
      localStorage.setItem(storageKey, String(widthRef.value))
    } catch {}
  }
  window.addEventListener('mousemove', onMove)
  window.addEventListener('mouseup', onUp)
}

// Saved queries
const saveDialogOpen = ref(false)
const editingQuery = ref<SavedQuery | null>(null)
const savedSearch = ref('')

const filteredSaved = computed(() => {
  const q = savedSearch.value.trim().toLowerCase()
  if (!q) return savedStore.queries
  return savedStore.queries.filter(
    sq => sq.name.toLowerCase().includes(q) || sq.sql.toLowerCase().includes(q),
  )
})

function openSaveDialog(editing: SavedQuery | null = null) {
  editingQuery.value = editing
  saveDialogOpen.value = true
}

function toggleSaved() {
  showSaved.value = !showSaved.value
  if (showSaved.value) showHistory.value = false
}

function toggleHistory() {
  showHistory.value = !showHistory.value
  if (showHistory.value) showSaved.value = false
}

function loadFromSaved(sq: SavedQuery) {
  sql.value = sq.sql
}

async function deleteSaved(id: string) {
  await savedStore.remove(id)
}

const queryColumnWidths = ref<Record<string, number>>({})

function queryCellValue(row: Record<string, unknown> | unknown[], colName: string): string {
  const value = rowValue(row, colName, result.value?.columns ?? [])
  if (value === null || value === undefined) return ''
  if (typeof value === 'object') return JSON.stringify(value)
  return String(value)
}

function startQueryColResize(e: MouseEvent, colName: string) {
  e.preventDefault()
  const startX = e.clientX
  const startWidth = queryColumnWidths.value[colName] ?? 160
  const onMove = (ev: MouseEvent) => {
    queryColumnWidths.value[colName] = Math.max(80, startWidth + ev.clientX - startX)
  }
  const onUp = () => {
    window.removeEventListener('mousemove', onMove)
    window.removeEventListener('mouseup', onUp)
  }
  window.addEventListener('mousemove', onMove)
  window.addEventListener('mouseup', onUp)
}

watch(() => props.database, () => {
  columnCache.value = {}
})

function loadHistory() {
  try {
    const raw = localStorage.getItem(HISTORY_KEY)
    history.value = raw ? JSON.parse(raw) : []
  } catch {
    history.value = []
  }
}

function persistHistory() {
  localStorage.setItem(HISTORY_KEY, JSON.stringify(history.value))
}

function addToHistory(entry: HistoryEntry) {
  history.value.unshift(entry)
  if (history.value.length > MAX_HISTORY) history.value.length = MAX_HISTORY
  persistHistory()
}

function clearHistory() {
  history.value = []
  localStorage.removeItem(HISTORY_KEY)
}

function removeHistoryEntry(id: string) {
  history.value = history.value.filter(e => e.id !== id)
  persistHistory()
}

const isExplainResult = ref(props.initialIsExplain ?? false)
interface BatchResult { sql: string; result: RawQueryResult | null; error: string | null; duration_ms: number }
const batchResults = shallowRef<BatchResult[]>(props.initialBatchResults ?? [])
const activeResult = ref(0)
const parameterDialogOpen = ref(false), parameterError = ref(''), parameters = ref<QueryParameter[]>([])
let parameterTarget = { sql: '', all: false, explain: false }
function runQuery(explain = false) {
  if (isRunning.value) return
  try {
    const selection = editorView?.state.selection.main ?? { from: 0, to: 0 }
    prepareExecution(executionSql(sql.value, selection, engine.value), false, explain)
  } catch (error) { queryError.value = String(error); syncResultState() }
}
function runAll() { if (!isRunning.value) prepareExecution(sql.value, true, false) }
function prepareExecution(source: string, all: boolean, explain: boolean) {
  if (!source.trim()) return
  const names = settings.value.queryParameters ? parameterNames(source, engine.value) : []
  if (names.length) {
    parameters.value = names.map(name => ({ name, type: 'text', value: '' }))
    parameterTarget = { sql: source, all, explain }; parameterError.value = ''; parameterDialogOpen.value = true
  } else void executePrepared(source, all, explain)
}
function confirmParameters() {
  try {
    const source = substituteParameters(parameterTarget.sql, parameters.value, engine.value)
    parameterDialogOpen.value = false; void executePrepared(source, parameterTarget.all, parameterTarget.explain)
  } catch (error) { parameterError.value = String(error) }
}
async function executePrepared(source: string, all: boolean, explain: boolean) {
  if (all && !explain) await executeBatch(source)
  else { batchResults.value = []; emit('update:batch-results', []); await executeSingle(explain, source) }
}
function selectBatchResult(index: number) {
  const item = batchResults.value[index]; if (!item) return
  activeResult.value = index; result.value = item.result ? markRaw(item.result) : null
  queryError.value = item.error; executionTime.value = item.duration_ms
  resultTotalRows.value = item.result?.rows_affected ?? null
  resultRowsLimited.value = !!item.result?.is_select && item.result.rows.length < item.result.rows_affected
  isExplainResult.value = false; emit('update:is-explain', false); syncResultState()
}
async function executeBatch(source: string) {
  let statements: string[]
  try { statements = splitSqlStatements(source, engine.value).map(statement => statement.sql) }
  catch (error) { queryError.value = String(error); syncResultState(); return }
  if (!statements.length || isRunning.value) return
  const queryId = crypto.randomUUID(); activeQueryId.value = queryId; isRunning.value = true
  isCancelling.value = false; queryError.value = null; batchResults.value = []
  try {
    const items = await invoke<BatchResult[]>('execute_query_batch', { connectionId: props.connectionId, database: selectedDb.value, statements, queryId,
      rowLimit: Math.min(50000, Math.max(100, settings.value.resultRowLimit)), timeoutSecs: Math.min(3600, Math.max(1, settings.value.queryTimeout)) })
    batchResults.value = items.map(item => markRaw(item)); emit('update:batch-results', batchResults.value)
    selectBatchResult(Math.max(0, items.length - 1))
    for (const item of items) addToHistory({ id: crypto.randomUUID(), sql: item.sql, database: selectedDb.value, executedAt: new Date().toISOString(), durationMs: item.duration_ms,
      rowCount: item.result?.rows_affected ?? 0, isSelect: item.result?.is_select ?? false, error: item.error ?? undefined })
  } catch (error) { if (!isCancelling.value) { queryError.value = String(error); syncResultState() } }
  finally { isRunning.value = false; isCancelling.value = false; activeQueryId.value = null }
}
async function executeSingle(explain = false, source = sql.value) {
  let q = source.trim()
  if (!q || isRunning.value) return
  if (explain) {
    try { q = explainSql(q, engine.value) }
    catch (error) { queryError.value = String(error); syncResultState(); return }
  }
  isExplainResult.value = explain; emit('update:is-explain', explain)

  const queryId = crypto.randomUUID()
  activeQueryId.value = queryId
  isRunning.value = true
  isCancelling.value = false
  queryError.value = null
  result.value = null
  resultRowsLimited.value = false
  resultTotalRows.value = null
  rowsFetched.value = null
  executionTime.value = null
  syncResultState()
  const start = Date.now()
  let streamedRowsSeen = 0

  let unlisten: (() => void) | null = null
  let unlistenProgress: (() => void) | null = null
  let unlistenChunk: (() => void) | null = null

  try {
    // Register ALL listeners BEFORE invoking to avoid race conditions
    let resolvePayload!: (p: any) => void
    const payloadPromise = new Promise<any>(r => { resolvePayload = r })

    unlisten = await listen<any>(`query-result:${queryId}`, event => {
      resolvePayload(event.payload)
    })

    unlistenProgress = await listen<{ rows_fetched: number }>(`query-progress:${queryId}`, event => {
      rowsFetched.value = event.payload.rows_fetched
    })

    // Chunk listener: builds result incrementally as rows arrive
    unlistenChunk = await listen<{ columns?: ColumnInfo[]; rows: DataRow[] }>(
      `query-chunk:${queryId}`,
      event => {
        const wasEmpty = !result.value
        const nextState = applyQueryChunk({
          result: result.value,
          rowsLimited: resultRowsLimited.value,
          streamedRowsSeen,
        }, event.payload, settings.value.resultRowLimit, queryResultCellLimit)

        streamedRowsSeen = nextState.streamedRowsSeen
        resultRowsLimited.value = nextState.rowsLimited
        result.value = nextState.result ? markRaw(nextState.result) : null
        syncResultState()

        if (wasEmpty && result.value) {
          // First chunk creates the result object; DataGrid owns scrolling.
        } else {
          // Subsequent chunks — append rows and force virtualizer to re-count
          triggerRef(result)
        }
        rowsFetched.value = streamedRowsSeen
      },
    )

    // This returns immediately — the query runs in a background task on the backend
    await invoke('execute_query', {
      connectionId: props.connectionId,
      database: selectedDb.value || null,
      sql: q,
      queryId,
      maxRetainedCells: queryResultCellLimit,
      timeoutSecs: settings.value.queryTimeout,
    })

    // Wait for the backend to signal completion
    const payload = await payloadPromise
    executionTime.value = payload.duration_ms as number
    syncResultState()

    if (payload.error) {
      if (shouldSurfaceQueryError(isCancelling.value)) {
        // Clear any partial streamed result on error
        result.value = null
        syncResultState()
        throw new Error(payload.error)
      }
    } else {
      const meta = payload.ok as RawQueryResult
      if (payload.streamed) {
        // Rows already arrived via chunks — just update final metadata
        const finalized = finalizeStreamedQueryResult(result.value, resultRowsLimited.value, meta)
        result.value = finalized.result ? markRaw(finalized.result) : null
        resultRowsLimited.value = finalized.rowsLimited
        resultTotalRows.value = finalized.totalRows
        syncResultState()
        triggerRef(result)
      } else {
        // Non-SELECT or legacy buffered result
        const limited = limitBufferedQueryResult(meta, settings.value.resultRowLimit, queryResultCellLimit)
        resultRowsLimited.value = limited.rowsLimited
        resultTotalRows.value = limited.totalRows
        result.value = markRaw(limited.result)
        syncResultState()
      }
      addToHistory({
        id: crypto.randomUUID(),
        sql: q,
        database: selectedDb.value,
        executedAt: new Date().toISOString(),
        durationMs: payload.duration_ms as number,
        rowCount: meta.rows_affected,
        isSelect: meta.is_select,
      })
    }
  } catch (e: any) {
    if (shouldSurfaceQueryError(isCancelling.value)) {
      queryError.value = String(e)
      executionTime.value = executionTime.value ?? Date.now() - start
      syncResultState()
      addToHistory({
        id: crypto.randomUUID(),
        sql: q,
        database: selectedDb.value,
        executedAt: new Date().toISOString(),
        durationMs: executionTime.value,
        rowCount: 0,
        isSelect: false,
        error: String(e),
      })
    }
  } finally {
    unlisten?.()
    unlistenProgress?.()
    unlistenChunk?.()
    isRunning.value = false
    isCancelling.value = false
    activeQueryId.value = null
  }
}

async function cancelQuery() {
  if (!canCancelQuery.value || !cancelButtonState.value.canRequestCancel || !activeQueryId.value) return
  isCancelling.value = true
  try {
    await invoke('cancel_query', {
      connectionId: props.connectionId,
      queryId: activeQueryId.value,
    })
  } catch {
    // If the cancel invoke itself failed, undo the flag so the
    // query result/error is shown normally when it eventually finishes.
    isCancelling.value = false
  }
}


function beautify() {
  if (!sql.value.trim()) return
  try {
    sql.value = formatSql(sql.value, { language: databaseEngines[engine.value].formatter, tabWidth: settings.value.tabSize, useTabs: settings.value.indent === 'tabs', keywordCase: settings.value.uppercaseKeywords ? 'upper' : 'preserve' })
  } catch {
    // leave as-is if formatter fails
  }
}

function loadFromHistory(entry: HistoryEntry) {
  sql.value = entry.sql
  showHistory.value = false
}

function formatTimeAgo(isoString: string): string {
  const diff = Date.now() - new Date(isoString).getTime()
  const s = Math.floor(diff / 1000)
  if (s < 60) return `${s}s ago`
  const m = Math.floor(s / 60)
  if (m < 60) return `${m}m ago`
  const h = Math.floor(m / 60)
  if (h < 24) return `${h}h ago`
  return `${Math.floor(h / 24)}d ago`
}

function formatDuration(ms: number): string {
  if (ms < 1000) return `${ms}ms`
  return `${(ms / 1000).toFixed(2)}s`
}

// ── CodeMirror editor ────────────────────────────────────────────────────────

const editorEl = ref<HTMLElement | null>(null)
let editorView: EditorView | null = null
let suppressSync = false
const keymapCompartment = new Compartment()
const kb = useKeybindings()

const editorSettingsCompartment = new Compartment(), appearanceCompartment = new Compartment()
function editorExtensions() {
  const value = settings.value
  const keys = completionKeymap.filter(binding => binding.key !== 'Enter' && binding.key !== 'Tab')
  return [EditorState.tabSize.of(value.tabSize), indentUnit.of(value.indent === 'tabs' ? '\t' : ' '.repeat(value.tabSize)),
    autocompletion({ defaultKeymap: false }), keymap.of([
      ...keys, ...(value.completionKey !== 'tab' ? [{ key: 'Enter', run: acceptCompletion }] : []),
      ...(value.completionKey !== 'enter' ? [{ key: 'Tab', run: acceptCompletion }] : []),
      { key: 'Tab', run: (view: EditorView) => { view.dispatch(view.state.replaceSelection(value.indent === 'tabs' ? '\t' : ' '.repeat(value.tabSize))); return true } },
      ...(value.closeBrackets ? closeBracketsKeymap : []),
    ]), ...(value.closeBrackets ? [closeBrackets()] : [])]
}
function editorAppearance() {
  return EditorView.theme({
    '&': { backgroundColor: 'var(--bg-0)', height: '100%' },
    '.cm-scroller': { fontFamily: fontCss(settings.value.editorFont), fontSize: `${settings.value.editorFontSize}px`, lineHeight: '1.5' },
    '.cm-content': { color: 'var(--fg-1)', padding: '8px 0', caretColor: 'var(--fg-1)' },
    '.cm-cursor': { borderLeftColor: 'var(--fg-1)' },
    '.cm-gutters': { backgroundColor: 'var(--bg-1)', borderRight: '1px solid var(--line-2)', color: 'var(--fg-3)' },
    '.cm-activeLine, .cm-activeLineGutter': { backgroundColor: 'var(--acc-soft)' },
    '.cm-tooltip': { backgroundColor: 'var(--bg-1)', color: 'var(--fg-1)', border: '1px solid var(--line-1)' },
    '.cm-tooltip-autocomplete li[aria-selected]': { backgroundColor: 'var(--acc-soft)', color: 'var(--fg-1)' },
  }, { dark: document.documentElement.dataset.theme !== 'light' })
}
watch(settings, () => editorView?.dispatch({ effects: [editorSettingsCompartment.reconfigure(editorExtensions()), appearanceCompartment.reconfigure(editorAppearance())] }), { deep: true })
function refreshEditorAppearance() { editorView?.dispatch({ effects: appearanceCompartment.reconfigure(editorAppearance()) }) }
onMounted(() => window.addEventListener('tupledb:appearance', refreshEditorAppearance))
onBeforeUnmount(() => window.removeEventListener('tupledb:appearance', refreshEditorAppearance))


const sqlHighlight = HighlightStyle.define([
  { tag: tags.keyword, color: '#ff7b72', fontWeight: 'bold' },
  { tag: tags.string, color: '#a5d6ff' },
  { tag: tags.number, color: '#79c0ff' },
  { tag: tags.comment, color: '#8b949e', fontStyle: 'italic' },
  { tag: tags.operator, color: '#ff7b72' },
  { tag: tags.punctuation, color: '#e6edf3' },
  { tag: tags.name, color: '#e6edf3' },
  { tag: tags.typeName, color: '#ffa657' },
  { tag: tags.function(tags.name), color: '#d2a8ff' },
  { tag: tags.special(tags.name), color: '#ffa657' },
])

function buildCurrentSchema(): Record<string, string[]> {
  const db = selectedDb.value
  if (!db) return {}
  const tables = connStore.openConnections[props.connectionId]?.tables[db] ?? []
  const schema: Record<string, string[]> = {}
  for (const t of tables) schema[tableSqlName(t.reference)] = []
  // Merge column info from tabs that have loaded this table's structure
  if (props.openTabsSchema) {
    for (const [table, cols] of Object.entries(props.openTabsSchema)) {
      if (table in schema && cols.length > 0) schema[table] = cols
    }
  }
  // Merge from local fetch cache
  for (const [table, cols] of Object.entries(columnCache.value)) {
    if (table in schema && cols.length > 0) schema[table] = cols
  }
  return schema
}

async function ensureColumnsForTables(tableNames: string[]) {
  const db = selectedDb.value
  if (!db) return
  const schema = buildCurrentSchema()
  const toFetch = tableNames.filter(
    t => t in schema && schema[t].length === 0 && !(t in columnCache.value)
  )
  if (toFetch.length === 0) return
  await Promise.all(toFetch.map(async (t) => {
    try {
      const table = connStore.openConnections[props.connectionId]?.tables[db]?.find(table => tableSqlName(table.reference) === t)
      if (!table) return
      const structure = await connStore.fetchTableStructure(props.connectionId, db, table.reference)
      columnCache.value[t] = structure.map((c: any) => c.field)
    } catch {
      columnCache.value[t] = []
    }
  }))
}

// Common SQL keywords for completion
const SQL_KEYWORDS: Completion[] = [
  'SELECT', 'FROM', 'WHERE', 'ORDER BY', 'GROUP BY', 'HAVING', 'LIMIT', 'OFFSET',
  'JOIN', 'LEFT JOIN', 'RIGHT JOIN', 'INNER JOIN', 'CROSS JOIN', 'FULL JOIN',
  'ON', 'AS', 'DISTINCT', 'AND', 'OR', 'NOT', 'IN', 'NOT IN', 'LIKE', 'ILIKE',
  'IS NULL', 'IS NOT NULL', 'BETWEEN', 'EXISTS', 'CASE', 'WHEN', 'THEN', 'ELSE', 'END',
  'INSERT INTO', 'VALUES', 'UPDATE', 'SET', 'DELETE FROM',
  'COUNT', 'SUM', 'AVG', 'MAX', 'MIN', 'COALESCE', 'IFNULL', 'IF', 'NOW',
  'UNION', 'UNION ALL', 'WITH', 'EXPLAIN',
].map(k => ({ label: k, type: 'keyword' as const }))

// Returns the last FROM/JOIN/SELECT/WHERE/... keyword found before the cursor
function lastSqlKeyword(text: string): string {
  const re = /\b(SELECT|FROM|WHERE|JOIN|LEFT\s+JOIN|RIGHT\s+JOIN|INNER\s+JOIN|CROSS\s+JOIN|UPDATE|INTO|SET|ON|HAVING|GROUP\s+BY|ORDER\s+BY|AND|OR)\b/gi
  let m: RegExpExecArray | null
  let last = ''
  while ((m = re.exec(text)) !== null) last = m[0].replace(/\s+/g, ' ').toUpperCase()
  return last
}

function makeSqlCompletion() {
  return async (context: CompletionContext): Promise<CompletionResult | null> => {
    const word = context.matchBefore(/\w+/)
    if (!word || (word.from === word.to && !context.explicit)) return null

    // Detect tables referenced in FROM/JOIN and fetch their columns if needed
    const fullQuery = context.state.doc.toString()
    const tableRe = /\b(?:FROM|JOIN)\s+((?:"(?:[^"]|"")+"|[\w$]+)(?:\s*\.\s*(?:"(?:[^"]|"")+"|[\w$]+))?)/gi
    const referencedTables: string[] = []
    let rm: RegExpExecArray | null
    while ((rm = tableRe.exec(fullQuery)) !== null) referencedTables.push(rm[1])
    if (referencedTables.length > 0) await ensureColumnsForTables(referencedTables)

    const textBefore = context.state.doc.sliceString(0, word.from)
    const kw = lastSqlKeyword(textBefore)
    const schema = buildCurrentSchema()
    const tableNames = Object.keys(schema)

    let options: Completion[]

    const cols = new Set<string>()
    for (const c of Object.values(schema)) for (const col of c) cols.add(col)
    const colOptions = Array.from(cols).map(c => ({ label: c, type: 'property' as const, detail: 'column' }))
    const tableOptions = tableNames.map(t => ({ label: t, type: 'class' as const, detail: 'table' }))

    // Always include keywords so e.g. WHERE/ORDER are available after a table name.
    // Context controls the ordering (most relevant first).
    if (['FROM', 'JOIN', 'LEFT JOIN', 'RIGHT JOIN', 'INNER JOIN', 'CROSS JOIN', 'UPDATE', 'INTO'].includes(kw)) {
      options = [...tableOptions, ...SQL_KEYWORDS]
    } else if (['SELECT', 'WHERE', 'ON', 'SET', 'HAVING', 'AND', 'OR'].includes(kw)) {
      options = [...colOptions, ...tableOptions, ...SQL_KEYWORDS]
    } else {
      options = [...SQL_KEYWORDS, ...tableOptions]
    }

    const prefix = word.text.toLowerCase()
    return {
      from: word.from,
      options: options.filter(o => o.label.toLowerCase().startsWith(prefix)).map(option => option.type === 'keyword' ? { ...option, apply: settings.value.uppercaseKeywords ? option.label.toUpperCase() : option.label.toLowerCase() } : option),
      validFor: /^\w*$/,
    }
  }
}

watch(sqlDialect, (dialect) => {
  editorView?.dispatch({ effects: dialectCompartment.reconfigure([
    dialect.language,
    dialect.language.data.of({ autocomplete: makeSqlCompletion() }),
  ]) })
})

onMounted(() => {
  loadHistory()
  savedStore.fetch()

  if (!editorEl.value) return

  editorView = new EditorView({
    state: EditorState.create({
      doc: sql.value,
      extensions: [
        keymapCompartment.of(keymap.of([
          { key: kb.getCodeMirrorKey('runQuery'), run: () => { runQuery(); return true } },
          { key: kb.getCodeMirrorKey('formatQuery'), run: () => { beautify(); return true } },
        ])),
        minimalSetup,
        lineNumbers(), highlightActiveLine(), highlightActiveLineGutter(), bracketMatching(), foldGutter(),
        editorSettingsCompartment.of(editorExtensions()),
        dialectCompartment.of([
          sqlDialect.value.language,
          sqlDialect.value.language.data.of({ autocomplete: makeSqlCompletion() }),
        ]),
        syntaxHighlighting(sqlHighlight, { fallback: true }),
        appearanceCompartment.of(editorAppearance()),
        placeholder(`SELECT * FROM table WHERE ...  (${kb.getBinding('runQuery')} to run)`),
        EditorView.updateListener.of((update) => {
          if (update.docChanged) {
            suppressSync = true
            sql.value = update.state.doc.toString()
            suppressSync = false
            emit('update:sql', sql.value)
          }
        }),
      ],
    }),
    parent: editorEl.value,
  })
})

// Sync external sql changes into editor (beautify, load from history)
watch(sql, (newVal) => {
  if (suppressSync || !editorView) return
  const current = editorView.state.doc.toString()
  if (newVal !== current) {
    editorView.dispatch({
      changes: { from: 0, to: current.length, insert: newVal },
    })
  }
})

// Reconfigure CodeMirror keymap when keybindings change
watch(
  () => [kb.getCodeMirrorKey('runQuery'), kb.getCodeMirrorKey('formatQuery')] as const,
  () => {
    if (!editorView) return
    editorView.dispatch({
      effects: keymapCompartment.reconfigure(keymap.of([
        { key: kb.getCodeMirrorKey('runQuery'), run: () => { runQuery(); return true } },
        { key: kb.getCodeMirrorKey('formatQuery'), run: () => { beautify(); return true } },
      ])),
    })
  },
)

onBeforeUnmount(() => {
  editorView?.destroy()
  editorView = null
})

</script>
