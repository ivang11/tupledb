import { ref, type Ref } from 'vue'
import { useConnectionStore } from '@/stores/connections'
import type { Connection } from '@/types/connection'
import type { TableTab, QueryTab, PaneState } from '@/types/workspace'

export function useWorkspace(panesContainer: Ref<HTMLElement | null>) {
  const store = useConnectionStore()

  function createPane(): PaneState {
    return { id: crypto.randomUUID(), tabs: [], activeTabId: null, viewMode: 'content', page: 0, pageSize: 300, showFilters: false }
  }

  const panes = ref<PaneState[]>([createPane()])
  const activePaneId = ref<string>(panes.value[0].id)
  const paneWidths = ref<number[]>([1])
  const draggingPaneIdx = ref<number | null>(null)
  const focusedPaneId = ref<string | null>(null)

  function getPane(paneId?: string): PaneState {
    return panes.value.find(p => p.id === (paneId ?? activePaneId.value)) ?? panes.value[0]
  }

  function addPane(): string {
    const pane = createPane()
    panes.value.push(pane)
    paneWidths.value.push(1)
    activePaneId.value = pane.id
    return pane.id
  }

  function removePane(paneId: string) {
    if (panes.value.length <= 1) return
    const idx = panes.value.findIndex(p => p.id === paneId)
    if (idx === -1) return
    panes.value.splice(idx, 1)
    paneWidths.value.splice(idx, 1)
    if (activePaneId.value === paneId) {
      activePaneId.value = panes.value[Math.min(idx, panes.value.length - 1)].id
    }
    if (focusedPaneId.value === paneId) focusedPaneId.value = null
  }

  function toggleFocusPane(paneId: string) {
    focusedPaneId.value = focusedPaneId.value === paneId ? null : paneId
  }

  function startPaneResize(e: MouseEvent, idx: number) {
    e.preventDefault()
    draggingPaneIdx.value = idx
    const startX = e.clientX
    const startWidths = [...paneWidths.value]
    const onMove = (e: MouseEvent) => {
      if (!panesContainer.value) return
      const containerWidth = panesContainer.value.offsetWidth
      const delta = e.clientX - startX
      const total = startWidths.reduce((a, b) => a + b, 0)
      const deltaFlex = (delta / containerWidth) * total
      const newWidths = [...startWidths]
      newWidths[idx] = Math.max(0.15, newWidths[idx] + deltaFlex)
      newWidths[idx + 1] = Math.max(0.15, newWidths[idx + 1] - deltaFlex)
      paneWidths.value = newWidths
    }
    const onUp = () => {
      draggingPaneIdx.value = null
      window.removeEventListener('mousemove', onMove)
      window.removeEventListener('mouseup', onUp)
    }
    window.addEventListener('mousemove', onMove)
    window.addEventListener('mouseup', onUp)
  }

  function getPaneTab(pane: PaneState): TableTab | null {
    const tab = pane.tabs.find(t => t.id === pane.activeTabId)
    return tab?.type === 'table' ? (tab as TableTab) : null
  }

  function isPaneActiveTabQuery(pane: PaneState): boolean {
    return (pane.tabs.find(t => t.id === pane.activeTabId)?.type ?? '') === 'query'
  }

  function getPaneQueryTabs(pane: PaneState): QueryTab[] {
    return pane.tabs.filter((t): t is QueryTab => t.type === 'query')
  }

  function getPrimaryKey(pane: PaneState): string | null {
    const tab = getPaneTab(pane)
    if (!tab) return null
    return (tab.tableStructure as any[]).find(c => c.key === 'PRI')?.field || null
  }

  function hasPendingChangesInPane(pane: PaneState): boolean {
    const tab = getPaneTab(pane)
    if (!tab) return false
    return tab.pendingDrop || tab.pendingTruncate || tab.pendingInserts.length > 0 || Object.keys(tab.pendingChanges).length > 0 || Object.keys(tab.pendingStructureChanges ?? {}).length > 0 || Object.keys(tab.pendingDeletions).length > 0
  }

  function getFkMap(pane: PaneState): Record<string, { table: string; column: string }> {
    const tab = getPaneTab(pane)
    if (!tab) return {}
    const map: Record<string, { table: string; column: string }> = {}
    for (const fk of tab.foreignKeys as any[]) {
      map[fk.column] = { table: fk.referenced_table, column: fk.referenced_column }
    }
    const connTables = store.openConnections[tab.connectionId]?.tables[tab.database] ?? []
    const tableNames = (connTables as any[]).map((t: any) => t.name.toLowerCase())
    const heuristicCols = [
      ...((tab.queryResult as any)?.columns ?? []).map((c: any) => c.name),
      ...((tab.tableStructure as any[]) ?? []).map((c: any) => c.field),
    ]
    for (const colName of heuristicCols) {
      if (colName.endsWith('_id') && !map[colName]) {
        const prefix = colName.slice(0, -3)
        for (const candidate of [prefix + 's', prefix + 'es', prefix]) {
          const i = tableNames.indexOf(candidate.toLowerCase())
          if (i !== -1) {
            map[colName] = { table: (connTables as any[])[i].name, column: 'id' }
            break
          }
        }
      }
    }
    return map
  }

  function getPaneConnection(pane: PaneState): Connection | null {
    const tab = getPaneTab(pane)
    if (!tab) return null
    return store.openConnections[tab.connectionId]?.connection ?? null
  }

  function isTableOpenInAnyPane(tableName: string, database: string, connectionId: string): boolean {
    return panes.value.some(pane =>
      pane.tabs.some(t => t.type === 'table' && (t as TableTab).tableName === tableName && (t as TableTab).database === database && t.connectionId === connectionId)
    )
  }

  function isTableActiveInAnyPane(tableName: string, database: string, connectionId: string): boolean {
    return panes.value.some(pane => {
      const tab = getPaneTab(pane)
      return tab?.tableName === tableName && tab?.database === database && tab?.connectionId === connectionId
    })
  }

  return {
    panes,
    activePaneId,
    paneWidths,
    draggingPaneIdx,
    focusedPaneId,
    getPane,
    addPane,
    removePane,
    toggleFocusPane,
    startPaneResize,
    getPaneTab,
    isPaneActiveTabQuery,
    getPaneQueryTabs,
    getPrimaryKey,
    hasPendingChangesInPane,
    getFkMap,
    getPaneConnection,
    isTableOpenInAnyPane,
    isTableActiveInAnyPane
  }
}
