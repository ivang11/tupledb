import type { PaneState, TableTab } from '@/types/workspace'

export interface PendingChangesSummary {
  pendingDrop: boolean
  pendingTruncate: boolean
  pendingChangesCount: number
  pendingStructureChangesCount: number
  pendingDeletionsCount: number
  pendingInsertionsCount: number
}

export function tabHasPendingChanges(tab: TableTab): boolean {
  return tab.pendingDrop ||
    tab.pendingTruncate ||
    tab.pendingInserts.length > 0 ||
    Object.keys(tab.pendingChanges).length > 0 ||
    Object.keys(tab.pendingStructureChanges ?? {}).length > 0 ||
    Object.keys(tab.pendingDeletions).length > 0
}

export function pendingTabsForDatabase(panes: PaneState[], target: TableTab): TableTab[] {
  return panes.flatMap(pane =>
    pane.tabs.filter((tab): tab is TableTab =>
      tab.type === 'table' &&
      tab.connectionId === target.connectionId &&
      tab.database === target.database &&
      tabHasPendingChanges(tab),
    ),
  )
}

export function summarizePendingTabs(tabs: TableTab[]): PendingChangesSummary {
  const summary: PendingChangesSummary = {
    pendingDrop: false,
    pendingTruncate: false,
    pendingChangesCount: 0,
    pendingStructureChangesCount: 0,
    pendingDeletionsCount: 0,
    pendingInsertionsCount: 0,
  }

  for (const tab of tabs) {
    summary.pendingDrop ||= tab.pendingDrop
    summary.pendingTruncate ||= tab.pendingTruncate
    summary.pendingChangesCount += Object.keys(tab.pendingChanges).length
    summary.pendingStructureChangesCount += Object.keys(tab.pendingStructureChanges ?? {}).length
    summary.pendingDeletionsCount += Object.keys(tab.pendingDeletions).length
    summary.pendingInsertionsCount += tab.pendingInserts.length
  }

  return summary
}
