import { afterEach, describe, expect, it, vi } from 'vitest'
import { mount } from '@vue/test-utils'
import ContextMenuSurface from '@/components/ContextMenuSurface.vue'

const wrappers: ReturnType<typeof mount>[] = []
afterEach(() => { wrappers.forEach(wrapper => wrapper.unmount()); wrappers.length = 0 })

const macrotask = () => new Promise(resolve => setTimeout(resolve, 0))

describe('ContextMenuSurface dismissal timing', () => {
  // Browsers run a microtask checkpoint between each natively dispatched event
  // listener. The surface used to schedule its dismissal from a capture-phase
  // click listener with nextTick, so the menu was unmounted before the clicked
  // item's own handler ran and Vue dropped the item's emit (the "Close DB" /
  // "Disconnect" actions silently did nothing). Dismissal must therefore wait
  // for a macrotask, after the whole click dispatch has finished.
  it('does not dismiss during the microtask checkpoint that precedes the item handler', async () => {
    const wrapper = mount(ContextMenuSurface, { props: { show: true, x: 0, y: 0 }, slots: { default: '<button>Close DB</button>' } })
    wrappers.push(wrapper)
    wrapper.find('button').element.dispatchEvent(new MouseEvent('click', { bubbles: true }))
    await Promise.resolve()
    await Promise.resolve()
    expect(wrapper.emitted('dismiss')).toBeUndefined()
    await macrotask()
    expect(wrapper.emitted('dismiss')).toHaveLength(1)
  })

  it('runs the item handler before dismissing, even when the handler stops propagation', async () => {
    const order: string[] = []
    const wrapper = mount(ContextMenuSurface, {
      props: { show: true, x: 0, y: 0, onDismiss: () => order.push('dismiss') },
      slots: { default: '<button>Close DB</button>' },
    })
    wrappers.push(wrapper)
    wrapper.find('button').element.addEventListener('click', event => { event.stopPropagation(); order.push('action') })
    await wrapper.find('button').trigger('click')
    await macrotask()
    expect(order).toEqual(['action', 'dismiss'])
  })

  it('does not dismiss for disabled items and stops pending dismissals on unmount', async () => {
    const wrapper = mount(ContextMenuSurface, { props: { show: true, x: 0, y: 0 }, slots: { default: '<button disabled>Nope</button><button>Yes</button>' } })
    wrappers.push(wrapper)
    await wrapper.findAll('button')[0].trigger('click')
    await macrotask()
    expect(wrapper.emitted('dismiss')).toBeUndefined()

    const onDismiss = vi.fn()
    const second = mount(ContextMenuSurface, { props: { show: true, x: 0, y: 0, onDismiss }, slots: { default: '<button>Yes</button>' } })
    await second.find('button').trigger('click')
    second.unmount()
    await macrotask()
    expect(onDismiss).not.toHaveBeenCalled()
  })
})
