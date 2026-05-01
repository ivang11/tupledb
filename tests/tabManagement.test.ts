import assert from 'node:assert/strict'
import test from 'node:test'
import { findTabInsertIndex, findNextActiveIndex } from '../src/lib/tabManagement.js'

// ── findTabInsertIndex ──────────────────────────────────────────────────────

type Tab = { connectionId: string; database?: string | null }

test('findTabInsertIndex: appends at end when tabs are empty', () => {
  assert.equal(findTabInsertIndex([], 'c1', 'db1'), 0)
})

test('findTabInsertIndex: appends at end when no tab matches the connection', () => {
  const tabs: Tab[] = [
    { connectionId: 'c2', database: 'db1' },
    { connectionId: 'c2', database: 'db2' },
  ]
  assert.equal(findTabInsertIndex(tabs, 'c1', 'db1'), 2)
})

test('findTabInsertIndex: inserts after rightmost exact conn+db match', () => {
  const tabs: Tab[] = [
    { connectionId: 'c1', database: 'db1' },
    { connectionId: 'c1', database: 'db2' },
    { connectionId: 'c1', database: 'db1' },
    { connectionId: 'c2', database: 'db1' },
  ]
  // Rightmost c1+db1 is at index 2 → insert at 3
  assert.equal(findTabInsertIndex(tabs, 'c1', 'db1'), 3)
})

test('findTabInsertIndex: falls back to rightmost same-connection tab when no exact db match', () => {
  const tabs: Tab[] = [
    { connectionId: 'c1', database: 'db1' },
    { connectionId: 'c1', database: 'db1' },
    { connectionId: 'c2', database: 'db1' },
  ]
  // No c1+db2 tab exists; rightmost c1 tab is at index 1 → insert at 2
  assert.equal(findTabInsertIndex(tabs, 'c1', 'db2'), 2)
})

test('findTabInsertIndex: exact match takes priority over same-connection fallback', () => {
  const tabs: Tab[] = [
    { connectionId: 'c1', database: 'db2' },
    { connectionId: 'c1', database: 'db1' },
    { connectionId: 'c1', database: 'db1' },
  ]
  // Opening c1+db2: exact match at index 0 should win over fallback at index 2
  assert.equal(findTabInsertIndex(tabs, 'c1', 'db2'), 1)
})

test('findTabInsertIndex: handles null database', () => {
  const tabs: Tab[] = [
    { connectionId: 'c1', database: null },
    { connectionId: 'c1', database: 'db1' },
  ]
  assert.equal(findTabInsertIndex(tabs, 'c1', null), 1)
})

test('findTabInsertIndex: treats missing database as null', () => {
  const tabs: Tab[] = [
    { connectionId: 'c1' },  // no database field
  ]
  assert.equal(findTabInsertIndex(tabs, 'c1', null), 1)
})

test('findTabInsertIndex: mixed connections — inserts after own connection group', () => {
  const tabs: Tab[] = [
    { connectionId: 'c1', database: 'db1' },
    { connectionId: 'c2', database: 'db1' },
    { connectionId: 'c3', database: 'db1' },
  ]
  assert.equal(findTabInsertIndex(tabs, 'c2', 'db2'), 2)
})

// ── findNextActiveIndex ─────────────────────────────────────────────────────

test('findNextActiveIndex: closing first tab activates new first tab', () => {
  // 3 tabs remain after closing index 0
  assert.equal(findNextActiveIndex(3, 0), 0)
})

test('findNextActiveIndex: closing last tab activates new last tab', () => {
  // 2 tabs remain after closing index 2 (was last)
  assert.equal(findNextActiveIndex(2, 2), 1)
})

test('findNextActiveIndex: closing middle tab activates same index (next tab)', () => {
  // 4 tabs remain after closing index 2
  assert.equal(findNextActiveIndex(4, 2), 2)
})

test('findNextActiveIndex: closing only remaining tab (0 tabs left) returns -1', () => {
  // 0 tabs remain after closing index 0
  assert.equal(findNextActiveIndex(0, 0), -1)
})
