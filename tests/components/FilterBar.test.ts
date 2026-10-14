import { describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'
import FilterBar from '@/components/FilterBar.vue'

const columns = [{ name: 'id', type_name: 'INT' }]
describe('filter section', () => {
  it('adds a blank draft on opening and preserves it when reopened without applying', async () => {
    const wrapper = mount(FilterBar, { props: { visible: false, columns } })
    expect(wrapper.find('input[placeholder="Value"]').exists()).toBe(false)
    await wrapper.setProps({ visible: true })
    expect(wrapper.findAll('input[placeholder="Value"]')).toHaveLength(1)
    await wrapper.find('input[placeholder="Value"]').setValue('42')
    await wrapper.setProps({ visible: false })
    await wrapper.setProps({ visible: true })
    expect(wrapper.findAll('input[placeholder="Value"]')).toHaveLength(1)
    expect((wrapper.find('input[placeholder="Value"]').element as HTMLInputElement).value).toBe('42')
    expect(wrapper.emitted('apply')).toBeUndefined()
    await wrapper.find('input[placeholder="Value"]').trigger('keyup', { key: 'Enter' })
    expect(wrapper.emitted('apply')?.[0]).toEqual([{ match_all: true, rows: [{ active: true, column: 'id', operator: 'equals', value: '42' }] }])
    wrapper.unmount()
  })

  it('waits for columns to load and keeps existing relation filters', async () => {
    const wrapper = mount(FilterBar, { props: { visible: true, columns: [] as typeof columns } })
    expect(wrapper.find('input[placeholder="Value"]').exists()).toBe(false)
    await wrapper.setProps({ columns })
    expect(wrapper.findAll('input[placeholder="Value"]')).toHaveLength(1)
    await wrapper.setProps({ initialFilter: { match_all: true, rows: [{ active: true, column: 'id', operator: 'equals', value: '7' }] } })
    await wrapper.setProps({ visible: false })
    await wrapper.setProps({ visible: true })
    expect(wrapper.findAll('input[placeholder="Value"]')).toHaveLength(1)
    expect((wrapper.find('input[placeholder="Value"]').element as HTMLInputElement).value).toBe('7')
    wrapper.unmount()
  })
})
