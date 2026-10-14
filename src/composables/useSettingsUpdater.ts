import { onMounted, onBeforeUnmount, ref } from 'vue'
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow'
import type { UnlistenFn } from '@tauri-apps/api/event'
import { UPDATER_REQUEST, UPDATER_STATE, type UpdaterAction, type UpdaterState } from '@/lib/updaterEvents'
import packageInfo from '../../package.json' with { type: 'json' }
const { version } = packageInfo

export function useSettingsUpdater() {
  const win = getCurrentWebviewWindow()
  const state = ref<UpdaterState>({ currentVersion: version, availableVersion: null, isChecking: false, isInstalling: false, isUpToDate: false, status: 'idle', downloadedBytes: 0, totalBytes: 0, downloadPercent: 0, errorMessage: null })
  let unlisten: UnlistenFn | undefined
  let disposed = false
  let listening: Promise<void> | undefined
  function connect() {
    return listening ??= win.listen<UpdaterState>(UPDATER_STATE, event => { state.value = event.payload }).then(stop => {
      if (disposed) stop()
      else unlisten = stop
    }).catch(error => { listening = undefined; throw error })
  }
  async function request(action: UpdaterAction) {
    if (action === 'check') state.value = { ...state.value, isChecking: true, isUpToDate: false, errorMessage: null }
    try {
      await connect()
      await win.emitTo('main', UPDATER_REQUEST, action)
    } catch (error) {
      state.value = { ...state.value, isChecking: false, isInstalling: false, errorMessage: error instanceof Error ? error.message : String(error) }
    }
  }
  onMounted(() => { void request('sync') })
  onBeforeUnmount(() => { disposed = true; unlisten?.() })
  return { state, request }
}
