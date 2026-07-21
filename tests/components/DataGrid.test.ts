import { describe, it, expect, vi } from 'vitest'
import { mount } from '@vue/test-utils'
import { ref } from 'vue'
import DataGrid from '@/components/DataGrid.vue'

// useVirtualizer needs a scrollable container with real height to yield items.
// In happy-dom that's always 0px, so we stub it to return all rows directly.
// DataGrid calls useVirtualizer(computed(() => ({ count, ... }))), so opts
// arrives as a ComputedRef — we unwrap it before reading count.
vi.mock('@tanstack/vue-virtual', () => ({
  useVirtualizer: (opts: any) => {
    const ROW_H = 33
    const getCount = () => {
      const o = opts?.value ?? opts
      const c = o?.count
      return typeof c === 'function' ? c() : (c ?? 0)
    }
    return ref({
      getVirtualItems: () =>
        Array.from({ length: getCount() }, (_, i) => ({
          index: i,
          start: i * ROW_H,
          end: (i + 1) * ROW_H,
          size: ROW_H,
          key: i,
        })),
      getTotalSize: () => getCount() * ROW_H,
      scrollToIndex: vi.fn(),
      measureElement: vi.fn(),
    })
  },
}))

const columns = [
  { name: 'id', type_name: 'INT' },
  { name: 'name', type_name: 'VARCHAR' },
  { name: 'value', type_name: 'VARCHAR' },
]

function defaults() {
  return {
    columns,
    rows: [],
    primaryKey: 'id',
    totalCount: 0,
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
    columnWidths: {},
    fkMap: {},
    isColAutoIncrement: () => false,
    isBooleanCol: () => false,
    getCellValue: (row: any, col: string) => (row[col] == null ? '' : String(row[col])),
  }
}

describe('DataGrid — empty state', () => {
  it('shows empty-state message when rows is empty', () => {
    const w = mount(DataGrid, { props: defaults() })
    expect(w.text()).toContain('No records')
  })
})

describe('DataGrid — NULL and EMPTY indicators', () => {
  const rows = [{ id: 1, name: null, value: '' }]

  it('renders NULL label for null cell values', () => {
    const w = mount(DataGrid, {
      props: { ...defaults(), rows, totalCount: 1 },
    })
    expect(w.text()).toContain('NULL')
  })

  it('renders EMPTY label for empty-string cell values', () => {
    const w = mount(DataGrid, {
      props: {
        ...defaults(),
        rows,
        totalCount: 1,
        getCellValue: (row: any, col: string) => {
          if (row[col] === null || row[col] === undefined) return ''
          return String(row[col])
        },
      },
    })
    expect(w.text()).toContain('EMPTY')
  })
})

describe('DataGrid — row selection', () => {
  const rows = [
    { id: 1, name: 'Alice', value: 'a' },
    { id: 2, name: 'Bob', value: 'b' },
  ]

  it('applies selected-row class to the active row', () => {
    const w = mount(DataGrid, {
      props: { ...defaults(), rows, totalCount: 2, selectedRowPk: '1' },
    })
    const trs = w.findAll('tbody tr')
    expect(trs[0].classes()).toContain('bg-primary/10')
  })

  it('does not apply selected-row class to other rows', () => {
    const w = mount(DataGrid, {
      props: { ...defaults(), rows, totalCount: 2, selectedRowPk: '1' },
    })
    const trs = w.findAll('tbody tr')
    expect(trs[1].classes()).not.toContain('bg-primary/10')
  })

  it('applies multi-selected class to rows in selectedRowPks', () => {
    const w = mount(DataGrid, {
      props: { ...defaults(), rows, totalCount: 2, selectedRowPk: '1', selectedRowPks: ['1', '2'] },
    })
    const trs = w.findAll('tbody tr')
    // row 2 is multi-selected but not the primary selection
    expect(trs[1].classes()).toContain('bg-primary/25')
  })
})

describe('DataGrid — pending deletions', () => {
  const rows = [{ id: 1, name: 'Alice', value: 'a' }]

  it('applies strike-through class to rows marked for deletion', () => {
    const w = mount(DataGrid, {
      props: { ...defaults(), rows, totalCount: 1, pendingDeletions: { '1': true } },
    })
    const tr = w.find('tbody tr')
    expect(tr.classes()).toContain('bg-destructive/10')
  })
})

describe('DataGrid — no-PK table', () => {
  const rows = [{ name: 'Row1' }, { name: 'Row2' }]
  const noPkCols = [{ name: 'name', type_name: 'VARCHAR' }]

  it('renders rows without a primary key', () => {
    const w = mount(DataGrid, {
      props: {
        ...defaults(),
        columns: noPkCols,
        rows,
        primaryKey: null,
        totalCount: 2,
      },
    })
    expect(w.findAll('tbody tr')).toHaveLength(2)
  })

  it('selects a no-PK row by row-index key', () => {
    const w = mount(DataGrid, {
      props: {
        ...defaults(),
        columns: noPkCols,
        rows,
        primaryKey: null,
        totalCount: 2,
        selectedRowPk: '__row_index:0',
      },
    })
    const trs = w.findAll('tbody tr')
    expect(trs[0].classes()).toContain('bg-primary/10')
  })
})

describe('DataGrid — sort indicator', () => {
  it('emits sort when a column header is clicked', async () => {
    const w = mount(DataGrid, {
      props: { ...defaults(), rows: [{ id: 1, name: 'A', value: 'x' }], totalCount: 1 },
    })
    const headers = w.findAll('th')
    await headers[0].trigger('click')
    expect(w.emitted('sort')).toBeTruthy()
    expect(w.emitted('sort')![0]).toEqual(['id'])
  })

  it('shows active sort indicator on the sorted column', () => {
    const w = mount(DataGrid, {
      props: {
        ...defaults(),
        rows: [{ id: 1, name: 'A', value: 'x' }],
        totalCount: 1,
        sortColumn: 'name',
        sortDesc: false,
      },
    })
    const headers = w.findAll('th')
    expect(headers[1].classes()).toContain('bg-primary/12')
    expect(headers[0].classes()).not.toContain('bg-primary/12')
  })
})
