import { defineStore } from 'pinia'
import { ref } from 'vue'
import type { Connection } from '@/types/connection'
import { invoke } from '@tauri-apps/api/core'

interface OpenConnectionState {
  connection: Connection
  databases: string[]
  selectedDatabase: string | null
  openedDatabases: string[]
  serverVersion: string | null
  status: 'connected' | 'error'
  statusMessage: string | null
  tables: Record<string, any[]>
}

export const useConnectionStore = defineStore('connections', () => {
  const connections = ref<Connection[]>([])
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
      connections.value = await invoke<Connection[]>('get_connections')
      for (const connection of connections.value) {
        if (openConnections.value[connection.id]) {
          openConnections.value[connection.id].connection = connection
        }
      }
    } catch (error) {
      console.error('Failed to fetch connections:', error)
    }
  }

  async function addConnection(connection: Connection) {
    await invoke('add_connection', { connection })
    await fetchConnections()
    if (openConnections.value[connection.id]) {
      openConnections.value[connection.id].connection =
        connections.value.find((saved) => saved.id === connection.id) ?? connection
    }
  }

  async function removeConnection(id: string) {
    await invoke('remove_connection', { id })
    delete openConnections.value[id]
    await fetchConnections()
  }

  async function testConnection(connection: Connection) {
    return invoke<string>('test_connection', { connection })
  }

  async function exportConnections(path: string) {
    await invoke('export_connections', { path })
  }

  async function importConnections(path: string) {
    const count = await invoke<number>('import_connections', { path })
    await fetchConnections()
    return count
  }

  async function connect(connection: Connection) {
    const serverVersion = await invoke<string>('connect', { connection })
    if (!openConnections.value[connection.id]) {
      openConnections.value[connection.id] = {
        connection,
        databases: [],
        selectedDatabase: null,
        openedDatabases: [],
        serverVersion,
        status: 'connected',
        statusMessage: null,
        tables: {},
      }
    } else {
      openConnections.value[connection.id].connection = connection
      openConnections.value[connection.id].openedDatabases ??= []
      openConnections.value[connection.id].serverVersion = serverVersion
      markConnectionConnected(connection.id)
    }
    await fetchDatabasesForConnection(connection.id)
  }

  function disconnectConnection(id: string) {
    delete openConnections.value[id]
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
      const tbls = await invoke<any[]>('get_tables', { connectionId, database })
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
    tableName: string,
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
        table: tableName,
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

  async function fetchTableStructure(connectionId: string, database: string, tableName: string) {
    try {
      const result = await invoke<any[]>('get_table_structure', { connectionId, database, table: tableName })
      markConnectionConnected(connectionId)
      return result
    } catch (error) {
      markConnectionError(connectionId, error)
      throw error
    }
  }

  async function fetchTableIndexes(connectionId: string, database: string, tableName: string) {
    try {
      const result = await invoke<any[]>('get_table_indexes', { connectionId, database, table: tableName })
      markConnectionConnected(connectionId)
      return result
    } catch (error) {
      markConnectionError(connectionId, error)
      throw error
    }
  }

  async function fetchForeignKeys(connectionId: string, database: string, tableName: string) {
    try {
      const result = await invoke<any[]>('get_foreign_keys', { connectionId, database, table: tableName })
      markConnectionConnected(connectionId)
      return result
    } catch (error) {
      markConnectionError(connectionId, error)
      return []
    }
  }

  async function fetchTableDdl(connectionId: string, database: string, tableName: string) {
    try {
      const result = await invoke<string>('get_table_ddl', { connectionId, database, table: tableName })
      markConnectionConnected(connectionId)
      return result
    } catch (error) {
      markConnectionError(connectionId, error)
      return null
    }
  }

  return {
    connections,
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
    exportConnections,
    importConnections,
  }
})
