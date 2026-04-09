<script setup lang="ts">
import { ref, watch, nextTick, computed } from 'vue'
import {
  TerminalIcon,
  TrashIcon,
  ChevronUpIcon,
  ChevronDownIcon,
  UploadIcon,
  DownloadIcon,
} from 'lucide-vue-next'
import { useQueryLogStore } from '@/stores/queryLog'
import { useProgressStore } from '@/stores/progress'

const queryLog = useQueryLogStore()
const progress = useProgressStore()

// ── Query log resize ──────────────────────────────────────────────────────────

const LOG_HEIGHT_KEY = 'db-viewer:query-log-height'

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
  () => queryLog.entries.length,
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
  return total ? Math.round((current / total) * 100) : 0
})

const exportPct = computed(() => {
  const { current, total } = progress.exportProgress
  return total ? Math.round((current / total) * 100) : 0
})

const hasActivity = computed(() => progress.isImporting || progress.isExporting)
</script>

<template>
  <div class="flex flex-col border-t border-border bg-background">

    <!-- Expanded import detail (above status bar) -->
    <Transition
      enter-active-class="transition-all duration-200 ease-out overflow-hidden"
      enter-from-class="max-h-0 opacity-0"
      enter-to-class="max-h-32 opacity-100"
      leave-active-class="transition-all duration-150 ease-in overflow-hidden"
      leave-from-class="max-h-32 opacity-100"
      leave-to-class="max-h-0 opacity-0"
    >
      <div
        v-if="progress.isImporting && progress.importExpanded"
        class="border-b border-border bg-muted/20 px-4 py-2.5"
      >
        <div class="flex items-center justify-between mb-1.5">
          <span class="text-xs font-medium flex items-center gap-1.5">
            <UploadIcon class="w-3 h-3 text-muted-foreground" />
            Importing SQL
          </span>
          <span class="text-xs font-semibold tabular-nums text-primary">{{ importPct }}%</span>
        </div>
        <div class="h-1 w-full bg-muted rounded-full overflow-hidden mb-1.5">
          <div
            class="h-full bg-primary transition-all duration-300 ease-out rounded-full"
            :style="{ width: `${importPct}%` }"
          />
        </div>
        <div class="flex items-center justify-between">
          <span class="text-[11px] text-muted-foreground truncate pr-2">{{ progress.importProgress.status }}</span>
          <span class="text-[10px] text-muted-foreground/60 tabular-nums shrink-0">
            {{ progress.importProgress.current.toLocaleString() }} / {{ progress.importProgress.total.toLocaleString() }}
          </span>
        </div>
      </div>
    </Transition>

    <!-- Expanded export detail (above status bar) -->
    <Transition
      enter-active-class="transition-all duration-200 ease-out overflow-hidden"
      enter-from-class="max-h-0 opacity-0"
      enter-to-class="max-h-32 opacity-100"
      leave-active-class="transition-all duration-150 ease-in overflow-hidden"
      leave-from-class="max-h-32 opacity-100"
      leave-to-class="max-h-0 opacity-0"
    >
      <div
        v-if="progress.isExporting && progress.exportExpanded"
        class="border-b border-border bg-muted/20 px-4 py-2.5"
      >
        <div class="flex items-center justify-between mb-1.5">
          <span class="text-xs font-medium flex items-center gap-1.5">
            <DownloadIcon class="w-3 h-3 text-muted-foreground" />
            Exporting Data
          </span>
          <span class="text-xs font-semibold tabular-nums text-primary">{{ exportPct }}%</span>
        </div>
        <div class="h-1 w-full bg-muted rounded-full overflow-hidden mb-1.5">
          <div
            class="h-full bg-primary transition-all duration-300 ease-out rounded-full"
            :style="{ width: `${exportPct}%` }"
          />
        </div>
        <div class="flex items-center justify-between">
          <span class="text-[11px] text-muted-foreground truncate pr-2">{{ progress.exportProgress.status }}</span>
          <span class="text-[10px] text-muted-foreground/60 tabular-nums shrink-0">
            {{ progress.exportProgress.current.toLocaleString() }} / {{ progress.exportProgress.total.toLocaleString() }}
          </span>
        </div>
      </div>
    </Transition>

    <!-- Query Log expanded panel -->
    <div v-if="queryLog.isOpen" class="flex flex-col border-b border-border bg-[#0d1117]" :style="{ height: logHeight + 'px' }">
      <!-- Drag handle -->
      <div
        class="h-1 shrink-0 cursor-row-resize hover:bg-primary/40 transition-colors"
        @mousedown="startLogResize"
      />
      <div
        ref="scrollEl"
        class="flex-1 overflow-y-auto font-mono text-xs leading-relaxed px-3 py-1.5 space-y-2 min-h-0"
        @scroll="onScroll"
      >
      <div v-if="queryLog.entries.length === 0" class="text-muted-foreground/50 italic py-2">
        No queries yet...
      </div>
      <div v-for="entry in queryLog.entries" :key="entry.id" class="group">
        <div class="text-[#58a6ff] text-[10px] mb-0.5 tabular-nums">
          --{{ entry.timestamp }}
          <span class="text-muted-foreground/50 ml-2">({{ entry.duration_ms }}ms)</span>
        </div>
        <div :class="entry.error ? 'text-red-400' : 'text-[#79c0ff]'" class="whitespace-pre-wrap break-all">
          {{ entry.sql }}
        </div>
        <div v-if="entry.error" class="text-red-400/70 text-[10px] mt-0.5">{{ entry.error }}</div>
      </div>
      </div>
    </div>

    <!-- ── Status Bar ───────────────────────────────────────────────────────── -->
    <div class="h-7 flex items-center shrink-0 bg-muted/30 select-none">

      <!-- Query Log toggle -->
      <button
        class="flex items-center gap-1.5 px-3 h-full text-xs text-muted-foreground hover:text-foreground hover:bg-muted/50 transition-colors"
        :class="{ 'text-foreground bg-muted/40': queryLog.isOpen }"
        @click="queryLog.toggle()"
      >
        <TerminalIcon class="w-3.5 h-3.5 shrink-0" />
        <span class="font-medium">Query Log</span>
        <span class="tabular-nums opacity-60">{{ queryLog.entries.length }}</span>
      </button>

      <!-- Clear button (only when log is open and has entries) -->
      <button
        v-if="queryLog.isOpen && queryLog.entries.length > 0"
        class="flex items-center justify-center w-5 h-5 rounded mr-1 text-muted-foreground hover:text-foreground hover:bg-muted/60 transition-colors"
        title="Clear log"
        @click="queryLog.clear()"
      >
        <TrashIcon class="w-3 h-3" />
      </button>

      <!-- Expand/collapse chevron -->
      <button
        class="flex items-center justify-center w-5 h-5 rounded text-muted-foreground hover:text-foreground hover:bg-muted/60 transition-colors"
        @click="queryLog.toggle()"
      >
        <ChevronUpIcon v-if="queryLog.isOpen" class="w-3.5 h-3.5" />
        <ChevronDownIcon v-else class="w-3.5 h-3.5" />
      </button>

      <!-- Spacer -->
      <div class="flex-1" />

      <!-- Separator before progress chips -->
      <div v-if="hasActivity" class="w-px h-3.5 bg-border mx-2" />

      <!-- Import progress chip -->
      <button
        v-if="progress.isImporting"
        class="flex items-center gap-2 px-3 h-full text-xs text-muted-foreground hover:text-foreground hover:bg-muted/50 transition-colors group"
        @click="progress.importExpanded = !progress.importExpanded"
        :title="progress.importExpanded ? 'Collapse import progress' : 'Expand import progress'"
      >
        <UploadIcon class="w-3 h-3 shrink-0 text-primary" />
        <span class="font-medium">Importing SQL</span>
        <!-- Mini progress bar -->
        <div class="w-16 h-1.5 bg-muted rounded-full overflow-hidden">
          <div
            class="h-full bg-primary rounded-full transition-all duration-300 ease-out"
            :style="{ width: `${importPct}%` }"
          />
        </div>
        <span class="tabular-nums font-semibold text-primary w-8 text-right">{{ importPct }}%</span>
        <ChevronUpIcon
          class="w-3 h-3 shrink-0 transition-transform duration-200"
          :class="{ 'rotate-180': !progress.importExpanded }"
        />
      </button>

      <!-- Export progress chip -->
      <button
        v-if="progress.isExporting"
        class="flex items-center gap-2 px-3 h-full text-xs text-muted-foreground hover:text-foreground hover:bg-muted/50 transition-colors group"
        :class="{ 'border-l border-border': progress.isImporting }"
        @click="progress.exportExpanded = !progress.exportExpanded"
        :title="progress.exportExpanded ? 'Collapse export progress' : 'Expand export progress'"
      >
        <DownloadIcon class="w-3 h-3 shrink-0 text-primary" />
        <span class="font-medium">Exporting</span>
        <!-- Mini progress bar -->
        <div class="w-16 h-1.5 bg-muted rounded-full overflow-hidden">
          <div
            class="h-full bg-primary rounded-full transition-all duration-300 ease-out"
            :style="{ width: `${exportPct}%` }"
          />
        </div>
        <span class="tabular-nums font-semibold text-primary w-8 text-right">{{ exportPct }}%</span>
        <ChevronUpIcon
          class="w-3 h-3 shrink-0 transition-transform duration-200"
          :class="{ 'rotate-180': !progress.exportExpanded }"
        />
      </button>

    </div>
  </div>
</template>
