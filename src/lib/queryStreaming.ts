import type { DataRow } from './rowAccess.js'

export interface ColumnInfo {
  name: string
  type_name: string
}

export interface RawQueryResult {
  columns: ColumnInfo[]
  rows: DataRow[]
  rows_affected: number
  is_select: boolean
}

export interface QueryChunkPayload {
  columns?: ColumnInfo[]
  rows: DataRow[]
}

export interface QueryChunkState {
  result: RawQueryResult | null
  rowsLimited: boolean
  streamedRowsSeen: number
}

export const QUERY_RESULT_ROW_LIMIT = 5000
export const QUERY_RESULT_CELL_LIMIT = 300_000

export function queryResultRowLimit(
  columns: ColumnInfo[] | undefined,
  rowLimit = QUERY_RESULT_ROW_LIMIT,
  cellLimit = Number.POSITIVE_INFINITY,
) {
  const columnCount = columns?.length ?? 0
  if (columnCount <= 0) return rowLimit
  return Math.max(1, Math.min(rowLimit, Math.floor(cellLimit / columnCount)))
}

export function applyQueryChunk(
  state: QueryChunkState,
  chunk: QueryChunkPayload,
  rowLimit = QUERY_RESULT_ROW_LIMIT,
  cellLimit?: number,
): QueryChunkState {
  const streamedRowsSeen = state.streamedRowsSeen + chunk.rows.length
  const currentRows = state.result?.rows.length ?? 0
  const effectiveRowLimit = queryResultRowLimit(chunk.columns ?? state.result?.columns, rowLimit, cellLimit)
  const remainingRows = Math.max(effectiveRowLimit - currentRows, 0)
  const rowsToKeep = remainingRows > 0 ? chunk.rows.slice(0, remainingRows) : []
  const rowsLimited =
    state.rowsLimited || rowsToKeep.length < chunk.rows.length || streamedRowsSeen > effectiveRowLimit

  if (!state.result) {
    return {
      result: {
        columns: chunk.columns ?? [],
        rows: rowsToKeep,
        rows_affected: rowsToKeep.length,
        is_select: true,
      },
      rowsLimited,
      streamedRowsSeen,
    }
  }

  if (rowsToKeep.length === 0) {
    return {
      result: state.result,
      rowsLimited,
      streamedRowsSeen,
    }
  }

  return {
    result: {
      ...state.result,
      rows: [...state.result.rows, ...rowsToKeep],
      rows_affected: state.result.rows.length + rowsToKeep.length,
    },
    rowsLimited,
    streamedRowsSeen,
  }
}

export function finalizeStreamedQueryResult(
  current: RawQueryResult | null,
  rowsLimited: boolean,
  meta: RawQueryResult,
): { result: RawQueryResult | null; rowsLimited: boolean; totalRows: number | null } {
  if (!current) {
    return {
      result: null,
      rowsLimited,
      totalRows: null,
    }
  }

  return {
    result: {
      ...current,
      rows_affected: meta.rows_affected,
    },
    rowsLimited: rowsLimited || current.rows.length < meta.rows_affected,
    totalRows: meta.rows_affected,
  }
}

export function limitBufferedQueryResult(
  meta: RawQueryResult,
  rowLimit = QUERY_RESULT_ROW_LIMIT,
  cellLimit?: number,
): { result: RawQueryResult; rowsLimited: boolean; totalRows: number | null } {
  const effectiveRowLimit = queryResultRowLimit(meta.columns, rowLimit, cellLimit)
  if (!meta.is_select || meta.rows.length <= effectiveRowLimit) {
    return {
      result: meta,
      rowsLimited: false,
      totalRows: null,
    }
  }

  return {
    result: {
      ...meta,
      rows: meta.rows.slice(0, effectiveRowLimit),
    },
    rowsLimited: true,
    totalRows: meta.rows_affected,
  }
}
