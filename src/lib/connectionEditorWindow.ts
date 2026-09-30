import { getCurrentWebviewWindow, WebviewWindow } from '@tauri-apps/api/webviewWindow'
import type { UnlistenFn } from '@tauri-apps/api/event'
import type { Connection, DriverDescriptor } from '@/types/connection'

export const editorEvents = {
  ready: 'connection-editor-ready',
  initialize: 'connection-editor-initialize',
  save: 'connection-editor-save',
  result: 'connection-editor-save-result',
} as const

export interface EditorDraft {
  connection: Connection
  editing: boolean
  drivers: DriverDescriptor[]
  showConnectButton: boolean
}

export interface EditorSaveRequest {
  label: string
  connection: Connection
  andConnect: boolean
}

/** Drafts and passwords travel only through local, targeted IPC, never URLs. */
export function openConnectionEditorWindow(options: {
  draft: EditorDraft
  save: (connection: Connection, andConnect: boolean) => Promise<boolean>
  saveError: () => string | null
  closed: () => void
  failed: (error: unknown) => void
}) {
  const label = `connection-editor-${crypto.randomUUID()}`
  const unlisteners: UnlistenFn[] = []
  let child: WebviewWindow | undefined
  let disposed = false
  let created = false
  let saving = false

  function dispose() {
    if (disposed) return
    disposed = true
    unlisteners.splice(0).forEach(unlisten => unlisten())
    if (created) void child?.destroy().catch(() => {})
  }

  async function retain(listener: Promise<UnlistenFn>) {
    const unlisten = await listener
    if (disposed) unlisten()
    else unlisteners.push(unlisten)
  }

  void (async () => {
    try {
      const main = getCurrentWebviewWindow()
      await retain(main.listen<{ label: string }>(editorEvents.ready, async event => {
        if (disposed || event.payload.label !== label) return
        try {
          await main.emitTo(label, editorEvents.initialize, options.draft)
        } catch (error) {
          dispose()
          options.failed(error)
        }
      }))
      await retain(main.listen<EditorSaveRequest>(editorEvents.save, async event => {
        const request = event.payload
        if (disposed || saving || request.label !== label || request.connection.id !== options.draft.connection.id) return
        saving = true
        let error: string | null = null
        try {
          if (!await options.save(request.connection, request.andConnect)) {
            error = options.saveError() ?? 'Could not save the connection. Please try again.'
          }
        } catch (cause) {
          error = String(cause)
        } finally {
          saving = false
        }
        // The main workspace closes this editor immediately after persistence,
        // before Save & Connect starts its potentially slow network operation.
        if (!disposed) await main.emitTo(label, editorEvents.result, { error })
      }))
      if (disposed) return
      child = new WebviewWindow(label, {
        url: 'index.html?view=connection-editor',
        title: options.draft.editing ? 'Edit Connection — TupleDB' : 'New Connection — TupleDB',
        width: 640,
        height: 560, minWidth: 560, minHeight: 480,
        center: true, decorations: false, resizable: true, maximizable: false,
        minimizable: false, skipTaskbar: true, parent: 'main', visible: true,
        backgroundColor: '#1c1d20',
      })
      // Keep the creation callbacks alive if the host is unmounted while the
      // platform is still creating its window, so that late windows are closed.
      void child.once('tauri://created', () => {
        created = true
        if (disposed) void child?.destroy().catch(() => {})
      })
      void child.once('tauri://error', event => {
        if (disposed) return
        dispose()
        options.failed(event.payload)
      })
      await retain(child.once('tauri://destroyed', () => {
        if (disposed) return
        created = false
        dispose()
        options.closed()
      }))
    } catch (error) {
      if (disposed) return
      dispose()
      options.failed(error)
    }
  })()

  return { dispose }
}
