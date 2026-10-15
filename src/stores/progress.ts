import { defineStore } from 'pinia'
import { computed, ref } from 'vue'

export interface ImportReport {
  database: string
  fileName: string
  executed: number | null
  errors: string[]
}

export const useProgressStore = defineStore('progress', () => {
  const importReport = ref<ImportReport | null>(null)
  const importErrorsOpen = ref(false)
  const isImporting = ref(false)
  const importProgress = ref({ current: 0, total: 0, status: '' })
  const importExpanded = ref(false)
  const importConnectionId = ref<string | null>(null)
  const importDatabase = ref<string | null>(null)
  const importFileName = ref<string | null>(null)
  const importId = ref<string | null>(null)
  const isCancellingImport = ref(false)

  const isExporting = ref(false)
  const exportProgress = ref({ current: 0, total: 0, status: '' })
  const exportExpanded = ref(false)
  const exportTables = ref<string[]>([])
  const exportDoneCount = ref(0)
  const exportStartTime = ref<number | null>(null)
  const exportConnectionId = ref<string | null>(null)
  const exportDatabase = ref<string | null>(null)
  const exportId = ref<string | null>(null)
  const isCancellingExport = ref(false)

  const importPercent = computed(() => {
    const { current, total } = importProgress.value
    if (total <= 0 || current <= 0) return 0
    return Math.min(100, Math.max(0.1, Math.round(current / total * 1000) / 10))
  })
  const exportPercent = computed(() => {
    const { current, total } = exportProgress.value
    return total > 0 ? Math.min(100, Math.max(0, Math.round(current / total * 100))) : 0
  })

  return {
    importReport, importErrorsOpen,
    importDatabase, importFileName, exportDatabase, importPercent, exportPercent,
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
