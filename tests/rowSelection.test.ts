import assert from 'node:assert/strict'
import test from 'node:test'
import {
  buildSortPayload,
  resolveKeysetColumn,
  computeRowClickSelection,
  computeNoPkRowClick,
  type RowSelectionState,
} from '../src/lib/rowSelection.js'

// ── buildSortPayload ────────────────────────────────────────────────────────

test('buildSortPayload: returns null when no sort column', () => {
  assert.equal(buildSortPayload(null, false), null)
  assert.equal(buildSortPayload(null, true), null)
})

test('buildSortPayload: returns object with column and desc flag', () => {
  assert.deepEqual(buildSortPayload('name', false), { column: 'name', desc: false })
  assert.deepEqual(buildSortPayload('created_at', true), { column: 'created_at', desc: true })
})

// ── resolveKeysetColumn ─────────────────────────────────────────────────────

const struct = [
  { field: 'id', key: 'PRI' },
  { field: 'name', key: '' },
]

test('resolveKeysetColumn: returns PK field when no sort and no filters', () => {
  assert.equal(resolveKeysetColumn(struct, null, null), 'id')
})

test('resolveKeysetColumn: returns null when sort is active', () => {
  assert.equal(resolveKeysetColumn(struct, 'name', null), null)
})

test('resolveKeysetColumn: returns null when filters are active', () => {
  assert.equal(resolveKeysetColumn(struct, null, { match_all: true, rows: [] }), null)
})

test('resolveKeysetColumn: returns null when table has no PK', () => {
  const noPk = [{ field: 'name', key: '' }, { field: 'value', key: '' }]
  assert.equal(resolveKeysetColumn(noPk, null, null), null)
})

test('resolveKeysetColumn: returns null for empty structure', () => {
  assert.equal(resolveKeysetColumn([], null, null), null)
})

// ── computeRowClickSelection ────────────────────────────────────────────────

const empty: RowSelectionState = { selectedRowPk: null, selectedRowPks: [], inlineEditColumn: null }
const noMod = { ctrl: false, meta: false, shift: false }
const allPks = ['1', '2', '3', '4', '5']

// Single click — first column (row selector)
test('single click first col: selects the row', () => {
  const result = computeRowClickSelection('2', 0, null, noMod, empty, allPks)
  assert.deepEqual(result, { selectedRowPk: '2', selectedRowPks: ['2'], inlineEditColumn: null })
})

test('single click first col: no-op when already selected', () => {
  const current: RowSelectionState = { selectedRowPk: '2', selectedRowPks: ['2'], inlineEditColumn: null }
  assert.equal(computeRowClickSelection('2', 0, null, noMod, current, allPks), null)
})

// Single click — data column
test('single click data col: selects a different row', () => {
  const current: RowSelectionState = { selectedRowPk: '1', selectedRowPks: ['1'], inlineEditColumn: null }
  const result = computeRowClickSelection('3', 1, 'name', noMod, current, allPks)
  assert.deepEqual(result, { selectedRowPk: '3', selectedRowPks: ['3'], inlineEditColumn: null })
})

test('single click data col: no-op when same row and no inline edit active', () => {
  const current: RowSelectionState = { selectedRowPk: '2', selectedRowPks: ['2'], inlineEditColumn: null }
  assert.equal(computeRowClickSelection('2', 1, 'name', noMod, current, allPks), null)
})

test('single click data col: cancels inline edit when clicking a different column on same row', () => {
  const current: RowSelectionState = { selectedRowPk: '2', selectedRowPks: ['2'], inlineEditColumn: 'email' }
  const result = computeRowClickSelection('2', 1, 'name', noMod, current, allPks)
  assert.deepEqual(result, { selectedRowPk: '2', selectedRowPks: ['2'], inlineEditColumn: null })
})

