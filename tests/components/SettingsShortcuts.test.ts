import { beforeEach, describe, it, expect, vi } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import KeyboardShortcutsPanel from '@/components/KeyboardShortcutsPanel.vue'
import SettingsView from '@/views/SettingsView.vue'
import { useAppSettings } from '@/composables/useAppSettings'
import { useKeybindings } from '@/composables/useKeybindings'
import { DEFAULT_KEYBINDINGS } from '@/config/keybindingActions'

vi.mock('@tauri-apps/plugin-fs', () => ({ readTextFile: vi.fn(), writeTextFile: vi.fn() }))
const transport = vi.hoisted(() => ({ show: vi.fn(), setFocus: vi.fn(), close: vi.fn(), emitTo: vi.fn(), mainFocus: vi.fn() }))
vi.mock('@tauri-apps/api/webviewWindow', () => ({
  getCurrentWebviewWindow: () => ({ label: 'settings', show: transport.show, setFocus: transport.setFocus, close: transport.close, emitTo: transport.emitTo, listen: async () => () => {} }),
  WebviewWindow: { getByLabel: vi.fn(async (label: string) => label === 'main' ? { setFocus: transport.mainFocus } : null) },
}))

beforeEach(() => {
  vi.clearAllMocks()
  for (const fn of [transport.show, transport.setFocus, transport.close, transport.emitTo, transport.mainFocus]) fn.mockResolvedValue(undefined)
})

async function render() {
  const wrapper = mount(SettingsView, { attachTo: document.body, global: { stubs: { WindowResizeHandles: true } } })
  await flushPromises()
  await new Promise(resolve => setTimeout(resolve, 30))
  return wrapper
}

describe('Settings window', () => {
  it('reveals itself after rendering and hands update checks to the main window', async () => {
    const wrapper = await render()
    expect(transport.show).toHaveBeenCalledOnce()
    expect(wrapper.find('[data-slot="dialog-content"]').exists()).toBe(false)
    expect(wrapper.find('[data-tauri-drag-region]').exists()).toBe(true)
    await wrapper.findAll('button').find(button => button.text() === 'Check for updates')!.trigger('click')
    await flushPromises()
    expect(transport.emitTo).toHaveBeenCalledWith('main', 'tupledb:check-updates')
    expect(transport.mainFocus).toHaveBeenCalledOnce()
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }))
    await flushPromises()
    expect(transport.close).toHaveBeenCalledOnce()
    wrapper.unmount()
  })

  it('restores default preferences from General', async () => {
    const { settings } = useAppSettings()
    settings.value.tableFontSize = 16
    const wrapper = await render()
    await wrapper.findAll('button').find(button => button.text() === 'Restore defaults')!.trigger('click')
    expect(settings.value.tableFontSize).toBe(12)
    wrapper.unmount()
  })

  it('opens quick search with the standard Command/Ctrl+P shortcut and supports customization', async () => {
    const kb = useKeybindings(); kb.resetAll()
    expect(DEFAULT_KEYBINDINGS.quickSearch).toBe('Ctrl+P')
    expect(kb.matchesEvent('quickSearch', new KeyboardEvent('keydown', { key: 'p', metaKey: true }))).toBe(true)
    expect(kb.matchesEvent('quickSearch', new KeyboardEvent('keydown', { key: 'p', metaKey: true, shiftKey: true }))).toBe(false)
    const wrapper = await render()
    await wrapper.findAll('[role="tab"]').find(tab => tab.text() === 'Keyboard shortcuts')!.trigger('click')
    expect(wrapper.text()).toContain('Quick Search')
    const binding = wrapper.findAll('button').find(button => button.attributes('title') === 'Click to reassign (current: Ctrl+P)')!
    await binding.trigger('click')
    expect(document.activeElement).toBe(wrapper.findComponent(KeyboardShortcutsPanel).element)
    await wrapper.findComponent(KeyboardShortcutsPanel).trigger('keydown', { key: 'p', ctrlKey: true, shiftKey: true })
    await wrapper.find('[title="Confirm"]').trigger('click')
    expect(kb.getBinding('quickSearch')).toBe('Ctrl+Shift+P')
    expect(kb.matchesEvent('quickSearch', new KeyboardEvent('keydown', { key: 'p', ctrlKey: true, shiftKey: true }))).toBe(true)
    kb.resetAll(); wrapper.unmount()
  })
})
