import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { flushPromises, mount } from '@vue/test-utils'
import { defineComponent, ref } from 'vue'
import WorkspaceConnectionPanel from '@/components/WorkspaceConnectionPanel.vue'
import ConnectionRail from '@/components/ConnectionRail.vue'
import ConnectionStartScreen from '@/components/ConnectionStartScreen.vue'
import { provideWorkspaceConnectionPanelContext } from '@/composables/useWorkspaceConnectionPanelContext'
import { useWorkspaceConnectionState } from '@/composables/useWorkspaceConnectionState'
import { useConnectionStore } from '@/stores/connections'
import type { Connection } from '@/types/connection'

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({ invoke }))

const connection: Connection = {
  id: 'local', name: 'Local', environment: 'LOCAL',
  database: { engine: 'mysql', settings: { host: 'localhost', port: 3306, user: 'root' } },
}

beforeEach(() => {
  setActivePinia(createPinia())
  localStorage.clear()
  invoke.mockReset()
  invoke.mockImplementation(async command => {
    if (command === 'get_databases') return ['app']
    if (command === 'get_tables') return []
    if (command === 'connect') return { serverVersion: '8.4', capabilities: {} }
  })
})

async function setup() {
  const store = useConnectionStore()
  store.connections = [connection]
  const selectedConnectionId = ref<string | null>(null)
  const selectDatabase = vi.fn(async (id: string, database: string) => {
    await store.selectDatabase(id, database)
    selectedConnectionId.value = id
  })
  const Harness = defineComponent({
    components: { WorkspaceConnectionPanel },
    setup() {
      const state = useWorkspaceConnectionState({
        selectedSidebarConnectionId: selectedConnectionId, sidebarToggleVisible: ref(false),
        connectSaved: async conn => {
          await store.connect(conn)
          selectedConnectionId.value = conn.id
          return true
        },
        saveNewConn: vi.fn(), resetWorkspaceState: vi.fn(),
      })
      provideWorkspaceConnectionPanelContext({
        ...state, selectedSidebarConnectionId: selectedConnectionId,
        sidebarRef: ref(null),
        sidebarVisible: ref(true), sidebarWidth: ref(250), search: ref(''), connectingId: ref(null),
        expandedConnections: ref(new Set()), showNewDb: ref(null), newDbName: ref(''),
        handleSelectDatabase: selectDatabase, filteredTables: () => [], isTableSelected: () => false,
        isTableActiveInAnyPane: () => false, isTableOpenInAnyPane: () => false,
        pendingTableAction: () => null,
      })
      return {}
    },
    template: '<div class="flex"><WorkspaceConnectionPanel /></div>',
  })
  const wrapper = mount(Harness, { attachTo: document.body, global: { stubs: { Sidebar: { template: '<aside data-test="tables-sidebar" />' } } } })
  await flushPromises()
  return { wrapper, store, selectDatabase }
}

describe('connection navigation', () => {
  it('shows the connections screen without a rail before opening a connection', async () => {
    const { wrapper } = await setup()
    expect(wrapper.findComponent(ConnectionRail).exists()).toBe(false)
    expect(wrapper.findComponent(ConnectionStartScreen).exists()).toBe(true)
    expect(wrapper.get('[data-test="tables-sidebar"]').isVisible()).toBe(false)
    wrapper.unmount()
  })

  it('opens Home over the workspace and returns using the close button', async () => {
    const { wrapper, store, selectDatabase } = await setup()
    await wrapper.get('[data-connection-id="local"]').trigger('dblclick')
    await flushPromises()
    await store.selectDatabase('local', 'app')
    await flushPromises()
    expect(wrapper.get('[data-test="tables-sidebar"]').isVisible()).toBe(true)
    expect(wrapper.find('button[title="Local / app"]').exists()).toBe(true)
    await wrapper.get('button[title="Workspace"]').trigger('click')
    await flushPromises()
    expect(wrapper.findComponent(ConnectionStartScreen).exists()).toBe(true)
    expect(wrapper.findComponent(ConnectionStartScreen).props('overlay')).toBe(true)
    expect(wrapper.findComponent(ConnectionStartScreen).classes()).toContain('absolute')
    await wrapper.get('button[title="Close"]').trigger('click')
    await flushPromises()
    await wrapper.get('button[title="Local / app"]').trigger('click')
    await flushPromises()
    expect(selectDatabase).toHaveBeenCalledWith('local', 'app')
    expect(wrapper.findComponent(ConnectionStartScreen).exists()).toBe(false)
    expect(wrapper.get('[data-test="tables-sidebar"]').isVisible()).toBe(true)
    wrapper.unmount()
  })

  it('shows a connection before a database is chosen and replaces it when a database opens', async () => {
    const { wrapper, store, selectDatabase } = await setup()
    await wrapper.get('[data-connection-id="local"]').trigger('dblclick')
    await flushPromises()
    expect(store.openConnections.local.selectedDatabase).toBeNull()
    expect(wrapper.find('button[title="Local / Choose database"]').exists()).toBe(true)
    await wrapper.get('button[title="Local / Choose database"]').trigger('click')
    await flushPromises()
    expect(wrapper.findComponent(ConnectionStartScreen).exists()).toBe(false)
    expect(wrapper.get('[data-test="tables-sidebar"]').isVisible()).toBe(true)
    expect(store.openConnections.local.selectedDatabase).toBeNull()
    expect(selectDatabase).not.toHaveBeenCalled()
    await store.selectDatabase('local', 'app')
    await flushPromises()
    expect(wrapper.find('button[title="Local / Choose database"]').exists()).toBe(false)
    expect(wrapper.find('button[title="Local / app"]').exists()).toBe(true)
    wrapper.unmount()
  })

  it('removes the rail and returns Home when the last connection is disconnected', async () => {
    const { wrapper, store } = await setup()
    await wrapper.get('[data-connection-id="local"]').trigger('dblclick')
    await flushPromises()
    await store.selectDatabase('local', 'app')
    store.disconnectConnection('local')
    await flushPromises()
    expect(wrapper.findComponent(ConnectionRail).exists()).toBe(false)
    expect(wrapper.findComponent(ConnectionStartScreen).exists()).toBe(true)
    expect(wrapper.find('button[title="Local / app"]').exists()).toBe(false)
    expect(wrapper.find('button[title="Close"]').exists()).toBe(false)
    wrapper.unmount()
  })

  it('lets a connection without a database be selected alongside an open database', async () => {
    const { wrapper, store } = await setup()
    await wrapper.get('[data-connection-id="local"]').trigger('dblclick')
    await flushPromises()
    await store.selectDatabase('local', 'app')
    const other = { ...connection, id: 'other', name: 'Other' }
    await store.connect(other)
    await flushPromises()
    expect(wrapper.find('button[title="Local / app"]').exists()).toBe(true)
    await wrapper.get('button[title="Other / Choose database"]').trigger('click')
    await flushPromises()
    expect(wrapper.findComponent(ConnectionRail).props('selectedConnectionId')).toBe('other')
    expect(store.openConnections.other.selectedDatabase).toBeNull()
    wrapper.unmount()
  })
})
