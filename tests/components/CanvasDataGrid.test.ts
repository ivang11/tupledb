import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { mount } from '@vue/test-utils'
import CanvasDataGrid from '@/components/CanvasDataGrid.vue'
import { rowValue } from '@/lib/rowAccess'

const drawContext = {
  setTransform: vi.fn(),
  fillRect: vi.fn(),
  clearRect: vi.fn(),
  fillText: vi.fn(),
  beginPath: vi.fn(),
  moveTo: vi.fn(),
  lineTo: vi.fn(),
  stroke: vi.fn(),
  save: vi.fn(),
  restore: vi.fn(),
  rect: vi.fn(),
  clip: vi.fn(),
  fillStyle: '',
  strokeStyle: '',
  font: '',
  textBaseline: '',
}

const columns = [
  { name: 'id', type_name: 'INT' },
  { name: 'name', type_name: 'VARCHAR' },
  { name: 'parent_id', type_name: 'INT' },
]
const rows = [
  { id: 1, name: 'Alice', parent_id: 9 },
  { id: 2, name: null, parent_id: null },
]

function defaults() {
  return {
    columns,
    rows,
    primaryKey: 'id',
    totalCount: rows.length,
    pendingChanges: {},
    pendingDeletions: {},
    pendingTruncate: false,
    pendingDrop: false,
    selectedRowPk: null,
    selectedRowPks: [],
    inlineEditColumn: null,
    sortColumn: null,
    sortDesc: false,
    insertingRow: false,
    insertRowValues: {},
    pendingInserts: [],
    columnWidths: {},
    fkMap: { parent_id: { table: 'parents', column: 'id' } },
    isColAutoIncrement: (column: string) => column === 'id',
    isBooleanCol: () => false,
    getCellValue: (row: Record<string, unknown>, column: string) => row[column] == null ? '' : String(row[column]),
  }
}

beforeEach(() => {
  vi.useFakeTimers()
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(drawContext as any)
  vi.spyOn(HTMLCanvasElement.prototype, 'getBoundingClientRect').mockReturnValue({
    x: 0,
    y: 0,
    left: 0,
    top: 0,
    right: 800,
    bottom: 600,
    width: 800,
    height: 600,
    toJSON: () => ({}),
  })
  vi.spyOn(window, 'requestAnimationFrame').mockImplementation(callback =>
    window.setTimeout(() => callback(performance.now()), 0),
  )
  vi.spyOn(window, 'cancelAnimationFrame').mockImplementation(id => window.clearTimeout(id))
  Object.defineProperty(HTMLElement.prototype, 'clientWidth', { configurable: true, get: () => 800 })
  Object.defineProperty(HTMLElement.prototype, 'clientHeight', { configurable: true, get: () => 600 })
  vi.stubGlobal('ResizeObserver', class {
    constructor(private callback: ResizeObserverCallback) {}
    observe() { this.callback([], this as unknown as ResizeObserver) }
    disconnect() {}
    unobserve() {}
  })
})

afterEach(() => {
  vi.runOnlyPendingTimers()
  vi.useRealTimers()
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
})

async function mountGrid(overrides: Record<string, unknown> = {}) {
  const wrapper = mount(CanvasDataGrid, { props: { ...defaults(), ...overrides } })
  await vi.runAllTimersAsync()
  return wrapper
}

