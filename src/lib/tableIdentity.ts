import { rowValue, type DataRow, type NamedColumn } from './rowAccess.js'

// Arrays preserve every component and its type. A scalar is accepted for
// reusable grids whose consumers still supply a single-column key.
export type PrimaryKey = string | string[] | null

export function primaryKeyColumns(structure: Array<{
  field: string; primary_key_position?: number | null; key?: string
}>): string[] | null {
  const keys = structure.filter(c => c.primary_key_position != null || c.key === 'PRI')
    .sort((a, b) => (a.primary_key_position ?? Infinity) - (b.primary_key_position ?? Infinity))
    .map(c => c.field)
  return keys.length ? keys : null
}

export function isPrimaryKeyColumn(key: PrimaryKey, column: string): boolean {
  return Array.isArray(key) ? key.includes(column) : key === column
}

export function rowKey(row: DataRow, key: PrimaryKey, columns: NamedColumn[] = []): string {
  if (!key) return ''
  if (typeof key === 'string') return String(rowValue(row, key, columns))
  return JSON.stringify(key.map(column => {
    const value = rowValue(row, column, columns)
    if (value === undefined || value === null) throw new Error(`Missing primary key value: ${column}`)
    return { column, value }
  }))
}

export function decodeRowKey(encoded: string, columns: string[]): Array<{ column: string; value: unknown }> {
  const parts: unknown = JSON.parse(encoded)
  if (!Array.isArray(parts) || parts.length !== columns.length || !columns.length
    || parts.some((p, i) => !p || p.column !== columns[i] || p.value == null
      || !['string', 'number', 'boolean'].includes(typeof p.value))) {
    throw new Error('Invalid row identity')
  }
  return parts
}
