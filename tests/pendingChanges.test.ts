import assert from 'node:assert/strict'
import test from 'node:test'
import { pendingTabsForDatabase, summarizePendingTabs } from '../src/lib/pendingChanges.js'
import type { PaneState, TableTab } from '../src/types/workspace.js'

function tableTab(
  id: string,
  connectionId: string,
  database: string,
  pendingInserts = 0,
): TableTab {
  return {
    type: 'table', id, connectionId, database, tableName: `table_${id}`,
    queryResult: null, tableStructure: [], tableIndexes: [], foreignKeys: [], ddl: null,
    page: 0, pageSize: 300, viewMode: 'content', filters: null,
    sortColumn: null, sortDesc: false, pendingChanges: {}, pendingStructureChanges: {},
    pendingDeletions: {}, pendingInserts: Array.from({ length: pendingInserts }, () => ({ values: [] })),
    pendingTruncate: false, pendingDrop: false, selectedRowPk: null, selectedRowPks: [],
    inlineEditColumn: null,
  }
}

function pane(id: string, tabs: TableTab[]): PaneState {
  return { id, tabs, activeTabId: tabs[0]?.id ?? null, viewMode: 'content', page: 0, pageSize: 300, showFilters: false }
}

test('pending changes are isolated by connection', () => {
  const first = tableTab('first', 'connection-a', 'app', 1)
  const second = tableTab('second', 'connection-b', 'app', 1)
  const panes = [pane('pane-a', [first]), pane('pane-b', [second])]

  assert.deepEqual(pendingTabsForDatabase(panes, first).map(tab => tab.id), ['first'])
  assert.equal(summarizePendingTabs(pendingTabsForDatabase(panes, first)).pendingInsertionsCount, 1)
})

test('pending changes are isolated by database within one connection', () => {
  const first = tableTab('first', 'connection-a', 'database-a', 1)
  const second = tableTab('second', 'connection-a', 'database-b', 1)
  const panes = [pane('pane-a', [first, second])]

  assert.deepEqual(pendingTabsForDatabase(panes, first).map(tab => tab.id), ['first'])
  assert.equal(summarizePendingTabs(pendingTabsForDatabase(panes, second)).pendingInsertionsCount, 1)
})

test('pending changes from different tables in the same database share one scope', () => {
  const first = tableTab('first', 'connection-a', 'app', 1)
  const second = tableTab('second', 'connection-a', 'app', 2)
  const panes = [pane('pane-a', [first, second])]

  const scopedTabs = pendingTabsForDatabase(panes, first)
  assert.deepEqual(scopedTabs.map(tab => tab.id), ['first', 'second'])
  assert.equal(summarizePendingTabs(scopedTabs).pendingInsertionsCount, 3)
})
