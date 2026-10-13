interface SidebarTable {
  table_type?: string
  reference?: { schema?: string | null }
}

export function groupSidebarTables<T extends SidebarTable>(tables: T[]) {
  const groups = new Map<string, { key: string; label: string; tables: T[] }>()
  for (const table of tables) {
    const schema = table.reference?.schema
    const kind = String(table.table_type ?? '').toUpperCase().includes('VIEW') ? 'Views' : 'Tables'
    const key = JSON.stringify([schema ?? null, kind])
    if (!groups.has(key)) groups.set(key, { key, label: schema ? `${schema} · ${kind}` : kind, tables: [] })
    groups.get(key)!.tables.push(table)
  }
  return [...groups.values()]
}
