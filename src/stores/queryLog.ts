import { defineStore } from 'pinia'
import { ref } from 'vue'
import { listen } from '@tauri-apps/api/event'

export interface QueryLogEntry {
  id: number
  sql: string
  timestamp: string
  duration_ms: number
  error?: string | null
}

const MAX_ENTRIES = 500

export const useQueryLogStore = defineStore('queryLog', () => {
  const entries = ref<QueryLogEntry[]>([])
  const isOpen = ref(false)
  let nextId = 1

  function addEntry(entry: Omit<QueryLogEntry, 'id'>) {
    entries.value.push({ id: nextId++, ...entry })
    if (entries.value.length > MAX_ENTRIES) {
      entries.value.splice(0, entries.value.length - MAX_ENTRIES)
    }
  }

  function clear() {
    entries.value = []
  }

  function toggle() {
    isOpen.value = !isOpen.value
  }

  // Start listening to Tauri events
  listen<Omit<QueryLogEntry, 'id'>>('query-log', (event) => {
    addEntry(event.payload)
  })

  return { entries, isOpen, addEntry, clear, toggle }
})
