import type { TableRef, DatabaseTable, ColumnStructure, ForeignKeyColumn } from '@/types/database'
import { resolveTableReference } from '@/lib/tableReference'
import { defineStore } from 'pinia'
import { ref } from 'vue'
import type { Connection, ConnectionInfo, ConnectionTestProgress, DatabaseCapabilities, DriverDescriptor } from '@/types/connection'
import { Channel, invoke } from '@tauri-apps/api/core'

interface OpenConnectionState {
  connection: Connection
  databases: string[]
  selectedDatabase: string | null
  openedDatabases: string[]
  serverVersion: string | null
  capabilities: DatabaseCapabilities
  status: 'connected' | 'error'
  statusMessage: string | null
  tables: Record<string, DatabaseTable[]>
}

export const useConnectionStore = defineStore('connections', () => {
  const connections = ref<Connection[]>([])
  const storageError = ref<string | null>(null)
  const storageInfo = ref<{ development: boolean; directory: string } | null>(null)
  const availableDrivers = ref<DriverDescriptor[]>([])
  const openConnections = ref<Record<string, OpenConnectionState>>({})

  function markConnectionConnected(connectionId: string) {
    if (!openConnections.value[connectionId]) return
    openConnections.value[connectionId].status = 'connected'
    openConnections.value[connectionId].statusMessage = null
  }

  function markConnectionError(connectionId: string, error: unknown) {
    if (!openConnections.value[connectionId]) return
    openConnections.value[connectionId].status = 'error'
    openConnections.value[connectionId].statusMessage = String(error)
  }

  // ── Connection management ──────────────────────────────────────────────────

  async function fetchConnections() {
    try {
      storageInfo.value = await invoke('get_connection_storage_info')
      availableDrivers.value = await invoke<DriverDescriptor[]>('get_available_drivers')
      connections.value = await invoke<Connection[]>('get_connections')
      storageError.value = null
      for (const connection of connections.value) {
        if (openConnections.value[connection.id]) {
          openConnections.value[connection.id].connection = connection
        }
      }
    } catch (error) {
      storageError.value = String(error)
      console.error('Failed to fetch connections:', error)
    }
  }

  async function writeConnections(command: string, args: Record<string, unknown>) {
    try {
      const result = await invoke(command, args)
      storageError.value = null
      return result
    } catch (error) {
      storageError.value = String(error)
      throw error
    }
  }

  async function addConnection(connection: Connection) {
    await writeConnections('add_connection', { connection })
    await fetchConnections()
    if (openConnections.value[connection.id]) {
      openConnections.value[connection.id].connection =
        connections.value.find((saved) => saved.id === connection.id) ?? connection
    }
  }

  async function removeConnection(id: string) {
    await writeConnections('remove_connection', { id })
    delete openConnections.value[id]
    await fetchConnections()
  }

  async function testConnection(connection: Connection, onProgress?: (progress: ConnectionTestProgress) => void) {
    const channel = onProgress ? new Channel<ConnectionTestProgress>() : undefined
    if (channel && onProgress) channel.onmessage = onProgress
    return invoke<string>('test_connection', { connection, onProgress: channel ?? null })
  }

  async function exportConnections(path: string) {
    await invoke('export_connections', { path })
  }

  async function importConnections(path: string) {
    const count = await writeConnections('import_connections', { path }) as number
    await fetchConnections()
    return count
  }

  async function connect(connection: Connection) {
    const info = await invoke<ConnectionInfo>('connect', { connection })
    const serverVersion = info.serverVersion
    if (!openConnections.value[connection.id]) {
      openConnections.value[connection.id] = {
        connection,
        databases: [],
        selectedDatabase: null,
        openedDatabases: [],
        serverVersion,
        capabilities: info.capabilities,
        status: 'connected',
        statusMessage: null,
        tables: {},
      }
    } else {
      openConnections.value[connection.id].connection = connection
      openConnections.value[connection.id].openedDatabases ??= []
      openConnections.value[connection.id].serverVersion = serverVersion
      openConnections.value[connection.id].capabilities = info.capabilities
      markConnectionConnected(connection.id)
    }
    await fetchDatabasesForConnection(connection.id)
  }

  function disconnectConnection(id: string) {
    delete openConnections.value[id]
    void invoke('disconnect', { connectionId: id }).catch(console.error)
  }

  function closeDatabase(connectionId: string, database: string) {
    const connState = openConnections.value[connectionId]
    if (!connState) return

    const openedDatabases = connState.openedDatabases?.length
      ? connState.openedDatabases
      : connState.selectedDatabase
        ? [connState.selectedDatabase]
        : []

    connState.openedDatabases = openedDatabases.filter((db) => db !== database)
    delete connState.tables[database]

    if (connState.selectedDatabase === database || !connState.selectedDatabase) {
      connState.selectedDatabase = connState.openedDatabases[0] ?? null
    }
  }

  async function fetchDatabasesForConnection(connectionId: string) {
    try {
      const dbs = await invoke<string[]>('get_databases', { connectionId })
      if (openConnections.value[connectionId]) {
        markConnectionConnected(connectionId)
        openConnections.value[connectionId].databases = dbs
        if (
          openConnections.value[connectionId].selectedDatabase &&
          !dbs.includes(openConnections.value[connectionId].selectedDatabase)
        ) {
          openConnections.value[connectionId].selectedDatabase = null
        }
        openConnections.value[connectionId].openedDatabases = (
          openConnections.value[connectionId].openedDatabases ?? []
        ).filter((database) => dbs.includes(database))
      }
    } catch (error) {
      markConnectionError(connectionId, error)
      throw error
    }
  }

  async function selectDatabase(connectionId: string, database: string) {
    const connState = openConnections.value[connectionId]
    if (!connState) return
    connState.openedDatabases ??= []
    connState.selectedDatabase = database
    if (!connState.openedDatabases.includes(database)) {
      connState.openedDatabases.push(database)
    }
    if (!connState.tables[database]) {
      await fetchTablesForConnection(connectionId, database)
    }
  }

  async function fetchTablesForConnection(connectionId: string, database: string) {
    try {
      const tbls = await invoke<DatabaseTable[]>('get_tables', { connectionId, database })
      if (openConnections.value[connectionId]) {
        markConnectionConnected(connectionId)
        openConnections.value[connectionId].tables[database] = tbls
      }
      return tbls
    } catch (error) {
      markConnectionError(connectionId, error)
      throw error
    }
  }

  // ── Data fetchers — pure: accept explicit params, return data, no shared state ──

  async function fetchTableData(
    connectionId: string,
    database: string,
    tableName: string | TableRef,
    page = 0,
    pageSize = 300,
    filters: any = null,
    sort: { column: string; desc: boolean } | null = null,
    exactCount = true,
    keyset: { column: string; value: any; direction: 'next' | 'prev' } | null = null,
  ) {
    try {
      const result = await invoke<any>('get_table_data', {
        connectionId,
        database,
        table: tableReference(connectionId, database, tableName),
        page,
        pageSize,
        filters,
        sortColumn: sort?.column ?? null,
        sortDesc: sort?.desc ?? null,
        exactCount,
        keyset,
      })
      markConnectionConnected(connectionId)
      return result
    } catch (error) {
      markConnectionError(connectionId, error)
      throw error
    }
  }

  async function fetchTableStructure(connectionId: string, database: string, tableName: string | TableRef) {
    try {
      const result = await invoke<ColumnStructure[]>('get_table_structure', { connectionId, database, table: tableReference(connectionId, database, tableName) })
      markConnectionConnected(connectionId)
      return result
    } catch (error) {
      markConnectionError(connectionId, error)
      throw error
    }
  }

  async function fetchTableIndexes(connectionId: string, database: string, tableName: string | TableRef) {
    try {
      const result = await invoke<any[]>('get_table_indexes', { connectionId, database, table: tableReference(connectionId, database, tableName) })
      markConnectionConnected(connectionId)
      return result
    } catch (error) {
      markConnectionError(connectionId, error)
      throw error
    }
  }

  async function fetchForeignKeys(connectionId: string, database: string, tableName: string | TableRef) {
    try {
      const result = await invoke<ForeignKeyColumn[]>('get_foreign_keys', { connectionId, database, table: tableReference(connectionId, database, tableName) })
      markConnectionConnected(connectionId)
      return result
    } catch (error) {
      markConnectionError(connectionId, error)
      return []
    }
  }

  async function fetchTableDdl(connectionId: string, database: string, tableName: string | TableRef) {
    // An unsupported optional feature is not a failed database connection.
    const capabilities = openConnections.value[connectionId]?.capabilities
    if ((capabilities?.inspectDdl ?? capabilities?.exportSql) === false) return null
    try {
      const result = await invoke<string>('get_table_ddl', { connectionId, database, table: tableReference(connectionId, database, tableName) })
      markConnectionConnected(connectionId)
      return result
    } catch (error) {
      const message = String(error)
      if (message.startsWith('Cannot inspect ') || message === 'Circular DDL dependencies') {
        // Object-specific export limitations must not mark a live connection
        // offline. Show the reason in the DDL panel as non-executable comments.
        return message.split(/[\r\n]+/).map(line => `-- ${line}`).join('\n')
      }
      markConnectionError(connectionId, error)
      return null
    }
  }

  async function alterTableColumn(
    connectionId: string,
    database: string,
    tableName: string | TableRef,
    oldName: string,
    newName: string,
    newType: string,
  ) {
    try {
      await invoke('alter_table_column', {
        connectionId,
        database,
        table: tableReference(connectionId, database, tableName),
        oldName,
        newName,
        newType,
      })
      markConnectionConnected(connectionId)
    } catch (error) {
      markConnectionError(connectionId, error)
      throw error
    }
  }

  function tableReference(connectionId: string, database: string, table: string | TableRef): TableRef {
    return resolveTableReference(database, table, openConnections.value[connectionId]?.tables[database])
  }

  return {
    tableReference,
    connections,
    storageError,
    storageInfo,
    availableDrivers,
    openConnections,
    fetchConnections,
    addConnection,
    removeConnection,
    testConnection,
    connect,
    disconnectConnection,
    closeDatabase,
    fetchDatabasesForConnection,
    selectDatabase,
    fetchTablesForConnection,
    fetchTableData,
    fetchTableStructure,
    fetchTableIndexes,
    fetchForeignKeys,
    fetchTableDdl,
    alterTableColumn,
    exportConnections,
    importConnections,
  }
})
