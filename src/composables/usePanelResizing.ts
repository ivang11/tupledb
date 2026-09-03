import { ref, nextTick } from 'vue'
import type { TableTab } from '@/types/workspace'

const SIDEBAR_WIDTH_KEY = 'tupledb:sidebar-width'

function loadSidebarWidth(): number {
  try {
    const raw = localStorage.getItem(SIDEBAR_WIDTH_KEY)
    return raw ? parseInt(raw, 10) : 288
  } catch {
    return 288
  }
}

export function usePanelResizing() {
  const sidebarWidth = ref<number>(loadSidebarWidth())

  function startSidebarResize(e: MouseEvent) {
    e.preventDefault()
    const startX = e.clientX
    const startWidth = sidebarWidth.value
    const onMove = (ev: MouseEvent) => {
      sidebarWidth.value = Math.max(180, Math.min(600, startWidth + (ev.clientX - startX)))
    }
    const onUp = () => {
      window.removeEventListener('mousemove', onMove)
      window.removeEventListener('mouseup', onUp)
      try { localStorage.setItem(SIDEBAR_WIDTH_KEY, String(sidebarWidth.value)) } catch {}
    }
    window.addEventListener('mousemove', onMove)
    window.addEventListener('mouseup', onUp)
  }

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

  const LS_KEY = 'tupledb:column-widths'

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
    const header = (e.currentTarget as HTMLElement).closest('[role="columnheader"]') as HTMLElement | null
    const startX = e.clientX
    const startWidth = header?.getBoundingClientRect().width ?? columnWidths.value[key]?.[colName] ?? 180
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
    sidebarWidth,
    startSidebarResize,
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
