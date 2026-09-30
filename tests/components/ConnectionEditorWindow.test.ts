import { beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises } from '@vue/test-utils'
import { editorEvents, openConnectionEditorWindow } from '@/lib/connectionEditorWindow'

const transport = vi.hoisted(() => ({
  mainListeners: new Map<string, (event: any) => any>(),
  childListeners: new Map<string, (event: any) => any>(),
  emitTo: vi.fn(), destroy: vi.fn(), create: vi.fn(),
}))
vi.mock('@tauri-apps/api/webviewWindow', () => ({
  getCurrentWebviewWindow: () => ({
    listen: async (name: string, handler: (event: any) => any) => {
      transport.mainListeners.set(name, handler)
      return () => transport.mainListeners.delete(name)
    },
    emitTo: transport.emitTo,
  }),
  WebviewWindow: class {
    destroy = transport.destroy
    constructor(label: string, options: any) { transport.create(label, options) }
    async once(name: string, handler: (event: any) => any) {
      transport.childListeners.set(name, handler)
      return () => transport.childListeners.delete(name)
    }
  },
}))

function setup(save = vi.fn(async () => false)) {
  const draft = {
    connection: { id: 'new', name: 'Local', environment: 'LOCAL' as const,
      database: { engine: 'mysql' as const, settings: { host: 'localhost', port: 3306, user: 'root', password: 'fixture-password' } } },
    editing: false, drivers: [{ engine: 'mysql' as const, label: 'MySQL', defaultPort: 3306 }], showConnectButton: true,
  }
  const closed = vi.fn()
  const failed = vi.fn()
  const session = openConnectionEditorWindow({ draft, save, saveError: () => 'Storage unavailable', closed, failed })
  return { session, draft, save, closed, failed }
}

beforeEach(() => {
  transport.mainListeners.clear()
  transport.childListeners.clear()
  vi.clearAllMocks()
  transport.emitTo.mockResolvedValue(undefined)
  transport.destroy.mockResolvedValue(undefined)
})

describe('native connection editor lifecycle', () => {
  it('creates a separate window without a title bar and transfers its draft only to the matching editor', async () => {
    const { session, draft } = setup()
    await flushPromises()
    const [label, options] = transport.create.mock.calls[0]
    expect(options).toMatchObject({ decorations: false, parent: 'main', width: 640, height: 560, resizable: true })
    expect(options.url).not.toContain(draft.connection.database.settings.password)
    expect(transport.emitTo).not.toHaveBeenCalled()
    await transport.mainListeners.get(editorEvents.ready)!({ payload: { label: 'unrelated-editor' } })
    expect(transport.emitTo).not.toHaveBeenCalled()
    await transport.mainListeners.get(editorEvents.ready)!({ payload: { label } })
    expect(transport.emitTo).toHaveBeenCalledWith(label, editorEvents.initialize, draft)
    session.dispose()
  })

  it('reports a persistence failure to the editor so it can retain the entered settings and retry', async () => {
    const { session, draft, save } = setup()
    await flushPromises()
    const label = transport.create.mock.calls[0][0]
    transport.childListeners.get('tauri://created')!({})
    const changed = { ...draft.connection, name: 'Changed name' }
    await transport.mainListeners.get(editorEvents.save)!({ payload: { label, connection: changed, andConnect: true } })
    expect(save).toHaveBeenCalledWith(changed, true)
    expect(transport.emitTo).toHaveBeenCalledWith(label, editorEvents.result, { error: 'Storage unavailable' })
    expect(transport.destroy).not.toHaveBeenCalled()
    session.dispose()
  })

  it('ignores mismatched editor or connection identities', async () => {
    const { session, draft, save } = setup()
    await flushPromises()
    const label = transport.create.mock.calls[0][0]
    const handler = transport.mainListeners.get(editorEvents.save)!
    await handler({ payload: { label: 'old-editor', connection: draft.connection, andConnect: false } })
    await handler({ payload: { label, connection: { ...draft.connection, id: 'other' }, andConnect: false } })
    expect(save).not.toHaveBeenCalled()
    session.dispose()
  })

  it('closes after persistence even while connecting is still pending and ignores duplicate saves', async () => {
    let finish!: (value: boolean) => void
    const pending = new Promise<boolean>(resolve => { finish = resolve })
    const { session, draft, save } = setup(vi.fn(() => pending))
    await flushPromises()
    transport.childListeners.get('tauri://created')!({})
    const label = transport.create.mock.calls[0][0]
    const handler = transport.mainListeners.get(editorEvents.save)!
    const event = { payload: { label, connection: draft.connection, andConnect: true } }
    const first = handler(event)
    await handler(event)
    expect(save).toHaveBeenCalledTimes(1)
    session.dispose()
    expect(transport.destroy).toHaveBeenCalledTimes(1)
    finish(true)
    await first
    expect(transport.emitTo).not.toHaveBeenCalled()
    expect(transport.mainListeners.size).toBe(0)
  })

  it('destroys a window that finishes creating after its host was closed', async () => {
    const { session } = setup()
    await flushPromises()
    session.dispose()
    expect(transport.destroy).not.toHaveBeenCalled()
    transport.childListeners.get('tauri://created')!({})
    expect(transport.destroy).toHaveBeenCalledTimes(1)
  })

  it('updates the parent when the native close button is used and releases its listeners', async () => {
    const { closed, session } = setup()
    await flushPromises()
    transport.childListeners.get('tauri://created')!({})
    transport.childListeners.get('tauri://destroyed')!({})
    expect(closed).toHaveBeenCalledTimes(1)
    expect(transport.mainListeners.size).toBe(0)
    session.dispose()
    expect(transport.destroy).not.toHaveBeenCalled()
  })

  it('allows the host to fall back to the form if native window creation fails', async () => {
    const { failed } = setup()
    await flushPromises()
    transport.childListeners.get('tauri://error')!({ payload: 'Window creation rejected' })
    expect(failed).toHaveBeenCalledWith('Window creation rejected')
    expect(transport.mainListeners.size).toBe(0)
  })
})
