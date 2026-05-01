// ── Sort / keyset utilities ──────────────────────────────────────────────────

export function buildSortPayload(
  sortColumn: string | null,
  sortDesc: boolean,
): { column: string; desc: boolean } | null {
  if (!sortColumn) return null
  return { column: sortColumn, desc: sortDesc }
}

// Returns the PK column to use for keyset pagination, or null when keyset is
// unsafe (active sort or filters change the natural order).
export function resolveKeysetColumn(
  structure: Array<{ field: string; key: string }>,
  sortColumn: string | null,
  filters: unknown | null,
): string | null {
  if (sortColumn || filters) return null
  return structure.find(c => c.key === 'PRI')?.field ?? null
}

// ── Row click selection ──────────────────────────────────────────────────────

export interface RowSelectionState {
  selectedRowPk: string | null
  selectedRowPks: string[]
  inlineEditColumn: string | null
}

// Compute the new selection state after a row click in the data grid.
//
// columnIndex: 0 = first col (row-selector), 1+ = data column.
// colName: name of the clicked data column (null when columnIndex === 0).
// allRowPks: ordered list of all visible row PKs, used for shift-range selection.
//
// Returns the new state, or null if the click should be a no-op.
export function computeRowClickSelection(
  pkVal: string,
  columnIndex: number,
  colName: string | null,
  modifiers: { ctrl: boolean; meta: boolean; shift: boolean },
  current: RowSelectionState,
  allRowPks: string[],
): RowSelectionState | null {
  // Ctrl / Meta: toggle individual row
  if (modifiers.ctrl || modifiers.meta) {
    const idx = current.selectedRowPks.indexOf(pkVal)
    const next = [...current.selectedRowPks]
    if (idx === -1) next.push(pkVal)
    else next.splice(idx, 1)
    return { selectedRowPk: pkVal, selectedRowPks: next, inlineEditColumn: null }
  }

  // Shift: range select from anchor
  if (modifiers.shift && current.selectedRowPk) {
    const anchorIdx = allRowPks.indexOf(current.selectedRowPk)
    const currentIdx = allRowPks.indexOf(pkVal)
    if (anchorIdx !== -1 && currentIdx !== -1) {
      const start = Math.min(anchorIdx, currentIdx)
      const end = Math.max(anchorIdx, currentIdx)
      return {
        selectedRowPk: current.selectedRowPk,
        selectedRowPks: allRowPks.slice(start, end + 1),
        inlineEditColumn: null,
      }
    }
  }

  // Single click on first column (row selector)
  if (columnIndex === 0) {
    if (current.selectedRowPk === pkVal) return null  // already selected
    return { selectedRowPk: pkVal, selectedRowPks: [pkVal], inlineEditColumn: null }
  }

  // Single click on a data column
  if (current.selectedRowPk === pkVal) {
    // Clicking a different column while another is in inline-edit: cancel the edit
    if (current.inlineEditColumn && current.inlineEditColumn !== colName) {
      return { ...current, inlineEditColumn: null }
    }
    return null  // same row, same (or no) inline-edit col — no-op
  }
  return { selectedRowPk: pkVal, selectedRowPks: [pkVal], inlineEditColumn: null }
}

// Compute the selection state for a table without a primary key.
// Selection is by row index; only one row can be selected at a time.
// Returns null if the row is already selected.
export function computeNoPkRowClick(
  rowKey: string,
  currentSelectedRowPk: string | null,
): RowSelectionState | null {
  if (currentSelectedRowPk === rowKey) return null
  return { selectedRowPk: rowKey, selectedRowPks: [], inlineEditColumn: null }
}
