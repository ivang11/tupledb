import assert from 'node:assert/strict'
import test from 'node:test'
import {
  normalizeInsertValue,
  normalizeChangeValue,
  coercePkValue,
  computeCellEditValue,
  buildDuplicateInsertValues,
  buildDuplicatePendingInserts,
} from '../src/lib/tableEditing.js'

test('buildDuplicateInsertValues: creates a pending copy without the auto-increment column', () => {
  const values = buildDuplicateInsertValues(
    { id: 7, name: 'Alice', enabled: true },
    [
      { field: 'id', extra: 'auto_increment' },
      { field: 'name' },
      { field: 'enabled' },
    ],
  )

  assert.deepEqual(values, [
    { column: 'name', value: 'Alice' },
    { column: 'enabled', value: true },
  ])
})

test('buildDuplicateInsertValues: preserves empty strings, literal null strings, and NULL', () => {
  const values = buildDuplicateInsertValues(
    { empty: '', literalNull: 'null', nullable: null },
    [{ field: 'empty' }, { field: 'literalNull' }, { field: 'nullable' }],
  )

  assert.deepEqual(values, [
    { column: 'empty', value: '' },
    { column: 'literalNull', value: 'null' },
    { column: 'nullable', value: null },
  ])
})

test('buildDuplicatePendingInserts: creates one pending insert per selected row', () => {
  const rows = Array.from({ length: 5 }, (_, index) => ({
    id: index + 1,
    name: `Row ${index + 1}`,
  }))

  const inserts = buildDuplicatePendingInserts(rows, [
    { field: 'id', extra: 'auto_increment' },
    { field: 'name' },
  ])

  assert.equal(inserts.length, 5)
  assert.deepEqual(inserts[4], {
    values: [{ column: 'name', value: 'Row 5' }],
  })
})

// ── normalizeInsertValue ────────────────────────────────────────────────────

test('normalizeInsertValue: empty string and null become null', () => {
  assert.equal(normalizeInsertValue(''), null)
  assert.equal(normalizeInsertValue(null), null)
})

test('normalizeInsertValue: "null" string (any case) becomes null', () => {
  assert.equal(normalizeInsertValue('null'), null)
  assert.equal(normalizeInsertValue('NULL'), null)
  assert.equal(normalizeInsertValue('  Null  '), null)
})

test('normalizeInsertValue: boolean strings become 0/1', () => {
  assert.equal(normalizeInsertValue('true'), 1)
  assert.equal(normalizeInsertValue('TRUE'), 1)
  assert.equal(normalizeInsertValue('false'), 0)
  assert.equal(normalizeInsertValue('FALSE'), 0)
})

test('normalizeInsertValue: numeric strings are kept as strings', () => {
  assert.equal(normalizeInsertValue('42'), '42')
  assert.equal(normalizeInsertValue('3.14'), '3.14')
})

test('normalizeInsertValue: regular strings are returned as-is', () => {
  assert.equal(normalizeInsertValue('hello'), 'hello')
  assert.equal(normalizeInsertValue('  '), '  ')
})

// ── normalizeChangeValue ────────────────────────────────────────────────────

test('normalizeChangeValue: null stays null', () => {
  assert.equal(normalizeChangeValue(null), null)
})

test('normalizeChangeValue: empty string stays as empty string', () => {
  assert.equal(normalizeChangeValue(''), '')
})

test('normalizeChangeValue: "null" string becomes null', () => {
  assert.equal(normalizeChangeValue('null'), null)
  assert.equal(normalizeChangeValue('NULL'), null)
  assert.equal(normalizeChangeValue('  NULL  '), null)
})

test('normalizeChangeValue: boolean strings become 0/1', () => {
  assert.equal(normalizeChangeValue('true'), 1)
  assert.equal(normalizeChangeValue('TRUE'), 1)
  assert.equal(normalizeChangeValue('false'), 0)
  assert.equal(normalizeChangeValue('FALSE'), 0)
})

test('normalizeChangeValue: numeric strings are coerced to numbers', () => {
  assert.equal(normalizeChangeValue('42'), 42)
  assert.equal(normalizeChangeValue('3.14'), 3.14)
  assert.equal(normalizeChangeValue('-7'), -7)
})

test('normalizeChangeValue: non-numeric strings stay as strings', () => {
  assert.equal(normalizeChangeValue('hello'), 'hello')
  assert.equal(normalizeChangeValue('12abc'), '12abc')
})

test('normalizeChangeValue: already-typed values are handled', () => {
  assert.equal(normalizeChangeValue(0), 0)
  assert.equal(normalizeChangeValue(99), 99)
})

// ── coercePkValue ───────────────────────────────────────────────────────────

test('coercePkValue: numeric string becomes number', () => {
  assert.equal(coercePkValue('1'), 1)
  assert.equal(coercePkValue('42'), 42)
})

test('coercePkValue: non-numeric string stays string', () => {
  assert.equal(coercePkValue('abc'), 'abc')
  assert.equal(coercePkValue('12abc'), '12abc')
})

test('coercePkValue: uuid string stays string', () => {
  const uuid = '550e8400-e29b-41d4-a716-446655440000'
  assert.equal(coercePkValue(uuid), uuid)
})

// ── computeCellEditValue ────────────────────────────────────────────────────

const pk = 'id'

test('computeCellEditValue: returns raw string value when no pending changes', () => {
  assert.equal(computeCellEditValue({}, pk, { id: 1, name: 'Alice' }, 'name'), 'Alice')
})

test('computeCellEditValue: returns empty string for null row value', () => {
  assert.equal(computeCellEditValue({}, pk, { id: 1, name: null }, 'name'), '')
})

test('computeCellEditValue: returns empty string for undefined row value', () => {
  assert.equal(computeCellEditValue({}, pk, { id: 1 }, 'name'), '')
})

test('computeCellEditValue: serializes object values as JSON', () => {
  assert.equal(computeCellEditValue({}, pk, { id: 1, data: { x: 1 } }, 'data'), '{"x":1}')
})

test('computeCellEditValue: returns pending change when present', () => {
  const pending = { '1': { name: 'Bob' } }
  assert.equal(computeCellEditValue(pending, pk, { id: 1, name: 'Alice' }, 'name'), 'Bob')
})

test('computeCellEditValue: returns empty string when pending change is null', () => {
  const pending = { '1': { name: null } }
  assert.equal(computeCellEditValue(pending, pk, { id: 1, name: 'Alice' }, 'name'), '')
})

test('computeCellEditValue: converts pending numeric value to string', () => {
  const pending = { '5': { age: 30 } }
  assert.equal(computeCellEditValue(pending, pk, { id: 5, age: 25 }, 'age'), '30')
})

test('computeCellEditValue: falls back to row when pending is for a different column', () => {
  const pending = { '1': { email: 'x@y.com' } }
  assert.equal(computeCellEditValue(pending, pk, { id: 1, name: 'Alice' }, 'name'), 'Alice')
})

test('computeCellEditValue: works when pk is null (no-PK table)', () => {
  assert.equal(computeCellEditValue({}, null, { name: 'Alice' }, 'name'), 'Alice')
  assert.equal(computeCellEditValue({}, null, { name: null }, 'name'), '')
})
