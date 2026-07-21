<template>
  <div class="flex flex-col border-t border-(--line-2) bg-(--bg-0)">

    <!-- Expanded import detail -->
    <Transition
      enter-active-class="transition-all duration-200 ease-out overflow-hidden"
      enter-from-class="max-h-0 opacity-0"
      enter-to-class="max-h-50 opacity-100"
      leave-active-class="transition-all duration-150 ease-in overflow-hidden"
      leave-from-class="max-h-50 opacity-100"
      leave-to-class="max-h-0 opacity-0"
    >
      <div
        v-if="progress.isImporting && progress.importExpanded"
        class="border-b border-(--line-2) bg-(--bg-1) px-4 py-2.5"
      >
        <div class="flex items-center justify-between mb-1.5">
          <span class="text-[11px] font-medium flex items-center gap-1.5 text-(--fg-2)">
            <UploadIcon class="w-3 h-3 text-(--acc)" />
            Importing
          </span>
          <div class="flex items-center gap-2">
            <button
              class="inline-flex items-center gap-1 rounded-md border border-(--line-2) px-2 py-0.5 text-[10px] font-medium text-(--fg-4) hover:text-red-400 hover:border-red-400/30 hover:bg-red-400/5 transition-colors disabled:opacity-40"
              :disabled="progress.isCancellingImport"
              @click="cancelImport"
            >
              <XIcon class="w-2.5 h-2.5 shrink-0" />
              {{ progress.isCancellingImport ? 'Cancelling…' : 'Cancel' }}
            </button>
            <span class="text-[11px] font-semibold tabular-nums text-(--acc)">{{ importPct }}%</span>
          </div>
        </div>
        <div class="h-0.75 w-full bg-(--bg-3) rounded-full overflow-hidden mb-1.5">
          <div class="h-full bg-(--acc) transition-all duration-300 ease-out rounded-full" :style="{ width: `${importPct}%` }" />
        </div>
        <div class="flex items-center justify-between">
          <span class="text-[11px] text-(--fg-4) font-mono truncate pr-2">{{ progress.importProgress.status }}</span>
          <span class="text-[10px] text-(--fg-5) font-mono tabular-nums shrink-0">
            {{ progress.importProgress.current.toLocaleString() }} / {{ progress.importProgress.total.toLocaleString() }}
          </span>
        </div>
      </div>
    </Transition>

    <!-- Expanded export detail -->
    <Transition
      enter-active-class="transition-all duration-200 ease-out overflow-hidden"
      enter-from-class="max-h-0 opacity-0"
      enter-to-class="max-h-105 opacity-100"
      leave-active-class="transition-all duration-150 ease-in overflow-hidden"
      leave-from-class="max-h-105 opacity-100"
      leave-to-class="max-h-0 opacity-0"
    >
      <div
        v-if="progress.isExporting && progress.exportExpanded"
        class="border-b border-(--line-2) bg-(--bg-1) px-4 pt-3 pb-2"
      >
        <div class="flex items-start justify-between mb-2.5">
          <!-- Left: collapse button + done count -->
          <div class="flex items-start gap-2">
            <button
              class="shrink-0 w-5 h-5 flex items-center justify-center rounded text-(--fg-4) hover:text-(--fg-1) hover:bg-(--bg-2) transition-colors mt-1"
              title="Minimize"
              @click="progress.exportExpanded = false"
            >
              <ChevronDownIcon class="w-3.5 h-3.5" />
            </button>
            <div>
              <div class="font-mono text-[20px] font-bold text-(--fg-1) leading-none tabular-nums">
                {{ progress.exportProgress.current }}
                <span class="text-(--fg-4) font-normal text-[16px]"> / {{ progress.exportTables.length }}</span>
              </div>
              <div class="text-[11px] text-(--fg-4) mt-1">
                tables · exporting
              </div>
            </div>
          </div>
          <!-- Right: current table progress + cancel -->
          <div class="flex items-center gap-3 pt-0.5">
            <div v-if="progress.exportProgress.status" class="flex items-center gap-1.5 text-[11px] font-mono">
              <span class="text-(--fg-3) truncate max-w-40">{{ progress.exportProgress.status }}</span>
              <div class="w-16 h-0.75 bg-(--bg-3) rounded-full overflow-hidden">
                <div class="h-full bg-(--acc) rounded-full transition-all duration-300" :style="{ width: `${exportPct}%` }" />
              </div>
              <span class="text-(--acc) w-7 text-right tabular-nums">{{ exportPct }}%</span>
            </div>
            <button
              class="h-6 px-2.5 rounded text-[11px] text-(--fg-3) border border-(--line-2) hover:text-red-400 hover:border-red-400/40 hover:bg-red-400/5 transition-colors disabled:opacity-40"
              :disabled="progress.isCancellingExport"
              @click="cancelExport"
            >{{ progress.isCancellingExport ? 'Cancelling…' : 'Cancel' }}</button>
          </div>
        </div>
        <div class="max-h-48 overflow-y-auto space-y-0.5 custom-scrollbar">
          <div v-for="(table, idx) in progress.exportTables" :key="table" class="flex items-center gap-2 py-0.75">
            <Loader2Icon v-if="idx === progress.exportProgress.current" class="w-3 h-3 shrink-0 text-(--acc) animate-spin" />
            <CheckIcon v-else-if="idx < progress.exportProgress.current" class="w-3 h-3 shrink-0 text-(--acc)" />
            <div v-else class="w-3 h-3 shrink-0 rounded-full border border-(--line-2)" />
            <span
              class="text-[11px] font-mono truncate"
              :class="
                idx === progress.exportProgress.current
                  ? 'text-(--fg-1)'
                  : idx < progress.exportProgress.current
                    ? 'text-(--fg-3)'
                    : 'text-(--fg-2)'
              "
            >{{ table }}</span>
          </div>
        </div>
      </div>
    </Transition>

    <!-- Query Log panel -->
    <div v-if="queryLog.isOpen" class="flex flex-col border-b border-border bg-background" :style="{ height: logHeight + 'px' }">
      <div class="h-2 shrink-0 cursor-row-resize flex items-center justify-center group" @mousedown="startLogResize">
        <div class="w-8 h-0.5 rounded-full bg-border/60 group-hover:bg-primary/50 transition-colors" />
      </div>
      <div ref="scrollEl" class="flex-1 overflow-y-auto font-mono text-xs leading-relaxed px-3 py-1.5 space-y-2 min-h-0" @scroll="onScroll">
        <div class="flex items-center justify-between border-b border-border/60 px-0 pb-1 text-[10px] text-muted-foreground">
          <span class="truncate">{{ logScopeLabel }}</span>
          <span class="tabular-nums">{{ visibleEntries.length }} queries</span>
        </div>
        <div v-if="visibleEntries.length === 0" class="text-muted-foreground/50 italic py-2">No queries yet...</div>
        <div v-for="entry in visibleEntries" :key="entry.id" class="group">
          <div class="text-[#58a6ff] text-[10px] mb-0.5 tabular-nums">
            --{{ entry.timestamp }}
            <span class="text-muted-foreground/50 ml-2">({{ entry.duration_ms }}ms)</span>
          </div>
          <div :class="entry.error ? 'text-red-400' : 'text-[#79c0ff]'" class="whitespace-pre-wrap break-all">{{ entry.sql }}</div>
          <div v-if="entry.error" class="text-red-400/70 text-[10px] mt-0.5">{{ entry.error }}</div>
        </div>
      </div>
    </div>

    <!-- ── Unified bottom bar ─────────────────────────────────────────────── -->
    <div class="h-9 flex items-center shrink-0 select-none gap-2 px-3">

      <!-- Query Log button -->
      <button
        class="flex items-center gap-1.5 px-1.5 h-full text-xs font-mono transition-colors border-b-[1.5px] -mb-px"
        :class="queryLog.isOpen ? 'border-(--acc) text-(--fg-1)' : 'border-transparent text-(--fg-1) hover:text-(--fg-2)'"
        @click="queryLog.toggle()"
      >
        <TerminalIcon class="w-3.5 h-3.5 shrink-0" />
        <span>Query Log</span>
        <span class="tabular-nums text-(--fg-3)">{{ visibleEntries.length }}</span>
      </button>

      <!-- Clear log button -->
      <button
        v-if="queryLog.isOpen && visibleEntries.length > 0"
        class="flex items-center justify-center w-5 h-5 rounded text-(--fg-1) hover:bg-(--bg-2) transition-colors"
        title="Clear log"
        @click="clearVisibleLog"
      >
        <TrashIcon class="w-3.5 h-3.5" />
      </button>

      <div class="w-px h-4 bg-(--line-2) mx-1" />

      <!-- Import/export buttons (left-aligned, next to Query Log) -->
      <button
        v-if="progress.isImporting"
        class="flex items-center gap-1.5 px-1.5 h-full text-xs font-mono transition-colors border-b-[1.5px] -mb-px"
        :class="progress.importExpanded ? 'border-(--acc) text-(--fg-1)' : 'border-transparent text-(--fg-1) hover:text-(--fg-2)'"
        @click="progress.importExpanded = !progress.importExpanded"
      >
        <UploadIcon class="w-3.5 h-3.5 shrink-0 text-(--acc)" />
        <span>Importing</span>
        <span class="tabular-nums text-(--acc)">{{ importPct }}%</span>
      </button>

      <button
        v-if="progress.isExporting"
        class="flex items-center gap-1.5 px-1.5 h-full text-xs font-mono transition-colors border-b-[1.5px] -mb-px"
        :class="progress.exportExpanded ? 'border-(--acc) text-(--fg-1)' : 'border-transparent text-(--fg-1) hover:text-(--fg-2)'"
        @click="progress.exportExpanded = !progress.exportExpanded"
      >
        <DownloadIcon class="w-3.5 h-3.5 shrink-0 text-(--acc)" />
        <span>Exporting</span>
        <span class="tabular-nums text-(--acc)">{{ exportPct }}%</span>
      </button>

      <!-- Insert row mode -->
      <template v-if="isInsertingRow">
        <span v-if="insertRowError" class="text-xs text-destructive truncate max-w-xs cursor-help" :title="insertRowError ?? ''">{{ insertRowError }}</span>
        <div class="flex-1" />
      </template>

