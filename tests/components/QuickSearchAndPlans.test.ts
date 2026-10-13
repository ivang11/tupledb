import { describe, it, expect, vi } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import { createPinia } from 'pinia'
import QuickSearchDialog from '@/components/dialogs/QuickSearchDialog.vue'
import ExplainPlanView from '@/components/ExplainPlanView.vue'
import QueryEditor from '@/components/QueryEditor.vue'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(async () => []) }))
const dialogStubs = Object.fromEntries(['Dialog', 'DialogContent', 'DialogHeader', 'DialogTitle', 'DialogDescription'].map(name => [name, { template: '<div><slot /></div>' }]))
describe('Quick search', () => {
  it('filters by schema and database, and opens the highlighted result with the keyboard', async () => {
    const wrapper = mount(QuickSearchDialog, { props: { open: true, busy: false, items: [
      { id: '1', kind: 'Table', label: 'public.users', detail: 'Local · app', connectionId: 'pg', database: 'app', table: 'public.users' },
      { id: '2', kind: 'Table', label: 'sales.users', detail: 'Local · app', connectionId: 'pg', database: 'app', table: 'sales.users' },
      { id: '3', kind: 'Table', label: 'sales.users', detail: 'Local · other', connectionId: 'pg', database: 'other', table: 'sales.users' },
    ] }, global: { stubs: dialogStubs } })
    const input = wrapper.find('input')
    await input.setValue('users app')
    expect(wrapper.findAll('[role="option"]')).toHaveLength(2)
    await input.trigger('keydown', { key: 'ArrowDown' }); await input.trigger('keydown', { key: 'Enter' })
    expect(wrapper.emitted('select')?.[0][0]).toMatchObject({ id: '2', database: 'app' })
    wrapper.unmount()
  })
})
describe('Execution plans', () => {
  it('renders nested PostgreSQL nodes and planner estimates from compact JSON rows', () => {
    const plan = [{ Plan: { 'Node Type': 'Hash Join', 'Total Cost': 42, Plans: [{ 'Node Type': 'Seq Scan', 'Relation Name': 'users', 'Plan Rows': 100 }] } }]
    const wrapper = mount(ExplainPlanView, { props: { result: { columns: [{ name: 'QUERY PLAN' }], rows: [[JSON.stringify(plan)]] } } })
    expect(wrapper.text()).toContain('Hash Join'); expect(wrapper.text()).toContain('Seq Scan'); expect(wrapper.text()).toContain('Plan Rows: 100')
    expect(wrapper.text()).toContain('the query was not executed'); wrapper.unmount()
  })
  it('renders MySQL access methods and chosen indexes', () => {
    const wrapper = mount(ExplainPlanView, { props: { result: { columns: [{ name: 'EXPLAIN' }], rows: [{ EXPLAIN: JSON.stringify({ query_block: { table: { table_name: 'users', access_type: 'ref', key: 'email_idx' } } }) }] } } })
    expect(wrapper.text()).toContain('users'); expect(wrapper.text()).toContain('access_type: ref'); expect(wrapper.text()).toContain('key: email_idx'); wrapper.unmount()
  })
  it('keeps Run and Explain available together in the real SQL editor', async () => {
    const wrapper = mount(QueryEditor, { props: { connectionId: 'local', database: 'app', initialSql: 'SELECT 1' }, global: { plugins: [createPinia()], stubs: { ...dialogStubs, ScrollArea: { template: '<div><slot /></div>' } } } })
    await flushPromises()
    expect(wrapper.findAll('button').some(button => button.text().startsWith('Run'))).toBe(true)
    expect(wrapper.findAll('button').some(button => button.text() === 'Explain')).toBe(true)
    wrapper.unmount()
  })
})
