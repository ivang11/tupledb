import { defineStore } from 'pinia'
import { ref } from 'vue'
import type { Connection } from '@/types/connection'
import { invoke } from '@tauri-apps/api/core'

interface OpenConnectionState {
  connection: Connection
  databases: string[]
  tables: Record<string, any[]>
}

export const useConnectionStore = defineStore('connections', () => {
  const connections = ref<Connection[]>([])
  const openConnections = ref<Record<string, OpenConnectionState>>({})

  // ── Connection management ──────────────────────────────────────────────────

  async function fetchConnections() {
    try {
      connections.value = await invoke<Connection[]>('get_connections')
    } catch (error) {
      console.error('Failed to fetch connections:', error)
    }
  }

  async function addConnection(connection: Connection) {
    await invoke('add_connection', { connection })
    await fetchConnections()
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
    await invoke('connect', { connection })
    if (!openConnections.value[connection.id]) {
      openConnections.value[connection.id] = { connection, databases: [], tables: {} }
    } else {
      openConnections.value[connection.id].connection = connection
    }
    await fetchDatabasesForConnection(connection.id)
  }

  function disconnectConnection(id: string) {
    delete openConnections.value[id]
  }

  async function fetchDatabasesForConnection(connectionId: string) {
    const dbs = await invoke<string[]>('get_databases', { connectionId })
    if (openConnections.value[connectionId]) {
      openConnections.value[connectionId].databases = dbs
    }
  }

  async function fetchTablesForConnection(connectionId: string, database: string) {
    const tbls = await invoke<any[]>('get_tables', { connectionId, database })
    if (openConnections.value[connectionId]) {
      openConnections.value[connectionId].tables[database] = tbls
    }
    return tbls
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
    return invoke<any>('get_table_data', {
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
  }

  async function fetchTableStructure(connectionId: string, database: string, tableName: string) {
    return invoke<any[]>('get_table_structure', { connectionId, database, table: tableName })
  }

  async function fetchTableIndexes(connectionId: string, database: string, tableName: string) {
    return invoke<any[]>('get_table_indexes', { connectionId, database, table: tableName })
  }

  async function fetchForeignKeys(connectionId: string, database: string, tableName: string) {
    try {
      return await invoke<any[]>('get_foreign_keys', { connectionId, database, table: tableName })
    } catch {
      return []
    }
  }

  async function fetchTableDdl(connectionId: string, database: string, tableName: string) {
    try {
      return await invoke<string>('get_table_ddl', { connectionId, database, table: tableName })
    } catch {
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
    fetchDatabasesForConnection,
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
