export interface ColumnInfo {
  name: string
  type_name: string
}

export interface RawQueryResult {
  columns: ColumnInfo[]
  rows: Record<string, unknown>[]
  rows_affected: number
  is_select: boolean
}

export interface QueryChunkPayload {
  columns?: ColumnInfo[]
  rows: Record<string, unknown>[]
}

export interface QueryChunkState {
  result: RawQueryResult | null
  rowsLimited: boolean
  streamedRowsSeen: number
}

export const QUERY_RESULT_ROW_LIMIT = 5000

export function applyQueryChunk(
  state: QueryChunkState,
  chunk: QueryChunkPayload,
  rowLimit = QUERY_RESULT_ROW_LIMIT,
): QueryChunkState {
  const streamedRowsSeen = state.streamedRowsSeen + chunk.rows.length
  const currentRows = state.result?.rows.length ?? 0
  const remainingRows = Math.max(rowLimit - currentRows, 0)
  const rowsToKeep = remainingRows > 0 ? chunk.rows.slice(0, remainingRows) : []
  const rowsLimited =
    state.rowsLimited || rowsToKeep.length < chunk.rows.length || streamedRowsSeen > rowLimit

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
): { result: RawQueryResult; rowsLimited: boolean; totalRows: number | null } {
  if (!meta.is_select || meta.rows.length <= rowLimit) {
    return {
      result: meta,
      rowsLimited: false,
      totalRows: null,
    }
  }

  return {
    result: {
      ...meta,
      rows: meta.rows.slice(0, rowLimit),
    },
    rowsLimited: true,
    totalRows: meta.rows_affected,
  }
}
