import { computed, ref, shallowRef } from 'vue'
import { check, type Update } from '@tauri-apps/plugin-updater'
import { relaunch } from '@tauri-apps/plugin-process'
import { useToast } from '@/composables/useToast'

const isChecking = ref(false)
const isInstalling = ref(false)
const dialogOpen = ref(false)
const availableUpdate = shallowRef<Update | null>(null)
const downloadedBytes = ref(0)
const totalBytes = ref(0)
const status = ref<'idle' | 'available' | 'downloading' | 'installing' | 'readyToRestart' | 'restarting'>('idle')

const downloadPercent = computed(() => (
  totalBytes.value > 0 ? Math.min(100, Math.round((downloadedBytes.value / totalBytes.value) * 100)) : 0
))

function formatBytes(bytes: number) {
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

  async function checkForUpdates(options: { silent?: boolean } = {}) {
    if (isChecking.value || isInstalling.value) return

    isChecking.value = true

    try {
      const update = await check()

      if (!update) {
        if (!options.silent) {
          toast.show({
            type: 'info',
            title: 'DB Viewer is up to date',
            message: 'You are already using the latest version.',
          })
        }
        return
      }

      availableUpdate.value = update
      downloadedBytes.value = 0
      totalBytes.value = 0
      status.value = 'available'
      dialogOpen.value = true
    } catch (error) {
      if (!options.silent) {
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
    if (isInstalling.value || status.value === 'restarting') return

    dialogOpen.value = false
    availableUpdate.value = null
    status.value = 'idle'
    downloadedBytes.value = 0
    totalBytes.value = 0
  }

  async function installUpdate() {
    if (!availableUpdate.value || isInstalling.value) return

    try {
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
      toast.error(
        'Could not update DB Viewer',
        error instanceof Error ? error.message : String(error),
      )
      status.value = availableUpdate.value ? 'available' : 'idle'
    } finally {
      isInstalling.value = false
    }
  }

  async function restartToUpdate() {
    if (status.value !== 'readyToRestart') return

    status.value = 'restarting'
    isInstalling.value = true
    await relaunch()
  }

  return {
    availableUpdate,
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
