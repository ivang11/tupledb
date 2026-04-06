import { type Ref } from 'vue'
import { useConnectionStore } from '@/stores/connections'
import type { PaneState, TableTab, QueryTab } from '@/types/workspace'

interface WorkspaceContext {
  panes: Ref<PaneState[]>
  activePaneId: Ref<string>
  getPane: (paneId?: string) => PaneState
  getPaneTab: (pane: PaneState) => TableTab | null
  getPrimaryKey: (pane: PaneState) => string | null
  getPaneConnection: (pane: PaneState) => any
}

export function useTableTabs(ctx: WorkspaceContext) {
  const store = useConnectionStore()
  const { panes, activePaneId, getPane, getPaneTab } = ctx

  // ── Store sync ──────────────────────────────────────────────────────────────

  function syncStoreForFetch(connectionId: string, database: string) {
    const connState = store.openConnections[connectionId]
    store.activeConnection = connState?.connection ?? null
    store.activeDatabase = database
    store.tables = connState?.tables[database] ?? []
  }

  function saveToActiveTab(pane: PaneState) {
    const tab = getPaneTab(pane)
    if (!tab) return
    tab.queryResult = store.queryResult
    tab.tableStructure = store.tableStructure
    tab.tableIndexes = store.tableIndexes
    tab.foreignKeys = store.foreignKeys
  }

  // ── Tab lifecycle ───────────────────────────────────────────────────────────

  function openQueryTab(connectionId: string, database: string | null = null, paneId?: string) {
    const pane = getPane(paneId)
    const id = crypto.randomUUID()
    const tab: QueryTab = { type: 'query', id, connectionId, database }
    let insertIndex = pane.tabs.length
    for (let i = pane.tabs.length - 1; i >= 0; i--) {
      const t = pane.tabs[i]
      if (t.connectionId === connectionId && t.database === database) {
        insertIndex = i + 1; break
      } else if (t.connectionId === connectionId && insertIndex === pane.tabs.length) {
        insertIndex = i + 1
      }
    }
    pane.tabs.splice(insertIndex, 0, tab)
    pane.activeTabId = id
  }

  function switchToTab(tabId: string, paneId?: string) {
    const pane = getPane(paneId)
    const tab = pane.tabs.find(t => t.id === tabId)
    if (!tab) return
    pane.activeTabId = tabId
    if (tab.type === 'table') {
      const t = tab as TableTab
      pane.viewMode = t.viewMode
      pane.page = t.page
      pane.pageSize = t.pageSize
      if (pane.id === activePaneId.value) {
        syncStoreForFetch(t.connectionId, t.database)
        store.activeTable = t.tableName
        store.queryResult = t.queryResult
        store.tableStructure = t.tableStructure
        store.tableIndexes = t.tableIndexes
        store.foreignKeys = t.foreignKeys
      }
    } else {
      if (pane.id === activePaneId.value) {
        store.activeTable = null
        store.queryResult = null
        store.tableStructure = []
        store.tableIndexes = []
        store.foreignKeys = []
      }
    }
  }

  function closeTab(tabId: string, paneId?: string, event?: MouseEvent) {
    event?.stopPropagation()
    const pane = getPane(paneId)
    const idx = pane.tabs.findIndex(t => t.id === tabId)
    if (idx === -1) return
    pane.tabs.splice(idx, 1)
    if (pane.activeTabId === tabId) {
      if (pane.tabs.length === 0) {
        pane.activeTabId = null
        if (pane.id === activePaneId.value) {
          store.activeTable = null
          store.queryResult = null
          store.tableStructure = []
          store.foreignKeys = []
        }
      } else {
        switchToTab(pane.tabs[Math.min(idx, pane.tabs.length - 1)].id, pane.id)
      }
    }
  }

  // ── Data loading ────────────────────────────────────────────────────────────

  async function loadTableData(
    tableName: string,
    connectionId: string,
    database: string,
    initialFilter?: any,
    paneId?: string,
  ) {
    const pane = getPane(paneId)
    if (!initialFilter) {
      const existing = pane.tabs.find(t =>
        t.type === 'table' &&
        (t as TableTab).tableName === tableName &&
        (t as TableTab).database === database &&
        t.connectionId === connectionId,
      )
      if (existing) { switchToTab(existing.id, pane.id); return }
    }
    syncStoreForFetch(connectionId, database)
    const id = crypto.randomUUID()
    const tab: TableTab = {
      type: 'table', id, connectionId, tableName, database,
      queryResult: null, tableStructure: [], tableIndexes: [], foreignKeys: [],
      page: 0, pageSize: pane.pageSize, viewMode: 'content',
      filters: initialFilter ?? null, sortColumn: null, sortDesc: false,
      pendingChanges: {}, pendingDeletions: {}, pendingTruncate: false,
      selectedRowPk: null, inlineEditColumn: null,
    }
    let insertIndex = pane.tabs.length
    for (let i = pane.tabs.length - 1; i >= 0; i--) {
      const t = pane.tabs[i]
      if (t.connectionId === connectionId && t.database === database) {
        insertIndex = i + 1; break
      } else if (t.connectionId === connectionId && insertIndex === pane.tabs.length) {
        insertIndex = i + 1
      }
    }
    pane.tabs.splice(insertIndex, 0, tab)
    pane.activeTabId = id
    pane.page = 0
    pane.viewMode = 'content'
    store.queryResult = null
    store.tableStructure = []
    store.tableIndexes = []
    store.foreignKeys = []
    try {
      await Promise.all([
        store.fetchTableData(tableName, 0, tab.pageSize, initialFilter ?? null),
        store.fetchTableStructure(tableName),
        store.fetchTableIndexes(tableName),
        store.fetchForeignKeys(tableName),
      ])
      saveToActiveTab(pane)
    } catch (e: any) {
      if (String(e).includes('No active session')) store.disconnectConnection(connectionId)
    }
  }

  async function refreshActiveTab(paneId?: string) {
    const pane = getPane(paneId)
    const tab = getPaneTab(pane)
    if (!tab) return
    syncStoreForFetch(tab.connectionId, tab.database)
    try {
      await Promise.all([
        store.fetchTableData(tab.tableName, pane.page, pane.pageSize, tab.filters ?? null, sortPayload(tab)),
        store.fetchTableStructure(tab.tableName),
        store.fetchTableIndexes(tab.tableName),
        store.fetchForeignKeys(tab.tableName),
      ])
      saveToActiveTab(pane)
    } catch (e: any) {
      if (String(e).includes('No active session')) store.disconnectConnection(tab.connectionId)
    }
  }

  // ── Pagination & sort ───────────────────────────────────────────────────────

  function sortPayload(tab: TableTab | null): { column: string; desc: boolean } | null {
    if (!tab || !tab.sortColumn) return null
    return { column: tab.sortColumn, desc: tab.sortDesc }
  }

  async function changePage(pane: PaneState, delta: number) {
    const tab = getPaneTab(pane)
    if (!tab) return
    tab.selectedRowPk = null; tab.inlineEditColumn = null
    pane.page += delta; tab.page = pane.page
    syncStoreForFetch(tab.connectionId, tab.database)
    await store.fetchTableData(tab.tableName, pane.page, pane.pageSize, tab.filters, sortPayload(tab))
    saveToActiveTab(pane)
  }

  async function changeLimit(pane: PaneState, newLimit: number) {
    if (!newLimit || newLimit < 1) return
    const tab = getPaneTab(pane)
    if (!tab) return
    const offset = pane.page * pane.pageSize
    pane.pageSize = newLimit; tab.pageSize = newLimit
    pane.page = Math.floor(offset / newLimit); tab.page = pane.page
    tab.selectedRowPk = null; tab.inlineEditColumn = null
    syncStoreForFetch(tab.connectionId, tab.database)
    await store.fetchTableData(tab.tableName, pane.page, pane.pageSize, tab.filters, sortPayload(tab))
    saveToActiveTab(pane)
  }

  async function gotoOffset(pane: PaneState, newOffset: number) {
    if (newOffset < 0) return
    const tab = getPaneTab(pane)
    if (!tab) return
    tab.selectedRowPk = null; tab.inlineEditColumn = null
    pane.page = Math.floor(newOffset / pane.pageSize); tab.page = pane.page
    syncStoreForFetch(tab.connectionId, tab.database)
    await store.fetchTableData(tab.tableName, pane.page, pane.pageSize, tab.filters, sortPayload(tab))
    saveToActiveTab(pane)
  }

  async function onSortColumn(pane: PaneState, column: string) {
    const tab = getPaneTab(pane)
    if (!tab) return
    if (tab.sortColumn === column) tab.sortDesc = !tab.sortDesc
    else { tab.sortColumn = column; tab.sortDesc = false }
    tab.selectedRowPk = null; tab.inlineEditColumn = null
    pane.page = 0; tab.page = 0
    syncStoreForFetch(tab.connectionId, tab.database)
    await store.fetchTableData(tab.tableName, pane.page, pane.pageSize, tab.filters, sortPayload(tab))
    tab.queryResult = store.queryResult
    saveToActiveTab(pane)
  }

  // ── Helpers ─────────────────────────────────────────────────────────────────

  function connectionNames(): Record<string, string> {
    return Object.fromEntries(
      Object.entries(store.openConnections).map(([id, s]) => [id, s.connection.name]),
    )
  }

  function getAvailableDatabases(connectionId: string): string[] {
    return store.openConnections[connectionId]?.databases ?? []
  }

  return {
    syncStoreForFetch,
    saveToActiveTab,
    openQueryTab,
    switchToTab,
    closeTab,
    loadTableData,
    refreshActiveTab,
    sortPayload,
    changePage,
    changeLimit,
    gotoOffset,
    onSortColumn,
    connectionNames,
    getAvailableDatabases,
  }
}
