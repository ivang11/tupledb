import { describe, it, expect, vi } from 'vitest'
import { mount } from '@vue/test-utils'
import KeyboardShortcutsPanel from '@/components/KeyboardShortcutsPanel.vue'
import SettingsDialog from '@/components/dialogs/SettingsDialog.vue'
import { useAppSettings } from '@/composables/useAppSettings'
import { useKeybindings } from '@/composables/useKeybindings'
import { DEFAULT_KEYBINDINGS } from '@/config/keybindingActions'
vi.mock('@tauri-apps/plugin-fs', () => ({ readTextFile: vi.fn(), writeTextFile: vi.fn() }))
const stubs = Object.fromEntries(['Dialog', 'DialogContent', 'DialogHeader', 'DialogTitle', 'DialogDescription', 'DialogFooter'].map(name => [name, { template: '<div><slot /></div>' }]))
describe('Shortcut settings', () => {
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
