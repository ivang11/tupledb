import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { flushPromises, mount } from '@vue/test-utils'
import { defineComponent, ref } from 'vue'
import ConnectionDialog from '@/components/dialogs/ConnectionDialog.vue'
import ConnectionStartScreen from '@/components/ConnectionStartScreen.vue'
import { useSidebarManager } from '@/composables/useSidebarManager'
import { useWorkspaceConnectionState } from '@/composables/useWorkspaceConnectionState'
import { useConnectionStore } from '@/stores/connections'
import { useToast } from '@/composables/useToast'
import type { Connection, ConnectionInfo } from '@/types/connection'

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({ invoke }))

const info: ConnectionInfo = {
  engine: 'mysql', serverVersion: '8.4',
  capabilities: {
    schemas: false, createDatabase: true, databaseCollations: true, editRows: true,
    alterColumns: true, truncateTable: true, disableForeignKeyChecks: true,
    cancelQuery: true, importSql: true, exportSql: true, estimatedRowCount: true,
  },
}

let saved: Connection[]
beforeEach(() => {
  setActivePinia(createPinia())
  localStorage.clear()
  useToast().toasts.value = []
  saved = []
  invoke.mockReset()
  invoke.mockImplementation(async (command, args) => {
    if (command === 'add_connection') saved = [structuredClone(args.connection)]
    if (command === 'get_connections') return saved
    if (command === 'get_available_drivers') return [{ engine: 'mysql', label: 'MySQL', defaultPort: 3306 }]
    if (command === 'get_connection_storage_info') return { development: true, directory: '/test' }
    if (command === 'connect') return info
    if (command === 'get_databases') return ['app']
  })
})

async function setup() {
  let sidebar!: ReturnType<typeof useSidebarManager>
  let state!: ReturnType<typeof useWorkspaceConnectionState>
  const Harness = defineComponent({
    components: { ConnectionDialog, ConnectionStartScreen },
    setup() {
      sidebar = useSidebarManager({
        panes: ref([]), activePaneId: ref('pane'), getPane: vi.fn(), getPaneTab: vi.fn(),
        switchToTab: vi.fn(), closeTab: vi.fn(), refreshActiveTab: vi.fn(),
        loadTableData: vi.fn(), openQueryTab: vi.fn(),
      })
      state = useWorkspaceConnectionState({
        selectedSidebarConnectionId: sidebar.selectedSidebarConnectionId,
        sidebarToggleVisible: ref(false), connectSaved: sidebar.connectSaved,
        saveNewConn: sidebar.saveNewConn, resetWorkspaceState: vi.fn(),
      })
      return { ...sidebar, ...state, store: useConnectionStore() }
    },
    template: `
      <ConnectionStartScreen v-if="showConnectionManager" :connections="store.connections"
        :open-connection-ids="openConnectionIds" :selected-connection-id="selectedSidebarConnectionId"
        :connecting-id="connectingId" @connect-saved="connectFromManager" />
      <ConnectionDialog v-if="showNewConnDialog" :open="showNewConnDialog" :connection="newConn"
        :is-saving="isSavingConn" :show-connect-button="true" @save="saveNewConn" />
    `,
  })
  const dialogStubs = Object.fromEntries(
    ['Dialog', 'DialogContent', 'DialogHeader', 'DialogTitle', 'DialogDescription']
      .map(name => [name, { template: '<div><slot /></div>' }]),
  )
  const wrapper = mount(Harness, { global: { stubs: dialogStubs } })
  await flushPromises()
  sidebar.openNewConnDialog()
  sidebar.newConn.value.name = 'New connection'
  await flushPromises()
  return { wrapper, sidebar, state, store: useConnectionStore() }
}

