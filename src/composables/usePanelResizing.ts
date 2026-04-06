import { ref, nextTick } from 'vue'
import type { TableTab } from '@/types/workspace'

export function usePanelResizing() {
  const sidePanelWidths = ref<Record<string, number>>({})

  function startSidePanelResize(e: MouseEvent, paneId: string) {
    e.preventDefault()
    const startX = e.clientX
    const startWidth = sidePanelWidths.value[paneId] ?? 320
    const onMove = (ev: MouseEvent) => {
      sidePanelWidths.value[paneId] = Math.max(200, Math.min(700, startWidth + (startX - ev.clientX)))
    }
    const onUp = () => { window.removeEventListener('mousemove', onMove); window.removeEventListener('mouseup', onUp) }
    window.addEventListener('mousemove', onMove)
    window.addEventListener('mouseup', onUp)
  }

  const LS_KEY = 'db-viewer:column-widths'

  function tableColKey(tab: TableTab): string {
    return `${tab.connectionId}:${tab.database}:${tab.tableName}`
  }

  function loadColumnWidths(): Record<string, Record<string, number>> {
    try {
      const raw = localStorage.getItem(LS_KEY)
      return raw ? JSON.parse(raw) : {}
    } catch {
      return {}
    }
  }

  function saveColumnWidths(widths: Record<string, Record<string, number>>) {
    try {
      localStorage.setItem(LS_KEY, JSON.stringify(widths))
    } catch {}
  }

  const columnWidths = ref<Record<string, Record<string, number>>>(loadColumnWidths())

  function startColResize(e: MouseEvent, tab: TableTab | null, colName: string) {
    e.preventDefault(); e.stopPropagation()
    if (!tab) return
    const key = tableColKey(tab)
    const th = (e.currentTarget as HTMLElement).closest('th') as HTMLElement
    const startX = e.clientX
    const startWidth = th.getBoundingClientRect().width
    const onMove = (ev: MouseEvent) => {
      const newWidth = Math.max(60, startWidth + ev.clientX - startX)
      if (!columnWidths.value[key]) columnWidths.value[key] = {}
      columnWidths.value[key][colName] = newWidth
    }
    const onUp = () => {
      window.removeEventListener('mousemove', onMove)
      window.removeEventListener('mouseup', onUp)
      saveColumnWidths(columnWidths.value)
    }
    window.addEventListener('mousemove', onMove)
    window.addEventListener('mouseup', onUp)
  }

  function getColumnWidths(tab: TableTab | null | undefined): Record<string, number> {
    if (!tab) return {}
    return columnWidths.value[tableColKey(tab)] ?? {}
  }

  const structureIndexHeights = ref<Record<string, number>>({})

  function startStructureResize(e: MouseEvent, paneId: string) {
    e.preventDefault()
    const handle = e.currentTarget as HTMLElement
    const container = handle.parentElement as HTMLElement
    const startY = e.clientY
    const startHeight = structureIndexHeights.value[paneId] ?? container.offsetHeight * 0.4
    const onMove = (ev: MouseEvent) => {
      const delta = startY - ev.clientY
      structureIndexHeights.value[paneId] = Math.max(60, Math.min(container.offsetHeight - 80, startHeight + delta))
    }
    const onUp = () => { window.removeEventListener('mousemove', onMove); window.removeEventListener('mouseup', onUp) }
    window.addEventListener('mousemove', onMove)
    window.addEventListener('mouseup', onUp)
  }

  function resizeAllPanelTextareas() {
    nextTick(() => {
      document.querySelectorAll<HTMLTextAreaElement>('[data-row-detail-panel] textarea').forEach(el => {
        el.style.height = 'auto'; el.style.height = el.scrollHeight + 'px'
      })
    })
  }

  return {
    sidePanelWidths,
    startSidePanelResize,
    columnWidths,
    startColResize,
    getColumnWidths,
    structureIndexHeights,
    startStructureResize,
    resizeAllPanelTextareas
  }
}
