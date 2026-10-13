import test from 'node:test'
import assert from 'node:assert/strict'
import { explainSql } from '../src/lib/explainPlan.js'
import { pendingSqlPreview } from '../src/lib/sqlPreview.js'
import type { TableTab } from '../src/types/workspace.js'

test('EXPLAIN uses estimates for both dialects and keeps semicolons in literals', () => {
  assert.equal(explainSql("SELECT 'a;b';", 'mysql'), "EXPLAIN FORMAT=JSON SELECT 'a;b'")
  assert.equal(explainSql('WITH x AS (SELECT 1) SELECT * FROM x', 'postgresql'), 'EXPLAIN (FORMAT JSON) WITH x AS (SELECT 1) SELECT * FROM x')
  assert.equal(explainSql('SELECT $$a;b$$', 'postgresql'), 'EXPLAIN (FORMAT JSON) SELECT $$a;b$$')
  assert.throws(() => explainSql('SELECT 1; DELETE FROM users', 'mysql'), /one statement/)
  assert.throws(() => explainSql('EXPLAIN ANALYZE SELECT 1', 'postgresql'), /SELECT and WITH/)
  assert.throws(() => explainSql("SELECT 'unfinished", 'postgresql'), /Complete/)
})

test('SQL review preserves composite identities, exact values, NULLs and quoted identifiers', () => {
  const key = JSON.stringify([{ column: 'tenant', value: '007' }, { column: 'id', value: '9007199254740993' }])
  const tab = {
    type: 'table', tableName: 'user"s', database: 'app', reference: { catalog: 'app', schema: 'sales', name: 'user"s' },
    tableStructure: [
      { field: 'tenant', primary_key_position: 1, value_kind: 'text' },
      { field: 'id', primary_key_position: 2, value_kind: 'integer' },
      { field: 'note', primary_key_position: null, value_kind: 'text' },
    ],
    pendingChanges: { [key]: { note: "It's exact" } }, pendingDeletions: {},
    pendingInserts: [{ values: [{ column: 'note', value: null }] }], pendingStructureChanges: {}, pendingDrop: false, pendingTruncate: false,
  } as unknown as TableTab
  const preview = pendingSqlPreview([tab], 'postgresql')
  assert.match(preview, /UPDATE "sales"\."user""s" SET "note" = \$1 WHERE "tenant" = \$2 AND "id" = \$3;/)
  assert.match(preview, /Parameters: \["It's exact","007","9007199254740993"\]/)
  assert.match(preview, /Parameters: \[null\]/)
  tab.pendingDeletions[key] = true
  const deleted = pendingSqlPreview([tab], 'mysql')
  assert.ok(!deleted.includes('UPDATE'))
  assert.match(deleted, /DELETE FROM `app`\.`user"s` WHERE `tenant` = \? AND `id` = \?/)
})

test('EXPLAIN does not conceal a second statement in dialect-specific comments or escapes', () => {
  assert.throws(() => explainSql('SELECT 1--not_a_mysql_comment; DELETE FROM users', 'mysql'), /one statement/)
  assert.throws(() => explainSql('SELECT 1 /*!; DELETE FROM users */', 'mysql'), /executable SQL comments/)
  assert.throws(() => explainSql("SELECT 'a\\'; DELETE FROM users; --'", 'postgresql'), /one statement/)
  assert.ok(explainSql('/* outer /* inner */ outer */ SELECT 1', 'postgresql').startsWith('EXPLAIN (FORMAT JSON)'))
})
