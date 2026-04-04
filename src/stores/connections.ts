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

  // All currently connected sessions, keyed by connection.id
  const openConnections = ref<Record<string, OpenConnectionState>>({})

  // Reflects the active tab's context — set directly by HomeView
  const activeConnection = ref<Connection | null>(null)
  const databases = ref<string[]>([])
  const activeDatabase = ref<string | null>(null)
  const tables = ref<any[]>([])
  const activeTable = ref<any | null>(null)
  const activeFilters = ref<any | null>(null)
  const queryResult = ref<any | null>(null)
  const tableStructure = ref<any[]>([])
  const isLoading = ref(false)
  const foreignKeys = ref<any[]>([])

  async function fetchForeignKeys(tableName: string) {
    if (!activeConnection.value || !activeDatabase.value) return
    try {
      foreignKeys.value = await invoke('get_foreign_keys', {
        connectionId: activeConnection.value.id,
        database: activeDatabase.value,
        table: tableName,
      })
    } catch {
      foreignKeys.value = []
    }
  }

  async function fetchTableStructure(tableName: string) {
    if (!activeConnection.value || !activeDatabase.value) return
    try {
      tableStructure.value = await invoke('get_table_structure', {
        connectionId: activeConnection.value.id,
        database: activeDatabase.value,
        table: tableName,
      })
    } catch (error) {
      console.error('Failed to fetch table structure:', error)
    }
  }

  async function fetchTableData(tableName: string, page = 0, pageSize = 50, filters = activeFilters.value) {
    if (!activeConnection.value || !activeDatabase.value) return

    isLoading.value = true
    activeTable.value = tableName
    activeFilters.value = filters

    try {
      queryResult.value = await invoke('get_table_data', {
        connectionId: activeConnection.value.id,
        database: activeDatabase.value,
        table: tableName,
        page,
        pageSize,
        filters
      })
    } catch (error) {
      console.error('Failed to fetch table data:', error)
      throw error
    } finally {
      isLoading.value = false
    }
  }

  async function fetchConnections() {
    try {
      connections.value = await invoke<Connection[]>('get_connections')
    } catch (error) {
      console.error('Failed to fetch connections:', error)
    }
  }

  async function addConnection(connection: Connection) {
    try {
      await invoke('add_connection', { connection })
      await fetchConnections()
    } catch (error) {
      console.error('Failed to add connection:', error)
      throw error
    }
  }

  async function removeConnection(id: string) {
    try {
      await invoke('remove_connection', { id })
      if (activeConnection.value?.id === id) {
        activeConnection.value = null
      }
      delete openConnections.value[id]
      await fetchConnections()
    } catch (error) {
      console.error('Failed to remove connection:', error)
      throw error
    }
  }

  async function testConnection(connection: Connection) {
    try {
      return await invoke<string>('test_connection', { connection })
    } catch (error) {
      console.error('Failed to test connection:', error)
      throw error
    }
  }

  async function connect(connection: Connection) {
    try {
      await invoke('connect', { connection })
      if (!openConnections.value[connection.id]) {
        openConnections.value[connection.id] = {
          connection,
          databases: [],
          tables: {}
        }
      } else {
        openConnections.value[connection.id].connection = connection
      }
      activeConnection.value = connection
      await fetchDatabasesForConnection(connection.id)
    } catch (error) {
      console.error('Failed to connect:', error)
      throw error
    }
  }

  function disconnectConnection(id: string) {
    delete openConnections.value[id]
    if (activeConnection.value?.id === id) {
      activeConnection.value = null
      activeDatabase.value = null
      databases.value = []
      tables.value = []
      activeTable.value = null
    }
  }

  async function fetchDatabasesForConnection(connectionId: string) {
    try {
      const dbs = await invoke<string[]>('get_databases', { connectionId })
      if (openConnections.value[connectionId]) {
        openConnections.value[connectionId].databases = dbs
      }
      if (activeConnection.value?.id === connectionId) {
        databases.value = dbs
      }
    } catch (error) {
      console.error('Failed to fetch databases:', error)
      throw error
    }
  }

  async function fetchDatabases() {
    if (!activeConnection.value) return
    await fetchDatabasesForConnection(activeConnection.value.id)
  }

  async function fetchTablesForConnection(connectionId: string, database: string) {
    try {
      const tbls = await invoke<any[]>('get_tables', { connectionId, database })
      if (openConnections.value[connectionId]) {
        openConnections.value[connectionId].tables[database] = tbls
      }
      if (activeConnection.value?.id === connectionId && activeDatabase.value === database) {
        tables.value = tbls
      }
      return tbls
    } catch (error) {
      console.error('Failed to fetch tables:', error)
      throw error
    }
  }

  async function fetchTables(database: string) {
    if (!activeConnection.value) return
    activeDatabase.value = database
    tables.value = await fetchTablesForConnection(activeConnection.value.id, database) ?? []
  }

  return {
    connections,
    openConnections,
    activeConnection,
    databases,
    activeDatabase,
    tables,
    activeTable,
    activeFilters,
    queryResult,
    tableStructure,
    isLoading,
    foreignKeys,
    fetchForeignKeys,
    fetchConnections,
    addConnection,
    removeConnection,
    testConnection,
    connect,
    disconnectConnection,
    fetchDatabases,
    fetchDatabasesForConnection,
    fetchTables,
    fetchTablesForConnection,
    fetchTableData,
    fetchTableStructure
  }
})