<!-- Pagination (data view, not inserting) -->
      <template v-else-if="isDataView && viewMode">
        <div class="flex-1" />

        <span class="font-mono text-xs text-(--fg-1)">{{ rowRangeLabel }}</span>

        <button
          v-if="totalCountApproximate"
          type="button"
          class="h-5 inline-flex items-center gap-1 rounded border border-(--line-2) px-1.5 text-[11px] font-semibold text-(--fg-1) hover:bg-(--bg-2) transition-colors disabled:opacity-50"
          :disabled="exactCountLoading"
          title="Calculate exact row count"
          @click="emit('request-exact-count')"
        >
          <HashIcon class="size-3" />
          {{ exactCountLoading ? 'Counting…' : 'Exact' }}
        </button>

        <div class="w-px h-4 bg-(--line-2)" />

        <span class="font-mono text-[11px] text-(--fg-1) uppercase tracking-wider">limit</span>
        <input
          type="number"
          :value="pageSize"
          min="1"
          class="font-mono text-xs text-(--fg-1) bg-(--bg-1) border border-(--line-2) rounded h-6 w-14 px-1.5 text-center focus:outline-none focus:border-(--acc-line) [appearance:textfield] [&::-webkit-outer-spin-button]:appearance-none [&::-webkit-inner-spin-button]:appearance-none"
          @change="emit('change-limit', +($event.target as HTMLInputElement).value)"
        />
        <span class="font-mono text-[11px] text-(--fg-1) uppercase tracking-wider">offset</span>
        <input
          type="number"
          :value="(page ?? 0) * (pageSize ?? 0)"
          min="0"
          class="font-mono text-xs text-(--fg-1) bg-(--bg-1) border border-(--line-2) rounded h-6 w-16 px-1.5 text-center focus:outline-none focus:border-(--acc-line) [appearance:textfield] [&::-webkit-outer-spin-button]:appearance-none [&::-webkit-inner-spin-button]:appearance-none"
          @change="emit('goto-offset', +($event.target as HTMLInputElement).value)"
        />

        <div class="flex items-center gap-0.5">
          <button
            class="size-6 flex items-center justify-center rounded text-(--fg-1) hover:bg-(--bg-2) transition-colors disabled:opacity-20 disabled:cursor-default"
            :disabled="!canPrevPage"
            @click="emit('change-page', -1)"
          ><ChevronLeftIcon class="size-3.5" /></button>
          <span class="font-mono text-xs text-(--fg-1) tabular-nums w-6 text-center">{{ (page ?? 0) + 1 }}</span>
          <button
            class="size-6 flex items-center justify-center rounded text-(--fg-1) hover:bg-(--bg-2) transition-colors disabled:opacity-20 disabled:cursor-default"
            :disabled="!canNextPage"
            @click="emit('change-page', 1)"
          ><ChevronRightIcon class="size-3.5" /></button>
        </div>
      </template>

      <template v-else>
        <div class="flex-1" />
      </template>

    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, watch, nextTick, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import {
  TerminalIcon,
  TrashIcon,
  UploadIcon,
  DownloadIcon,
  XIcon,
  Loader2Icon,
  CheckIcon,
  ChevronLeftIcon,
  ChevronRightIcon,
  ChevronDownIcon,
  HashIcon,
} from 'lucide-vue-next'
import { useQueryLogStore } from '@/stores/queryLog'
import { useProgressStore } from '@/stores/progress'
import { useToast } from '@/composables/useToast'
import type { TableViewMode } from '@/types/workspace'

