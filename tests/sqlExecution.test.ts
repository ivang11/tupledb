import test from 'node:test'
import assert from 'node:assert/strict'
import { executionSql, parameterNames, splitSqlStatements, substituteParameters } from '../src/lib/sqlExecution.js'
import { parseCsv, serializeCsv } from '../src/lib/csv.js'

test('SQL batches preserve literals, nested comments and dollar-quoted bodies', () => {
  assert.deepEqual(splitSqlStatements("SELECT 'a;b'; SELECT $$c;d$$; /* outer /* inner; */ outer */ SELECT 3", 'postgresql').map(item => item.sql), ["SELECT 'a;b'", 'SELECT $$c;d$$', '/* outer /* inner; */ outer */ SELECT 3'])
  assert.deepEqual(splitSqlStatements("SELECT 'a;b'; # skipped;\n SELECT 2", 'mysql').map(item => item.sql), ["SELECT 'a;b'", '# skipped;\n SELECT 2'])
  assert.throws(() => splitSqlStatements("SELECT 'unfinished", 'mysql'), /Complete/)
})

test('Run uses the selection or the statement containing the cursor', () => {
  const source = 'SELECT 1;\nSELECT 2;'
  assert.equal(executionSql(source, { from: 12, to: 12 }, 'postgresql'), 'SELECT 2')
  assert.equal(executionSql(source, { from: 0, to: 8 }, 'postgresql'), 'SELECT 1')
})

test('Named parameters skip casts, comments and literals and preserve exact values', () => {
  const sql = "SELECT :id::bigint, ':ignore', $$:ignore$$, :id, :note /* :ignore */"
  assert.deepEqual(parameterNames(sql, 'postgresql'), ['id', 'note'])
  const parameters = [{ name: 'id', type: 'number' as const, value: '9007199254740993' }, { name: 'note', type: 'text' as const, value: "x'\\y" }]
  assert.equal(substituteParameters('SELECT :id, :note', parameters, 'postgresql'), "SELECT 9007199254740993, E'x''\\\\y'")
  assert.equal(substituteParameters('SELECT :note', parameters, 'mysql'), 'SELECT CONVERT(0x78275c79 USING utf8mb4)')
  assert.throws(() => substituteParameters('SELECT :id', [{ name: 'id', type: 'number', value: '1; DELETE FROM users' }], 'mysql'), /Invalid number/)
  assert.throws(() => substituteParameters('SELECT :missing', parameters, 'mysql'), /Missing parameter/)
})

test('CSV export preserves quotes, multiline values, nulls and column ordering', () => {
  const columns = [{ name: 'note' }, { name: 'amount' }]
  const text = serializeCsv(columns, [{ amount: 1.5, note: 'a"b\nc' }, { note: null, amount: 2 }], { delimiter: ';', quotes: 'needed', lineBreak: '\r\n', decimal: ',', nullMarker: '\\N' })
  assert.deepEqual(parseCsv(text, ';'), [['note', 'amount'], ['a"b\nc', '1,5'], ['\\N', '2']])
  assert.throws(() => parseCsv('"unfinished'), /Unclosed/)
  assert.throws(() => parseCsv('"closed"suffix'), /Unexpected/)
})
