import { beforeEach, afterEach, describe, it, expect, vi } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import { defineComponent } from 'vue'
import SettingsUpdates from '@/components/SettingsUpdates.vue'
import { useUpdaterBridge } from '@/composables/useUpdaterBridge'
import { useUpdater } from '@/composables/useUpdater'

const bus = vi.hoisted(() => ({ handlers: new Map<string, Set<(event: any) => unknown>>(), emitTo: vi.fn(), check: vi.fn(), download: vi.fn(), relaunch: vi.fn() }))
vi.mock('@tauri-apps/api/event', () => ({
  listen: async (name: string, callback: (event: any) => unknown) => {
    if (!bus.handlers.has(name)) bus.handlers.set(name, new Set())
    bus.handlers.get(name)!.add(callback)
    return () => bus.handlers.get(name)!.delete(callback)
  },
  emit: async (name: string, payload: unknown) => { for (const callback of bus.handlers.get(name) ?? []) void callback({ payload }) },
}))
vi.mock('@tauri-apps/api/webviewWindow', async () => {
  const { listen } = await import('@tauri-apps/api/event')
  return { getCurrentWebviewWindow: () => ({ listen, emitTo: bus.emitTo }) }
})
vi.mock('@tauri-apps/plugin-updater', () => ({ check: bus.check }))
vi.mock('@tauri-apps/plugin-process', () => ({ relaunch: bus.relaunch }))
const wrappers: ReturnType<typeof mount>[] = []
beforeEach(() => {
  bus.handlers.clear(); vi.clearAllMocks()
  bus.emitTo.mockImplementation(async (_window, name, payload) => { for (const callback of bus.handlers.get(name) ?? []) void callback({ payload }) })
  bus.check.mockResolvedValue(null)
  bus.relaunch.mockResolvedValue(undefined)
  const updater = useUpdater()
  updater.isChecking.value = false; updater.isInstalling.value = false; updater.status.value = 'idle'
  updater.dismissUpdate(); updater.errorMessage.value = null; updater.isUpToDate.value = false
})
afterEach(() => { wrappers.forEach(wrapper => wrapper.unmount()); wrappers.length = 0 })
async function render() {
  const owner = mount(defineComponent({ setup() { useUpdaterBridge(); return () => null } }))
  wrappers.push(owner)
  await flushPromises()
  const wrapper = mount(SettingsUpdates)
  wrappers.push(wrapper)
  await flushPromises()
  return wrapper
}
async function click(wrapper: ReturnType<typeof mount>) { await wrapper.get('button').trigger('click'); await flushPromises() }

describe('updates in the Settings window', () => {
  it('shows checking immediately, prevents duplicate requests and reports up to date inline', async () => {
    let finish!: (value: null) => void
    bus.check.mockImplementationOnce(() => new Promise(resolve => { finish = resolve }))
    const wrapper = await render()
    await click(wrapper)
    expect(wrapper.get('button').attributes('disabled')).toBeDefined()
    expect(wrapper.get('[role="status"]').text()).toContain('Checking for updates')
    await click(wrapper)
    expect(bus.check).toHaveBeenCalledOnce()
    finish(null); await flushPromises()
    expect(wrapper.get('[role="status"]').text()).toContain('latest version')
    expect(wrapper.text()).toContain('Installed version:')
    expect(useUpdater().dialogOpen.value).toBe(false)
  })

  it('downloads in the main window and keeps progress and restart controls in Settings', async () => {
    let progress!: (event: any) => void, finish!: () => void
    bus.download.mockImplementationOnce((callback: typeof progress) => { progress = callback; return new Promise<void>(resolve => { finish = resolve }) })
    bus.check.mockResolvedValueOnce({ version: '0.8.5', downloadAndInstall: bus.download })
    const wrapper = await render()
    await click(wrapper)
    expect(wrapper.text()).toContain('New version available: 0.8.5')
    expect(useUpdater().dialogOpen.value).toBe(false)
    await click(wrapper)
    expect(bus.download).toHaveBeenCalledOnce()
    progress({ event: 'Started', data: { contentLength: 100 } })
    progress({ event: 'Progress', data: { chunkLength: 40 } })
    await flushPromises()
    expect(wrapper.get('[role="progressbar"]').attributes('aria-valuenow')).toBe('40')
    expect(wrapper.text()).toContain('40%')
    progress({ event: 'Finished' }); await flushPromises()
    expect(wrapper.text()).toContain('Preparing the update')
    finish(); await flushPromises()
    expect(wrapper.get('button').text()).toBe('Restart to update')
    bus.relaunch.mockRejectedValueOnce(new Error('Restart failed'))
    await click(wrapper)
    expect(wrapper.get('[role="alert"]').text()).toBe('Restart failed')
    expect(wrapper.get('button').attributes('disabled')).toBeUndefined()
    await click(wrapper)
    expect(bus.relaunch).toHaveBeenCalledTimes(2)
  })

  it('shows check and installation errors in Settings and allows retrying', async () => {
    bus.check.mockRejectedValueOnce(new Error('Network unavailable'))
    const wrapper = await render()
    await click(wrapper)
    expect(wrapper.get('[role="alert"]').text()).toBe('Network unavailable')
    expect(wrapper.get('button').text()).toBe('Retry')
    bus.check.mockResolvedValueOnce({ version: '0.8.5', downloadAndInstall: bus.download })
    await click(wrapper)
    expect(wrapper.find('[role="alert"]').exists()).toBe(false)
    bus.download.mockRejectedValueOnce(new Error('Download failed'))
    await click(wrapper)
    expect(wrapper.get('[role="alert"]').text()).toBe('Download failed')
    expect(wrapper.get('button').text()).toBe('Retry update')
    bus.download.mockResolvedValueOnce(undefined)
    await click(wrapper)
    expect(wrapper.find('[role="alert"]').exists()).toBe(false)
    expect(wrapper.get('button').text()).toBe('Restart to update')
  })

  it('recovers progress when Settings is reopened and retains automatic main-window dialogs', async () => {
    bus.check.mockResolvedValueOnce({ version: '0.8.5', downloadAndInstall: bus.download })
    const updater = useUpdater()
    await updater.checkForUpdates({ silent: true })
    expect(updater.dialogOpen.value).toBe(true)
    updater.presentInline()
    updater.isInstalling.value = true; updater.status.value = 'downloading'
    updater.downloadedBytes.value = 50; updater.totalBytes.value = 100
    const wrapper = await render()
    expect(wrapper.get('[role="progressbar"]').attributes('aria-valuenow')).toBe('50')
    expect(wrapper.get('button').attributes('disabled')).toBeDefined()
  })
})
