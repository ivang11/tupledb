import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import { createPinia, setActivePinia } from 'pinia'
import { defineComponent, h, ref } from 'vue'
import FilterBar from '@/components/FilterBar.vue'
import { useWorkspace } from '@/composables/useWorkspace'
import { useTableTabs } from '@/composables/useTableTabs'
import { useWorkspaceTableUi } from '@/composables/useWorkspaceTableUi'
import { useConnectionStore } from '@/stores/connections'
import type { FilterSet } from '@/types/filters'
const wrappers: ReturnType<typeof mount>[] = []
beforeEach(() => setActivePinia(createPinia()))
afterEach(() => { wrappers.forEach(wrapper => wrapper.unmount()); wrappers.length = 0 })

describe('individual filter execution', () => {
  it('runs either row alone without losing the configured rows across tab remounts, then applies all or clears', async () => {
    const store = useConnectionStore()
    const fetch = vi.spyOn(store, 'fetchTableData').mockResolvedValue({ columns: [], rows: [], total_count: 0 })
    vi.spyOn(store, 'fetchTableStructure').mockResolvedValue([])
    vi.spyOn(store, 'fetchForeignKeys').mockResolvedValue([])
    const workspace = useWorkspace(ref(null))
    const tabs = useTableTabs(workspace)
    await tabs.loadTableData({ catalog: 'app', schema: null, name: 'users' }, 'local', 'app')
    const pane = workspace.getPane(), tab = workspace.getPaneTab(pane)!
    const filters: FilterSet = { match_all: false, rows: [
      { active: true, column: 'id', operator: 'greater_than', value: '10' },
      { active: true, column: 'name', operator: 'contains', value: 'Alice' },
    ] }
    tab.filters = structuredClone(filters)
    const ui = useWorkspaceTableUi({ ...workspace, ...tabs, selectedSidebarConnectionId: ref('local'), insertingRowTabId: ref(null) })
    const show = ref(true)
    const wrapper = mount(defineComponent({ setup: () => () => show.value ? h(FilterBar, {
      visible: true, columns: [{ name: 'id', type_name: 'INT' }, { name: 'name', type_name: 'TEXT' }],
      initialFilter: tab.configuredFilters ?? tab.filters,
      onApply: (effective: FilterSet, configured?: FilterSet) => { void ui.applyFilters(pane, effective, configured) },
      onClear: () => { void ui.clearFilters(pane) },
    }) : null }))
    wrappers.push(wrapper)
    await wrapper.findAll('[title="Apply this filter"]')[0].trigger('click')
    await flushPromises()
    expect(fetch.mock.lastCall?.[5]).toEqual({ match_all: false, rows: [filters.rows[0]] })
    expect(tab.configuredFilters).toEqual(filters)
    expect(wrapper.findAll('input[placeholder="Value"]')).toHaveLength(2)
    expect(wrapper.findAll('input[type="checkbox"]').every(input => (input.element as HTMLInputElement).checked)).toBe(true)
    show.value = false; await flushPromises(); show.value = true; await flushPromises()
    expect(wrapper.findAll('input[placeholder="Value"]').map(input => (input.element as HTMLInputElement).value)).toEqual(['10', 'Alice'])
    await wrapper.findAll('[title="Apply this filter"]')[1].trigger('click')
    await flushPromises()
    expect(fetch.mock.lastCall?.[5]).toEqual({ match_all: false, rows: [filters.rows[1]] })
    expect(tab.configuredFilters).toEqual(filters)
    await wrapper.findAll('button').find(button => button.text() === 'Apply')!.trigger('click')
    await flushPromises()
    expect(fetch.mock.lastCall?.[5]).toEqual(filters)
    expect(tab.filters).toEqual(filters)
    await wrapper.findAll('button').find(button => button.text() === 'Clear')!.trigger('click')
    await flushPromises()
    expect(tab.filters).toBeNull(); expect(tab.configuredFilters).toBeNull()
    expect(wrapper.findAll('input[placeholder="Value"]')).toHaveLength(0)
  })
})