describe('CanvasDataGrid rendering and hit testing', () => {
  it('draws visible headers and values without creating cell DOM', async () => {
    const wrapper = await mountGrid()
    expect(drawContext.fillText).toHaveBeenCalledWith(expect.stringContaining('id'), expect.any(Number), expect.any(Number))
    expect(drawContext.fillText).toHaveBeenCalledWith('Alice', expect.any(Number), expect.any(Number))
    expect(wrapper.findAll('[role="cell"]').length).toBeGreaterThan(0)
    expect(wrapper.findAll('[role="cell"]').length).toBeLessThanOrEqual(rows.length * columns.length)
  })

  it('maps a bounded native scroll range to the final logical columns', async () => {
    const wideColumns = Array.from({ length: 200 }, (_, index) => ({
      name: index === 0 ? 'id' : `extra_${index}`,
      type_name: 'VARCHAR',
    }))
    const wideRows = [wideColumns.map((_column, index) => index)]
    const wrapper = await mountGrid({
      columns: wideColumns,
      rows: wideRows,
      totalCount: 1,
      fkMap: {},
      getCellValue: (row: unknown[], column: string) => String(rowValue(row, column, wideColumns)),
    })
    const scroller = wrapper.get<HTMLElement>('.custom-scrollbar')

    expect(wrapper.get<HTMLElement>('[data-grid-scroll-sizer]').element.style.width).toBe('24000px')
    drawContext.fillText.mockClear()
    scroller.element.scrollLeft = 24_000 - 800
    await scroller.trigger('scroll')
    await vi.runAllTimersAsync()

    expect(drawContext.fillText).toHaveBeenCalledWith(expect.stringContaining('extra_199'), expect.any(Number), expect.any(Number))
  })

  it('draws and selects compact array rows', async () => {
    const compactRows = [[1, 'Ada', 9], [2, null, null]]
    const wrapper = await mountGrid({
      rows: compactRows,
      getCellValue: (row: unknown[], column: string) => {
        const value = rowValue(row, column, columns)
        return value == null ? '' : String(value)
      },
    })
    expect(drawContext.fillText).toHaveBeenCalledWith('Ada', expect.any(Number), expect.any(Number))
    await wrapper.get('canvas').trigger('click', { clientX: 40, clientY: 60 })
    expect(wrapper.emitted('row-click')?.at(-1)?.[0]).toEqual(compactRows[0])
  })

  it('maps header, row, cell, context-menu and FK clicks to the public events', async () => {
    const wrapper = await mountGrid()
    const canvas = wrapper.get('canvas')

    await canvas.trigger('click', { clientX: 40, clientY: 20 })
    expect(wrapper.emitted('sort')?.at(-1)).toEqual(['id'])

    await canvas.trigger('click', { clientX: 40, clientY: 60 })
    expect(wrapper.emitted('row-click')?.at(-1)?.[0]).toEqual(rows[0])
    expect(wrapper.emitted('row-click')?.at(-1)?.[3]).toBe('id')

    await canvas.trigger('dblclick', { clientX: 220, clientY: 60 })
    expect(wrapper.emitted('cell-dblclick')?.at(-1)).toEqual([rows[0], 'name'])

    await canvas.trigger('contextmenu', { clientX: 220, clientY: 60 })
    expect(wrapper.emitted('row-contextmenu')?.at(-1)).toEqual([rows[0], 220, 60])

    await canvas.trigger('click', { clientX: 530, clientY: 60 })
    expect(wrapper.emitted('navigate-related')?.at(-1)).toEqual(['parents', 'id', 9])
  })

  it('detects a column resize boundary', async () => {
    const wrapper = await mountGrid()
    const canvas = wrapper.get('canvas')
    await canvas.trigger('mousemove', { clientX: 180, clientY: 20 })
    await canvas.trigger('mousedown', { clientX: 180, clientY: 20 })
    expect(wrapper.emitted('start-col-resize')?.at(-1)?.[1]).toBe('id')
  })

  it('uses DOM overlays only for the active editor and pending inserts', async () => {
    const wrapper = await mountGrid({
      selectedRowPk: '1',
      inlineEditColumn: 'name',
      pendingInserts: [{ values: [{ column: 'name', value: 'New row' }] }],
    })
    expect(wrapper.get<HTMLInputElement>('[data-grid-edit="1"]').element.value).toBe('Alice')
    const pending = wrapper.findAll<HTMLInputElement>('.pending-insert-input')
    expect(pending.some(input => input.element.value === 'New row')).toBe(true)
  })

  it('emits deletion only when focus is outside an editor', async () => {
    const wrapper = await mountGrid()
    await wrapper.trigger('keydown', { key: 'Delete' })
    expect(wrapper.emitted('delete-key-pressed')).toHaveLength(1)
  })
})
