import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { mount } from '@vue/test-utils'
import { ref } from 'vue'
import Sidebar from '@/components/Sidebar.vue'
import DatabaseContextMenu from '@/components/DatabaseContextMenu.vue'
import ExportDialog from '@/components/dialogs/ExportDialog.vue'
import ConnectionDialog from '@/components/dialogs/ConnectionDialog.vue'
import NewDatabaseDialog from '@/components/dialogs/NewDatabaseDialog.vue'
import { useConnectionStore } from '@/stores/connections'
import { useWorkspace } from '@/composables/useWorkspace'
import { useTableTabs } from '@/composables/useTableTabs'
import { tableHandle } from '@/lib/tableReference'
import type { Connection, DatabaseCapabilities } from '@/types/connection'

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({ invoke }))

const connection: Connection = { id: 'pg', name: 'PostgreSQL', environment: 'LOCAL', database: {
  engine: 'postgresql', settings: { host: 'localhost', port: 5432, user: 'postgres', database: 'app', ssl_mode: 'prefer' },
} }
const capabilities: DatabaseCapabilities = {
  schemas: true, createDatabase: true, databaseCollations: true, editRows: true, alterColumns: true,
  truncateTable: true, disableForeignKeyChecks: false, cancelQuery: true, importSql: true, exportSql: true, inspectDdl: true, estimatedRowCount: true,
}
const tables = ['public', 'sales'].map(schema => ({ name: 'users', table_type: 'BASE TABLE', reference: { catalog: 'app', schema, name: 'users' } }))

beforeEach(() => {
  setActivePinia(createPinia())
  invoke.mockReset()
  invoke.mockImplementation(async command => command === 'get_table_data'
    ? { columns: [{ name: 'id', type_name: 'integer' }], rows: [], total_count: 0 }
    : [])
  const store = useConnectionStore()
  store.openConnections.pg = { connection, capabilities, databases: ['app'], selectedDatabase: 'app', openedDatabases: ['app'], serverVersion: '17', status: 'connected', statusMessage: null, tables: { app: tables } }
})

