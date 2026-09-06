export interface TableRef {
  catalog: string
  schema: string | null
  name: string
}

export interface DatabaseTable {
  reference: TableRef
  name: string
  table_type: string
}

export type ValueKind = 'text' | 'integer' | 'decimal' | 'float' | 'boolean' | 'date_time' | 'json' | 'binary' | 'other'

export interface ColumnStructure {
  field: string
  field_type: string
  nullable: boolean
  primary_key_position: number | null
  value_kind: ValueKind
  is_identity: boolean
  is_generated: boolean
  default_value: string | null
  // Native labels for display only, not engine-independent behavior.
  key: string
  extra: string
}

export interface ForeignKeyColumn {
  constraint_name: string
  position: number
  column: string
  referenced: TableRef
  referenced_column: string
}
