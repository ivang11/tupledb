import { WebviewWindow } from '@tauri-apps/api/webviewWindow'
import { usesNativeMacWindowControls } from '@/lib/platform'

export const SETTINGS_WINDOW_LABEL = 'settings'
export const SETTINGS_TAB_EVENT = 'tupledb:settings-tab'

/** Opens the preferences window, or brings the existing one to the front, optionally on a section. */
export async function openSettingsWindow(tab?: string): Promise<WebviewWindow> {
  const existing = await WebviewWindow.getByLabel(SETTINGS_WINDOW_LABEL)
  if (existing) {
    await existing.show()
    await existing.setFocus()
    if (tab) await existing.emitTo(SETTINGS_WINDOW_LABEL, SETTINGS_TAB_EVENT, tab)
    return existing
  }
  return new WebviewWindow(SETTINGS_WINDOW_LABEL, {
    url: `index.html?view=settings${tab ? `&tab=${encodeURIComponent(tab)}` : ''}`,
    title: 'Settings — TupleDB',
    width: 680,
    height: 540,
    minWidth: 560,
    minHeight: 420,
    center: true,
    resizable: true,
    maximizable: false,
    minimizable: false,
    skipTaskbar: true,
    // Same chrome as the connection editor: native corners and traffic lights on
    // macOS, a frameless square window elsewhere. Created hidden; the view
    // reveals it once painted so the WebView never flashes white.
    ...(usesNativeMacWindowControls
      ? { decorations: true, titleBarStyle: 'overlay' as const, hiddenTitle: true }
      : { decorations: false }),
    visible: false,
    backgroundColor: '#17181b',
  })
}
