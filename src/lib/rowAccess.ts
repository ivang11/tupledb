export type DataRow = Record<string, unknown> | unknown[]

export interface NamedColumn {
  name?: string
  field?: string
}

const columnIndexCache = new WeakMap<NamedColumn[], Map<string, number>>()

function columnName(column: NamedColumn): string | undefined {
  return column.name ?? column.field
}

export function columnIndex(columns: NamedColumn[], name: string): number {
  let indexes = columnIndexCache.get(columns)
  if (!indexes) {
    indexes = new Map(
      columns.map((column, index) => [columnName(column) ?? String(index), index]),
    )
    columnIndexCache.set(columns, indexes)
  }
  return indexes.get(name) ?? -1
}

export function rowValue(
  row: DataRow,
  name: string,
  columns: NamedColumn[],
): unknown {
  if (!Array.isArray(row)) return row[name]
  const index = columnIndex(columns, name)
  return index < 0 ? undefined : row[index]
}

export function rowRecord(row: DataRow, columns: NamedColumn[]): Record<string, unknown> {
  if (!Array.isArray(row)) return row
  return Object.fromEntries(
    columns.map((column, index) => [columnName(column) ?? String(index), row[index]]),
  )
}
