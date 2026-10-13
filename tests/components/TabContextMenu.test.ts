import { afterEach, describe, expect, it, vi } from 'vitest'
import { createPinia } from 'pinia'
import { mount, flushPromises } from '@vue/test-utils'
import TabBar from '@/components/TabBar.vue'
import ContextMenuSurface from '@/components/ContextMenuSurface.vue'
import { rowTsv } from '@/lib/clipboard'

const wrappers: ReturnType<typeof mount>[] = []
afterEach(() => { wrappers.forEach(wrapper => wrapper.unmount()); wrappers.length = 0 })
const tabs = ['first', 'second', 'third'].map(id => ({ id, type: 'table' as const, connectionId: 'pg', database: 'app', tableName: id }))
async function render() {
  const wrapper = mount(TabBar, { attachTo: document.body, props: {
    paneId: 'pane', tabs, activeTabId: 'first', connectionNames: { pg: 'Local' }, connectionEnvironments: { pg: 'LOCAL' },
    hasOpenConnections: true, firstConnectionId: 'pg', canOpenQuery: true, showFilters: false, hasActiveTableTab: true,
    isLastPane: true, showClosePaneButton: false, isFocused: false, showFocusButton: false, canSplitPane: true,
  }, global: { plugins: [createPinia()] } })
  wrappers.push(wrapper)
  await wrapper.findAll('button[draggable]')[1].trigger('contextmenu', { clientX: 100, clientY: 100 })
  await flushPromises()
  return wrapper
}
describe('tab context menus', () => {
  it('refreshes the clicked tab rather than the previously active tab', async () => {
    const wrapper = await render()
    await wrapper.find('[role="menu"]').findAll('button').find(button => button.text() === 'Refresh table')!.trigger('click')
    expect(wrapper.emitted('switch-tab')?.[0]).toEqual(['second'])
    expect(wrapper.emitted('refresh')).toHaveLength(1)
    expect(wrapper.find('[role="menu"]').exists()).toBe(false)
  })
  it('closes only tabs to the right of the clicked tab', async () => {
    const wrapper = await render()
    await wrapper.find('[role="menu"]').findAll('button').find(button => button.text() === 'Close tabs to the right')!.trigger('click')
    expect(wrapper.emitted('close-tab')?.map(args => args[0])).toEqual(['third'])
  })
  it('closes other tabs while keeping the clicked tab', async () => {
    const wrapper = await render()
    await wrapper.find('[role="menu"]').findAll('button').find(button => button.text() === 'Close other tabs')!.trigger('click')
    expect(wrapper.emitted('close-tab')?.map(args => args[0])).toEqual(['first', 'third'])
  })
})

describe('context menu surface', () => {
  it('clamps to the viewport, skips disabled actions and closes with Escape', async () => {
    const wrapper = mount(ContextMenuSurface, { attachTo: document.body,
      props: { show: true, x: 10000, y: 10000 }, slots: { default: '<button>First</button><button disabled>Disabled</button><button>Last</button>' },
    })
    wrappers.push(wrapper)
    await flushPromises()
    const buttons = wrapper.findAll('button')
    expect(document.activeElement).toBe(buttons[0].element)
    expect(parseInt((wrapper.element as HTMLElement).style.left)).toBeLessThanOrEqual(window.innerWidth)
    await buttons[0].trigger('keydown', { key: 'ArrowDown' })
    expect(document.activeElement).toBe(buttons[2].element)
    await buttons[2].trigger('keydown', { key: 'Escape' })
    expect(wrapper.emitted('dismiss')).toHaveLength(1)
  })
  it('dismisses after action handlers even when they stop propagation', async () => {
    const action = vi.fn((event: MouseEvent) => event.stopPropagation())
    const wrapper = mount(ContextMenuSurface, { props: { show: true, x: 0, y: 0 }, slots: { default: '<button>Close</button>' } })
    wrappers.push(wrapper)
    wrapper.find('button').element.addEventListener('click', action)
    await wrapper.find('button').trigger('click')
    expect(action).toHaveBeenCalledOnce()
    expect(wrapper.emitted('dismiss')).toHaveLength(1)
  })
})

it('preserves tabs, newlines, quotes, NULLs and JSON values when copying TSV', () => {
  expect(rowTsv({ id: 7, empty: null, text: 'a\tb\n"c"', data: { ok: true } })).toBe('7\t\t"a\tb\n""c"""\t"{""ok"":true}"')
})
