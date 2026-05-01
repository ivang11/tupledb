export type NormalizedValue = string | number | null

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
  row: Record<string, unknown>,
  colName: string,
): string {
  if (pk) {
    const pkVal = String(row[pk])
    const pending = pendingChanges[pkVal]?.[colName]
    if (pending !== undefined) return pending === null ? '' : String(pending)
  }
  const v = row[colName]
  if (v === null || v === undefined) return ''
  if (typeof v === 'object') return JSON.stringify(v)
  return String(v)
}