const queryLog = useQueryLogStore()
const progress = useProgressStore()
const { error: toastError } = useToast()

const props = defineProps<{
  activeConnectionId?: string | null
  activeDatabase?: string | null
  activeConnectionName?: string | null
  // Pagination
  viewMode?: TableViewMode | null
  page?: number
  pageSize?: number
  rowCount?: number
  totalCount?: number
  totalCountApproximate?: boolean
  exactCountLoading?: boolean
  isInsertingRow?: boolean
  insertRowError?: string | null
}>()

const emit = defineEmits<{
  'change-page': [delta: number]
  'change-limit': [newLimit: number]
  'goto-offset': [offset: number]
  'request-exact-count': []
}>()

// ── Pagination ────────────────────────────────────────────────────────────────

const isDataView = computed(() => props.viewMode === 'content')

const rowRangeLabel = computed(() => {
  const rowCount = props.rowCount ?? 0
  const page = props.page ?? 0
  const pageSize = props.pageSize ?? 0
  const totalCount = props.totalCount ?? 0
  if (rowCount === 0) return '0 rows'
  const from = page * pageSize + 1
  const currentPageEnd = page * pageSize + rowCount
  const to = props.totalCountApproximate
    ? currentPageEnd
    : Math.min(currentPageEnd, totalCount)
  const displayTotal = Math.max(totalCount, currentPageEnd)
  const prefix = props.totalCountApproximate ? '~' : ''
  const suffix = props.totalCountApproximate && rowCount >= pageSize ? '+' : ''
  return `${from.toLocaleString()} – ${to.toLocaleString()} of ${prefix}${displayTotal.toLocaleString()}${suffix}`
})

