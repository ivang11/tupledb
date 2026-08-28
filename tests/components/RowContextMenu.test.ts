import { describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'
import RowContextMenu from '@/components/RowContextMenu.vue'

describe('RowContextMenu', () => {
  it('offers duplicating all selected rows', async () => {
    const wrapper = mount(RowContextMenu, {
      props: {
        show: true,
        x: 10,
        y: 20,
        hasPrimaryKey: true,
        selectedCount: 5,
      },
    })

    const duplicateButton = wrapper.findAll('button')[0]
    expect(duplicateButton.text()).toBe('Duplicate 5 rows')

    await duplicateButton.trigger('click')
    expect(wrapper.emitted('duplicate')).toHaveLength(1)
  })
})
