import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'
import ConnectionStartScreen from '@/components/ConnectionStartScreen.vue'
import type { Connection } from '@/types/connection'

function connection(id: string, name: string, environment: Connection['environment'] = 'LOCAL'): Connection {
  return { id, name, environment, database: { engine: 'mysql', settings: { host: '127.0.0.1', port: 3306, user: 'root', password: '' } } }
}

const connections = [connection('a', 'Alpha', 'DEV'), connection('b', 'Beta'), connection('c', 'Gamma', 'PRODUCTION')]

// happy-dom in this setup does not expose localStorage; the screen must still work without it,
// but these tests need it to assert persistence.
const memory = new Map<string, string>()
const storage = {
  getItem: (key: string) => memory.get(key) ?? null,
  setItem: (key: string, value: string) => { memory.set(key, String(value)) },
  removeItem: (key: string) => { memory.delete(key) },
  clear: () => memory.clear(),
}

const wrappers: ReturnType<typeof mount>[] = []
beforeEach(() => {
  memory.clear()
  Object.defineProperty(globalThis, 'localStorage', { value: storage, configurable: true })
})
afterEach(() => { wrappers.forEach(wrapper => wrapper.unmount()); wrappers.length = 0 })

function render(overrides: Record<string, unknown> = {}) {
  const wrapper = mount(ConnectionStartScreen, {
    attachTo: document.body,
    props: { connections, openConnectionIds: [], selectedConnectionId: null, connectingId: null, ...overrides },
  })
  wrappers.push(wrapper)
  return wrapper
}

function orderOf(wrapper: ReturnType<typeof mount>) {
  return wrapper.findAll('[data-connection-id]').map(button => button.attributes('data-connection-id'))
}

// Pointer-based reorder: press a row, move it by whole row heights (56px fallback in
// happy-dom, where rows have no layout), release. The slot the row lands on is
// derived from the vertical distance, so the pointer never needs to be over a row.
const ROW = 56
async function drag(wrapper: ReturnType<typeof mount>, from: string, to: string) {
  const fromIndex = orderOf(wrapper).indexOf(from)
  const toIndex = orderOf(wrapper).indexOf(to)
  await wrapper.get(`[data-connection-id="${from}"]`).trigger('pointerdown', { button: 0, pointerId: 1, clientY: 100 })
  window.dispatchEvent(new PointerEvent('pointermove', { pointerId: 1, clientY: 100 + (toIndex - fromIndex) * ROW }))
  await wrapper.vm.$nextTick()
  window.dispatchEvent(new PointerEvent('pointerup', { pointerId: 1 }))
  await wrapper.vm.$nextTick()
}

describe('ConnectionStartScreen', () => {
  it('shows the engine and environment for each connection and connects on double click', async () => {
    const wrapper = render()
    expect(wrapper.text()).toContain('MySQL · root@127.0.0.1:3306')
    expect(wrapper.text()).toContain('Development')
    expect(wrapper.text()).toContain('Production')
    await wrapper.get('[data-connection-id="b"]').trigger('dblclick')
    expect(wrapper.emitted('connect-saved')?.[0]?.[0]).toEqual(connections[1])
  })

  it('reorders connections by pointer drag in custom order and persists it', async () => {
    const wrapper = render()
    expect(orderOf(wrapper)).toEqual(['a', 'b', 'c'])

    await drag(wrapper, 'a', 'c')
    expect(orderOf(wrapper)).toEqual(['b', 'c', 'a'])

    await drag(wrapper, 'a', 'b')
    expect(orderOf(wrapper)).toEqual(['a', 'b', 'c'])
    expect(JSON.parse(memory.get('tupledb:connection-order') ?? '[]')).toEqual(['a', 'b', 'c'])

    const again = render()
    await drag(again, 'c', 'a')
    const fresh = render()
    expect(orderOf(fresh)).toEqual(['c', 'a', 'b'])
  })

  it('lifts the dragged row and shifts the rows it passes while dragging', async () => {
    const wrapper = render()
    await wrapper.get('[data-connection-id="a"]').trigger('pointerdown', { button: 0, pointerId: 1, clientY: 100 })
    window.dispatchEvent(new PointerEvent('pointermove', { pointerId: 1, clientY: 100 + ROW * 2 }))
    await wrapper.vm.$nextTick()
    const style = (id: string) => (wrapper.get(`[data-connection-id="${id}"]`).element as HTMLElement).style.transform
    expect(wrapper.get('[data-connection-id="a"]').classes()).toContain('drag-lifted')
    expect(style('a')).toContain(`translateY(${ROW * 2}px)`)
    expect(style('b')).toBe(`translateY(-${ROW}px)`)
    expect(style('c')).toBe(`translateY(-${ROW}px)`)
    window.dispatchEvent(new PointerEvent('pointerup', { pointerId: 1 }))
    await wrapper.vm.$nextTick()
    expect(style('b')).toBe('')
    expect(orderOf(wrapper)).toEqual(['b', 'c', 'a'])
  })

  it('ignores presses that do not move past the drag threshold', async () => {
    const wrapper = render()
    await wrapper.get('[data-connection-id="a"]').trigger('pointerdown', { button: 0, pointerId: 1, clientY: 100 })
    window.dispatchEvent(new PointerEvent('pointermove', { pointerId: 1, clientY: 102 }))
    await wrapper.vm.$nextTick()
    expect(wrapper.find('.drag-lifted').exists()).toBe(false)
    window.dispatchEvent(new PointerEvent('pointerup', { pointerId: 1 }))
    await wrapper.vm.$nextTick()
    expect(orderOf(wrapper)).toEqual(['a', 'b', 'c'])
  })

  it('nudges a connection with Alt+Arrow and ignores reordering while sorted by name', async () => {
    const wrapper = render()
    await wrapper.get('[data-connection-id="a"]').trigger('keydown', { key: 'ArrowDown', altKey: true })
    expect(orderOf(wrapper)).toEqual(['b', 'a', 'c'])
    await wrapper.get('[data-connection-id="a"]').trigger('keydown', { key: 'ArrowUp', altKey: true })
    expect(orderOf(wrapper)).toEqual(['a', 'b', 'c'])

    await wrapper.get('[aria-label="Sort connections"]').trigger('click')
    await wrapper.findAll('.sort-item').find(item => item.text() === 'Name')!.trigger('click')
    await drag(wrapper, 'c', 'a')
    expect(orderOf(wrapper)).toEqual(['a', 'b', 'c'])
  })

  it('filters by environment with the chips', async () => {
    const wrapper = render()
    await wrapper.findAll('button').find(button => button.text() === 'Production')!.trigger('click')
    expect(orderOf(wrapper)).toEqual(['c'])
    expect(wrapper.text()).not.toContain('Drag to reorder')
    await wrapper.findAll('button').find(button => button.text() === 'All')!.trigger('click')
    expect(orderOf(wrapper)).toEqual(['a', 'b', 'c'])
  })

  it('covers the whole workspace as an overlay without a competing relative position', () => {
    const wrapper = render({ overlay: true })
    expect(wrapper.classes()).toContain('absolute')
    expect(wrapper.classes()).not.toContain('relative')
    const inline = render()
    expect(inline.classes()).toContain('relative')
    expect(inline.classes()).not.toContain('absolute')
  })
})
