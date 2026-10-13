// Mock the Tauri IPC bridge so Vue components can render in happy-dom without
// a real Tauri runtime.  Tests that need specific invoke responses should
// override `tauriHandlers` per-test.

export const tauriHandlers: Record<string, (...args: unknown[]) => unknown> = {}

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(async (cmd: string, args?: unknown) => {
    if (tauriHandlers[cmd]) return tauriHandlers[cmd](args)
    return null
  }),
}))

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async () => () => {}),
  emit: vi.fn(async () => {}),
}))

vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: vi.fn(async () => null),
  save: vi.fn(async () => null),
}))

vi.mock('@tauri-apps/plugin-fs', () => ({
  readTextFile: vi.fn(async () => ''),
}))

vi.mock('@tauri-apps/plugin-notification', () => ({
  sendNotification: vi.fn(),
}))

vi.mock('@tauri-apps/plugin-opener', () => ({
  openUrl: vi.fn(),
}))

// happy-dom 20 no longer exposes a working localStorage on the vitest global;
// the app reads and writes it defensively, but tests need a real store to assert on.
if (typeof (globalThis as any).localStorage?.clear !== 'function') {
  const memory = new Map<string, string>()
  const storage: Storage = {
    get length() { return memory.size },
    clear: () => memory.clear(),
    getItem: (key) => memory.get(String(key)) ?? null,
    key: (index) => [...memory.keys()][index] ?? null,
    removeItem: (key) => { memory.delete(String(key)) },
    setItem: (key, value) => { memory.set(String(key), String(value)) },
  }
  Object.defineProperty(globalThis, 'localStorage', { value: storage, configurable: true })
  if (typeof window !== 'undefined') Object.defineProperty(window, 'localStorage', { value: storage, configurable: true })
}
