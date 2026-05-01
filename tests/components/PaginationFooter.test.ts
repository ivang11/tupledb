import { describe, it, expect } from 'vitest'
import { mount } from '@vue/test-utils'
import PaginationFooter from '@/components/PaginationFooter.vue'

function make(overrides: Partial<InstanceType<typeof PaginationFooter>['$props']> = {}) {
  return mount(PaginationFooter, {
    props: {
      viewMode: 'content',
      page: 0,
      pageSize: 50,
      rowCount: 50,
      totalCount: 200,
      totalCountApproximate: false,
      exactCountLoading: false,
      isInsertingRow: false,
      insertRowError: null,
      insertRowLoading: false,
      ...overrides,
    },
  })
}

describe('PaginationFooter — row range label', () => {
  it('shows 0 rows when rowCount is 0', () => {
    const w = make({ rowCount: 0, totalCount: 0 })
    expect(w.text()).toContain('0 rows')
  })

  it('shows exact range for a full first page', () => {
    const w = make({ page: 0, pageSize: 50, rowCount: 50, totalCount: 200 })
    expect(w.text()).toContain('1 – 50 of 200')
  })

  it('shows exact range for a partial last page', () => {
    const w = make({ page: 3, pageSize: 50, rowCount: 20, totalCount: 170 })
    expect(w.text()).toContain('151 – 170 of 170')
  })

  it('shows ~ prefix and + suffix when count is approximate and page is full', () => {
    const w = make({ page: 0, pageSize: 50, rowCount: 50, totalCount: 10000, totalCountApproximate: true })
    expect(w.text()).toMatch(/~.*\+/)
  })

  it('shows ~ prefix without + suffix on a partial approximate page', () => {
    const w = make({ page: 0, pageSize: 50, rowCount: 30, totalCount: 10000, totalCountApproximate: true })
    const text = w.text()
    expect(text).toContain('~')
    expect(text).not.toContain('+')
  })
})

describe('PaginationFooter — Exact button', () => {
  it('is hidden when count is exact', () => {
    const w = make({ totalCountApproximate: false })
    expect(w.find('button[title="Calculate exact row count"]').exists()).toBe(false)
  })

  it('is visible and enabled when count is approximate', () => {
    const w = make({ totalCountApproximate: true, exactCountLoading: false })
    const btn = w.find('button[title="Calculate exact row count"]')
    expect(btn.exists()).toBe(true)
    expect((btn.element as HTMLButtonElement).disabled).toBe(false)
  })

  it('is disabled while loading', () => {
    const w = make({ totalCountApproximate: true, exactCountLoading: true })
    const btn = w.find('button[title="Calculate exact row count"]')
    expect((btn.element as HTMLButtonElement).disabled).toBe(true)
  })

  it('emits request-exact-count when clicked', async () => {
    const w = make({ totalCountApproximate: true })
    await w.find('button[title="Calculate exact row count"]').trigger('click')
    expect(w.emitted('request-exact-count')).toHaveLength(1)
  })
})

describe('PaginationFooter — pagination controls', () => {
  it('prev button is disabled on first page', () => {
    const w = make({ page: 0 })
    // ChevronLeft is the first of the two arrow buttons
    const arrows = w.findAll('.border-l button')
    expect((arrows[0].element as HTMLButtonElement).disabled).toBe(true)
  })

  it('next button is disabled when all rows fit on one page', () => {
    const w = make({ page: 0, pageSize: 50, rowCount: 30, totalCount: 30 })
    const arrows = w.findAll('.border-l button')
    expect((arrows[1].element as HTMLButtonElement).disabled).toBe(true)
  })

  it('emits change-page -1 when prev is clicked', async () => {
    const w = make({ page: 2, rowCount: 50, totalCount: 200 })
    const arrows = w.findAll('.border-l button')
    await arrows[0].trigger('click')
    expect(w.emitted('change-page')?.[0]).toEqual([-1])
  })

  it('emits change-page +1 when next is clicked', async () => {
    const w = make({ page: 0, rowCount: 50, totalCount: 200 })
    const arrows = w.findAll('.border-l button')
    await arrows[1].trigger('click')
    expect(w.emitted('change-page')?.[0]).toEqual([1])
  })
})

describe('PaginationFooter — view mode toggle', () => {
  it('emits set-view-mode structure when Structure tab is clicked', async () => {
    const w = make()
    const buttons = w.findAll('button')
    const structBtn = buttons.find(b => b.text().includes('Structure'))!
    await structBtn.trigger('click')
    expect(w.emitted('set-view-mode')?.[0]).toEqual(['structure'])
  })

  it('hides pagination controls in structure mode', () => {
    const w = make({ viewMode: 'structure' })
    expect(w.find('input[type="number"]').exists()).toBe(false)
  })
})
