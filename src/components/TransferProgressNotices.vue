<template>
  <section v-for="task in tasks" :key="task.kind" :aria-label="task.title" role="region"
    class="pointer-events-auto shrink-0 rounded-xl border border-(--line-2) bg-(--bg-1) p-4 text-(--fg-1) shadow-[0_12px_40px_#0005]">
    <div class="flex items-start gap-3">
      <div class="flex size-8 shrink-0 items-center justify-center rounded-lg bg-(--acc-soft) text-(--acc)"><component :is="task.kind === 'import' ? UploadIcon : DownloadIcon" class="size-4" /></div>
      <div class="min-w-0 flex-1">
        <h2 class="text-[12.5px] font-semibold leading-5">{{ task.title }}</h2>
        <p v-if="task.database" class="mt-0.5 truncate text-[11.5px] text-(--fg-3)" :title="task.database">{{ task.database }}</p>
      </div>
      <button type="button" :aria-label="`Hide ${task.kind} progress`" class="-mr-1 -mt-1 flex size-6 shrink-0 items-center justify-center rounded-md text-(--fg-3) hover:bg-(--bg-2) hover:text-(--fg-1)" @click="hide(task.kind)"><XIcon class="size-3.5" /></button>
    </div>
    <div class="mt-3 space-y-1.5">
      <div class="flex items-center justify-between gap-3 text-[10.5px] text-(--fg-3)"><span>{{ task.countLabel }}</span><span v-if="task.total > 0" class="tabular-nums text-(--acc)">{{ task.percent }}%</span><LoaderCircleIcon v-else class="size-3 animate-spin" /></div>
      <div role="progressbar" :aria-label="`${task.kind === 'import' ? 'Import' : 'Export'} progress`" :aria-valuenow="task.total > 0 ? task.percent : undefined" aria-valuemin="0" aria-valuemax="100" class="h-1 overflow-hidden rounded-full bg-(--bg-3)">
        <div v-if="task.total > 0" class="h-full rounded-full bg-(--acc) transition-all duration-200" :style="{ width: `${task.percent}%` }" />
        <div v-else class="h-full w-1/2 animate-pulse rounded-full bg-(--acc)" />
      </div>
    </div>
    <p role="status" aria-live="polite" class="mt-2 truncate text-[11px] text-(--fg-3)" :title="task.status">{{ task.status }}</p>
    <div class="mt-3 flex justify-end gap-2">
      <button type="button" class="h-7 rounded-md px-2.5 text-[11.5px] text-(--fg-3) hover:bg-(--bg-2) hover:text-(--fg-1)" @click="hide(task.kind)">Hide</button>
      <button type="button" :disabled="task.cancelling" class="h-7 rounded-md border border-(--line-2) px-2.5 text-[11.5px] text-(--fg-2) hover:border-destructive/30 hover:bg-destructive/10 hover:text-destructive disabled:cursor-default disabled:opacity-50" @click="cancel(task.kind)">{{ task.cancelling ? 'Cancelling…' : 'Cancel' }}</button>
    </div>
  </section>
  <section v-if="!progress.isImporting && progress.importReport?.errors.length && progress.importExpanded" aria-label="Import errors" role="region" aria-live="polite"
    class="pointer-events-auto shrink-0 rounded-xl border border-(--line-2) bg-(--bg-1) p-4 text-(--fg-1) shadow-[0_12px_40px_#0005]">
    <div class="flex items-start gap-3">
      <TriangleAlertIcon class="mt-1 size-5 shrink-0 text-destructive" />
      <div class="min-w-0 flex-1">
        <h2 class="text-[12.5px] font-semibold leading-5">{{ progress.importReport.executed === null ? 'Import failed' : `Import finished with ${progress.importReport.errors.length} error${progress.importReport.errors.length === 1 ? '' : 's'}` }}</h2>
        <p class="mt-0.5 truncate text-[11.5px] text-(--fg-3)" :title="`${progress.importReport.database} · ${progress.importReport.fileName}`">{{ progress.importReport.database }} · {{ progress.importReport.fileName }}</p>
      </div>
      <button type="button" aria-label="Hide import errors" class="-mr-1 -mt-1 flex size-6 shrink-0 items-center justify-center rounded-md text-(--fg-3) hover:bg-(--bg-2)" @click="hide('import')"><XIcon class="size-3.5" /></button>
    </div>
    <p v-if="progress.importReport.executed !== null" class="mt-2 text-[11px] text-(--fg-3)">{{ progress.importReport.executed.toLocaleString() }} statements imported.</p>
    <div class="mt-3 flex justify-end gap-2">
      <button type="button" class="h-7 rounded-md px-2.5 text-[11.5px] text-(--fg-3) hover:bg-(--bg-2)" @click="hide('import')">Hide</button>
      <button type="button" class="h-7 rounded-md bg-(--acc) px-2.5 text-[11.5px] text-white" aria-haspopup="dialog" @click="progress.importErrorsOpen = true">View errors</button>
    </div>
  </section>
  <ImportErrorsDialog v-if="progress.importReport?.errors.length" v-model:open="progress.importErrorsOpen" :report="progress.importReport" />
