import { ref, nextTick, type Ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useToast } from '@/composables/useToast'
import { useConnectionStore } from '@/stores/connections'
import type { PaneState, TableTab, TableViewMode } from '@/types/workspace'
import {
  normalizeInsertValue,
  normalizeChangeValue,
  coercePkValue,
  computeCellEditValue,
} from '@/lib/tableEditing'
import {
  computeRowClickSelection,
  computeNoPkRowClick,
} from '@/lib/rowSelection'

interface RowEditingContext {
  panes: Ref<PaneState[]>
  getPaneTab: (pane: PaneState) => TableTab | null
  getPrimaryKey: (pane: PaneState) => string | null
  getPaneConnection: (pane: PaneState) => any
  refreshActiveTab: (paneId?: string) => Promise<void>
  loadTableData: (tableName: string, connectionId: string, database: string, initialFilter?: any, paneId?: string) => Promise<void>
}

function tabHasPendingChanges(tab: TableTab): boolean {
  return tab.pendingDrop ||
    tab.pendingTruncate ||
    tab.pendingInserts.length > 0 ||
    Object.keys(tab.pendingChanges).length > 0 ||
    Object.keys(tab.pendingDeletions).length > 0
}

export function useRowEditing(ctx: RowEditingContext) {
  const { getPaneTab, getPrimaryKey, loadTableData } = ctx
  const store = useConnectionStore()

  // ── Core state ──────────────────────────────────────────────────────────────

  const { error: toastError } = useToast()
  const isSaving = ref(false)
  const disableFkChecks = ref(false)

  // ── Insert row ──────────────────────────────────────────────────────────────

  const insertingRowPaneId = ref<string | null>(null)
  const insertingRowTabId = ref<string | null>(null)
  const insertRowValues = ref<Record<string, string>>({})
  const insertRowError = ref<string | null>(null)
  const pendingInsertDraft = ref<{ tabId: string; index: number } | null>(null)

  function buildInsertValues() {
    return Object.entries(insertRowValues.value).map(([column, value]) => ({
      column,
      value: normalizeInsertValue(value),
    }))
  }

  function updatePendingInsertDraft(tab: TableTab) {
    const draft = pendingInsertDraft.value
    if (!draft || draft.tabId !== tab.id) return
    const pendingInsert = tab.pendingInserts[draft.index]
    if (!pendingInsert) return
    pendingInsert.values = buildInsertValues()
  }

  function isColAutoIncrement(pane: PaneState, colName: string): boolean {
    const tab = getPaneTab(pane)
    if (!tab) return false
    return (tab.tableStructure as any[]).find((c: any) => c.field === colName)?.extra === 'auto_increment'
  }

  function isBooleanCol(pane: PaneState, colName: string): boolean {
    const tab = getPaneTab(pane)
    if (!tab) return false
    const col = (tab.tableStructure as any[]).find((c: any) => c.field === colName)
    const type = (col?.type ?? '').toLowerCase()
    return type === 'tinyint(1)' || type === 'boolean' || type === 'bool'
  }

  function openInsertRowDialog(pane: PaneState) {
    const tab = getPaneTab(pane)
    if (!tab) return
    if (insertingRowPaneId.value === pane.id && insertingRowTabId.value === tab.id) {
      nextTick(() => document.querySelector<HTMLInputElement>('.insert-row-input')?.focus())
      return
    }
    insertRowValues.value = Object.fromEntries(
      (tab.tableStructure as any[])
        .filter((col: any) => col.extra !== 'auto_increment')
        .map((col: any) => [col.field, col.default ?? '']),
    )
    tab.pendingInserts.push({ values: buildInsertValues() })
    pendingInsertDraft.value = { tabId: tab.id, index: tab.pendingInserts.length - 1 }
    insertRowError.value = null
    insertingRowPaneId.value = pane.id
    insertingRowTabId.value = tab.id
    nextTick(() => document.querySelector<HTMLInputElement>('.insert-row-input')?.focus())
  }

  function cancelInsertRow() {
    const tab = ctx.panes.value
      .flatMap(pane => pane.tabs)
      .find((t): t is TableTab => t.type === 'table' && t.id === pendingInsertDraft.value?.tabId)
    if (tab && pendingInsertDraft.value) {
      tab.pendingInserts.splice(pendingInsertDraft.value.index, 1)
    }
    pendingInsertDraft.value = null
    insertingRowPaneId.value = null
    insertingRowTabId.value = null
    insertRowError.value = null
  }

  function updateInsertRowValue(pane: PaneState, column: string, value: string) {
    const tab = getPaneTab(pane)
    if (!tab) return
    insertRowValues.value[column] = value
    updatePendingInsertDraft(tab)
  }

  function duplicateRow(pane: PaneState, row: any) {
    const tab = getPaneTab(pane)
    if (!tab) return
    insertRowValues.value = Object.fromEntries(
      (tab.tableStructure as any[])
        .filter((col: any) => col.extra !== 'auto_increment')
        .map((col: any) => {
          const val = row[col.field]
          return [col.field, val === null || val === undefined ? '' : String(val)]
        }),
    )
    insertRowError.value = null
    insertingRowPaneId.value = pane.id
    insertingRowTabId.value = tab.id
    nextTick(() => document.querySelector<HTMLInputElement>('.insert-row-input')?.focus())
  }

  async function duplicateSelectedRows(pane: PaneState) {
    const tab = getPaneTab(pane)
    const pk = getPrimaryKey(pane)
    if (!tab || !pk || !tab.queryResult?.rows) return
    const selectedPks = tab.selectedRowPks
    if (selectedPks.length === 0) return

    const rows = tab.queryResult.rows.filter((r: any) =>
      selectedPks.includes(String(r[pk]))
    )
    const cols = (tab.tableStructure as any[]).filter(
      (col: any) => col.extra !== 'auto_increment'
    )

    try {
      for (const row of rows) {
        const values = cols.map((col: any) => ({
          column: col.field,
          value: normalizeInsertValue(
            row[col.field] === null || row[col.field] === undefined
              ? ''
              : String(row[col.field])
          ),
        }))
        tab.pendingInserts.push({ values })
      }
    } catch (e: any) {
      toastError('Failed to stage duplicated rows', String(e))
    }
  }

  // ── Cell editing ────────────────────────────────────────────────────────────

  function updatePendingChange(pane: PaneState, row: any, column: string, newValue: any) {
    const tab = getPaneTab(pane)
    const pk = getPrimaryKey(pane)
    if (!tab || !pk) return
    const pkVal = String(row[pk])
    const originalValue = row[column]
    if (newValue === originalValue) {
      if (tab.pendingChanges[pkVal]) {
        delete tab.pendingChanges[pkVal][column]
        if (Object.keys(tab.pendingChanges[pkVal]).length === 0)
          delete tab.pendingChanges[pkVal]
      }
      return
    }
    if (!tab.pendingChanges[pkVal]) tab.pendingChanges[pkVal] = {}
    tab.pendingChanges[pkVal][column] = newValue
  }

  function toggleDeletion(pane: PaneState, row: any) {
    const tab = getPaneTab(pane)
    const pk = getPrimaryKey(pane)
    if (!tab || !pk) return
    const pkVal = String(row[pk])
    if (tab.pendingDeletions[pkVal]) delete tab.pendingDeletions[pkVal]
    else tab.pendingDeletions[pkVal] = true
  }

  function toggleDeletionSelected(pane: PaneState) {
    const tab = getPaneTab(pane)
    const pk = getPrimaryKey(pane)
    if (!tab || !pk || !tab.selectedRowPks.length) return
    const rows = tab.queryResult?.rows ?? []
    for (const pkVal of tab.selectedRowPks) {
      const row = rows.find((r: any) => String(r[pk]) === pkVal)
      if (row) toggleDeletion(pane, row)
    }
  }

  function discardChanges(pane: PaneState) {
    const tab = getPaneTab(pane)
    if (!tab) return
    tab.pendingChanges = {}
    tab.pendingDeletions = {}
    tab.pendingInserts = []
    tab.pendingTruncate = false
    tab.pendingDrop = false
    if (insertingRowTabId.value === tab.id) {
      pendingInsertDraft.value = null
      insertingRowPaneId.value = null
      insertingRowTabId.value = null
      insertRowError.value = null
    }
    tab.selectedRowPk = null
    tab.selectedRowPks = []
    tab.inlineEditColumn = null
  }

  function clearRowSelection(pane: PaneState) {
    const tab = getPaneTab(pane)
    if (!tab) return
    tab.selectedRowPk = null
    tab.selectedRowPks = []
    tab.inlineEditColumn = null
  }

  function getSelectedRow(pane: PaneState): Record<string, any> | null {
    const tab = getPaneTab(pane)
    const pk = getPrimaryKey(pane)
    if (!tab?.queryResult?.rows || !tab.selectedRowPk) return null
    if (!pk) {
      const match = tab.selectedRowPk.match(/^__row_index:(\d+)$/)
      if (!match) return null
      return tab.queryResult.rows[Number(match[1])] ?? null
    }
    return tab.queryResult.rows.find((r: any) => String(r[pk]) === tab.selectedRowPk) ?? null
  }

  function onTableRowClick(pane: PaneState, row: any, e: MouseEvent, rowIndex?: number) {
    const el = e.target as HTMLElement
    if (el.closest('button')) return
    const td = el.closest('td')
    if (!td?.parentElement) return
    const tdIdx = Array.from(td.parentElement.children).indexOf(td)
    const pk = getPrimaryKey(pane)
    const tab = getPaneTab(pane)
    if (!tab) return

    if (!pk) {
      const key = `__row_index:${rowIndex ?? tab.queryResult?.rows?.indexOf(row) ?? 0}`
      const next = computeNoPkRowClick(key, tab.selectedRowPk)
      if (next) Object.assign(tab, next)
      return
    }

    const pkVal = String((row as any)[pk])
    const cols = tab.queryResult?.columns
    const colName = tdIdx > 0 ? ((cols?.[tdIdx - 1] as { name: string } | undefined)?.name ?? null) : null
    const allRowPks = (tab.queryResult?.rows as any[] ?? []).map((r: any) => String(r[pk]))
    const next = computeRowClickSelection(
      pkVal,
      tdIdx,
      colName,
      { ctrl: e.ctrlKey, meta: e.metaKey, shift: e.shiftKey },
      { selectedRowPk: tab.selectedRowPk, selectedRowPks: tab.selectedRowPks, inlineEditColumn: tab.inlineEditColumn },
      allRowPks,
    )
    if (next) Object.assign(tab, next)
  }

  function onCellDblclick(pane: PaneState, row: any, colName: string) {
    const tab = getPaneTab(pane)
    const pk = getPrimaryKey(pane)
    if (!tab || !pk) return
    const pkVal = String((row as any)[pk])
    if (tab.pendingDeletions[pkVal]) return
    tab.selectedRowPk = pkVal
    tab.selectedRowPks = [pkVal]
    tab.inlineEditColumn = colName
    nextTick(() => {
      try {
        const sel = `input[data-grid-edit="${CSS.escape(String(pkVal))}"][data-col="${CSS.escape(colName)}"]`
        const el = document.querySelector<HTMLInputElement>(sel)
        el?.focus(); el?.select()
      } catch { /* ignore */ }
    })
  }

  function onCellBlur(pane: PaneState) {
    const tab = getPaneTab(pane)
    if (tab) tab.inlineEditColumn = null
  }

  function cellEditValue(pane: PaneState, row: any, colName: string): string {
    const tab = getPaneTab(pane)
    if (!tab) return ''
    return computeCellEditValue(tab.pendingChanges, getPrimaryKey(pane), row, colName)
  }

  function setViewMode(pane: PaneState, mode: TableViewMode) {
    pane.viewMode = mode
    const tab = getPaneTab(pane)
    if (tab) tab.viewMode = mode
    if (mode !== 'content') clearRowSelection(pane)
  }

  // ── Apply changes ───────────────────────────────────────────────────────────

  function getTabPrimaryKey(tab: TableTab): string | null {
    return (tab.tableStructure as any[]).find(c => c.key === 'PRI')?.field || null
  }

  function clearTabPendingState(tab: TableTab) {
    tab.pendingChanges = {}
    tab.pendingDeletions = {}
    tab.pendingInserts = []
    tab.pendingTruncate = false
    tab.pendingDrop = false
    tab.selectedRowPk = null
    tab.selectedRowPks = []
    tab.inlineEditColumn = null
  }

  function closeMatchingTableTabs(target: TableTab) {
    for (const p of ctx.panes.value) {
      const related = p.tabs.filter(t =>
        t.type === 'table' &&
        (t as TableTab).connectionId === target.connectionId &&
        (t as TableTab).database === target.database &&
        (t as TableTab).tableName === target.tableName,
      )
      for (const relatedTab of related) {
        const idx = p.tabs.findIndex(t => t.id === relatedTab.id)
        if (idx !== -1) p.tabs.splice(idx, 1)
        if (p.activeTabId === relatedTab.id) p.activeTabId = p.tabs[0]?.id ?? null
      }
    }
  }

  function matchingOpenTableTabs(target: TableTab): TableTab[] {
    return ctx.panes.value.flatMap(p =>
      p.tabs.filter((t): t is TableTab =>
        t.type === 'table' &&
        t.connectionId === target.connectionId &&
        t.database === target.database &&
        t.tableName === target.tableName,
      ),
    )
  }

  async function refreshMatchingTableTabs(target: TableTab) {
    for (const tab of matchingOpenTableTabs(target)) {
      tab.queryResult = await store.fetchTableData(
        tab.connectionId,
        tab.database,
        tab.tableName,
        tab.page,
        tab.pageSize,
        tab.filters ?? null,
        tab.sortColumn ? { column: tab.sortColumn, desc: tab.sortDesc } : null,
      )
    }
  }

  async function applyTabChanges(tab: TableTab): Promise<'dropped' | 'changed'> {
    if (tab.pendingDrop) {
      await invoke('drop_table', {
        connectionId: tab.connectionId, database: tab.database,
        table: tab.tableName, disableFkChecks: disableFkChecks.value,
      })
      closeMatchingTableTabs(tab)
      await store.fetchTablesForConnection(tab.connectionId, tab.database)
      return 'dropped'
    }

    if (tab.pendingTruncate) {
      await invoke('truncate_table', {
        connectionId: tab.connectionId, database: tab.database,
        table: tab.tableName, disableFkChecks: disableFkChecks.value,
      })
    }

    if (!tab.pendingTruncate || tab.pendingInserts.length > 0) {
      const pk = getTabPrimaryKey(tab)
      const hasRowMutations = Object.keys(tab.pendingChanges).length > 0 || Object.keys(tab.pendingDeletions).length > 0
      if (hasRowMutations && !pk) throw new Error(`Table \`${tab.tableName}\` has no Primary Key`)
      const updates = Object.entries(tab.pendingChanges).map(([pkValue, changes]) => ({
        pk_column: pk!,
        pk_value: coercePkValue(pkValue),
        changes: Object.entries(changes).map(([column, value]) => ({
          column,
          value: normalizeChangeValue(value),
        })),
      }))
      const deletions = Object.keys(tab.pendingDeletions).map(pkValue => ({
        pk_column: pk!,
        pk_value: coercePkValue(pkValue),
      }))
      if (updates.length > 0 || deletions.length > 0) {
        await invoke('apply_table_changes', {
          connectionId: tab.connectionId, database: tab.database, table: tab.tableName,
          updates, deletions, disableFkChecks: disableFkChecks.value,
        })
      }
    }

    for (const insert of tab.pendingInserts) {
      await invoke('insert_row', {
        connectionId: tab.connectionId,
        database: tab.database,
        table: tab.tableName,
        values: insert.values,
        disableFkChecks: disableFkChecks.value,
      })
    }

    clearTabPendingState(tab)
    return 'changed'
  }

  async function applyChanges(_pane: PaneState) {
    const pendingTabs = ctx.panes.value.flatMap(p =>
      p.tabs.filter((t): t is TableTab => t.type === 'table' && tabHasPendingChanges(t)),
    )
    if (pendingTabs.length === 0) return

    isSaving.value = true
    try {
      const changedTabs: TableTab[] = []
      for (const tab of [...pendingTabs]) {
        const result = await applyTabChanges(tab)
        if (result === 'changed') {
          changedTabs.push(tab)
        }
      }
      for (const tab of changedTabs) {
        await refreshMatchingTableTabs(tab)
      }
    } catch (e: any) {
      toastError('Failed to apply changes', String(e))
    } finally {
      isSaving.value = false
    }
  }

  async function navigateToRelated(pane: PaneState, targetTable: string, filterColumn: string, filterValue: any) {
    const tab = getPaneTab(pane)
    if (!tab) return
    const filter = {
      match_all: true,
      rows: [{ active: true, column: filterColumn, operator: 'equals', value: String(filterValue) }],
    }
    await loadTableData(targetTable, tab.connectionId, tab.database, filter, pane.id)
    pane.showFilters = true
  }

  return {
    isSaving,
    disableFkChecks,
    insertingRowPaneId,
    insertingRowTabId,
    insertRowValues,
    insertRowError,
    isColAutoIncrement,
    isBooleanCol,
    openInsertRowDialog,
    cancelInsertRow,
    updateInsertRowValue,
    duplicateRow,
    duplicateSelectedRows,
    updatePendingChange,
    toggleDeletion,
    toggleDeletionSelected,
    discardChanges,
    clearRowSelection,
    getSelectedRow,
    onTableRowClick,
    onCellDblclick,
    onCellBlur,
    cellEditValue,
    setViewMode,
    applyChanges,
    navigateToRelated,
  }
}
