import { defineStore } from 'pinia'
import { ref } from 'vue'

export const useProgressStore = defineStore('progress', () => {
  const isImporting = ref(false)
  const importProgress = ref({ current: 0, total: 0, status: '' })
  const importExpanded = ref(false)
  const importConnectionId = ref<string | null>(null)
  const importId = ref<string | null>(null)
  const isCancellingImport = ref(false)

  const isExporting = ref(false)
  const exportProgress = ref({ current: 0, total: 0, status: '' })
  const exportExpanded = ref(false)

  return {
    isImporting,
    importProgress,
    importExpanded,
    importConnectionId,
    importId,
    isCancellingImport,
    isExporting,
    exportProgress,
    exportExpanded,
  }
})
