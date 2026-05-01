import { type Ref } from 'vue'
import { useConnectionStore } from '@/stores/connections'
import type { PaneState, TableTab, QueryTab } from '@/types/workspace'
import { findTabInsertIndex, findNextActiveIndex } from '@/lib/tabManagement'
import { buildSortPayload, resolveKeysetColumn } from '@/lib/rowSelection'

interface WorkspaceContext {
  panes: Ref<PaneState[]>
  activePaneId: Ref<string>
  focusedPaneId: Ref<string | null>
  getPane: (paneId?: string) => PaneState
  getPaneTab: (pane: PaneState) => TableTab | null
  getPrimaryKey: (pane: PaneState) => string | null
  getPaneConnection: (pane: PaneState) => any
  addPane: () => string
}

export function useTableTabs(ctx: WorkspaceContext) {
  const store = useConnectionStore()
  const { getPane, getPaneTab } = ctx

  // ── Tab lifecycle ───────────────────────────────────────────────────────────

  function openQueryTab(connectionId: string, database: string | null = null, paneId?: string) {
    const pane = getPane(paneId)
    const id = crypto.randomUUID()
    const tab: QueryTab = { type: 'query', id, connectionId, database, sql: '' }
    const insertIndex = findTabInsertIndex(pane.tabs, connectionId, database)
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
    }
  }

  function closeTab(tabId: string, paneId?: string, event?: MouseEvent) {
    event?.stopPropagation()
    const pane = getPane(paneId)
    const idx = pane.tabs.findIndex(t => t.id === tabId)
    if (idx === -1) return
    pane.tabs.splice(idx, 1)
    if (pane.activeTabId === tabId) {
      const nextIdx = findNextActiveIndex(pane.tabs.length, idx)
      if (nextIdx === -1) pane.activeTabId = null
      else switchToTab(pane.tabs[nextIdx].id, pane.id)
    }
  }

  // ── Data loading ────────────────────────────────────────────────────────────

  async function _fetchInitialTabData(pane: PaneState, page: number, pageSize: number, filters: any, sort: { column: string; desc: boolean } | null) {
    // Use getPaneTab to get the reactive proxy, not the local plain-object reference
    const tab = getPaneTab(pane)
    if (!tab) { console.error('[fetchInitialTabData] getPaneTab returned null'); return }
    const { connectionId, database, tableName } = tab
    const [queryResult, tableStructure] = await Promise.all([
      store.fetchTableData(connectionId, database, tableName, page, pageSize, filters, sort).catch((e: any) => {
        console.error('[fetchTableData]', e);
        return { columns: [], rows: [], total_count: 0, total_count_is_estimate: false };
      }),
      store.fetchTableStructure(connectionId, database, tableName).catch((e: any) => { console.error('[fetchTableStructure]', e); return [] }),
    ])
    // Write to the reactive proxy so Vue detects the changes
    tab.queryResult = queryResult
    tab.tableStructure = tableStructure
  }

  async function ensureTableMetadata(pane: PaneState) {
    const tab = getPaneTab(pane)
    if (!tab || tab.metadataLoaded || tab.metadataLoading) return
    tab.metadataLoading = true
    const { connectionId, database, tableName } = tab
    try {
      const [tableIndexes, foreignKeys, ddl] = await Promise.all([
        store.fetchTableIndexes(connectionId, database, tableName).catch((e: any) => { console.error('[fetchTableIndexes]', e); return [] }),
        store.fetchForeignKeys(connectionId, database, tableName).catch((e: any) => { console.error('[fetchForeignKeys]', e); return [] }),
        store.fetchTableDdl(connectionId, database, tableName).catch(() => null),
      ])
      tab.tableIndexes = tableIndexes
      tab.foreignKeys = foreignKeys
      tab.ddl = ddl
      tab.metadataLoaded = true
    } finally {
      tab.metadataLoading = false
    }
  }

  async function loadStructureViewMetadata(pane: PaneState) {
    pane.viewMode = 'structure'
    const tab = getPaneTab(pane)
    if (tab) tab.viewMode = 'structure'
    await ensureTableMetadata(pane)
  }

  async function loadTableData(
    tableName: string,
    connectionId: string,
    database: string,
    initialFilter?: any,
    paneId?: string,
  ) {
    // Smart routing: keep tabs from the same connection+database together.
    let pane = getPane(paneId)
    if (!paneId) {
      const tabDb = (t: { type: string; database: string | null } & { database?: string | null }) =>
        t.type === 'table' ? (t as TableTab).database : t.database

      // 1. If another pane already has tabs from this connection+db, route there
      const sameConnDb = ctx.panes.value.find(p =>
        p.tabs.some(t => t.connectionId === connectionId && tabDb(t) === database)
      )
      if (sameConnDb) {
        pane = sameConnDb
      } else {
        // 2. If the active pane has tabs from a DIFFERENT connection/database,
        //    auto-split so tables don't mix — but never during focus mode to
        //    avoid creating ghost panes that confuse the layout on unpin.
        const activePaneHasOtherDb = pane.tabs.length > 0 &&
          !pane.tabs.some(t => t.connectionId === connectionId && tabDb(t) === database)
        if (activePaneHasOtherDb && !ctx.focusedPaneId.value) {
          const newPaneId = ctx.addPane()
          pane = ctx.panes.value.find(p => p.id === newPaneId) ?? pane
        }
      }
    }
    if (!initialFilter) {
      const existing = pane.tabs.find(t =>
        t.type === 'table' &&
        (t as TableTab).tableName === tableName &&
        (t as TableTab).database === database &&
        t.connectionId === connectionId,
      )
      if (existing) { switchToTab(existing.id, pane.id); return }
    }
    const id = crypto.randomUUID()
    const tab: TableTab = {
      type: 'table', id, connectionId, tableName, database,
      queryResult: null, exactCountLoading: false, metadataLoading: false, metadataLoaded: false, tableStructure: [], tableIndexes: [], foreignKeys: [], ddl: null,
      page: 0, pageSize: pane.pageSize, viewMode: 'content',
      filters: initialFilter ?? null, sortColumn: null, sortDesc: false,
      pendingChanges: {}, pendingDeletions: {}, pendingTruncate: false,
      selectedRowPk: null, selectedRowPks: [], inlineEditColumn: null,
    }
    const insertIndex = findTabInsertIndex(pane.tabs, connectionId, database)
    pane.tabs.splice(insertIndex, 0, tab)
    pane.activeTabId = id
    pane.page = 0
    pane.viewMode = 'content'
    try {
      await _fetchInitialTabData(pane, 0, tab.pageSize, initialFilter ?? null, null)
    } catch (e: any) {
      console.error('[loadTableData]', e)
      if (String(e).includes('No active session')) store.disconnectConnection(connectionId)
    }
  }

  async function refreshActiveTab(paneId?: string) {
    const pane = getPane(paneId)
    const tab = getPaneTab(pane)
    if (!tab) return
    try {
      await _fetchInitialTabData(pane, pane.page, pane.pageSize, tab.filters ?? null, sortPayload(tab))
      if (pane.viewMode === 'structure') await ensureTableMetadata(pane)
    } catch (e: any) {
      if (String(e).includes('No active session')) store.disconnectConnection(tab.connectionId)
    }
  }

  async function refreshExactCount(pane: PaneState) {
    const tab = getPaneTab(pane)
    if (!tab || tab.exactCountLoading) return
    tab.exactCountLoading = true
    try {
      tab.queryResult = await store.fetchTableData(
        tab.connectionId,
        tab.database,
        tab.tableName,
        pane.page,
        pane.pageSize,
        tab.filters,
        sortPayload(tab),
        true,
      )
    } finally {
      tab.exactCountLoading = false
    }
  }

  // ── Pagination & sort ───────────────────────────────────────────────────────

  function sortPayload(tab: TableTab | null): { column: string; desc: boolean } | null {
    if (!tab) return null
    return buildSortPayload(tab.sortColumn, tab.sortDesc)
  }

  function keysetColumn(tab: TableTab): string | null {
    return resolveKeysetColumn(tab.tableStructure as any[], tab.sortColumn, tab.filters)
  }

  async function changePage(pane: PaneState, delta: number) {
    const tab = getPaneTab(pane)
    if (!tab) return
    tab.selectedRowPk = null; tab.inlineEditColumn = null
    const pk = keysetColumn(tab)
    const rows = tab.queryResult?.rows ?? []
    const canUseKeyset = pk && rows.length > 0 && (delta === 1 || delta === -1)

    if (canUseKeyset) {
      const cursorRow = delta === 1 ? rows[rows.length - 1] : rows[0]
      const cursorValue = cursorRow?.[pk]
      if (cursorValue !== undefined && cursorValue !== null) {
        const nextPage = Math.max(0, pane.page + delta)
        const nextResult = await store.fetchTableData(
          tab.connectionId,
          tab.database,
          tab.tableName,
          nextPage,
          pane.pageSize,
          tab.filters,
          sortPayload(tab),
          true,
          { column: pk, value: cursorValue, direction: delta === 1 ? 'next' : 'prev' },
        )
        if (nextResult.rows?.length || delta === -1) {
          pane.page = nextPage
          tab.page = pane.page
          tab.keysetPage = pane.page
          tab.queryResult = nextResult
          return
        }
      }
    }

    pane.page += delta; tab.page = pane.page; tab.keysetPage = undefined
    tab.queryResult = await store.fetchTableData(tab.connectionId, tab.database, tab.tableName, pane.page, pane.pageSize, tab.filters, sortPayload(tab))
  }

  async function changeLimit(pane: PaneState, newLimit: number) {
    if (!newLimit || newLimit < 1) return
    const tab = getPaneTab(pane)
    if (!tab) return
    const offset = pane.page * pane.pageSize
    pane.pageSize = newLimit; tab.pageSize = newLimit
    pane.page = Math.floor(offset / newLimit); tab.page = pane.page
    tab.keysetPage = undefined
    tab.selectedRowPk = null; tab.inlineEditColumn = null
    tab.queryResult = await store.fetchTableData(tab.connectionId, tab.database, tab.tableName, pane.page, pane.pageSize, tab.filters, sortPayload(tab))
  }

  async function gotoOffset(pane: PaneState, newOffset: number) {
    if (newOffset < 0) return
    const tab = getPaneTab(pane)
    if (!tab) return
    tab.selectedRowPk = null; tab.inlineEditColumn = null
    pane.page = Math.floor(newOffset / pane.pageSize); tab.page = pane.page
    tab.keysetPage = undefined
    tab.queryResult = await store.fetchTableData(tab.connectionId, tab.database, tab.tableName, pane.page, pane.pageSize, tab.filters, sortPayload(tab))
  }

  async function onSortColumn(pane: PaneState, column: string) {
    const tab = getPaneTab(pane)
    if (!tab) return
    if (tab.sortColumn === column) tab.sortDesc = !tab.sortDesc
    else { tab.sortColumn = column; tab.sortDesc = false }
    tab.selectedRowPk = null; tab.inlineEditColumn = null
    pane.page = 0; tab.page = 0
    tab.keysetPage = undefined
    tab.queryResult = await store.fetchTableData(tab.connectionId, tab.database, tab.tableName, 0, pane.pageSize, tab.filters, sortPayload(tab))
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
    openQueryTab,
    switchToTab,
    closeTab,
    loadTableData,
    refreshActiveTab,
    refreshExactCount,
    ensureTableMetadata,
    loadStructureViewMetadata,
    sortPayload,
    changePage,
    changeLimit,
    gotoOffset,
    onSortColumn,
    connectionNames,
    getAvailableDatabases,
  }
}
