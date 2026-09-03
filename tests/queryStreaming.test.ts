import assert from 'node:assert/strict'
import test from 'node:test'
import {
  QUERY_RESULT_CELL_LIMIT,
  QUERY_RESULT_ROW_LIMIT,
  applyQueryChunk,
  finalizeStreamedQueryResult,
  limitBufferedQueryResult,
  queryResultRowLimit,
  type QueryChunkState,
  type RawQueryResult,
} from '../src/lib/queryStreaming.js'

const columns = [{ name: 'id', type_name: 'INT' }]

function rows(start: number, count: number): Record<string, unknown>[] {
  return Array.from({ length: count }, (_, index) => ({ id: start + index }))
}

test('applyQueryChunk keeps only the configured row limit while counting all streamed rows', () => {
  let state: QueryChunkState = {
    result: null,
    rowsLimited: false,
    streamedRowsSeen: 0,
  }

  state = applyQueryChunk(state, { columns, rows: rows(1, 3000) })
  assert.equal(state.result?.rows.length, 3000)
  assert.equal(state.result?.rows_affected, 3000)
  assert.equal(state.streamedRowsSeen, 3000)
  assert.equal(state.rowsLimited, false)

  state = applyQueryChunk(state, { rows: rows(3001, 2500) })
  assert.equal(state.result?.rows.length, QUERY_RESULT_ROW_LIMIT)
  assert.equal(state.result?.rows_affected, QUERY_RESULT_ROW_LIMIT)
  assert.equal(state.streamedRowsSeen, 5500)
  assert.equal(state.rowsLimited, true)
  assert.deepEqual(state.result?.rows.at(-1), { id: QUERY_RESULT_ROW_LIMIT })

  state = applyQueryChunk(state, { rows: rows(5501, 100) })
  assert.equal(state.result?.rows.length, QUERY_RESULT_ROW_LIMIT)
  assert.equal(state.streamedRowsSeen, 5600)
  assert.equal(state.rowsLimited, true)
})

test('applyQueryChunk limits wide query results by total retained cells', () => {
  const wideColumns = Array.from({ length: 200 }, (_, index) => ({
    name: `col_${index}`,
    type_name: 'VARCHAR',
  }))
  const effectiveLimit = queryResultRowLimit(wideColumns, QUERY_RESULT_ROW_LIMIT, QUERY_RESULT_CELL_LIMIT)
  assert.equal(effectiveLimit, QUERY_RESULT_CELL_LIMIT / wideColumns.length)

  const state = applyQueryChunk({
    result: null,
    rowsLimited: false,
    streamedRowsSeen: 0,
  }, { columns: wideColumns, rows: rows(1, QUERY_RESULT_ROW_LIMIT) }, QUERY_RESULT_ROW_LIMIT, QUERY_RESULT_CELL_LIMIT)

  assert.equal(state.result?.rows.length, effectiveLimit)
  assert.equal(state.result?.rows_affected, effectiveLimit)
  assert.equal(state.streamedRowsSeen, QUERY_RESULT_ROW_LIMIT)
  assert.equal(state.rowsLimited, true)
})

test('finalizeStreamedQueryResult promotes backend total without storing extra rows', () => {
  const current: RawQueryResult = {
    columns,
    rows: rows(1, QUERY_RESULT_ROW_LIMIT),
    rows_affected: QUERY_RESULT_ROW_LIMIT,
    is_select: true,
  }
  const meta: RawQueryResult = {
    columns,
    rows: [],
    rows_affected: 12_345,
    is_select: true,
  }

  const finalized = finalizeStreamedQueryResult(current, false, meta)

  assert.equal(finalized.result?.rows.length, QUERY_RESULT_ROW_LIMIT)
  assert.equal(finalized.result?.rows_affected, 12_345)
  assert.equal(finalized.totalRows, 12_345)
  assert.equal(finalized.rowsLimited, true)
})

test('limitBufferedQueryResult truncates legacy buffered selects but leaves DML alone', () => {
  const select: RawQueryResult = {
    columns,
    rows: rows(1, QUERY_RESULT_ROW_LIMIT + 1),
    rows_affected: QUERY_RESULT_ROW_LIMIT + 1,
    is_select: true,
  }
  const limited = limitBufferedQueryResult(select, QUERY_RESULT_ROW_LIMIT, QUERY_RESULT_CELL_LIMIT)

  assert.equal(limited.result.rows.length, QUERY_RESULT_ROW_LIMIT)
  assert.equal(limited.totalRows, QUERY_RESULT_ROW_LIMIT + 1)
  assert.equal(limited.rowsLimited, true)

  const dml: RawQueryResult = {
    columns: [],
    rows: [],
    rows_affected: 42,
    is_select: false,
  }
  const unchanged = limitBufferedQueryResult(dml)

  assert.equal(unchanged.result, dml)
  assert.equal(unchanged.totalRows, null)
  assert.equal(unchanged.rowsLimited, false)
})

test('limitBufferedQueryResult limits wide buffered selects by retained cells', () => {
  const wideColumns = Array.from({ length: 200 }, (_, index) => ({
    name: `col_${index}`,
    type_name: 'VARCHAR',
  }))
  const effectiveLimit = queryResultRowLimit(wideColumns, QUERY_RESULT_ROW_LIMIT, QUERY_RESULT_CELL_LIMIT)
  const select: RawQueryResult = {
    columns: wideColumns,
    rows: rows(1, QUERY_RESULT_ROW_LIMIT),
    rows_affected: QUERY_RESULT_ROW_LIMIT,
    is_select: true,
  }

  const limited = limitBufferedQueryResult(select, QUERY_RESULT_ROW_LIMIT, QUERY_RESULT_CELL_LIMIT)

  assert.equal(limited.result.rows.length, effectiveLimit)
  assert.equal(limited.totalRows, QUERY_RESULT_ROW_LIMIT)
  assert.equal(limited.rowsLimited, true)
})
