import assert from 'node:assert/strict'
import test from 'node:test'
import { rowKey, decodeRowKey, primaryKeyColumns } from '../src/lib/tableIdentity.js'
import { resolveKeysetColumn } from '../src/lib/rowSelection.js'
import { resolveTableReference, tableReferenceKey } from '../src/lib/tableReference.js'
import { normalizeChangeValue, normalizeInsertValue, buildDuplicateInsertValues } from '../src/lib/tableEditing.js'

test('composite row identities preserve every column, its type and exact value', () => {
  const columns = ['tenant', 'id']
  const row = { tenant: '01', id: '18446744073709551615' }
  const key = rowKey(row, columns)
  assert.deepEqual(decodeRowKey(key, columns), [
    { column: 'tenant', value: '01' }, { column: 'id', value: '18446744073709551615' },
  ])
  assert.notEqual(key, rowKey({ ...row, tenant: 1 }, columns))
  assert.notEqual(key, rowKey({ ...row, id: '18446744073709551614' }, columns))
  assert.equal(key, rowKey(['01', row.id], columns, columns.map(name => ({ name }))))
})

test('identities do not collide on delimiters and reject partial keys', () => {
  assert.notEqual(rowKey({ a: 'x:y', b: 'z' }, ['a', 'b']), rowKey({ a: 'x', b: 'y:z' }, ['a', 'b']))
  assert.throws(() => rowKey({ a: 1 }, ['a', 'b']))
  assert.throws(() => decodeRowKey('[{"column":"a","value":1}]', ['a', 'b']))
})

test('normalized metadata orders primary keys and disables scalar keyset for composites', () => {
  const structure = [
    { field: 'id', primary_key_position: 2 },
    { field: 'tenant', primary_key_position: 1 },
    { field: 'name', primary_key_position: null },
  ]
  assert.deepEqual(primaryKeyColumns(structure), ['tenant', 'id'])
  assert.equal(resolveKeysetColumn(structure, null, null), null)
  assert.equal(resolveKeysetColumn([structure[0]], null, null), 'id')
})

test('table identities preserve catalog, schema and punctuation without collisions', () => {
  const a = { catalog: 'app', schema: 'sales', name: 'users' }
  const b = { ...a, schema: 'support' }
  assert.notEqual(tableReferenceKey(a), tableReferenceKey(b))
  assert.notEqual(tableReferenceKey({ ...a, schema: 'a.b', name: 'c' }), tableReferenceKey({ ...a, schema: 'a', name: 'b.c' }))
  assert.deepEqual(resolveTableReference('app', b), b)
  assert.throws(() => resolveTableReference('other', b))
  const tables = [a, b].map(reference => ({ reference, name: reference.name, table_type: 'BASE TABLE' }))
  assert.throws(() => resolveTableReference('app', 'users', tables), /ambiguous/)
})

test('metadata-driven edits preserve text and exact numeric values', () => {
  for (const value of ['001', 'null', 'true', 'NOW()', '']) {
    assert.equal(normalizeChangeValue(value, 'text'), value)
  }
  for (const kind of ['integer', 'decimal']) {
    assert.equal(normalizeChangeValue('18446744073709551615', kind), '18446744073709551615')
    assert.equal(normalizeChangeValue('123456789.123456789', kind), '123456789.123456789')
  }
  assert.equal(normalizeChangeValue('false', 'boolean'), false)
  assert.equal(normalizeChangeValue(null, 'text'), null)
  assert.equal(normalizeInsertValue('NULL', 'text'), 'NULL')
})

test('duplicate inserts omit normalized identity/generated columns', () => {
  assert.deepEqual(buildDuplicateInsertValues({ id: 1, derived: 2, note: 'NULL' }, [
    { field: 'id', is_identity: true }, { field: 'derived', is_generated: true }, { field: 'note' },
  ]), [{ column: 'note', value: 'NULL' }])
})