const canPrevPage = computed(() => (props.page ?? 0) > 0)
const canNextPage = computed(() => {
  if (props.totalCountApproximate) return (props.rowCount ?? 0) >= (props.pageSize ?? 0)
  return ((props.page ?? 0) + 1) * (props.pageSize ?? 0) < (props.totalCount ?? 0)
})

// ── Query log ─────────────────────────────────────────────────────────────────

const visibleEntries = computed(() => {
  if (!props.activeConnectionId) return queryLog.entries
  return queryLog.entries.filter((entry) =>
    entry.connection_id === props.activeConnectionId &&
    (props.activeDatabase === undefined || (entry.database ?? null) === props.activeDatabase)
  )
})

const logScopeLabel = computed(() => {
  if (!props.activeConnectionId) return 'All'
  const connection = props.activeConnectionName ?? 'Connection'
  return props.activeDatabase ? `${connection} / ${props.activeDatabase}` : connection
})

// ── Query log resize ──────────────────────────────────────────────────────────

const LOG_HEIGHT_KEY = 'tupledb:query-log-height'

function loadLogHeight(): number {
  try {
    const raw = localStorage.getItem(LOG_HEIGHT_KEY)
    return raw ? parseInt(raw, 10) : 176
  } catch {
    return 176
  }
}