test('single click data col: no-op when clicking the same column already in inline edit', () => {
  const current: RowSelectionState = { selectedRowPk: '2', selectedRowPks: ['2'], inlineEditColumn: 'name' }
  assert.equal(computeRowClickSelection('2', 1, 'name', noMod, current, allPks), null)
})

// Ctrl+Click
test('ctrl+click: adds an unselected row to the selection', () => {
  const current: RowSelectionState = { selectedRowPk: '1', selectedRowPks: ['1'], inlineEditColumn: null }
  const result = computeRowClickSelection('3', 1, 'name', { ctrl: true, meta: false, shift: false }, current, allPks)
  assert.deepEqual(result?.selectedRowPks, ['1', '3'])
  assert.equal(result?.selectedRowPk, '3')
  assert.equal(result?.inlineEditColumn, null)
})

test('ctrl+click: removes an already-selected row from the selection', () => {
  const current: RowSelectionState = { selectedRowPk: '3', selectedRowPks: ['1', '2', '3'], inlineEditColumn: null }
  const result = computeRowClickSelection('2', 1, 'name', { ctrl: true, meta: false, shift: false }, current, allPks)
  assert.deepEqual(result?.selectedRowPks, ['1', '3'])
})

test('meta+click acts the same as ctrl+click', () => {
  const current: RowSelectionState = { selectedRowPk: '1', selectedRowPks: ['1'], inlineEditColumn: null }
  const result = computeRowClickSelection('4', 1, 'id', { ctrl: false, meta: true, shift: false }, current, allPks)
  assert.deepEqual(result?.selectedRowPks, ['1', '4'])
})

// Shift+Click
test('shift+click: selects a range from anchor to target (forward)', () => {
  const current: RowSelectionState = { selectedRowPk: '2', selectedRowPks: ['2'], inlineEditColumn: null }
  const result = computeRowClickSelection('4', 0, null, { ctrl: false, meta: false, shift: true }, current, allPks)
  assert.deepEqual(result?.selectedRowPks, ['2', '3', '4'])
  assert.equal(result?.selectedRowPk, '2')  // anchor unchanged
})

test('shift+click: selects a range from anchor to target (backward)', () => {
  const current: RowSelectionState = { selectedRowPk: '4', selectedRowPks: ['4'], inlineEditColumn: null }
  const result = computeRowClickSelection('2', 0, null, { ctrl: false, meta: false, shift: true }, current, allPks)
  assert.deepEqual(result?.selectedRowPks, ['2', '3', '4'])
})

test('shift+click: single row when anchor equals target', () => {
  const current: RowSelectionState = { selectedRowPk: '3', selectedRowPks: ['3'], inlineEditColumn: null }
  const result = computeRowClickSelection('3', 0, null, { ctrl: false, meta: false, shift: true }, current, allPks)
  assert.deepEqual(result?.selectedRowPks, ['3'])
})

test('shift+click without anchor falls through to regular single-click behavior', () => {
  // No anchor (selectedRowPk is null) → shift is ignored, acts like single click
  const result = computeRowClickSelection('3', 0, null, { ctrl: false, meta: false, shift: true }, empty, allPks)
  assert.deepEqual(result, { selectedRowPk: '3', selectedRowPks: ['3'], inlineEditColumn: null })
})

// ── computeNoPkRowClick ─────────────────────────────────────────────────────

test('computeNoPkRowClick: selects a row by key', () => {
  const result = computeNoPkRowClick('__row_index:2', null)
  assert.deepEqual(result, { selectedRowPk: '__row_index:2', selectedRowPks: [], inlineEditColumn: null })
})

test('computeNoPkRowClick: no-op when row is already selected', () => {
  assert.equal(computeNoPkRowClick('__row_index:2', '__row_index:2'), null)
})

test('computeNoPkRowClick: replaces a different selected row', () => {
  const result = computeNoPkRowClick('__row_index:5', '__row_index:2')
  assert.deepEqual(result, { selectedRowPk: '__row_index:5', selectedRowPks: [], inlineEditColumn: null })
})
