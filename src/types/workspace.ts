export type TableViewMode =
  | 'content'
  | 'structure'
  | 'indexes'

export interface TableTab {
  type: 'table'
  id: string
  connectionId: string
  tableName: string
  database: string
  queryResult: any | null
  exactCountLoading?: boolean
  metadataLoading?: boolean
  metadataLoaded?: boolean
  keysetPage?: number
  tableStructure: any[]
  tableIndexes: any[]
  foreignKeys: any[]
  ddl: string | null
  page: number
  pageSize: number
  viewMode: TableViewMode
  filters: any | null
  sortColumn: string | null
  sortDesc: boolean
  pendingChanges: Record<string, Record<string, any>>
  pendingDeletions: Record<string, boolean>
  pendingInserts: Array<{ values: Array<{ column: string; value: any }> }>
  pendingTruncate: boolean
  pendingDrop: boolean
  selectedRowPk: string | null
  selectedRowPks: string[]
  inlineEditColumn: string | null
}

export interface QueryTab {
  type: 'query'
  id: string
  connectionId: string
  database: string | null
  sql: string
  queryResult?: any | null
  queryError?: string | null
  executionTime?: number | null
  resultRowsLimited?: boolean
  resultTotalRows?: number | null
}

export type AnyTab = TableTab | QueryTab

export interface PaneState {
  id: string
  tabs: AnyTab[]
  activeTabId: string | null
  viewMode: TableViewMode
  page: number
  pageSize: number
  showFilters: boolean
}
