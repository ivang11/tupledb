// Find the insertion index for a new tab so tabs from the same connection and
// database stay grouped together. Scans right-to-left to find the rightmost
// exact (conn+db) match; falls back to the rightmost same-connection match;
// falls back to appending at the end.
export function findTabInsertIndex(
  tabs: Array<{ connectionId: string; database?: string | null }>,
  connectionId: string,
  database: string | null,
): number {
  let fallback = tabs.length
  for (let i = tabs.length - 1; i >= 0; i--) {
    const t = tabs[i]
    if (t.connectionId === connectionId && (t.database ?? null) === database) {
      return i + 1
    }
    if (t.connectionId === connectionId && fallback === tabs.length) {
      fallback = i + 1
    }
  }
  return fallback
}

// Returns the index to activate after a tab is closed.
// tabCount is the number of tabs AFTER the removal; closedIndex is where the
// closed tab was before removal.
export function findNextActiveIndex(tabCount: number, closedIndex: number): number {
  return Math.min(closedIndex, tabCount - 1)
}
