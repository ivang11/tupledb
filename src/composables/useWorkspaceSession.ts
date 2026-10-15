import { onMounted, onBeforeUnmount, ref, watch, type Ref } from 'vue'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { useConnectionStore } from '@/stores/connections'
import { useAppSettings } from './useAppSettings'
import { useToast } from './useToast'
import { tabHasPendingChanges } from '@/lib/pendingChanges'
import type { AnyTab, PaneState, TableTab } from '@/types/workspace'

const SESSION_KEY = 'tupledb:workspace-session:v1'
function draft(tab: AnyTab) {
  if (tab.type === 'query') return { type: tab.type, id: tab.id, connectionId: tab.connectionId, database: tab.database, sql: tab.sql }
  return { type: tab.type, id: tab.id, connectionId: tab.connectionId, database: tab.database, reference: tab.reference,
    tableName: tab.tableName, page: tab.page, pageSize: tab.pageSize, viewMode: tab.viewMode, filters: tab.filters, configuredFilters: tab.configuredFilters,
    sortColumn: tab.sortColumn, sortDesc: tab.sortDesc, pendingChanges: tab.pendingChanges,
    pendingStructureChanges: tab.pendingStructureChanges, pendingDeletions: tab.pendingDeletions,
    pendingInserts: tab.pendingInserts, pendingTruncate: tab.pendingTruncate, pendingDrop: tab.pendingDrop,
    tableStructure: tabHasPendingChanges(tab) ? tab.tableStructure : [],
  }
}
function hydrate(tab: any): AnyTab {
  if (tab.type === 'query') return { ...tab, queryResult: null, queryError: null }
  return { ...tab, queryResult: null, tableStructure: tab.tableStructure ?? [], tableIndexes: [], foreignKeys: [], ddl: null,
    metadataLoaded: false, metadataLoading: false, selectedRowPk: null, selectedRowPks: [], inlineEditColumn: null,
    pendingChanges: tab.pendingChanges ?? {}, pendingStructureChanges: tab.pendingStructureChanges ?? {}, pendingDeletions: tab.pendingDeletions ?? {},
    pendingInserts: tab.pendingInserts ?? [], pendingTruncate: tab.pendingTruncate ?? false, pendingDrop: tab.pendingDrop ?? false,
  }
}
export function useWorkspaceSession(ctx: {
  panes: Ref<PaneState[]>; activePaneId: Ref<string>; paneWidths: Ref<number[]>; focusedPaneId: Ref<string | null>;
  sidebarVisible: Ref<boolean>; selectedConnectionId: Ref<string | null>;
  connect: (connection: any) => Promise<boolean | void>; showWorkspace: () => void;
  requestClose: (tabs: AnyTab[], action: () => void) => void;
}) {
  const store = useConnectionStore()
  const { settings } = useAppSettings()
  const restoring = ref(true)
  let timer: ReturnType<typeof setTimeout> | undefined
  let unlisten: (() => void) | undefined
  function snapshot() {
    return { version: 1, activePaneId: ctx.activePaneId.value, paneWidths: ctx.paneWidths.value,
      focusedPaneId: ctx.focusedPaneId.value, sidebarVisible: ctx.sidebarVisible.value,
      selectedConnectionId: ctx.selectedConnectionId.value,
      connections: [...new Set([...Object.keys(store.openConnections), ...ctx.panes.value.flatMap(pane => pane.tabs.map(tab => tab.connectionId))])]
        .map(id => ({ id, openedDatabases: store.openConnections[id]?.openedDatabases ?? [], database: store.openConnections[id]?.selectedDatabase ?? ctx.panes.value.flatMap(pane => pane.tabs).find(tab => tab.connectionId === id)?.database ?? null })),
      panes: ctx.panes.value.map(pane => ({ ...pane, tabs: pane.tabs.map(draft) })),
    }
  }
  function save() {
    if (restoring.value) return
    try { localStorage.setItem(SESSION_KEY, JSON.stringify(snapshot())) }
    catch (error) { useToast().error('Could not save workspace', String(error)) }
  }
  watch(() => JSON.stringify(snapshot()), () => {
    clearTimeout(timer)
    if (!restoring.value) timer = setTimeout(save, 350)
  })
  onMounted(async () => {
    try {
      const raw = localStorage.getItem(SESSION_KEY)
      const session = raw ? JSON.parse(raw) : null
      if (settings.value.restoreSession && session?.version === 1 && Array.isArray(session.panes) && session.panes.length && Array.isArray(session.connections)) {
        await store.fetchConnections()
        for (const saved of session.connections) {
          const connection = store.connections.find(connection => connection.id === saved.id)
          if (!connection) continue
          if (await ctx.connect(connection) === false) continue
          const databases = [...new Set<string>([
            ...(saved.openedDatabases ?? []),
            ...session.panes.flatMap((pane: any) => pane.tabs.filter((tab: any) => tab.connectionId === saved.id && tab.database).map((tab: any) => tab.database)),
            ...(saved.database ? [saved.database] : []),
          ])]
          for (const database of databases) if (store.openConnections[saved.id]?.databases.includes(database)) {
            try { await store.selectDatabase(saved.id, database) }
            catch (error) { useToast().error(`Could not restore ${database}`, String(error)) }
          }
        }
        const validIds = new Set(store.connections.map(connection => connection.id))
        ctx.panes.value = session.panes.map((pane: any) => ({ ...pane, tabs: pane.tabs.filter((tab: any) => validIds.has(tab.connectionId)).map(hydrate) }))
        ctx.paneWidths.value = ctx.panes.value.map((_, index) => Math.max(.15, Number(session.paneWidths?.[index]) || 1))
        ctx.activePaneId.value = ctx.panes.value.find(pane => pane.id === session.activePaneId)?.id ?? ctx.panes.value[0].id
        ctx.focusedPaneId.value = ctx.panes.value.some(pane => pane.id === session.focusedPaneId) ? session.focusedPaneId : null
        ctx.sidebarVisible.value = session.sidebarVisible !== false
        ctx.selectedConnectionId.value = store.openConnections[session.selectedConnectionId] ? session.selectedConnectionId : Object.keys(store.openConnections)[0] ?? null
        for (const pane of ctx.panes.value) {
          if (!pane.tabs.some(tab => tab.id === pane.activeTabId)) pane.activeTabId = pane.tabs[0]?.id ?? null
          const active = pane.tabs.find(tab => tab.id === pane.activeTabId)
          if (active?.type === 'table' && store.openConnections[active.connectionId]) await reload(active)
        }
        if (Object.keys(store.openConnections).length) ctx.showWorkspace()
      }
    } catch (error) { useToast().error('Could not restore workspace', String(error)) }
    finally { restoring.value = false; save() }
    try {
      unlisten = await getCurrentWindow().onCloseRequested(event => {
        save()
        const tabs = ctx.panes.value.flatMap(pane => pane.tabs)
        if (tabs.some(tab => tab.type === 'table' && tabHasPendingChanges(tab))) {
          event.preventDefault()
          ctx.requestClose(tabs, () => {
            for (const tab of tabs) if (tab.type === 'table') {
              tab.pendingChanges = {}; tab.pendingStructureChanges = {}; tab.pendingDeletions = {};
              tab.pendingInserts = []; tab.pendingDrop = false; tab.pendingTruncate = false;
            }
            save(); void getCurrentWindow().destroy();
          })
        }
      })
    } catch { /* Browser previews have no native window. */ }
  })
  async function reload(tab: TableTab) {
    try {
      if (!store.openConnections[tab.connectionId]?.tables[tab.database]) await store.fetchTablesForConnection(tab.connectionId, tab.database)
      const target = tab.reference ?? tab.tableName
      const [data, structure, foreignKeys] = await Promise.all([
        store.fetchTableData(tab.connectionId, tab.database, target, tab.page, tab.pageSize, tab.filters, tab.sortColumn ? { column: tab.sortColumn, desc: tab.sortDesc } : null),
        store.fetchTableStructure(tab.connectionId, tab.database, target),
        store.fetchForeignKeys(tab.connectionId, tab.database, target),
      ])
      tab.queryResult = data; tab.tableStructure = structure; tab.foreignKeys = foreignKeys
    } catch (error) { useToast().error(`Could not restore ${tab.tableName}`, String(error)) }
  }
  watch(() => ctx.panes.value.map(pane => [pane.id, pane.activeTabId, ...Object.keys(store.openConnections)].join(':')).join('|'), () => {
    if (restoring.value) return
    for (const pane of ctx.panes.value) {
      const active = pane.tabs.find(tab => tab.id === pane.activeTabId)
      if (active?.type === 'table' && !active.queryResult && store.openConnections[active.connectionId]) void reload(active)
    }
  })
  window.addEventListener('beforeunload', save)
  onBeforeUnmount(() => { clearTimeout(timer); save(); unlisten?.(); window.removeEventListener('beforeunload', save) })
  return { restoring, save }
}