const logHeight = ref(loadLogHeight())

function startLogResize(e: MouseEvent) {
  e.preventDefault()
  const startY = e.clientY
  const startHeight = logHeight.value
  const onMove = (ev: MouseEvent) => {
    logHeight.value = Math.max(80, Math.min(600, startHeight + (startY - ev.clientY)))
  }
  const onUp = () => {
    window.removeEventListener('mousemove', onMove)
    window.removeEventListener('mouseup', onUp)
    try { localStorage.setItem(LOG_HEIGHT_KEY, String(logHeight.value)) } catch {}
  }
  window.addEventListener('mousemove', onMove)
  window.addEventListener('mouseup', onUp)
}

// ── Query log scroll ──────────────────────────────────────────────────────────

const scrollEl = ref<HTMLElement | null>(null)
const autoScroll = ref(true)

watch(
  () => visibleEntries.value.length,
  async () => {
    if (autoScroll.value && queryLog.isOpen) {
      await nextTick()
      scrollEl.value?.scrollTo({ top: scrollEl.value.scrollHeight, behavior: 'instant' })
    }
  }
)

watch(
  () => queryLog.isOpen,
  async (open) => {
    if (open) {
      await nextTick()
      scrollEl.value?.scrollTo({ top: scrollEl.value.scrollHeight, behavior: 'instant' })
    }
  }
)

function onScroll() {
  if (!scrollEl.value) return
  const { scrollTop, scrollHeight, clientHeight } = scrollEl.value
  autoScroll.value = scrollHeight - scrollTop - clientHeight < 40
}

// ── Progress helpers ──────────────────────────────────────────────────────────

const importPct = computed(() => {
  const { current, total } = progress.importProgress
  if (!total) return 0
  const pct = (current / total) * 100
  if (pct <= 0) return 0
  if (pct >= 100) return 100
  return Math.max(0.1, Math.round(pct * 10) / 10)
})

const exportPct = computed(() => {
  const { current, total } = progress.exportProgress
  return total ? Math.round((current / total) * 100) : 0
})



async function cancelExport() {
  if (!progress.exportConnectionId || !progress.exportId || progress.isCancellingExport) return
  progress.isCancellingExport = true
  try {
    await invoke('cancel_export', { exportId: progress.exportId })
  } catch (e: any) {
    progress.isCancellingExport = false
    toastError('Failed to cancel export', String(e))
  }
}

async function cancelImport() {
  if (!progress.importConnectionId || !progress.importId || progress.isCancellingImport) return
  progress.isCancellingImport = true
  progress.importProgress.status = 'Cancelling import...'
  try {
    await invoke('cancel_import', {
      connectionId: progress.importConnectionId,
      importId: progress.importId,
    })
  } catch (e: any) {
    progress.isCancellingImport = false
    toastError('Failed to cancel import', String(e))
  }
}


function clearVisibleLog() {
  queryLog.clearContext(props.activeConnectionId, props.activeDatabase)
}
</script>
