import { describe, expect, it, vi } from 'vitest'
import { mount } from '@vue/test-utils'
import StructureView from '@/components/StructureView.vue'

const column = {
  field: 'display_name',
  field_type: 'varchar(100)',
  nullable: false,
  key: '',
  default_value: null,
  extra: '',
}

function props(overrides: Record<string, unknown> = {}) {
  return {
    tableStructure: [column],
    tableIndexes: [],
    fkMap: {},
    ddl: null,
    paneId: 'pane-1',
    indexPanelHeight: undefined,
    ...overrides,
  }
}

function mountView(overrides: Record<string, unknown> = {}) {
  return mount(StructureView, {
    props: props(overrides),
    global: { stubs: { ScrollArea: { template: '<div><slot /></div>' } } },
  })
}

describe('StructureView — pending column editing', () => {
  it('starts editing the name by double-clicking its cell', async () => {
    const updateColumn = vi.fn()
    const wrapper = mountView({ updateColumn })

    const nameCell = wrapper.findAll('tbody td')[1]
    await nameCell.trigger('dblclick')
    const input = wrapper.get('[aria-label="Column name"]')
    expect(input.classes()).toContain('ring-(--line-2)')
    expect(input.classes()).toContain('bg-(--bg-0)')
    expect(input.classes()).toContain('p-0')
    expect(input.classes()).not.toContain('h-7')
    expect(input.classes()).not.toContain('min-w-28')
    expect(input.attributes('autocorrect')).toBe('off')
    expect(input.attributes('autocapitalize')).toBe('none')
    expect(input.attributes('spellcheck')).toBe('false')
    await input.setValue('name')
    await input.trigger('blur')

    expect(updateColumn).toHaveBeenCalledWith('display_name', 'name', 'varchar(100)')
    expect(wrapper.find('[aria-label="Column name"]').exists()).toBe(false)
  })

  it('starts editing the type by double-clicking its cell', async () => {
    const updateColumn = vi.fn()
    const wrapper = mountView({ updateColumn })

    const typeCell = wrapper.findAll('tbody td')[2]
    await typeCell.trigger('dblclick')
    const input = wrapper.get('[aria-label="Column type"]')
    await input.setValue('varchar(180)')
    await input.trigger('keydown', { key: 'Enter' })

    expect(updateColumn).toHaveBeenCalledWith('display_name', 'display_name', 'varchar(180)')
  })

  it('renders staged values in amber without an edit button', () => {
    const wrapper = mountView({
      pendingColumnChanges: {
        display_name: { newName: 'name', newType: 'varchar(180)' },
      },
    })

    const cells = wrapper.findAll('tbody td')
    expect(cells[1].text()).toBe('name')
    expect(cells[2].text()).toBe('varchar(180)')
    expect(cells[1].classes()).toContain('text-amber-500')
    expect(cells[2].classes()).toContain('text-amber-500')
    expect(wrapper.find('[aria-label^="Edit column"]').exists()).toBe(false)
  })

  it('does not enter edit mode for a read-only connection', async () => {
    const wrapper = mountView({ canEdit: false })
    await wrapper.findAll('tbody td')[1].trigger('dblclick')
    expect(wrapper.find('input').exists()).toBe(false)
  })
})
