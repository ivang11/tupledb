export type Operator =
  | 'equals'
  | 'not_equals'
  | 'contains'
  | 'starts_with'
  | 'ends_with'
  | 'in'
  | 'not_in'
  | 'is_null'
  | 'is_not_null'
  | 'greater_than'
  | 'greater_or_equal'
  | 'less_than'
  | 'less_or_equal'
  | 'between'
  | 'before'
  | 'after'
  | 'true'
  | 'false'

export interface FilterRow {
  active: boolean
  column: string
  operator: Operator
  value: string
}

export interface FilterSet {
  match_all: boolean
  rows: FilterRow[]
}
