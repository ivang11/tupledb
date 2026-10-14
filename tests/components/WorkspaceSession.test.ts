import { beforeEach, afterEach, describe, it, expect, vi } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import { createPinia, setActivePinia } from 'pinia'
import { defineComponent, ref } from 'vue'
import { useWorkspaceSession } from '@/composables/useWorkspaceSession'
import { useUnsavedChangesGuard } from '@/composables/useUnsavedChangesGuard'
import { useAppSettings } from '@/composables/useAppSettings'
import { useConnectionStore } from '@/stores/connections'
import type { Connection } from '@/types/connection'
import type { PaneState, TableTab } from '@/types/workspace'

const { invoke, close } = vi.hoisted(() => ({ invoke: vi.fn(), close: { callback: null as any, destroy: vi.fn() } }))
vi.mock('@tauri-apps/api/core', () => ({ invoke }))
vi.mock('@tauri-apps/api/window', () => ({ getCurrentWindow: () => ({ onCloseRequested: vi.fn(async callback => { close.callback = callback; return vi.fn() }), destroy: close.destroy }) }))
const connection: Connection = { id: 'local', name: 'Local', environment: 'LOCAL', database: { engine: 'mysql', settings: { host: 'localhost', port: 3306, user: 'root', password: 'never-store-this' } } }
const wrappers: ReturnType<typeof mount>[] = []
beforeEach(() => {
  setActivePinia(createPinia()); localStorage.clear(); useAppSettings().reset(); close.callback = null; close.destroy.mockReset()
  invoke.mockReset(); invoke.mockImplementation(async command => command === 'get_connections' ? [connection] : command === 'get_table_data' ? { columns: [{ name: 'id' }], rows: [[1]], total_count: 1 } : [])
})
afterEach(() => { wrappers.forEach(wrapper => wrapper.unmount()); wrappers.length = 0 })
function table(): TableTab { return {
  type: 'table', id: 'table', connectionId: 'local', database: 'app', tableName: 'users', page: 2, pageSize: 50, viewMode: 'content',
  queryResult: { columns: [{ name: 'id' }], rows: [['result-not-to-save']] }, tableStructure: [], tableIndexes: [], foreignKeys: [], ddl: null,
  filters: { match_all: true, rows: [{ active: true, column: 'id', operator: 'equals', value: '1' }] }, sortColumn: 'id', sortDesc: true,
  pendingChanges: { '1': { name: 'Draft' } }, pendingStructureChanges: {}, pendingDeletions: {}, pendingInserts: [], pendingDrop: false, pendingTruncate: false,
  selectedRowPk: null, selectedRowPks: [], inlineEditColumn: null,
} }
async function setup() {
  const store = useConnectionStore(), guard = useUnsavedChangesGuard()
  const panes = ref<PaneState[]>([{ id: 'pane', tabs: [], activeTabId: null, viewMode: 'content', page: 0, pageSize: 300, showFilters: false }])
  const connect = vi.fn(async () => { store.openConnections.local = { connection, databases: ['app'], selectedDatabase: 'app', openedDatabases: ['app'], status: 'connected', statusMessage: null, serverVersion: '8', capabilities: {} as any, tables: { app: [] } }; return true })
  let session!: ReturnType<typeof useWorkspaceSession>
  const showWorkspace = vi.fn()
  const wrapper = mount(defineComponent({ setup() {
    session = useWorkspaceSession({ panes, activePaneId: ref('pane'), paneWidths: ref([1]), focusedPaneId: ref(null), sidebarVisible: ref(true), selectedConnectionId: ref(null), connect, showWorkspace, requestClose: guard.request })
    return {}
  }, template: '<div />' }))
  wrappers.push(wrapper); await flushPromises()
  return { panes, connect, session, store, guard, showWorkspace }
}
describe('workspace recovery', () => {
  it('stores SQL drafts and pending changes while excluding credentials and result rows', async () => {
    const { panes, session } = await setup()
    panes.value[0].tabs = [table(), { type: 'query', id: 'query', connectionId: 'local', database: 'app', sql: 'SELECT * FROM users', queryResult: { rows: [['query-result']] } }]
    session.save()
    const text = localStorage.getItem('tupledb:workspace-session:v1')!
    expect(text).toContain('SELECT * FROM users'); expect(text).toContain('Draft')
    expect(text).not.toContain('result-not-to-save'); expect(text).not.toContain('query-result'); expect(text).not.toContain('never-store-this')
  })
  it('restores table filters and drafts without executing SQL', async () => {
    localStorage.setItem('tupledb:workspace-session:v1', JSON.stringify({ version: 1, connections: [{ id: 'local', database: 'app' }], activePaneId: 'restored', panes: [{ id: 'restored', activeTabId: 'table', page: 2, pageSize: 50, viewMode: 'content', showFilters: true, tabs: [table(), { type: 'query', id: 'draft', connectionId: 'local', database: 'app', sql: 'DELETE FROM users' }] }] }))
    const { panes, connect, showWorkspace } = await setup()
    expect(connect).toHaveBeenCalledOnce(); expect(showWorkspace).toHaveBeenCalledOnce()
    expect(panes.value[0].tabs[1]).toMatchObject({ sql: 'DELETE FROM users', queryResult: null })
    expect(invoke.mock.calls.some(([command]) => command === 'execute_query')).toBe(false)
    expect(invoke).toHaveBeenCalledWith('get_table_data', expect.objectContaining({ page: 2, pageSize: 50, sortColumn: 'id', sortDesc: true }))
    expect(invoke).toHaveBeenCalledWith('get_foreign_keys', expect.objectContaining({ database: 'app', table: expect.objectContaining({ name: 'users' }) }))
  })
  it('keeps pending changes when close is cancelled and discards them before native close', async () => {
    const { panes, guard } = await setup()
    panes.value[0].tabs = [table()]
    const preventDefault = vi.fn()
    close.callback({ preventDefault }); expect(preventDefault).toHaveBeenCalledOnce(); expect(close.destroy).not.toHaveBeenCalled()
    guard.cancel(); expect((panes.value[0].tabs[0] as TableTab).pendingChanges).toHaveProperty('1')
    close.callback({ preventDefault }); guard.discardAndContinue()
    expect(close.destroy).toHaveBeenCalledOnce()
    expect(JSON.parse(localStorage.getItem('tupledb:workspace-session:v1')!).panes[0].tabs[0].pendingChanges).toEqual({})
  })
})
