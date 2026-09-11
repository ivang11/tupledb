import { rowKey, type PrimaryKey } from './tableIdentity.js'
import { rowValue, type DataRow } from './rowAccess.js'

export type NormalizedValue = string | number | boolean | null

export interface InsertValue {
  column: string
  value: unknown
}

interface TableColumn {
  field: string
  extra?: string
  is_identity?: boolean
  is_generated?: boolean
}

// Build a pending insert from an existing result row. Values are copied as-is:
// unlike user-entered insert text, a duplicated empty string or literal "null"
// must not be reinterpreted as NULL.
export function buildDuplicateInsertValues(
  row: DataRow,
  columns: TableColumn[],
): InsertValue[] {
  return columns
    .filter(column => !column.is_identity && !column.is_generated && column.extra !== 'auto_increment')
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
export function normalizeInsertValue(value: string | null, kind?: string): NormalizedValue {
  if (value === '' || value === null) return null
  if (kind) return normalizeChangeValue(value, kind)
  const lower = value.toLowerCase().trim()
  if (lower === 'null') return null
  if (lower === 'true') return 1
  if (lower === 'false') return 0
  return value
}

// Used when applying pending cell edits: keeps empty strings as-is (they
// represent an intentional empty value, not NULL). Metadata controls parsing;
// the no-metadata branch is retained for legacy callers only.
export function normalizeChangeValue(value: unknown, kind?: string): NormalizedValue {
  if (value === null) return null
  // With metadata, never guess a type from a text cell's contents. Exact
  // numbers travel as strings; the adapter binds them to the destination type.
  if (kind) {
    if (kind === 'boolean') {
      if (value === true || value === 1 || value === '1' || String(value).toLowerCase() === 'true') return true
      if (value === false || value === 0 || value === '0' || String(value).toLowerCase() === 'false') return false
      throw new Error('Expected a boolean value')
    }
    if (kind === 'json' && typeof value === 'object') return JSON.stringify(value)
    return String(value)
  }
  const str = String(value)
  if (str === '') return ''
  const lower = str.toLowerCase().trim()
  if (lower === 'null') return null
  if (lower === 'true') return 1
  if (lower === 'false') return 0
  const n = Number(str)
  return isNaN(n) || !Number.isSafeInteger(n) ? str : n
}

// Coerce a PK string to a number when the string is a valid integer / float.
export function coercePkValue(pkVal: string): string | number {
  const n = Number(pkVal)
  return isNaN(n) || !Number.isSafeInteger(n) ? pkVal : n
}

// Return the string to display inside a cell's inline edit input.
// Checks pending changes first, then falls back to the raw row value.
export function computeCellEditValue(
  pendingChanges: Record<string, Record<string, unknown>>,
  pk: PrimaryKey,
  row: DataRow,
  colName: string,
  columns: TableColumn[] = [],
): string {
  if (pk) {
    const pkVal = rowKey(row, pk, columns)
    const pending = pendingChanges[pkVal]?.[colName]
    if (pending !== undefined) return pending === null ? '' : String(pending)
  }
  const v = rowValue(row, colName, columns)
  if (v === null || v === undefined) return ''
  if (typeof v === 'object') return JSON.stringify(v)
  return String(v)
}
