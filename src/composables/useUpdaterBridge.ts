import { onMounted, onBeforeUnmount, watch } from 'vue'
import { emit, listen, type UnlistenFn } from '@tauri-apps/api/event'
import { useUpdater } from './useUpdater'
import { UPDATER_REQUEST, UPDATER_STATE, type UpdaterAction, type UpdaterState } from '@/lib/updaterEvents'
import packageInfo from '../../package.json' with { type: 'json' }
const { version } = packageInfo

// The main window owns the updater; Settings only sends actions and displays state.
export function useUpdaterBridge() {
  const updater = useUpdater()
  let unlisten: UnlistenFn | undefined
  let disposed = false
  function snapshot(): UpdaterState {
    return {
      currentVersion: version, availableVersion: updater.availableUpdate.value?.version ?? null,
      isChecking: updater.isChecking.value, isInstalling: updater.isInstalling.value,
      isUpToDate: updater.isUpToDate.value, status: updater.status.value,
      downloadedBytes: updater.downloadedBytes.value, totalBytes: updater.totalBytes.value,
      downloadPercent: updater.downloadPercent.value, errorMessage: updater.errorMessage.value,
    }
  }
  function publish() { return emit(UPDATER_STATE, snapshot()).catch(error => console.error('Could not share update status:', error)) }
  watch(snapshot, () => { void publish() })
  onMounted(async () => {
    const stop = await listen<UpdaterAction>(UPDATER_REQUEST, async event => {
      if (event.payload === 'sync') { await publish(); return }
      if (!['check', 'install', 'restart'].includes(event.payload)) return
      updater.presentInline()
      if (event.payload === 'check') await updater.checkForUpdates({ inline: true })
      else if (event.payload === 'install') await updater.installUpdate()
      else await updater.restartToUpdate()
      await publish()
    })
    if (disposed) stop()
    else unlisten = stop
  })
  onBeforeUnmount(() => { disposed = true; unlisten?.() })
}
