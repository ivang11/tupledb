<script setup lang="ts">
import { ref, onMounted, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
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
} from 'lucide-vue-next'

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
const result = ref<RawQueryResult | null>(null)
const queryError = ref<string | null>(null)
const isRunning = ref(false)
const executionTime = ref<number | null>(null)
const showHistory = ref(false)
const history = ref<HistoryEntry[]>([])

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

  isRunning.value = true
  queryError.value = null
  result.value = null
  const start = Date.now()

  try {
    const res = await invoke<RawQueryResult>('execute_query', {
      connectionId: props.connectionId,
      database: selectedDb.value || null,
      sql: q,
    })
    result.value = res
    executionTime.value = Date.now() - start
    addToHistory({
      id: crypto.randomUUID(),
      sql: q,
      database: selectedDb.value,
      executedAt: new Date().toISOString(),
      durationMs: executionTime.value,
      rowCount: res.rows_affected,
      isSelect: res.is_select,
    })
  } catch (e: any) {
    queryError.value = String(e)
    executionTime.value = Date.now() - start
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
  } finally {
    isRunning.value = false
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

      <!-- Run button -->
      <button
        @click="runQuery"
        :disabled="isRunning || !sql.trim()"
        class="flex items-center gap-1.5 h-7 px-3 rounded-md text-xs font-bold bg-primary text-primary-foreground hover:bg-primary/90 transition-colors disabled:opacity-50 disabled:cursor-not-allowed shadow-sm"
      >
        <PlayIcon class="size-3.5" />
        {{ isRunning ? 'Running...' : 'Run' }}
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

        <!-- Results area: relative wrapper + absolute inner guarantees scroll works -->
        <div class="flex-1 min-h-0 relative bg-muted/5">
          <div class="absolute inset-0 overflow-auto results-scroll">

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

            <!-- SELECT results table -->
            <template v-else-if="result && result.is_select">
              <div v-if="result.rows.length === 0" class="flex flex-col items-center justify-center min-h-full gap-2 text-center p-8">
                <p class="text-sm font-bold text-muted-foreground">No rows returned</p>
                <p class="text-xs text-muted-foreground/50">
                  Query completed in {{ formatDuration(executionTime!) }}
                </p>
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
                      <div class="text-[10px] font-black tracking-widest text-foreground uppercase">{{ col.name }}</div>
                      <div class="text-[9px] font-bold uppercase tracking-tighter text-muted-foreground opacity-70">{{ col.type_name }}</div>
                    </th>
                  </tr>
                </thead>
                <tbody>
                  <tr
                    v-for="(row, idx) in result.rows"
                    :key="idx"
                    class="hover:bg-primary/5 transition-colors"
                    :class="idx % 2 === 0 ? 'bg-background/30' : 'bg-transparent'"
                  >
                    <td
                      v-for="col in result.columns"
                      :key="col.name"
                      class="px-4 py-2.5 text-sm border-b border-r last:border-r-0"
                      style="max-width: 320px;"
                    >
                      <span v-if="row[col.name] === null" class="text-[10px] italic font-normal uppercase tracking-widest text-muted-foreground/30">NULL</span>
                      <span v-else class="font-medium text-foreground/80 truncate block">{{ row[col.name] }}</span>
                    </td>
                  </tr>
                </tbody>
              </table>
            </template>

            <!-- Empty state (no query run yet) -->
            <div v-else class="flex flex-col items-center justify-center min-h-full gap-3 text-center p-8 text-muted-foreground/40">
              <p class="text-xs font-bold uppercase tracking-widest">Results will appear here</p>
            </div>

          </div>
        </div>

        <!-- Result count footer -->
        <div v-if="result && result.is_select && result.rows.length > 0" class="h-9 border-t flex items-center justify-between px-4 bg-background shrink-0">
          <span class="text-[11px] font-bold text-muted-foreground uppercase tracking-wider">
            {{ result.rows.length }} {{ result.rows.length === 1 ? 'row' : 'rows' }}
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

        <div class="flex-1 overflow-y-auto custom-scrollbar">
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
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.results-scroll::-webkit-scrollbar {
  width: 8px;
  height: 8px;
}
.results-scroll::-webkit-scrollbar-track {
  background: transparent;
}
.results-scroll::-webkit-scrollbar-thumb {
  background: hsl(var(--muted-foreground) / 0.35);
  border-radius: 99px;
}
.results-scroll::-webkit-scrollbar-thumb:hover {
  background: hsl(var(--muted-foreground) / 0.6);
}
.results-scroll::-webkit-scrollbar-corner {
  background: transparent;
}
</style>
