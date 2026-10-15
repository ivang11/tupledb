import { computed, ref, shallowRef } from 'vue'
import type { Update } from '@tauri-apps/plugin-updater'
import { useToast } from '@/composables/useToast'

const isChecking = ref(false)
const isInstalling = ref(false)
const dialogOpen = ref(false)
const errorMessage = ref<string | null>(null)
const isUpToDate = ref(false)
let inlinePresentation = false
const availableUpdate = shallowRef<Update | null>(null)
const downloadedBytes = ref(0)
const totalBytes = ref(0)
const status = ref<'idle' | 'available' | 'downloading' | 'installing' | 'readyToRestart' | 'restarting'>('idle')

const downloadPercent = computed(() => (
  totalBytes.value > 0 ? Math.min(100, Math.round((downloadedBytes.value / totalBytes.value) * 100)) : 0
))

export function formatBytes(bytes: number) {
  if (!Number.isFinite(bytes) || bytes <= 0) return '0 B'

  const units = ['B', 'KB', 'MB', 'GB']
  let value = bytes
  let unit = 0

  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024
    unit += 1
  }

  return `${value.toFixed(unit === 0 ? 0 : 1)} ${units[unit]}`
}

export function useUpdater() {
  const toast = useToast()

  function presentInline() {
    inlinePresentation = true
    dialogOpen.value = false
  }

  async function checkForUpdates(options: { silent?: boolean; inline?: boolean } = {}) {
    if (options.inline) presentInline()
    if (isInstalling.value || status.value === 'readyToRestart' || status.value === 'restarting') {
      if (!options.inline && !options.silent) { inlinePresentation = false; dialogOpen.value = true }
      return
    }
    if (isChecking.value) return

    inlinePresentation = options.inline === true
    errorMessage.value = null
    isUpToDate.value = false
    isChecking.value = true

    try {
      const { check } = await import('@tauri-apps/plugin-updater')
      const update = await check()

      if (!update) {
        availableUpdate.value = null
        status.value = 'idle'
        isUpToDate.value = true
        if (!options.silent && !inlinePresentation) {
          toast.show({
            type: 'info',
            title: 'TupleDB is up to date',
            message: 'You are already using the latest version.',
          })
        }
        return
      }

      availableUpdate.value = update
      downloadedBytes.value = 0
      totalBytes.value = 0
      status.value = 'available'
      dialogOpen.value = !inlinePresentation
    } catch (error) {
      errorMessage.value = error instanceof Error ? error.message : String(error)
      if (!options.silent && !inlinePresentation) {
        toast.error(
          'Could not check for updates',
          error instanceof Error ? error.message : String(error),
        )
      }
    } finally {
      isChecking.value = false
    }
  }

  function dismissUpdate() {
    // Dismiss the notice, retaining the update and any background installation.
    dialogOpen.value = false
  }

  async function installUpdate() {
    if (!availableUpdate.value || isInstalling.value) return

    try {
      errorMessage.value = null
      isInstalling.value = true
      status.value = 'downloading'
      downloadedBytes.value = 0
      totalBytes.value = 0

      await availableUpdate.value.downloadAndInstall((event) => {
        if (event.event === 'Started') {
          totalBytes.value = event.data.contentLength ?? 0
        }

        if (event.event === 'Progress') {
          downloadedBytes.value += event.data.chunkLength
        }

        if (event.event === 'Finished') {
          status.value = 'installing'
        }
      })

      status.value = 'readyToRestart'
    } catch (error) {
      errorMessage.value = error instanceof Error ? error.message : String(error)
      if (!inlinePresentation) toast.error('Could not update TupleDB', errorMessage.value)
      status.value = availableUpdate.value ? 'available' : 'idle'
    } finally {
      isInstalling.value = false
    }
  }

  async function restartToUpdate() {
    if (status.value !== 'readyToRestart') return

    status.value = 'restarting'
    isInstalling.value = true
    errorMessage.value = null
    try {
      const { relaunch } = await import('@tauri-apps/plugin-process')
      await relaunch()
    } catch (error) {
      errorMessage.value = error instanceof Error ? error.message : String(error)
      status.value = 'readyToRestart'
      isInstalling.value = false
      if (!inlinePresentation) toast.error('Could not restart TupleDB', errorMessage.value)
    }
  }

  return {
    availableUpdate,
    presentInline,
    errorMessage,
    isUpToDate,
    checkForUpdates,
    dialogOpen,
    dismissUpdate,
    downloadedBytes,
    downloadPercent,
    formatBytes,
    installUpdate,
    isChecking,
    isInstalling,
    restartToUpdate,
    status,
    totalBytes,
  }
}
