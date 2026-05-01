import { ref, nextTick, type Ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useToast } from '@/composables/useToast'
import type { PaneState, TableTab } from '@/types/workspace'
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

export function useRowEditing(ctx: RowEditingContext) {
  const { getPaneTab, getPrimaryKey, getPaneConnection, refreshActiveTab, loadTableData } = ctx

  // ── Core state ──────────────────────────────────────────────────────────────

  const { error: toastError } = useToast()
  const isSaving = ref(false)
  const disableFkChecks = ref(false)

  // ── Insert row ──────────────────────────────────────────────────────────────

  const insertingRowPaneId = ref<string | null>(null)
  const insertingRowTabId = ref<string | null>(null)
  const insertRowValues = ref<Record<string, string>>({})
  const insertRowLoading = ref(false)
  const insertRowError = ref<string | null>(null)

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
      insertingRowPaneId.value = null
      insertingRowTabId.value = null
      return
    }
    insertRowValues.value = Object.fromEntries(
      (tab.tableStructure as any[])
        .filter((col: any) => col.extra !== 'auto_increment')
        .map((col: any) => [col.field, col.default ?? '']),
    )
    insertRowError.value = null
    insertingRowPaneId.value = pane.id
    insertingRowTabId.value = tab.id
    nextTick(() => document.querySelector<HTMLInputElement>('.insert-row-input')?.focus())
  }

  function cancelInsertRow() {
    insertingRowPaneId.value = null
    insertingRowTabId.value = null
    insertRowError.value = null
  }

  async function submitInsertRow(pane: PaneState) {
    const tab = getPaneTab(pane)
    const conn = getPaneConnection(pane)
    if (!tab || !conn) return
    insertRowLoading.value = true
    insertRowError.value = null
    try {
      const values = Object.entries(insertRowValues.value).map(([column, value]) => ({
        column,
        value: normalizeInsertValue(value),
      }))
      await invoke('insert_row', {
        connectionId: conn.id, database: tab.database, table: tab.tableName,
        values, disableFkChecks: disableFkChecks.value,
      })
      insertingRowPaneId.value = null
      insertingRowTabId.value = null
      await refreshActiveTab(pane.id)
    } catch (e: any) {
      const msg = String(e)
      const match = msg.match(/: (\d{4} \(.+?\): .+)$/)
      insertRowError.value = match ? match[1] : msg
    } finally {
      insertRowLoading.value = false
    }
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

  // ── Delete row (immediate, no pending changes) ──────────────────────────────

  async function deleteRowImmediate(pane: PaneState, row: any) {
    const tab = getPaneTab(pane)
    const conn = getPaneConnection(pane)
    const pk = getPrimaryKey(pane)
    if (!tab || !conn || !pk) return
    const pkVal = String(row[pk])
    isSaving.value = true
    try {
      await invoke('apply_table_changes', {
        connectionId: conn.id, database: tab.database, table: tab.tableName,
        updates: [],
        deletions: [{ pk_column: pk, pk_value: coercePkValue(pkVal) }],
        disableFkChecks: disableFkChecks.value,
      })
      if (tab.selectedRowPk === pkVal) {
        tab.selectedRowPk = null
        tab.inlineEditColumn = null
      }
      tab.selectedRowPks = tab.selectedRowPks.filter(p => p !== pkVal)
      await refreshActiveTab(pane.id)
    } catch (e: any) {
      toastError('Failed to delete row', String(e))
    } finally {
      isSaving.value = false
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
    tab.pendingTruncate = false
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

  function setViewMode(pane: PaneState, mode: 'content' | 'structure') {
    pane.viewMode = mode
    const tab = getPaneTab(pane)
    if (tab) tab.viewMode = mode
    if (mode !== 'content') clearRowSelection(pane)
  }

  // ── Apply changes ───────────────────────────────────────────────────────────

  async function applyChanges(pane: PaneState) {
    const tab = getPaneTab(pane)
    const conn = getPaneConnection(pane)
    if (!tab || !conn) return
    isSaving.value = true
    try {
      if (tab.pendingTruncate) {
        await invoke('truncate_table', {
          connectionId: conn.id, database: tab.database,
          table: tab.tableName, disableFkChecks: disableFkChecks.value,
        })
      } else {
        const pk = getPrimaryKey(pane)
        if (!pk) throw new Error('Table has no Primary Key')
        const updates = Object.entries(tab.pendingChanges).map(([pkValue, changes]) => ({
          pk_column: pk,
          pk_value: coercePkValue(pkValue),
          changes: Object.entries(changes).map(([column, value]) => ({
            column,
            value: normalizeChangeValue(value),
          })),
        }))
        const deletions = Object.keys(tab.pendingDeletions).map(pkValue => ({
          pk_column: pk,
          pk_value: coercePkValue(pkValue),
        }))
        await invoke('apply_table_changes', {
          connectionId: conn.id, database: tab.database, table: tab.tableName,
          updates, deletions, disableFkChecks: disableFkChecks.value,
        })
      }
      tab.pendingChanges = {}
      tab.pendingDeletions = {}
      tab.pendingTruncate = false
      tab.selectedRowPk = null
      tab.selectedRowPks = []
      tab.inlineEditColumn = null
      await refreshActiveTab(pane.id)
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
    insertRowLoading,
    insertRowError,
    isColAutoIncrement,
    isBooleanCol,
    openInsertRowDialog,
    cancelInsertRow,
    submitInsertRow,
    duplicateRow,
    deleteRowImmediate,
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