describe('creating a connection', () => {
  it('closes the dialog while connecting and opens the workspace after the backend finishes', async () => {
    const { wrapper, sidebar, state, store } = await setup()
    let finish!: (value: ConnectionInfo) => void
    const pending = new Promise<ConnectionInfo>(resolve => { finish = resolve })
    const fallback = invoke.getMockImplementation()!
    invoke.mockImplementation((command, args) => command === 'connect' ? pending : fallback(command, args))
    await wrapper.findAll('button').find(button => button.text() === 'Save & Connect')!.trigger('click')
    await flushPromises()
    expect(wrapper.findComponent(ConnectionDialog).exists()).toBe(false)
    expect(wrapper.findComponent(ConnectionStartScreen).text()).toContain('connecting…')
    expect(store.openConnections).toEqual({})
    finish(info)
    await flushPromises()
    expect(invoke).toHaveBeenCalledWith('connect', { connection: expect.objectContaining({ id: saved[0].id }) })
    expect(store.openConnections[saved[0].id].databases).toEqual(['app'])
    expect(sidebar.selectedSidebarConnectionId.value).toBe(saved[0].id)
    expect(sidebar.connectingId.value).toBeNull()
    expect(state.showConnectionManager.value).toBe(false)
    expect(wrapper.findComponent(ConnectionStartScreen).exists()).toBe(false)
    wrapper.unmount()
  })

  it('saves without connecting when Save only is selected', async () => {
    const { wrapper, state, store } = await setup()
    await wrapper.findAll('button').find(button => button.text() === 'Save only')!.trigger('click')
    await flushPromises()
    expect(store.connections).toHaveLength(1)
    expect(invoke.mock.calls.some(([command]) => command === 'connect')).toBe(false)
    expect(wrapper.findComponent(ConnectionDialog).exists()).toBe(false)
    expect(state.showConnectionManager.value).toBe(true)
    wrapper.unmount()
  })

  it('reconnects an open connection with its updated settings', async () => {
    const { wrapper, sidebar, state, store } = await setup()
    await wrapper.findAll('button').find(button => button.text() === 'Save & Connect')!.trigger('click')
    await flushPromises()
    sidebar.openEditConnDialog(store.connections[0])
    await flushPromises()
    await wrapper.find('input[placeholder="127.0.0.1"]').setValue('new-host')
    invoke.mockClear()
    await wrapper.findAll('button').find(button => button.text() === 'Save & Connect')!.trigger('click')
    await flushPromises()
    expect(invoke).toHaveBeenCalledWith('connect', { connection: expect.objectContaining({
      id: saved[0].id, database: expect.objectContaining({ settings: expect.objectContaining({ host: 'new-host' }) }),
    }) })
    expect(store.openConnections[saved[0].id].connection.database.settings).toMatchObject({ host: 'new-host' })
    expect(sidebar.selectedSidebarConnectionId.value).toBe(saved[0].id)
    expect(state.showConnectionManager.value).toBe(false)
    wrapper.unmount()
  })

  it.each(['connect', 'get_databases'])('clears the connecting status and allows retry after %s fails', async failedCommand => {
    const { wrapper, sidebar, state, store } = await setup()
    const fallback = invoke.getMockImplementation()!
    invoke.mockImplementation((command, args) => command === failedCommand
      ? Promise.reject('Server unavailable') : fallback(command, args))
    await wrapper.findAll('button').find(button => button.text() === 'Save & Connect')!.trigger('click')
    await flushPromises()
    expect(store.connections).toHaveLength(1)
    expect(sidebar.connectingId.value).toBeNull()
    expect(state.showConnectionManager.value).toBe(true)
    expect(useToast().toasts.value.at(-1)?.message).toContain('Server unavailable')
    invoke.mockImplementation(fallback)
    invoke.mockClear()
    await state.connectFromManager(store.connections[0])
    await flushPromises()
    expect(invoke).toHaveBeenCalledWith('connect', { connection: store.connections[0] })
    expect(store.openConnections[saved[0].id].status).toBe('connected')
    expect(store.openConnections[saved[0].id].databases).toEqual(['app'])
    expect(state.showConnectionManager.value).toBe(false)
    wrapper.unmount()
  })

  it('keeps the dialog open and does not connect when saving fails', async () => {
    const { wrapper, sidebar, state } = await setup()
    invoke.mockRejectedValueOnce('Cannot save connections')
    await wrapper.findAll('button').find(button => button.text() === 'Save & Connect')!.trigger('click')
    await flushPromises()
    expect(wrapper.findComponent(ConnectionDialog).exists()).toBe(true)
    expect(invoke.mock.calls.some(([command]) => command === 'connect')).toBe(false)
    expect(sidebar.isSavingConn.value).toBe(false)
    expect(state.showConnectionManager.value).toBe(true)
    wrapper.unmount()
  })
})
