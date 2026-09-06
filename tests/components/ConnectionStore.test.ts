import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { useConnectionStore } from '@/stores/connections'
import type { Connection, ConnectionInfo } from '@/types/connection'

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({ invoke }))

const connection: Connection = {
  id: 'connection-test', name: 'Local', environment: 'LOCAL',
  database: { engine: 'mysql', settings: { host: 'localhost', port: 3306, user: 'root' } },
}
const info: ConnectionInfo = {
  engine: 'mysql', serverVersion: '8.4',
  capabilities: {
    schemas: false, createDatabase: true, databaseCollations: true, editRows: true,
    alterColumns: true, truncateTable: true, disableForeignKeyChecks: true,
    cancelQuery: true, importSql: true, exportSql: true, estimatedRowCount: true,
  },
}

beforeEach(() => {
  setActivePinia(createPinia())
  invoke.mockReset()
  invoke.mockImplementation(async (command: string) => {
    if (command === 'connect') return structuredClone(info)
    if (command === 'get_databases') return ['app']
    if (command === 'get_available_drivers') return [{ engine: 'mysql', label: 'MySQL', defaultPort: 3306 }]
    if (command === 'get_connections') return [structuredClone(connection)]
  })
})

describe('database connection IPC contract', () => {
  it('passes an explicit schema without flattening table identity at the IPC boundary', async () => {
    const store = useConnectionStore()
    const reference = { catalog: 'app', schema: 'sales', name: 'users' }
    await store.fetchTableData(connection.id, 'app', reference)
    expect(invoke).toHaveBeenCalledWith('get_table_data', expect.objectContaining({ table: reference }))
    await store.alterTableColumn(connection.id, 'app', reference, 'old', 'new', 'text')
    expect(invoke).toHaveBeenCalledWith('alter_table_column', expect.objectContaining({ table: reference }))
  })

  it('refuses an ambiguous bare name before issuing a table operation', async () => {
    const store = useConnectionStore()
    await store.connect(connection)
    store.openConnections[connection.id].tables.app = ['sales', 'support'].map(schema => ({
      name: 'users', table_type: 'BASE TABLE', reference: { catalog: 'app', schema, name: 'users' },
    }))
    invoke.mockClear()
    expect(() => store.tableReference(connection.id, 'app', 'users')).toThrow(/ambiguous/)
    expect(invoke).not.toHaveBeenCalled()
  })

  it('loads available adapters from the backend and retains typed configurations', async () => {
    const store = useConnectionStore()
    await store.fetchConnections()
    expect(store.availableDrivers.map(driver => driver.engine)).toEqual(['mysql'])
    expect(store.connections[0].database.engine).toBe('mysql')
  })

  it('stores server metadata and replaces capabilities on reconnection', async () => {
    const store = useConnectionStore()
    await store.connect(connection)
    expect(store.openConnections[connection.id].serverVersion).toBe('8.4')
    expect(store.openConnections[connection.id].capabilities.cancelQuery).toBe(true)
    expect(store.openConnections[connection.id].databases).toEqual(['app'])
    invoke.mockImplementation(async (command: string) => command === 'connect'
      ? { ...info, capabilities: { ...info.capabilities, cancelQuery: false } }
      : ['app'])
    await store.connect(connection)
    expect(store.openConnections[connection.id].capabilities.cancelQuery).toBe(false)
  })

  it('releases the backend session when disconnecting', async () => {
    const store = useConnectionStore()
    await store.connect(connection)
    store.disconnectConnection(connection.id)
    expect(store.openConnections[connection.id]).toBeUndefined()
    expect(invoke).toHaveBeenCalledWith('disconnect', { connectionId: connection.id })
  })
})
