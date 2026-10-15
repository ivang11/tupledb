import { afterEach, describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'
import Sidebar from '@/components/Sidebar.vue'
import { tableHandle } from '@/lib/tableReference'
import type { Connection } from '@/types/connection'

const connection: Connection = {
  id: 'pg', name: 'Local PostgreSQL', environment: 'LOCAL',
  database: { engine: 'postgresql', settings: { host: 'localhost', port: 5432, user: 'postgres', ssl_mode: 'prefer' } },
}
const tables = [
  { name: 'users', table_type: 'BASE TABLE', reference: { catalog: 'app', schema: 'public', name: 'users' } },
  { name: 'users', table_type: 'BASE TABLE', reference: { catalog: 'app', schema: 'sales', name: 'users' } },
  { name: 'summary', table_type: 'VIEW', reference: { catalog: 'app', schema: 'public', name: 'summary' } },
]
const wrappers: ReturnType<typeof mount>[] = []
afterEach(() => { wrappers.forEach(wrapper => wrapper.unmount()); wrappers.length = 0 })
function render() {
  const wrapper = mount(Sidebar, {
    attachTo: document.body,
    props: {
      width: 250, search: '', selectedConnectionId: 'pg',
      openConnections: { pg: { connection, databases: ['app', 'archive', 'analytics'], selectedDatabase: 'app', tables: { app: tables } } },
      isTableActive: () => false, isTableOpen: () => false, pendingTableAction: () => null,
      filteredTables: () => tables, isTableSelected: () => false,
    },
    global: { stubs: { ScrollArea: { template: '<div><slot /></div>' } } },
  })
  wrappers.push(wrapper)
  return wrapper
}

describe('Sidebar navigation', () => {
  it('opens database actions from the table area, headings and an empty search result', async () => {
    const wrapper = render()
    await wrapper.find('[data-table-list]').trigger('contextmenu', { clientX: 150, clientY: 300 })
    expect(wrapper.emitted('context-menu-database')?.[0]?.slice(1)).toEqual(['pg', 'app'])
    expect((wrapper.emitted('context-menu-database')![0][0] as MouseEvent).defaultPrevented).toBe(true)
    await wrapper.find('[aria-live="polite"]').trigger('contextmenu')
    expect(wrapper.emitted('context-menu-database')).toHaveLength(2)
    await wrapper.setProps({ search: 'missing', filteredTables: () => [] })
    await wrapper.find('[data-table-list] p').trigger('contextmenu')
    expect(wrapper.emitted('context-menu-database')).toHaveLength(3)
    await wrapper.setProps({ selectedConnectionId: null })
    await wrapper.find('[data-table-list]').trigger('contextmenu')
    expect(wrapper.emitted('context-menu-database')).toHaveLength(3)
  })

  it('keeps table context actions separate from the database menu', async () => {
    const wrapper = render()
    const users = wrapper.findAll('button').filter(button => button.text() === 'users')
    await users[1].trigger('contextmenu')
    expect(wrapper.emitted('context-menu-table')?.[0]?.slice(1)).toEqual(['pg', 'app', tableHandle(tables[1])])
    expect(wrapper.emitted('context-menu-database')).toBeUndefined()
  })

  it('collapses schema groups and reveals them again when a search changes', async () => {
    const wrapper = render()
    const group = wrapper.findAll('section > button').find(button => button.text().includes('public · Tables'))!
    await group.trigger('click')
    expect(group.attributes('aria-expanded')).toBe('false')
    expect(wrapper.findAll('button').filter(button => button.text() === 'users')).toHaveLength(1)
    await wrapper.setProps({ search: 'users' })
    expect(wrapper.findAll('button').filter(button => button.text() === 'users')).toHaveLength(2)
    expect(wrapper.text()).toContain('public · Views')
  })

  it('navigates identical names across schemas and opens their exact reference', async () => {
    const wrapper = render()
    const search = wrapper.find('input[aria-label="Search tables and views"]')
    const users = wrapper.findAll('button').filter(button => button.text() === 'users')
    await search.trigger('keydown', { key: 'ArrowDown' })
    expect(document.activeElement).toBe(users[0].element)
    await users[0].trigger('keydown', { key: 'ArrowDown' })
    expect(document.activeElement).toBe(users[1].element)
    await users[1].trigger('click')
    expect(wrapper.emitted('load-table')?.[0]).toEqual([tableHandle(tables[1]), 'pg', 'app'])
    await users[1].trigger('keydown', { key: 'Escape' })
    expect(document.activeElement).toBe(search.element)
    await search.trigger('keydown', { key: 'Enter' })
    expect(wrapper.emitted('load-table')?.[1]).toEqual([tableHandle(tables[0]), 'pg', 'app'])
  })

  it('selects a database from several matches using arrows and Enter', async () => {
    const wrapper = render()
    await wrapper.findAll('button').find(button => button.text() === 'app')!.trigger('click')
    const search = wrapper.find('input[aria-label="Search databases"]')
    await search.setValue('ar')
    await search.trigger('keydown', { key: 'Enter' })
    expect(wrapper.emitted('select-database')?.[0]).toEqual(['pg', 'archive'])
    await wrapper.findAll('button').find(button => button.text() === 'app')!.trigger('click')
    const reopenedSearch = wrapper.find('input[aria-label="Search databases"]')
    await reopenedSearch.trigger('keydown', { key: 'ArrowDown' })
    await reopenedSearch.trigger('keydown', { key: 'Enter' })
    expect(wrapper.emitted('select-database')?.[1]).toEqual(['pg', 'archive'])
    await wrapper.find('[aria-label="Refresh schema"]').trigger('click')
    expect(wrapper.emitted('refresh-schema')?.[0]).toEqual(['pg', 'app'])
  })
})
