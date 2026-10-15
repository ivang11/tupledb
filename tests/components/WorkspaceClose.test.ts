import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises, mount } from '@vue/test-utils'
import { createPinia, setActivePinia } from 'pinia'
import WorkspaceView from '@/views/WorkspaceView.vue'
import { useConnectionStore } from '@/stores/connections'
import type { Connection } from '@/types/connection'

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({ invoke, Channel: class { onmessage = null } }))
vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({ onCloseRequested: vi.fn(async () => vi.fn()), destroy: vi.fn() }),
}))

const connection: Connection = {
  id: 'local', name: 'Local', environment: 'LOCAL',
  database: { engine: 'mysql', settings: { host: 'localhost', port: 3306, user: 'root' } },
}

const wrappers: ReturnType<typeof mount>[] = []
beforeEach(() => {
  setActivePinia(createPinia())
  localStorage.clear()
  invoke.mockReset()
  invoke.mockImplementation(async (command: string) => {
    if (command === 'get_connections') return [connection]
    if (command === 'get_connection_storage_info') return { development: true, directory: '' }
    if (command === 'get_available_drivers') return []
    if (command === 'get_databases') return ['app', 'other']
    if (command === 'get_tables') return []
    if (command === 'connect') return { serverVersion: '8.4', capabilities: {} }
    return null
  })
})
afterEach(() => { wrappers.forEach(wrapper => wrapper.unmount()); wrappers.length = 0 })

// Rail, sidebar and panes are async components compiled on demand by Vite, so
// wait on real time until the DOM we need shows up instead of a fixed tick count.
async function settle() {
  await flushPromises()
  await new Promise(resolve => setTimeout(resolve, 0))
  await flushPromises()
}

async function waitFor(predicate: () => boolean, timeoutMs = 5000) {
  const start = Date.now()
  while (!predicate()) {
    if (Date.now() - start > timeoutMs) throw new Error('Timed out waiting for the workspace to render')
    await new Promise(resolve => setTimeout(resolve, 20))
    await flushPromises()
  }
}

async function openWorkspace() {
  const store = useConnectionStore()
  const wrapper = mount(WorkspaceView, { attachTo: document.body })
  wrappers.push(wrapper)
  await settle()
  await wrapper.get('[data-connection-id="local"]').trigger('dblclick')
  await waitFor(() => wrapper.find('button[title^="Local / "]').exists())
  return { wrapper, store }
}

function menuItem(wrapper: ReturnType<typeof mount>, label: string) {
  return wrapper.findAll('[role="menu"] button').find(button => button.text().includes(label))!
}

describe('closing databases and connections', () => {
  it('closes one of two open databases and keeps the other', async () => {
    const { wrapper, store } = await openWorkspace()
    await wrapper.get('button[title="Local / Choose database"]').trigger('click')
    await store.selectDatabase('local', 'app')
    await store.selectDatabase('local', 'other')
    await waitFor(() => wrapper.find('button[title="Local / other"]').exists())
    expect(store.openConnections.local.openedDatabases).toEqual(['app', 'other'])

    await waitFor(() => wrapper.find('[data-table-list]').exists())
    await wrapper.get('[data-table-list]').trigger('contextmenu', { clientX: 10, clientY: 10 })
    await waitFor(() => wrapper.find('[role="menu"]').exists())
    await menuItem(wrapper, 'Close DB').trigger('click')
    await settle()

    expect(store.openConnections.local.openedDatabases).toEqual(['app'])
    expect(wrapper.find('button[title="Local / other"]').exists()).toBe(false)
    expect(wrapper.find('button[title="Local / app"]').exists()).toBe(true)
  })

  it('disconnects when the last database is closed', async () => {
    const { wrapper, store } = await openWorkspace()
    await store.selectDatabase('local', 'app')
    await waitFor(() => wrapper.find('button[title="Local / app"]').exists())

    await waitFor(() => wrapper.find('[data-table-list]').exists())
    await wrapper.get('[data-table-list]').trigger('contextmenu', { clientX: 10, clientY: 10 })
    await waitFor(() => wrapper.find('[role="menu"]').exists())
    await menuItem(wrapper, 'Close DB').trigger('click')
    await settle()

    expect(store.openConnections.local).toBeUndefined()
    expect(wrapper.find('button[title="Local / app"]').exists()).toBe(false)
  })

  it('disconnects a connection from its context menu', async () => {
    const { wrapper, store } = await openWorkspace()
    await wrapper.get('button[title="Local / Choose database"]').trigger('contextmenu', { clientX: 10, clientY: 10 })
    await waitFor(() => wrapper.find('[role="menu"]').exists())
    await menuItem(wrapper, 'Close connection').trigger('click')
    await settle()

    expect(store.openConnections.local).toBeUndefined()
    expect(invoke).toHaveBeenCalledWith('disconnect', { connectionId: 'local' })
  })
})

describe('connection context menu variants', () => {
  it('shows only session actions from the rail and the full menu from the connections list', async () => {
    const { wrapper } = await openWorkspace()
    await wrapper.get('button[title="Local / Choose database"]').trigger('contextmenu', { clientX: 10, clientY: 10 })
    await waitFor(() => wrapper.find('[role="menu"]').exists())
    const railItems = wrapper.findAll('[role="menu"] button').map(button => button.text().trim())
    expect(railItems).toEqual(['Close connection', 'Close other connections'])
    expect(menuItem(wrapper, 'Close other connections').attributes('disabled')).toBeDefined()
    await menuItem(wrapper, 'Close connection').trigger('click')
    await settle()
    await new Promise(resolve => setTimeout(resolve, 0))
    await settle()

    await waitFor(() => wrapper.find('[data-connection-id="local"]').exists())
    await wrapper.get('[data-connection-id="local"]').trigger('contextmenu', { clientX: 10, clientY: 10 })
    await waitFor(() => wrapper.find('[role="menu"]').exists())
    const listItems = wrapper.findAll('[role="menu"] button').map(button => button.text().trim())
    expect(listItems).toContain('Edit Connection')
    expect(listItems).toContain('Delete Connection')
    expect(listItems).not.toContain('Close other connections')
  })
})
