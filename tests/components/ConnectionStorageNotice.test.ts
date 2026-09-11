import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { flushPromises, mount } from '@vue/test-utils'
import ConnectionStorageNotice from '@/components/ConnectionStorageNotice.vue'
import { useConnectionStore } from '@/stores/connections'

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({ invoke }))

beforeEach(() => {
  setActivePinia(createPinia())
  invoke.mockReset()
  invoke.mockResolvedValue([])
})

describe('connection storage notices', () => {
  it('explains why a development build has separate connections', () => {
    const view = mount(ConnectionStorageNotice, { props: { development: true } })
    expect(view.text()).toContain('separate saved connections')
    expect(view.text()).toContain('Import a connections export')
    expect(view.find('[role="alert"]').exists()).toBe(false)
    view.unmount()
  })

  it('shows a persistent error with a working reload action', async () => {
    const store = useConnectionStore()
    store.storageError = 'Invalid format at /isolated/connections.v2.json. Saving is blocked.'
    const view = mount(ConnectionStorageNotice, { props: { development: false } })
    expect(view.get('[role="alert"]').text()).toContain('Saving is blocked')
    await view.get('button').trigger('click')
    await flushPromises()
    expect(invoke).toHaveBeenCalledWith('get_connections')
    expect(view.find('[role="alert"]').exists()).toBe(false)
    view.unmount()
  })

  it('keeps the warning visible if reloading still fails', async () => {
    const store = useConnectionStore()
    store.storageError = 'Initial storage failure'
    invoke.mockRejectedValue('Storage still unreadable')
    const view = mount(ConnectionStorageNotice, { props: { development: false } })
    await view.get('button').trigger('click')
    await flushPromises()
    expect(view.get('[role="alert"]').text()).toContain('Storage still unreadable')
    view.unmount()
  })
})
