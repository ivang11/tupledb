import { rowValue, type DataRow } from './rowAccess.js'

export type NormalizedValue = string | number | null

export interface InsertValue {
  column: string
  value: unknown
}

interface TableColumn {
  field: string
  extra?: string
}

// Build a pending insert from an existing result row. Values are copied as-is:
// unlike user-entered insert text, a duplicated empty string or literal "null"
// must not be reinterpreted as NULL.
export function buildDuplicateInsertValues(
  row: DataRow,
  columns: TableColumn[],
): InsertValue[] {
  return columns
    .filter(column => column.extra !== 'auto_increment')
    .map(column => ({
      column: column.field,
      value: rowValue(row, column.field, columns) ?? null,
    }))
}

export function buildDuplicatePendingInserts(
  rows: DataRow[],
  columns: TableColumn[],
): Array<{ values: InsertValue[] }> {
  return rows.map(row => ({ values: buildDuplicateInsertValues(row, columns) }))
}

// Used when inserting a new row: empty string and null both become NULL.
// Does NOT coerce numeric strings — the column type is unknown at this layer.
export function normalizeInsertValue(value: string | null): NormalizedValue {
  if (value === '' || value === null) return null
  const lower = value.toLowerCase().trim()
  if (lower === 'null') return null
  if (lower === 'true') return 1
  if (lower === 'false') return 0
  return value
}

// Used when applying pending cell edits: keeps empty strings as-is (they
// represent an intentional empty value, not NULL), coerces numeric strings.
export function normalizeChangeValue(value: unknown): NormalizedValue {
  if (value === null) return null
  const str = String(value)
  if (str === '') return ''
  const lower = str.toLowerCase().trim()
  if (lower === 'null') return null
  if (lower === 'true') return 1
  if (lower === 'false') return 0
  const n = Number(str)
  return isNaN(n) ? str : n
}

// Coerce a PK string to a number when the string is a valid integer / float.
export function coercePkValue(pkVal: string): string | number {
  const n = Number(pkVal)
  return isNaN(n) ? pkVal : n
}

// Return the string to display inside a cell's inline edit input.
// Checks pending changes first, then falls back to the raw row value.
export function computeCellEditValue(
  pendingChanges: Record<string, Record<string, unknown>>,
  pk: string | null,
  row: DataRow,
  colName: string,
  columns: TableColumn[] = [],
): string {
  if (pk) {
    const pkVal = String(rowValue(row, pk, columns))
    const pending = pendingChanges[pkVal]?.[colName]
    if (pending !== undefined) return pending === null ? '' : String(pending)
  }
  const v = rowValue(row, colName, columns)
  if (v === null || v === undefined) return ''
  if (typeof v === 'object') return JSON.stringify(v)
  return String(v)
}
