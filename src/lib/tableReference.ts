import type { DatabaseTable, TableRef } from '../types/database.js'

export function tableReferenceKey(table: TableRef): string {
  return JSON.stringify([table.catalog, table.schema, table.name])
}

/** Composite key identifying a table tab/selection by connection, database and handle. */
export function tableSelectionKey(connectionId: string, database: string, tableName: string): string {
  return JSON.stringify([connectionId, database, tableName])
}

export function parseTableSelectionKey(key: string): [string, string, string] {
  return JSON.parse(key) as [string, string, string]
}

export function tableLabel(table: string | TableRef): string {
  return typeof table === 'string' ? table : table.schema ? `${table.schema}.${table.name}` : table.name
}

/** Opaque UI handle, resolved through catalog metadata before any IPC call. */
export function tableHandle(table: { name: string; reference?: TableRef } | TableRef): string {
  const ref = 'catalog' in table ? table : table.reference
  return ref?.schema != null ? `@table:${tableReferenceKey(ref)}` : table.name
}

export function tableSqlName(table: TableRef): string {
  const quote = (name: string) => `"${name.replaceAll('"', '""')}"`
  return table.schema != null ? `${quote(table.schema)}.${quote(table.name)}` : table.name
}

export function resolveTableReference(catalog: string, table: string | TableRef, tables: DatabaseTable[] = []): TableRef {
  if (typeof table !== 'string') {
    if (table.catalog !== catalog || !table.name || table.schema === '') throw new Error('Invalid table reference')
    return { ...table }
  }
  if (table.startsWith('@table:')) {
    const explicit = tables.find(t => tableHandle(t) === table && t.reference?.schema != null)
    if (explicit) return { ...explicit.reference }
    throw new Error('Unknown table reference; refresh the table list and try again')
  }
  const matches = tables.filter(t => t.name === table)
  if (matches.length > 1) throw new Error(`Table "${table}" is ambiguous; select its schema`)
  return matches[0]?.reference ?? { catalog, schema: null, name: table }
}
