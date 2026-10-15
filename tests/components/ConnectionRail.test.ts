import { afterEach, describe, it, expect } from 'vitest'
import { mount } from '@vue/test-utils'
import ConnectionRail from '@/components/ConnectionRail.vue'
import ConnectionContextMenu from '@/components/ConnectionContextMenu.vue'
import type { Connection } from '@/types/connection'

const connection: Connection = { id: 'local', name: 'Local', environment: 'LOCAL', database: { engine: 'mysql', settings: { host: 'localhost', port: 3306, user: 'root' } } }
const wrappers: ReturnType<typeof mount>[] = []
afterEach(() => { wrappers.forEach(wrapper => wrapper.unmount()); wrappers.length = 0 })
describe('connection rail context menu', () => {
  it('uses the connection menu for database icons as well as connections without a database', async () => {
    const other = { ...connection, id: 'other', name: 'Other' }
    const wrapper = mount(ConnectionRail, { props: { selectedConnectionId: 'local', openConnections: {
      local: { connection, databases: ['app', 'archive'], selectedDatabase: 'app', openedDatabases: ['app', 'archive'], tables: {} },
      other: { connection: other, databases: [], selectedDatabase: null, tables: {} },
    } } })
    wrappers.push(wrapper)
    for (const label of ['app', 'archive', 'Other']) await wrapper.findAll('button').find(button => button.text() === label)!.trigger('contextmenu')
    expect(wrapper.emitted('context-menu-connection')?.map(args => args[1])).toEqual([connection, connection, other])
    expect(wrapper.emitted('context-menu-database')).toBeUndefined()
  })

  it('offers only closing this connection or the other connections', async () => {
    const wrapper = mount(ConnectionContextMenu, { attachTo: document.body, props: { show: true, x: 10, y: 10, connection, isConnected: true, variant: 'rail', openConnectionCount: 2 } })
    wrappers.push(wrapper)
    const buttons = wrapper.findAll('button')
    expect(buttons.map(button => button.text())).toEqual(['Close connection', 'Close other connections'])
    await buttons[0].trigger('click')
    expect(wrapper.emitted('disconnect')?.[0]).toEqual(['local'])
    await buttons[1].trigger('click')
    expect(wrapper.emitted('disconnect-others')?.[0]).toEqual(['local'])
    await wrapper.setProps({ openConnectionCount: 1 })
    expect(buttons[1].attributes('disabled')).toBeDefined()
  })
})
