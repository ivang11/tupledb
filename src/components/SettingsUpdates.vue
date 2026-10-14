<template>
  <section class="space-y-2 border-t border-(--line-faint) pt-4" aria-label="App updates">
    <div class="flex items-center justify-between gap-4">
      <div class="min-w-0">
        <h3 class="text-[12.5px] font-medium text-(--fg-1)">App updates</h3>
        <p class="mt-0.5">Installed version: {{ state.currentVersion }}</p>
      </div>
      <button type="button"
        class="flex h-8 shrink-0 items-center gap-1.5 rounded-lg border border-(--line-2) bg-(--bg-1) px-3 text-[12px] font-medium text-(--fg-1) transition-colors hover:bg-(--bg-2) disabled:cursor-default disabled:opacity-60"
        :disabled="busy" @click="act">
        <LoaderCircleIcon v-if="busy" class="size-3.5 animate-spin" />
        <RotateCwIcon v-else-if="state.status === 'readyToRestart'" class="size-3.5" />
        <DownloadIcon v-else-if="state.availableVersion" class="size-3.5" />
        {{ buttonLabel }}
      </button>
    </div>
    <div role="status" aria-live="polite" class="text-[12px] text-(--fg-2)">{{ message }}</div>
    <template v-if="state.status === 'downloading'">
      <div v-if="state.totalBytes > 0" role="progressbar" aria-label="Update download" :aria-valuenow="state.downloadPercent" aria-valuemin="0" aria-valuemax="100" class="h-1.5 overflow-hidden rounded-full bg-(--bg-2)">
        <div class="h-full bg-(--acc) transition-all" :style="{ width: `${state.downloadPercent}%` }" />
      </div>
      <div v-else role="progressbar" aria-label="Update download" class="h-1.5 overflow-hidden rounded-full bg-(--bg-2)"><div class="h-full w-1/2 animate-pulse bg-(--acc)" /></div>
    </template>
    <p v-if="state.errorMessage" role="alert" class="break-words !text-destructive">{{ state.errorMessage }}</p>
    <p>Updates are also checked automatically when the app starts.</p>
  </section>
</template>
<script setup lang="ts">
import { computed } from 'vue'
import { DownloadIcon, LoaderCircleIcon, RotateCwIcon } from 'lucide-vue-next'
import { useSettingsUpdater } from '@/composables/useSettingsUpdater'
import { formatBytes } from '@/composables/useUpdater'
const { state, request } = useSettingsUpdater()
const busy = computed(() => state.value.isChecking || state.value.isInstalling)
const buttonLabel = computed(() => {
  const value = state.value
  if (value.isChecking) return 'Checking…'
  if (value.status === 'restarting') return 'Restarting…'
  if (value.status === 'downloading') return 'Downloading…'
  if (value.status === 'installing') return 'Installing…'
  if (value.status === 'readyToRestart') return 'Restart to update'
  if (value.availableVersion) return value.errorMessage ? 'Retry update' : 'Download and install'
  return value.errorMessage ? 'Retry' : 'Check for updates'
})
const message = computed(() => {
  const value = state.value
  if (value.isChecking) return 'Checking for updates…'
  if (value.status === 'downloading') return value.totalBytes > 0
    ? `Downloading ${formatBytes(value.downloadedBytes)} / ${formatBytes(value.totalBytes)} · ${value.downloadPercent}%`
    : `Downloading update · ${formatBytes(value.downloadedBytes)}`
  if (value.status === 'installing') return 'Preparing the update. Keep TupleDB open.'
  if (value.status === 'readyToRestart') return `Version ${value.availableVersion} is ready. Restart TupleDB to finish updating.`
  if (value.status === 'restarting') return 'Restarting TupleDB…'
  if (value.errorMessage) return value.availableVersion ? 'The update could not be completed. You can retry.' : 'Could not check for updates. You can retry.'
  if (value.availableVersion) return `New version available: ${value.availableVersion}`
  if (value.isUpToDate) return 'You are using the latest version.'
  return ''
})
function act() {
  void request(state.value.status === 'readyToRestart' ? 'restart' : state.value.availableVersion ? 'install' : 'check')
}
</script>

<style scoped>
p { font-size: 11.5px; line-height: 1.6; color: var(--fg-4); }
</style>
