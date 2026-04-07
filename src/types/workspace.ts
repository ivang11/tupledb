export interface TableTab {
  type: 'table'
  id: string
  connectionId: string
  tableName: string
  database: string
  queryResult: any | null
  tableStructure: any[]
  tableIndexes: any[]
  foreignKeys: any[]
  ddl: string | null
  page: number
  pageSize: number
  viewMode: 'content' | 'structure'
  filters: any | null
  sortColumn: string | null
  sortDesc: boolean
  pendingChanges: Record<string, Record<string, any>>
  pendingDeletions: Record<string, boolean>
  pendingTruncate: boolean
  selectedRowPk: string | null
  inlineEditColumn: string | null
}

export interface QueryTab {
  type: 'query'
  id: string
  connectionId: string
  database: string | null
}

export type AnyTab = TableTab | QueryTab

export interface PaneState {
  id: string
  tabs: AnyTab[]
  activeTabId: string | null
  viewMode: 'content' | 'structure'
  page: number
  pageSize: number
  showFilters: boolean
}
