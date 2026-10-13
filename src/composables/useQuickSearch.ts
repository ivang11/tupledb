import { computed, ref } from 'vue'
import { useConnectionStore } from '@/stores/connections'
import { useSavedQueriesStore } from '@/stores/savedQueries'
import { useActionShortcut } from './useKeyboardShortcut'
import { useToast } from './useToast'
import { tableHandle, tableLabel } from '@/lib/tableReference'
import { databaseEngines, connectionAddress } from '@/lib/databaseEngines'
import type { QuickSearchItem } from '@/components/dialogs/QuickSearchDialog.vue'
export function useQuickSearch(ctx: {
  connect: (connection: any) => Promise<boolean | void>;
  selectDatabase: (connectionId: string, database: string) => Promise<void>;
  openTable: (table: string, connectionId: string, database: string) => Promise<void>;
  openQuery: (connectionId: string, database: string | null) => void;
  setSql: (sql: string) => void;
  showWorkspace: () => void; selectedConnectionId: { value: string | null };
}) {
  const store = useConnectionStore(), saved = useSavedQueriesStore()
  const open = ref(false), busy = ref(false)
  async function show() {
    open.value = true
    try { await saved.fetch() } catch (error) { useToast().error('Could not load saved queries', String(error)) }
  }
  useActionShortcut('quickSearch', () => { void show() })
  const items = computed<QuickSearchItem[]>(() => {
    const result: QuickSearchItem[] = store.connections.map(connection => ({ id: `connection:${connection.id}`, kind: 'Connection', label: connection.name, detail: `${databaseEngines[connection.database.engine]?.label ?? connection.database.engine} · ${connectionAddress(connection)}`, connectionId: connection.id, engine: connection.database.engine }))
    for (const [id, state] of Object.entries(store.openConnections)) {
      for (const database of state.databases) result.push({ id: JSON.stringify(['db', id, database]), kind: 'Database', label: database, detail: state.connection.name, connectionId: id, database, engine: state.connection.database.engine })
      for (const [database, tables] of Object.entries(state.tables)) for (const table of tables) result.push({ id: JSON.stringify(['table', id, database, tableHandle(table)]), kind: table.table_type.includes('VIEW') ? 'View' : 'Table', label: tableLabel(table.reference ?? table.name), detail: `${state.connection.name} · ${database}`, connectionId: id, database, table: tableHandle(table), engine: state.connection.database.engine })
    }
    for (const query of saved.queries ?? []) result.push({ id: `query:${query.id}`, kind: 'Query', label: query.name, detail: query.database ?? 'Saved query', connectionId: query.connection_id ?? ctx.selectedConnectionId.value ?? '', database: query.database, sql: query.sql })
    return result
  })
  async function select(item: QuickSearchItem) {
    if (busy.value) return
    busy.value = true
    try {
      const connection = store.connections.find(connection => connection.id === item.connectionId)
      if (!connection) throw new Error('Choose a connection before opening this saved query')
      if (await ctx.connect(connection) === false) return
      ctx.selectedConnectionId.value = item.connectionId
      if (item.database) await ctx.selectDatabase(item.connectionId, item.database)
      if (item.table && item.database) await ctx.openTable(item.table, item.connectionId, item.database)
      else if (item.sql !== undefined) { ctx.openQuery(item.connectionId, item.database ?? null); ctx.setSql(item.sql) }
      ctx.showWorkspace(); open.value = false
    } catch (error) { useToast().error('Could not open search result', String(error)) }
    finally { busy.value = false }
  }
  return { open, busy, items, show, select }
}
