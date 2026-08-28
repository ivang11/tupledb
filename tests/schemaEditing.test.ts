import test from 'node:test'
import assert from 'node:assert/strict'
import { stageStructureChange } from '../src/lib/schemaEditing.js'

test('stageStructureChange stores a normalized pending rename and type change', () => {
  const pending = {}
  stageStructureChange(pending, 'display_name', 'varchar(100)', ' name ', ' varchar(180) ')
  assert.deepEqual(pending, {
    display_name: { newName: 'name', newType: 'varchar(180)' },
  })
})

test('stageStructureChange removes the pending entry when values return to the original', () => {
  const pending = {
    display_name: { newName: 'name', newType: 'varchar(180)' },
  }
  stageStructureChange(pending, 'display_name', 'varchar(100)', 'display_name', 'VARCHAR(100)')
  assert.deepEqual(pending, {})
})

test('stageStructureChange keeps the untouched field while editing the other one', () => {
  const pending = {}
  stageStructureChange(pending, 'display_name', 'varchar(100)', 'display_name', 'varchar(180)')
  stageStructureChange(pending, 'display_name', 'varchar(100)', 'name', 'varchar(180)')
  assert.deepEqual(pending, {
    display_name: { newName: 'name', newType: 'varchar(180)' },
  })
})
