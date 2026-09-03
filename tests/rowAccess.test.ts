import assert from 'node:assert/strict'
import { describe, it } from 'node:test'
import { columnIndex, rowRecord, rowValue } from '../src/lib/rowAccess.js'

const columns = [
  { name: 'id' },
  { name: 'name' },
  { name: 'nullable' },
]

describe('compact row access', () => {
  it('reads object and array rows through the same API', () => {
    assert.equal(rowValue({ id: 7, name: 'Ada' }, 'name', columns), 'Ada')
    assert.equal(rowValue([7, 'Ada', null], 'name', columns), 'Ada')
    assert.equal(rowValue([7, 'Ada', null], 'missing', columns), undefined)
  })

  it('supports table structure field names and materializes export-style objects', () => {
    const structure = [{ field: 'id' }, { field: 'name' }]
    assert.equal(columnIndex(structure, 'name'), 1)
    assert.deepEqual(rowRecord([7, 'Ada'], structure), { id: 7, name: 'Ada' })
  })
})
