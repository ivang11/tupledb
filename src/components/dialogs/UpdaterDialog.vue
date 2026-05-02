<script setup lang="ts">
import { computed } from 'vue'
import { DownloadIcon, LoaderCircleIcon, RotateCwIcon, SparklesIcon } from 'lucide-vue-next'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { useUpdater } from '@/composables/useUpdater'

const {
  availableUpdate,
  dialogOpen,
  dismissUpdate,
  downloadedBytes,
  downloadPercent,
  formatBytes,
  installUpdate,
  isInstalling,
  restartToUpdate,
  status,
  totalBytes,
} = useUpdater()

const title = computed(() => {
  if (status.value === 'downloading') return 'Downloading update'
  if (status.value === 'installing') return 'Installing update'
  if (status.value === 'readyToRestart') return 'Update ready'
  if (status.value === 'restarting') return 'Restarting DB Viewer'
  return 'Update available'
})

const description = computed(() => {
  if (status.value === 'downloading') return 'Keep DB Viewer open while the update is downloaded.'
  if (status.value === 'installing') return 'The update is being prepared. This should only take a moment.'
  if (status.value === 'readyToRestart') return 'Restart DB Viewer when you are ready to use the new version.'
  if (status.value === 'restarting') return 'DB Viewer will reopen with the new version.'
  return `DB Viewer ${availableUpdate.value?.version ?? ''} is ready to install.`
})

const progressLabel = computed(() => {
  if (status.value === 'downloading' && totalBytes.value > 0) {
    return `${formatBytes(downloadedBytes.value)} / ${formatBytes(totalBytes.value)}`
  }

  if (status.value === 'downloading') return formatBytes(downloadedBytes.value)
  if (status.value === 'installing') return 'Preparing update'
  if (status.value === 'readyToRestart') return 'Ready to restart'
  if (status.value === 'restarting') return 'Restarting'
  return 'Ready'
})
</script>

<template>
  <Dialog :open="dialogOpen" @update:open="(open) => !open && dismissUpdate()">
    <DialogContent
      class="sm:max-w-md"
      :show-close-button="!isInstalling"
      @escape-key-down="isInstalling && $event.preventDefault()"
      @interact-outside="isInstalling && $event.preventDefault()"
      @pointer-down-outside="isInstalling && $event.preventDefault()"
    >
      <DialogHeader>
        <div class="mb-1 flex size-10 items-center justify-center rounded-md bg-primary/10 text-primary">
          <LoaderCircleIcon v-if="isInstalling" class="size-5 animate-spin" />
          <RotateCwIcon v-else-if="status === 'readyToRestart'" class="size-5" />
          <SparklesIcon v-else class="size-5" />
        </div>
        <DialogTitle>{{ title }}</DialogTitle>
        <DialogDescription>{{ description }}</DialogDescription>
      </DialogHeader>

      <div class="space-y-3 py-2">
        <div class="flex items-center justify-between text-xs font-semibold uppercase tracking-wider text-muted-foreground">
          <span>{{ progressLabel }}</span>
          <span v-if="status === 'downloading' && totalBytes > 0">{{ downloadPercent }}%</span>
        </div>
        <div class="h-2 w-full overflow-hidden rounded-full bg-muted">
          <div
            v-if="status === 'downloading' && totalBytes > 0"
            class="h-full bg-primary transition-all duration-300 ease-out"
            :style="{ width: `${downloadPercent}%` }"
          />
          <div
          v-else-if="isInstalling"
          class="h-full w-1/2 animate-pulse rounded-full bg-primary/70"
          />
          <div v-else class="h-full w-full bg-primary/30" />
        </div>
      </div>

      <DialogFooter>
        <Button
          variant="outline"
          :disabled="isInstalling || status === 'restarting'"
          @click="dismissUpdate"
        >
          {{ status === 'readyToRestart' ? 'Not now' : 'Later' }}
        </Button>
        <Button
          :disabled="isInstalling || status === 'restarting'"
          @click="status === 'readyToRestart' ? restartToUpdate() : installUpdate()"
        >
          <LoaderCircleIcon v-if="isInstalling" class="mr-2 size-4 animate-spin" />
          <RotateCwIcon v-else-if="status === 'readyToRestart'" class="mr-2 size-4" />
          <DownloadIcon v-else class="mr-2 size-4" />
          {{ status === 'readyToRestart' ? 'Restart now' : isInstalling ? 'Updating' : 'Update now' }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
