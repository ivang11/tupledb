<script setup lang="ts">
import { ref, shallowRef, computed, markRaw, nextTick, onMounted, watch } from 'vue'
import { useVirtualizer } from '@tanstack/vue-virtual'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { format as formatSql } from 'sql-formatter'
import {
  PlayIcon,
  HistoryIcon,
  XIcon,
  ClockIcon,
  DatabaseIcon,
  CheckCircleIcon,
  AlertCircleIcon,
  TrashIcon,
  WandSparklesIcon,
  Square,
} from 'lucide-vue-next'
import { ScrollArea } from '@/components/ui/scroll-area'

const props = defineProps<{
  connectionId: string
  database: string | null
  availableDatabases: string[]
}>()

interface ColumnInfo {
  name: string
  type_name: string
}

interface RawQueryResult {
  columns: ColumnInfo[]
  rows: Record<string, any>[]
  rows_affected: number
  is_select: boolean
}

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

const HISTORY_KEY = 'db-viewer:query-history'
const MAX_HISTORY = 100

const sql = ref('')
const selectedDb = ref<string | null>(props.database)
// shallowRef + markRaw: Vue tracks the reference but never wraps row objects in Proxies
const result = shallowRef<RawQueryResult | null>(null)
const queryError = ref<string | null>(null)
const isRunning = ref(false)
const isCancelling = ref(false)
const executionTime = ref<number | null>(null)
const showHistory = ref(false)
const history = ref<HistoryEntry[]>([])
const activeQueryId = ref<string | null>(null)

// ---- Result table virtualizer ----
const resultScrollEl = ref<HTMLElement | null>(null)

const resultVirtualizer = useVirtualizer(computed(() => ({
  count: result.value?.rows.length ?? 0,
  getScrollElement: () => resultScrollEl.value,
  estimateSize: () => 40,
  overscan: 8,
})))

const virtualResultRows = computed(() => resultVirtualizer.value.getVirtualItems())
const resultTotalSize = computed(() => resultVirtualizer.value.getTotalSize())
const resultPaddingTop = computed(() => virtualResultRows.value[0]?.start ?? 0)
const resultPaddingBottom = computed(() =>
  virtualResultRows.value.length > 0
    ? resultTotalSize.value - virtualResultRows.value[virtualResultRows.value.length - 1].end
    : 0
)

