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
