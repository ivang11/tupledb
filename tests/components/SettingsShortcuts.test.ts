import { describe, it, expect, vi } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import { useUpdater } from '@/composables/useUpdater'
import KeyboardShortcutsPanel from '@/components/KeyboardShortcutsPanel.vue'
import SettingsDialog from '@/components/dialogs/SettingsDialog.vue'
import { useAppSettings } from '@/composables/useAppSettings'
import { useKeybindings } from '@/composables/useKeybindings'
import { DEFAULT_KEYBINDINGS } from '@/config/keybindingActions'
vi.mock('@tauri-apps/plugin-fs', () => ({ readTextFile: vi.fn(), writeTextFile: vi.fn() }))
vi.mock('@/composables/useUpdater', async () => {
  const { ref } = await import('vue')
  const updater = { checkForUpdates: vi.fn(), isChecking: ref(false), isInstalling: ref(false), dialogOpen: ref(false) }
  return { useUpdater: () => updater }
})
const stubs = Object.fromEntries(['Dialog', 'DialogContent', 'DialogHeader', 'DialogTitle', 'DialogDescription', 'DialogFooter'].map(name => [name, { template: '<div><slot /></div>' }]))
describe('Shortcut settings', () => {
  it('checks for updates from General settings and hands off to the update dialog', async () => {
    const { settingsOpen } = useAppSettings()
    const updater = useUpdater()
    settingsOpen.value = true
    const wrapper = mount(SettingsDialog, { global: { stubs } })
    const button = () => wrapper.findAll('button').find(button => /Check.*updates|Installing update/.test(button.text()))!
    updater.isChecking.value = true
    await wrapper.vm.$nextTick()
    expect(button().attributes('disabled')).toBeDefined()
    updater.isChecking.value = false
    vi.mocked(updater.checkForUpdates).mockImplementationOnce(async () => { updater.dialogOpen.value = true })
    await wrapper.vm.$nextTick()
    await button().trigger('click')
    await flushPromises()
    expect(updater.checkForUpdates).toHaveBeenCalledOnce()
    expect(settingsOpen.value).toBe(false)
    updater.dialogOpen.value = false
    wrapper.unmount()
  })
  it('opens quick search with the standard Command/Ctrl+P shortcut and supports customization', async () => {
    const kb = useKeybindings(); kb.resetAll()
    expect(DEFAULT_KEYBINDINGS.quickSearch).toBe('Ctrl+P')
    expect(kb.matchesEvent('quickSearch', new KeyboardEvent('keydown', { key: 'p', metaKey: true }))).toBe(true)
    expect(kb.matchesEvent('quickSearch', new KeyboardEvent('keydown', { key: 'p', metaKey: true, shiftKey: true }))).toBe(false)
    const { settingsOpen } = useAppSettings(); settingsOpen.value = true
    const wrapper = mount(SettingsDialog, { attachTo: document.body, global: { stubs } })
    await wrapper.findAll('[role="tab"]').find(tab => tab.text() === 'Keyboard shortcuts')!.trigger('click')
    expect(wrapper.text()).toContain('Quick Search')
    const binding = wrapper.findAll('button').find(button => button.attributes('title') === 'Click to reassign (current: Ctrl+P)')!
    await binding.trigger('click')
    expect(document.activeElement).toBe(wrapper.findComponent(KeyboardShortcutsPanel).element)
    await wrapper.findComponent(KeyboardShortcutsPanel).trigger('keydown', { key: 'p', ctrlKey: true, shiftKey: true })
    await wrapper.find('[title="Confirm"]').trigger('click')
    expect(kb.getBinding('quickSearch')).toBe('Ctrl+Shift+P')
    expect(kb.matchesEvent('quickSearch', new KeyboardEvent('keydown', { key: 'p', ctrlKey: true, shiftKey: true }))).toBe(true)
    kb.resetAll(); settingsOpen.value = false; wrapper.unmount()
  })
})