watch(() => props.database, (val) => {
  if (val && !selectedDb.value) selectedDb.value = val
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

async function runQuery() {
  const q = sql.value.trim()
  if (!q || isRunning.value) return

  const queryId = crypto.randomUUID()
  activeQueryId.value = queryId
  isRunning.value = true
  isCancelling.value = false
  queryError.value = null
  result.value = null
  const start = Date.now()

  let unlisten: (() => void) | null = null

  try {
    // Register result listener BEFORE invoking to avoid any race condition
    let resolvePayload!: (p: any) => void
    const payloadPromise = new Promise<any>(r => { resolvePayload = r })

    unlisten = await listen<any>(`query-result:${queryId}`, event => {
      resolvePayload(event.payload)
    })

    // This returns immediately — the query runs in a background task on the backend
    await invoke('execute_query', {
      connectionId: props.connectionId,
      database: selectedDb.value || null,
      sql: q,
      queryId,
    })

    // Now wait for the backend to emit the result event
    const payload = await payloadPromise
    // Use backend-measured time (pure MySQL execution, no IPC overhead)
    executionTime.value = payload.duration_ms as number

    if (payload.error) {
      if (!isCancelling.value) {
        throw new Error(payload.error)
      }
    } else {
      // markRaw prevents Vue from making every row object reactive (major perf win)
      result.value = markRaw(payload.ok as RawQueryResult)
      nextTick(() => resultScrollEl.value?.scrollTo(0, 0))
      addToHistory({
        id: crypto.randomUUID(),
        sql: q,
        database: selectedDb.value,
        executedAt: new Date().toISOString(),
        durationMs: payload.duration_ms as number,
        rowCount: (payload.ok as RawQueryResult).rows_affected,
        isSelect: (payload.ok as RawQueryResult).is_select,
      })
    }
  } catch (e: any) {
    if (!isCancelling.value) {
      queryError.value = String(e)
      executionTime.value = executionTime.value ?? Date.now() - start
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
    isRunning.value = false
    isCancelling.value = false
    activeQueryId.value = null
  }
}

async function cancelQuery() {
  if (!isRunning.value || !activeQueryId.value) return
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

function handleKeydown(e: KeyboardEvent) {
  if ((e.metaKey || e.ctrlKey) && e.key === 'Enter') {
    e.preventDefault()
    runQuery()
  }
  if ((e.metaKey || e.ctrlKey) && e.shiftKey && e.key === 'f') {
    e.preventDefault()
    beautify()
  }
}

function beautify() {
  if (!sql.value.trim()) return
  try {
    sql.value = formatSql(sql.value, { language: 'mysql', tabWidth: 2, keywordCase: 'upper' })
  } catch {
    // leave as-is if formatter fails
  }
}

function loadFromHistory(entry: HistoryEntry) {
  sql.value = entry.sql
  if (entry.database) selectedDb.value = entry.database
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

onMounted(() => {
  loadHistory()
})
</script>

<template>
  <div class="flex flex-col min-h-0 overflow-hidden">
    <!-- Toolbar -->
    <div class="h-12 border-b flex items-center gap-3 px-4 bg-background/50 backdrop-blur-sm shrink-0">
      <!-- Database selector -->
      <div class="flex items-center gap-1.5">
        <DatabaseIcon class="size-3.5 text-muted-foreground shrink-0" />
        <select
          v-model="selectedDb"
          class="h-7 text-xs bg-muted/40 border border-input rounded-md px-2 pr-6 focus:outline-none focus:ring-1 focus:ring-ring text-foreground appearance-none cursor-pointer min-w-[140px]"
        >
          <option :value="null">— no database —</option>
          <option v-for="db in availableDatabases" :key="db" :value="db">{{ db }}</option>
        </select>
      </div>

      <div class="flex-1" />

      <!-- Execution time badge -->
      <div v-if="executionTime !== null" class="flex items-center gap-1 text-[10px] font-bold text-muted-foreground">
        <ClockIcon class="size-3" />
        {{ formatDuration(executionTime) }}
      </div>

      <!-- Beautify button -->
      <button
        @click="beautify"
        :disabled="!sql.trim()"
        class="flex items-center gap-1.5 h-7 px-2.5 rounded-md text-xs font-bold text-muted-foreground hover:text-foreground hover:bg-muted/60 transition-colors disabled:opacity-30 disabled:cursor-not-allowed"
        title="Beautify SQL (⌘⇧F)"
      >
        <WandSparklesIcon class="size-3.5" />
        <span class="hidden sm:inline">Format</span>
      </button>

      <!-- History toggle -->
      <button
        @click="showHistory = !showHistory"
        :class="[
          'flex items-center gap-1.5 h-7 px-2.5 rounded-md text-xs font-bold transition-colors',
          showHistory
            ? 'bg-primary/10 text-primary'
            : 'text-muted-foreground hover:text-foreground hover:bg-muted/60'
        ]"
        title="Query History"
      >
        <HistoryIcon class="size-3.5" />
        <span class="hidden sm:inline">History</span>
        <span v-if="history.length > 0" class="text-[9px] bg-muted text-muted-foreground rounded-full px-1.5 py-0.5 font-black">{{ history.length }}</span>
      </button>

      <!-- Cancel button (only while running) -->
      <button
        v-if="isRunning"
        @click="cancelQuery"
        :disabled="isCancelling"
        class="flex items-center gap-1.5 h-7 px-3 rounded-md text-xs font-bold bg-destructive/10 text-destructive hover:bg-destructive/20 transition-colors disabled:opacity-50 disabled:cursor-not-allowed border border-destructive/20"
        title="Cancel query"
      >
        <Square class="size-3" />
        {{ isCancelling ? 'Cancelling…' : 'Cancel' }}
      </button>

      <!-- Run button -->
      <button
        v-else
        @click="runQuery"
        :disabled="!sql.trim()"
        class="flex items-center gap-1.5 h-7 px-3 rounded-md text-xs font-bold bg-primary text-primary-foreground hover:bg-primary/90 transition-colors disabled:opacity-50 disabled:cursor-not-allowed shadow-sm"
      >
        <PlayIcon class="size-3.5" />
        Run
        <span class="text-[9px] opacity-60 hidden sm:inline">⌘↵</span>
      </button>
    </div>

    <!-- Content area: editor + results + optional history panel -->
    <div class="flex-1 flex min-h-0 overflow-hidden">
      <!-- Left: SQL editor + results -->
      <div class="flex-1 flex flex-col min-w-0 min-h-0">
        <!-- SQL editor: fixed height so results get stable space -->
        <div class="h-44 shrink-0 border-b bg-[#0d1117]">
          <textarea
            v-model="sql"
            @keydown="handleKeydown"
            placeholder="SELECT * FROM table WHERE ...  (⌘+Enter to run)"
            class="w-full h-full resize-none bg-transparent text-[13px] font-mono text-[#e6edf3] placeholder:text-[#484f58] p-4 focus:outline-none leading-relaxed"
            spellcheck="false"
          />
        </div>

        <!-- Results -->
        <div ref="resultScrollEl" class="flex-1 min-h-0 min-w-0 overflow-auto bg-muted/5">

          <!-- Error state -->
          <div v-if="queryError" class="m-4 p-4 rounded-lg bg-destructive/10 border border-destructive/20 flex items-start gap-3">
            <AlertCircleIcon class="size-4 text-destructive shrink-0 mt-0.5" />
            <div>
              <p class="text-xs font-bold text-destructive mb-1 uppercase tracking-widest">Query Error</p>
              <p class="text-xs font-mono text-destructive/80 whitespace-pre-wrap break-all">{{ queryError }}</p>
            </div>
          </div>

          <!-- DML result -->
          <div v-else-if="result && !result.is_select" class="flex flex-col items-center justify-center min-h-full gap-3 text-center p-8">
            <div class="size-12 rounded-full bg-green-500/10 flex items-center justify-center">
              <CheckCircleIcon class="size-6 text-green-500" />
            </div>
            <p class="text-sm font-bold text-foreground">Query executed successfully</p>
            <p class="text-xs text-muted-foreground">
              <span class="font-black text-primary">{{ result.rows_affected }}</span>
              {{ result.rows_affected === 1 ? 'row' : 'rows' }} affected
              <span v-if="executionTime !== null" class="ml-2 opacity-60">· {{ formatDuration(executionTime) }}</span>
            </p>
          </div>

          <!-- SELECT results table (virtualized) -->
          <template v-else-if="result && result.is_select">
            <div v-if="result.rows.length === 0" class="flex flex-col items-center justify-center min-h-full gap-2 text-center p-8">
              <p class="text-sm font-bold text-muted-foreground">No rows returned</p>
              <p class="text-xs text-muted-foreground/50">Query completed in {{ formatDuration(executionTime!) }}</p>
            </div>
            <table v-else class="border-collapse" style="min-width: 100%;">
              <thead>
                <tr>
                  <th
                    v-for="col in result.columns"
                    :key="col.name"
                    class="sticky top-0 z-20 bg-background/95 backdrop-blur-md px-4 py-3 border-b border-r last:border-r-0 text-left whitespace-nowrap"
                    style="min-width: 140px;"
                  >
                    <div class="text-xs font-semibold font-mono tracking-normal text-foreground">{{ col.name }}</div>
                    <div class="text-[9px] font-medium font-mono tracking-normal text-muted-foreground opacity-70">{{ col.type_name }}</div>
                  </th>
                </tr>
              </thead>
              <tbody>
                <tr v-if="resultPaddingTop > 0">
                  <td :colspan="result.columns.length" :style="{ height: resultPaddingTop + 'px', padding: 0, border: 'none' }" />
                </tr>
                <tr
                  v-for="vRow in virtualResultRows"
                  :key="vRow.index"
                  class="hover:bg-primary/5 transition-colors"
                  :class="vRow.index % 2 === 0 ? 'bg-background/30' : 'bg-transparent'"
                >
                  <td
                    v-for="col in result.columns"
                    :key="col.name"
                    class="px-4 py-2.5 text-sm border-b border-r last:border-r-0"
                    style="max-width: 320px;"
                  >
                    <span v-if="result.rows[vRow.index][col.name] === null" class="text-[10px] italic font-normal uppercase tracking-widest text-muted-foreground/30">NULL</span>
                    <span v-else class="font-medium text-foreground/80 truncate block">{{ result.rows[vRow.index][col.name] }}</span>
                  </td>
                </tr>
                <tr v-if="resultPaddingBottom > 0">
                  <td :colspan="result.columns.length" :style="{ height: resultPaddingBottom + 'px', padding: 0, border: 'none' }" />
                </tr>
              </tbody>
            </table>
          </template>

          <!-- Empty state -->
          <div v-else class="flex flex-col items-center justify-center h-full gap-3 text-center p-8 text-muted-foreground/40">
            <p class="text-xs font-bold uppercase tracking-widest">Results will appear here</p>
          </div>

        </div>

        <!-- Result count footer -->
        <div v-if="result && result.is_select && result.rows_affected > 0" class="h-9 border-t flex items-center justify-between px-4 bg-background shrink-0">
          <span class="text-[11px] font-bold text-muted-foreground uppercase tracking-wider">
            {{ result.rows_affected }} {{ result.rows_affected === 1 ? 'row' : 'rows' }}
          </span>
          <span v-if="executionTime !== null" class="text-[11px] font-bold text-muted-foreground">
            {{ formatDuration(executionTime) }}
          </span>
        </div>
      </div>

      <!-- History panel -->
      <div
        v-if="showHistory"
        class="w-72 border-l flex flex-col bg-muted/5 shrink-0"
      >
        <div class="h-10 border-b flex items-center justify-between px-3 shrink-0">
          <span class="text-[10px] font-black uppercase tracking-widest text-muted-foreground">Query History</span>
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

        <ScrollArea class="flex-1">
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
                  'text-[9px] font-black uppercase tracking-wider px-1.5 py-0.5 rounded',
                  entry.error
                    ? 'bg-destructive/10 text-destructive'
                    : 'bg-green-500/10 text-green-500'
                ]">
                  {{ entry.error ? 'ERR' : (entry.isSelect ? 'SEL' : 'DML') }}
                </span>
                <span class="text-[9px] text-muted-foreground/50 ml-auto">{{ formatTimeAgo(entry.executedAt) }}</span>
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
  </div>
</template>
