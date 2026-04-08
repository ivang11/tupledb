export interface SavedQuery {
  id: string
  name: string
  description?: string
  sql: string
  connection_id?: string
  database?: string
  created_at: string
  updated_at: string
}