describe('PostgreSQL workspace', () => {
  const dialogStubs = Object.fromEntries(['Dialog', 'DialogContent', 'DialogHeader', 'DialogTitle', 'DialogDescription', 'DialogFooter'].map(name => [name, { template: '<div><slot /></div>' }]))

  it('labels the connection database optional and saves a blank value as unspecified', async () => {
    const draft = structuredClone(connection)
    draft.name = 'PostgreSQL'
    if (draft.database.engine !== 'postgresql') throw new Error('Expected PostgreSQL')
    delete draft.database.settings.database
    const wrapper = mount(ConnectionDialog, { props: { open: true, connection: draft, isSaving: false }, global: { stubs: dialogStubs } })
    expect(wrapper.text()).toMatch(/Database\s*\(optional\)/)
    expect(wrapper.text()).not.toContain('(required)')
    const input = wrapper.find('input[placeholder="Leave blank to pick after connecting"]')
    await input.setValue('')
    await wrapper.findAll('button').find(button => button.text() === 'Save only')!.trigger('click')
    expect((wrapper.emitted('save')![0][0] as Connection).database.settings).toMatchObject({ host: draft.database.settings.host })
    expect(((wrapper.emitted('save')![0][0] as Connection).database as typeof draft.database).settings.database).toBeUndefined()
    wrapper.unmount()
  })

  it('offers PostgreSQL encoding and collation including the default encoding', async () => {
    const options = { defaultCharacterSet: 'UTF8', defaultCollation: 'C', collations: [
      { name: 'C', characterSet: 'UTF8', isDefault: true },
      { name: 'es-x-icu', characterSet: 'UTF8', isDefault: false },
      { name: 'C', characterSet: 'LATIN1', isDefault: true },
    ] }
    const wrapper = mount(NewDatabaseDialog, { props: {
      open: true, connectionName: 'PostgreSQL', name: 'test_db', encodingLabel: 'Encoding', supportsCollations: true,
      characterSet: '__server_default__', collation: '__server_default__', options, isLoadingOptions: false, optionsError: '', isCreating: false,
    }, global: { stubs: dialogStubs } })
    expect(wrapper.text()).toContain('Encoding')
    expect(wrapper.text()).toContain('Collation')
    expect((wrapper.vm as any).collations.map((c: any) => c.name)).toEqual(['C', 'es-x-icu'])
    await wrapper.setProps({ characterSet: 'LATIN1' })
    expect((wrapper.vm as any).collations.map((c: any) => c.name)).toEqual(['C'])
    await wrapper.find('form').trigger('submit')
    expect(wrapper.emitted('create')).toHaveLength(1)
    wrapper.unmount()
  })

  it('lists databases without selecting one and keeps multiple catalogs in one saved connection', async () => {
    const store = useConnectionStore()
    delete store.openConnections.pg
    const draft = structuredClone(connection)
    if (draft.database.engine !== 'postgresql') throw new Error('Expected PostgreSQL')
    delete draft.database.settings.database
    invoke.mockImplementation(async (command, args) => {
      if (command === 'connect') return { engine: 'postgresql', serverVersion: '17', capabilities }
      if (command === 'get_databases') return ['app', 'other']
      if (command === 'get_tables') return [{ name: 'users', table_type: 'BASE TABLE', reference: { catalog: args.database, schema: 'public', name: 'users' } }]
      return []
    })
    await store.connect(draft)
    expect(store.openConnections.pg.selectedDatabase).toBeNull()
    expect(store.openConnections.pg.openedDatabases).toEqual([])
    expect(store.openConnections.pg.databases).toEqual(['app', 'other'])
    await store.selectDatabase('pg', 'app')
    await store.selectDatabase('pg', 'other')
    expect(Object.keys(store.openConnections)).toEqual(['pg'])
    expect(store.openConnections.pg.openedDatabases).toEqual(['app', 'other'])
    expect(store.tableReference('pg', 'app', 'users').catalog).toBe('app')
    expect(store.tableReference('pg', 'other', 'users').catalog).toBe('other')
  })

  it('resets an unsupported SQL export selection when switching engines', async () => {
    const wrapper = mount(ExportDialog, { props: {
      open: false, database: 'app', tables, selectedTables: tables.map(tableHandle), currentMode: 'data', supportsSql: true,
    } })
    await wrapper.setProps({ supportsSql: false })
    ;(wrapper.vm as any).handleExportStart()
    expect(wrapper.emitted('start')?.[0]?.[0]).toMatchObject({ format: 'csv' })
    wrapper.unmount()
  })

  it('offers SQL export for PostgreSQL with schema and data options', async () => {
    const wrapper = mount(ExportDialog, { props: {
      open: true, database: 'app', tables, selectedTables: tables.map(tableHandle), currentMode: 'full', supportsSql: capabilities.exportSql,
    }, global: { stubs: dialogStubs } })
    expect(wrapper.findAll('button').some(button => button.text().startsWith('SQL'))).toBe(true)
    await wrapper.findAll('button').find(button => button.text().startsWith('Start export'))!.trigger('click')
    expect(wrapper.emitted('start')?.[0]?.[0]).toMatchObject({ format: 'sql', options: { useTransactions: true, includeViews: true } })
    wrapper.unmount()
  })

  it('renders schemas and emits distinct handles for identical table names', async () => {
    const store = useConnectionStore()
    const wrapper = mount(Sidebar, { props: {
      width: 250, search: '', selectedConnectionId: 'pg', openConnections: store.openConnections,
      isTableActive: () => false, isTableOpen: () => false, pendingTableAction: () => null,
      filteredTables: () => tables, isTableSelected: () => false,
    }, global: { stubs: { ScrollArea: { template: '<div><slot /></div>' } } } })
    expect(wrapper.text()).toContain('public')
    expect(wrapper.text()).toContain('sales')
    const buttons = wrapper.findAll('button').filter(button => button.text() === 'users')
    expect(buttons).toHaveLength(2)
    await buttons[0].trigger('click'); await buttons[1].trigger('click')
    expect(wrapper.emitted('load-table')).toEqual(tables.map(table => [tableHandle(table), 'pg', 'app']))
    expect(wrapper.find('[title="New Database"]').exists()).toBe(true)
  })

  it('opens distinct tabs and refreshes the correct schema', async () => {
    const workspace = useWorkspace(ref(null))
    const tabs = useTableTabs(workspace)
    for (const table of tables) await tabs.loadTableData(tableHandle(table), 'pg', 'app')
    expect(workspace.panes.value[0].tabs).toHaveLength(2)
    await tabs.loadTableData(tableHandle(tables[0]), 'pg', 'app')
    expect(workspace.panes.value[0].tabs).toHaveLength(2)
    expect(workspace.getPaneTab(workspace.getPane())?.reference?.schema).toBe('public')
    invoke.mockClear()
    await tabs.refreshActiveTab()
    expect(invoke).toHaveBeenCalledWith('get_table_data', expect.objectContaining({ table: tables[0].reference }))
  })

  it('does not turn unsupported DDL inspection into a connection error', async () => {
    const store = useConnectionStore()
    store.openConnections.pg.capabilities = { ...capabilities, inspectDdl: false }
    expect(await store.fetchTableDdl('pg', 'app', tables[0].reference)).toBeNull()
    expect(invoke).not.toHaveBeenCalled()
    expect(store.openConnections.pg.status).toBe('connected')
  })

  it('fetches PostgreSQL DDL using the schema-qualified table reference', async () => {
    const store = useConnectionStore()
    invoke.mockResolvedValueOnce('CREATE TABLE "public"."users" (id integer);')
    expect(await store.fetchTableDdl('pg', 'app', tables[0].reference)).toContain('CREATE TABLE')
    expect(invoke).toHaveBeenCalledWith('get_table_ddl', expect.objectContaining({ table: tables[0].reference }))
    expect(store.openConnections.pg.status).toBe('connected')
  })

  it('shows object-specific DDL limitations without marking the connection offline', async () => {
    const store = useConnectionStore()
    invoke.mockRejectedValueOnce('Cannot inspect public.users: unsupported object\nDetails')
    expect(await store.fetchTableDdl('pg', 'app', tables[0].reference)).toBe('-- Cannot inspect public.users: unsupported object\n-- Details')
    expect(store.openConnections.pg.status).toBe('connected')
  })

  it('explains unavailable SQL import while retaining database management', async () => {
    const wrapper = mount(DatabaseContextMenu, { props: { show: true, x: 0, y: 0, databaseName: 'app', canImport: false, canDropDatabase: true } })
    const button = wrapper.find('button[title="SQL import is not implemented for this database engine yet"]')
    expect(button.attributes('disabled')).toBeDefined()
    expect(button.text()).toContain('not available yet')
    await button.trigger('click')
    expect(wrapper.emitted('import-sql')).toBeUndefined()
    expect(wrapper.text()).toContain('Drop Database')
    expect(wrapper.text()).toContain('Export')
    await wrapper.setProps({ canImport: true })
    await wrapper.find('button[title="Import SQL"]').trigger('click')
    expect(wrapper.emitted('import-sql')).toHaveLength(1)
  })

  it('enables SQL import for PostgreSQL capabilities', async () => {
    const wrapper = mount(DatabaseContextMenu, { props: { show: true, x: 0, y: 0, databaseName: 'app', canImport: capabilities.importSql, canDropDatabase: true } })
    const button = wrapper.find('button[title="Import SQL"]')
    expect(button.attributes('disabled')).toBeUndefined()
    await button.trigger('click')
    expect(wrapper.emitted('import-sql')).toHaveLength(1)
  })
})
