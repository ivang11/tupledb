import { afterEach, describe, expect, it, vi } from 'vitest'
import { DOMWrapper, flushPromises, mount } from '@vue/test-utils'
import UpdateNotice from '@/components/UpdateNotice.vue'
import { useUpdater } from '@/composables/useUpdater'
const native = vi.hoisted(() => ({ check: vi.fn(), relaunch: vi.fn() }))
vi.mock('@tauri-apps/plugin-updater', () => ({ check: native.check }))
vi.mock('@tauri-apps/plugin-process', () => ({ relaunch: native.relaunch }))
const wrappers: ReturnType<typeof mount>[] = []
afterEach(() => { wrappers.forEach(wrapper => wrapper.unmount()); wrappers.length = 0; vi.useRealTimers(); vi.clearAllMocks() })
function notice() { return new DOMWrapper(document.querySelector('[data-update-notice]') as HTMLElement) }
function render() {
  const wrapper = mount(UpdateNotice, { attachTo: document.body, props: { open: true, version: '0.8.7', status: 'available', busy: false, downloadedBytes: 0, totalBytes: 0, downloadPercent: 0, errorMessage: null } })
  wrappers.push(wrapper)
  return wrapper
}

describe('non-modal update notice', () => {
  it('keeps focus and background controls usable and ignores outside clicks and Escape', async () => {
    const background = document.createElement('input')
    document.body.append(background); background.focus()
    const wrapper = render()
    expect(document.activeElement).toBe(background)
    expect(document.querySelector('[data-slot="dialog-overlay"]')).toBeNull()
    expect(notice().attributes('aria-modal')).toBeUndefined()
    background.value = 'continue working'
    background.dispatchEvent(new MouseEvent('click', { bubbles: true }))
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }))
    await flushPromises()
    expect(wrapper.emitted('dismiss')).toBeUndefined()
    expect(notice().exists()).toBe(true)
    expect(document.activeElement).toBe(background)
    await notice().find('[aria-label="Close update notice"]').trigger('click')
    expect(wrapper.emitted('dismiss')).toHaveLength(1)
    await notice().findAll('button').find(button => button.text() === 'Later')!.trigger('click')
    expect(wrapper.emitted('dismiss')).toHaveLength(2)
    background.remove()
  })

  it('shows real progress and allows dismissing while downloading', async () => {
    const wrapper = render()
    await wrapper.setProps({ status: 'downloading', busy: true, downloadedBytes: 40, totalBytes: 100, downloadPercent: 40 })
    expect(notice().find('[role="progressbar"]').attributes('aria-valuenow')).toBe('40')
    expect(notice().findAll('button').find(button => button.text() === 'Updating…')!.attributes('disabled')).toBeDefined()
    await notice().find('[aria-label="Close update notice"]').trigger('click')
    expect(wrapper.emitted('dismiss')).toHaveLength(1)
    await wrapper.setProps({ status: 'readyToRestart', busy: false })
    await notice().findAll('button').find(button => button.text() === 'Restart now')!.trigger('click')
    expect(wrapper.emitted('restart')).toHaveLength(1)
  })

  it('retains the real background update when dismissed and can reopen it', async () => {
    const updater = useUpdater()
    updater.isChecking.value = false; updater.isInstalling.value = true; updater.status.value = 'downloading'
    updater.availableUpdate.value = { version: '0.8.7' } as any
    updater.downloadedBytes.value = 40; updater.totalBytes.value = 100; updater.dialogOpen.value = true
    updater.dismissUpdate()
    expect(updater.dialogOpen.value).toBe(false)
    expect(updater.isInstalling.value).toBe(true)
    expect(updater.availableUpdate.value?.version).toBe('0.8.7')
    await updater.checkForUpdates()
    expect(updater.dialogOpen.value).toBe(true)
    expect(updater.downloadPercent.value).toBe(40)
    expect(native.check).not.toHaveBeenCalled()
    updater.isInstalling.value = false; updater.status.value = 'idle'; updater.availableUpdate.value = null; updater.dismissUpdate()
  })

})
