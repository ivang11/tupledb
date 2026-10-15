<template>
  <div class="flex flex-col border-t border-(--line-2) bg-(--bg-0)">

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
    <div class="h-7 flex items-center shrink-0 select-none gap-1.5 px-2 text-[11px] bg-(--bg-1)">

      <!-- Query Log button -->
      <button
        class="flex items-center gap-1.5 px-1.5 h-full font-medium transition-colors border-b-[1.5px] -mb-px"
        :class="queryLog.isOpen ? 'border-(--acc) text-(--fg-1)' : 'border-transparent text-(--fg-2) hover:text-(--fg-1)'"
        @click="queryLog.toggle()"
      >
        <TerminalIcon class="size-3 shrink-0" />
        <span>Query Log</span>
        <span class="tabular-nums text-(--fg-4)">{{ visibleEntries.length }}</span>
      </button>

      <!-- Clear log button -->
      <button
        v-if="queryLog.isOpen && visibleEntries.length > 0"
        class="flex items-center justify-center size-5 rounded text-(--fg-2) hover:bg-(--bg-2) hover:text-(--fg-1) transition-colors"
        title="Clear log"
        @click="clearVisibleLog"
      >
        <TrashIcon class="size-3" />
      </button>

      <div class="w-px h-3.5 bg-(--line-2) mx-0.5" />

      <!-- Import/export buttons (left-aligned, next to Query Log) -->
      <button
        v-if="progress.isImporting || progress.importReport?.errors.length"
        class="flex items-center gap-1.5 px-1.5 h-full font-medium transition-colors border-b-[1.5px] -mb-px"
        :class="progress.importExpanded ? 'border-(--acc) text-(--fg-1)' : 'border-transparent text-(--fg-2) hover:text-(--fg-1)'"
        @click="progress.importExpanded = !progress.importExpanded"
      >
        <UploadIcon class="size-3 shrink-0 text-(--acc)" />
        <span>{{ progress.isImporting ? 'Importing' : 'Import errors' }}</span>
        <span class="tabular-nums text-(--acc)">{{ progress.isImporting ? `${importPct}%` : progress.importReport?.errors.length }}</span>
      </button>

      <button
        v-if="progress.isExporting"
        class="flex items-center gap-1.5 px-1.5 h-full font-medium transition-colors border-b-[1.5px] -mb-px"
        :class="progress.exportExpanded ? 'border-(--acc) text-(--fg-1)' : 'border-transparent text-(--fg-2) hover:text-(--fg-1)'"
        @click="progress.exportExpanded = !progress.exportExpanded"
      >
        <DownloadIcon class="size-3 shrink-0 text-(--acc)" />
        <span>Exporting</span>
        <span class="tabular-nums text-(--acc)">{{ exportPct }}%</span>
      </button>

      <!-- Insert row mode -->
      <template v-if="isInsertingRow">
        <span v-if="insertRowError" class="text-destructive truncate max-w-xs cursor-help" :title="insertRowError ?? ''">{{ insertRowError }}</span>
        <div class="flex-1" />
      </template>

<!-- Pagination (data view, not inserting) -->
      <template v-else-if="isDataView && viewMode">
        <div class="flex-1" />

        <span class="tabular-nums text-(--fg-2)">{{ rowRangeLabel }}</span>

        <button
          v-if="totalCountApproximate"
          type="button"
          class="h-5 inline-flex items-center gap-1 rounded-[4px] border border-(--line-2) px-1.5 text-[10.5px] font-medium text-(--fg-2) hover:bg-(--bg-2) hover:text-(--fg-1) transition-colors disabled:opacity-50"
          :disabled="exactCountLoading"
          title="Calculate exact row count"
          @click="emit('request-exact-count')"
        >
          <HashIcon class="size-3" />
          {{ exactCountLoading ? 'Counting…' : 'Exact' }}
        </button>

        <div class="w-px h-3.5 bg-(--line-2) mx-0.5" />

        <label class="flex items-center gap-1 text-(--fg-4)">
          Limit
          <input
            type="number"
            :value="pageSize"
            min="1"
            class="tabular-nums text-[11px] text-(--fg-1) bg-(--bg-0) border border-(--line-2) rounded-[4px] h-5 w-12 px-1 text-center focus:outline-none focus:border-(--acc-line) [appearance:textfield] [&::-webkit-outer-spin-button]:appearance-none [&::-webkit-inner-spin-button]:appearance-none"
            @change="emit('change-limit', +($event.target as HTMLInputElement).value)"
          />
        </label>
        <label class="flex items-center gap-1 text-(--fg-4)">
          Offset
          <input
            type="number"
            :value="(page ?? 0) * (pageSize ?? 0)"
            min="0"
            class="tabular-nums text-[11px] text-(--fg-1) bg-(--bg-0) border border-(--line-2) rounded-[4px] h-5 w-14 px-1 text-center focus:outline-none focus:border-(--acc-line) [appearance:textfield] [&::-webkit-outer-spin-button]:appearance-none [&::-webkit-inner-spin-button]:appearance-none"
            @change="emit('goto-offset', +($event.target as HTMLInputElement).value)"
          />
        </label>

        <div class="flex items-center">
          <button
            class="size-5 flex items-center justify-center rounded-[4px] text-(--fg-2) hover:bg-(--bg-2) hover:text-(--fg-1) transition-colors disabled:opacity-25 disabled:cursor-default"
            :disabled="!canPrevPage"
            @click="emit('change-page', -1)"
          ><ChevronLeftIcon class="size-3" /></button>
          <span class="tabular-nums text-(--fg-1) w-5 text-center">{{ (page ?? 0) + 1 }}</span>
          <button
            class="size-5 flex items-center justify-center rounded-[4px] text-(--fg-2) hover:bg-(--bg-2) hover:text-(--fg-1) transition-colors disabled:opacity-25 disabled:cursor-default"
            :disabled="!canNextPage"
            @click="emit('change-page', 1)"
          ><ChevronRightIcon class="size-3" /></button>
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
import {
  TerminalIcon,
  TrashIcon,
  UploadIcon,
  DownloadIcon,
  ChevronLeftIcon,
  ChevronRightIcon,
  HashIcon,
} from 'lucide-vue-next'
import { useQueryLogStore } from '@/stores/queryLog'
import { useProgressStore } from '@/stores/progress'
import type { TableViewMode } from '@/types/workspace'

const queryLog = useQueryLogStore()
const progress = useProgressStore()

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

const importPct = computed(() => progress.importPercent)
const exportPct = computed(() => progress.exportPercent)

function clearVisibleLog() {
  queryLog.clearContext(props.activeConnectionId, props.activeDatabase)
}
</script>
