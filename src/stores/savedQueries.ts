import { ref } from 'vue'
import { defineStore } from 'pinia'
import { invoke } from '@tauri-apps/api/core'
import type { SavedQuery } from '@/types/savedQuery'

export const useSavedQueriesStore = defineStore('savedQueries', () => {
  const queries = ref<SavedQuery[]>([])

  async function fetch() {
    queries.value = await invoke<SavedQuery[]>('get_saved_queries')
  }

  async function upsert(
    data: Omit<SavedQuery, 'id' | 'created_at' | 'updated_at'> & { id?: string; created_at?: string },
  ): Promise<string> {
    const now = new Date().toISOString()
    const existing = data.id ? queries.value.find(q => q.id === data.id) : null
    const entry: SavedQuery = {
      id: data.id ?? crypto.randomUUID(),
      created_at: existing?.created_at ?? data.created_at ?? now,
      updated_at: now,
      name: data.name,
      description: data.description,
      sql: data.sql,
      connection_id: data.connection_id,
      database: data.database,
    }
    await invoke('upsert_saved_query', { query: entry })
    await fetch()
    return entry.id
  }

  async function remove(id: string) {
    await invoke('delete_saved_query', { id })
    queries.value = queries.value.filter(q => q.id !== id)
  }

  return { queries, fetch, upsert, remove }
})
