import type { DatabaseTable, TableRef } from '../types/database.js'

export function tableReferenceKey(table: TableRef): string {
  return JSON.stringify([table.catalog, table.schema, table.name])
}

export function tableLabel(table: string | TableRef): string {
  return typeof table === 'string' ? table : table.schema ? `${table.schema}.${table.name}` : table.name
}

export function resolveTableReference(catalog: string, table: string | TableRef, tables: DatabaseTable[] = []): TableRef {
  if (typeof table !== 'string') {
    if (table.catalog !== catalog || !table.name || table.schema === '') throw new Error('Invalid table reference')
    return { ...table }
  }
  const matches = tables.filter(t => t.name === table)
  if (matches.length > 1) throw new Error(`Table "${table}" is ambiguous; select its schema`)
  return matches[0]?.reference ?? { catalog, schema: null, name: table }
}