</template>
<script setup lang="ts">
import { computed } from 'vue'
import ImportErrorsDialog from '@/components/dialogs/ImportErrorsDialog.vue'
import { invoke } from '@tauri-apps/api/core'
import { UploadIcon, DownloadIcon, LoaderCircleIcon, XIcon, TriangleAlertIcon } from 'lucide-vue-next'
import { useProgressStore } from '@/stores/progress'
import { useToast } from '@/composables/useToast'
import { formatBytes } from '@/composables/useUpdater'
const progress = useProgressStore()
const { error: toastError } = useToast()
type Kind = 'import' | 'export'
const tasks = computed(() => {
  const items = []
  if (progress.isImporting && progress.importExpanded) items.push({
    kind: 'import' as Kind, title: progress.isCancellingImport ? 'Cancelling import…' : 'Importing database',
    database: progress.importDatabase, total: progress.importProgress.total, percent: progress.importPercent,
    cancelling: progress.isCancellingImport,
    countLabel: progress.importProgress.total > 0 ? `${formatBytes(progress.importProgress.current)} / ${formatBytes(progress.importProgress.total)}` : progress.importFileName || 'Reading file…',
    status: progress.importProgress.status || 'Preparing import…',
  })
  if (progress.isExporting && progress.exportExpanded) items.push({
    kind: 'export' as Kind, title: progress.isCancellingExport ? 'Cancelling export…' : 'Exporting database',
    database: progress.exportDatabase, total: progress.exportProgress.total, percent: progress.exportPercent,
    cancelling: progress.isCancellingExport,
    countLabel: `${Math.min(progress.exportProgress.current, progress.exportTables.length).toLocaleString()} / ${progress.exportTables.length.toLocaleString()} tables`,
    status: progress.exportProgress.status || 'Preparing export…',
  })
  return items
})
function hide(kind: Kind) {
  if (kind === 'import') progress.importExpanded = false
  else progress.exportExpanded = false
}
async function cancel(kind: Kind) {
  if (kind === 'export') {
    if (!progress.exportConnectionId || !progress.exportId || progress.isCancellingExport) return
    progress.isCancellingExport = true
    try { await invoke('cancel_export', { exportId: progress.exportId }) }
    catch (error) { progress.isCancellingExport = false; toastError('Failed to cancel export', String(error)) }
  } else {
    if (!progress.importConnectionId || !progress.importId || progress.isCancellingImport) return
    progress.isCancellingImport = true
    try { await invoke('cancel_import', { connectionId: progress.importConnectionId, importId: progress.importId }) }
    catch (error) { progress.isCancellingImport = false; toastError('Failed to cancel import', String(error)) }
  }
}
</script>
