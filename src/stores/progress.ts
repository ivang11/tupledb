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
  const exportTables = ref<string[]>([])
  const exportDoneCount = ref(0)
  const exportStartTime = ref<number | null>(null)
  const exportConnectionId = ref<string | null>(null)
  const exportId = ref<string | null>(null)
  const isCancellingExport = ref(false)

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
    exportTables,
    exportDoneCount,
    exportStartTime,
    exportConnectionId,
    exportId,
    isCancellingExport,
  }
})
