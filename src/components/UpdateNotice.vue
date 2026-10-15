<template>
  <Transition name="update-notice">
    <section v-if="open" role="region" aria-label="App update" data-update-notice
      class="pointer-events-auto shrink-0 rounded-xl border border-(--line-2) bg-(--bg-1) p-4 text-(--fg-1) shadow-[0_12px_40px_#0005]">
      <div class="flex items-start gap-3">
        <div class="flex size-8 shrink-0 items-center justify-center rounded-lg bg-(--acc-soft) text-(--acc)">
          <LoaderCircleIcon v-if="busy" class="size-4 animate-spin" />
          <RotateCwIcon v-else-if="status === 'readyToRestart'" class="size-4" />
          <DownloadIcon v-else class="size-4" />
        </div>
        <div class="min-w-0 flex-1" role="status" aria-live="polite">
          <div class="flex items-center gap-2">
            <h2 class="text-[12.5px] font-semibold leading-5">{{ title }}</h2>
          </div>
          <p class="mt-0.5 text-[11.5px] leading-relaxed text-(--fg-3)">{{ description }}</p>
        </div>
        <button type="button" class="-mr-1 -mt-1 flex size-6 shrink-0 items-center justify-center rounded-md text-(--fg-3) hover:bg-(--bg-2) hover:text-(--fg-1)" aria-label="Close update notice" @click="emit('dismiss')"><XIcon class="size-3.5" /></button>
      </div>

      <div v-if="status === 'downloading' || status === 'installing'" class="mt-3 space-y-1.5">
        <div class="flex justify-between text-[10.5px] text-(--fg-3)"><span>{{ progressLabel }}</span><span v-if="totalBytes > 0 && status === 'downloading'">{{ downloadPercent }}%</span></div>
        <div role="progressbar" aria-label="Update progress" :aria-valuenow="status === 'downloading' && totalBytes > 0 ? downloadPercent : undefined" aria-valuemin="0" aria-valuemax="100" class="h-1 overflow-hidden rounded-full bg-(--bg-3)">
          <div v-if="status === 'downloading' && totalBytes > 0" class="h-full bg-(--acc) transition-all duration-200" :style="{ width: `${downloadPercent}%` }" />
          <div v-else class="h-full w-1/2 animate-pulse rounded-full bg-(--acc)" />
        </div>
      </div>
      <p v-if="errorMessage" role="alert" class="mt-2 break-words text-[11.5px] text-destructive">{{ errorMessage }}</p>

      <div class="mt-3 flex items-center justify-end gap-2">
        <button type="button" class="h-7 rounded-md px-2.5 text-[11.5px] text-(--fg-3) hover:bg-(--bg-2) hover:text-(--fg-1)" @click="emit('dismiss')">Later</button>
        <button type="button" :disabled="busy" class="flex h-7 items-center gap-1.5 rounded-md bg-(--acc) px-2.5 text-[11.5px] font-medium text-white transition-colors hover:brightness-110 disabled:cursor-default disabled:opacity-60" @click="status === 'readyToRestart' ? emit('restart') : emit('install')">
          <LoaderCircleIcon v-if="busy" class="size-3 animate-spin" />
          <RotateCwIcon v-else-if="status === 'readyToRestart'" class="size-3" />
          <DownloadIcon v-else class="size-3" />
          {{ actionLabel }}
        </button>
      </div>
    </section>
  </Transition>
</template>
<script setup lang="ts">
import { computed } from 'vue'
import { DownloadIcon, LoaderCircleIcon, RotateCwIcon, XIcon } from 'lucide-vue-next'
import { formatBytes } from '@/composables/useUpdater'
import type { UpdaterState } from '@/lib/updaterEvents'
const props = defineProps<{
  open: boolean; version: string | null; status: UpdaterState['status']; busy: boolean
  downloadedBytes: number; totalBytes: number; downloadPercent: number; errorMessage: string | null
}>()
const emit = defineEmits<{ dismiss: []; install: []; restart: [] }>()
const title = computed(() => {
  if (props.status === 'downloading') return 'Downloading update'
  if (props.status === 'installing') return 'Preparing update'
  if (props.status === 'readyToRestart') return 'Ready to restart'
  if (props.status === 'restarting') return 'Restarting TupleDB'
  return 'Update available'
})
const description = computed(() => {
  if (props.status === 'downloading' || props.status === 'installing') return 'You can keep working while the update is prepared.'
  if (props.status === 'readyToRestart') return `TupleDB ${props.version ?? ''} is ready. Restart when it suits you.`
  if (props.status === 'restarting') return 'TupleDB will reopen with the new version.'
  return `TupleDB ${props.version ?? ''} is available to download.`
})
const actionLabel = computed(() => {
  if (props.status === 'restarting') return 'Restarting…'
  if (props.status === 'readyToRestart') return 'Restart now'
  if (props.busy) return 'Updating…'
  return props.errorMessage ? 'Retry' : 'Update now'
})
const progressLabel = computed(() => props.status === 'installing' ? 'Preparing update' : props.totalBytes > 0 ? `${formatBytes(props.downloadedBytes)} / ${formatBytes(props.totalBytes)}` : formatBytes(props.downloadedBytes))
</script>
<style scoped>
.update-notice-enter-active, .update-notice-leave-active { transition: opacity 150ms ease, transform 150ms ease; }
.update-notice-enter-from, .update-notice-leave-to { opacity: 0; transform: translateY(8px); }
@media (prefers-reduced-motion: reduce) { .update-notice-enter-active, .update-notice-leave-active { transition: none; } }
</style>
