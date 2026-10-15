import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { DOMWrapper, flushPromises, mount } from '@vue/test-utils'
import { createPinia, setActivePinia } from 'pinia'
import { defineComponent, h } from 'vue'
import FloatingNoticeStack from '@/components/FloatingNoticeStack.vue'
import TransferProgressNotices from '@/components/TransferProgressNotices.vue'
import StatusBar from '@/components/StatusBar.vue'
import UpdateNotice from '@/components/UpdateNotice.vue'
import { useProgressStore } from '@/stores/progress'
const native = vi.hoisted(() => ({ invoke: vi.fn(), error: vi.fn(), copy: vi.fn() }))
vi.mock('@/lib/clipboard', () => ({ copyText: native.copy }))
vi.mock('@tauri-apps/api/core', () => ({ invoke: native.invoke }))
vi.mock('@/composables/useToast', () => ({ useToast: () => ({ error: native.error }) }))
const wrappers: ReturnType<typeof mount>[] = []
beforeEach(() => { setActivePinia(createPinia()); vi.clearAllMocks(); native.invoke.mockResolvedValue(undefined) })
afterEach(() => { wrappers.forEach(wrapper => wrapper.unmount()); wrappers.length = 0 })
function region(label: string) { return new DOMWrapper(document.querySelector(`[role="region"][aria-label="${label}"]`) as HTMLElement) }
function render(includeUpdate = false) {
  const wrapper = mount(defineComponent({ setup: () => () => h('div', [
    h(FloatingNoticeStack, {}, { default: () => [
      h(TransferProgressNotices),
      ...(includeUpdate ? [h(UpdateNotice, { open: true, version: '0.8.7', status: 'available', busy: false, downloadedBytes: 0, totalBytes: 0, downloadPercent: 0, errorMessage: null })] : []),
    ] }),
    h(StatusBar),
  ]) }), { attachTo: document.body })
  wrappers.push(wrapper)
  return wrapper
}
function startExport() {
  const progress = useProgressStore()
  progress.isExporting = true; progress.exportExpanded = true; progress.exportConnectionId = 'local'
  progress.exportId = 'export-1'; progress.exportDatabase = 'app'; progress.exportTables = ['users', 'orders']
  progress.exportProgress = { current: 1, total: 2, status: 'Exporting orders: 400 rows' }
  return progress
}

describe('floating transfer progress', () => {
  it('keeps all import errors accessible after completion, including after hiding the notice', async () => {
    const progress = useProgressStore()
    const errors = ['CREATE TABLE users: Table already exists', 'INSERT INTO orders: Duplicate entry', 'ALTER TABLE items: Unknown column']
    progress.importReport = { database: 'archive', fileName: 'dump.sql', executed: 42, errors }
    progress.importExpanded = true
    const wrapper = render()
    expect(region('Import errors').text()).toContain('Import finished with 3 errors')
    await region('Import errors').get('[aria-label="Hide import errors"]').trigger('click')
    expect(document.querySelector('[role="region"][aria-label="Import errors"]')).toBeNull()
    await wrapper.findAll('button').find(button => button.text().includes('Import errors'))!.trigger('click')
    await region('Import errors').findAll('button').find(button => button.text() === 'View errors')!.trigger('click')
    await flushPromises()
    const dialog = new DOMWrapper(document.querySelector('[role="dialog"]') as HTMLElement)
    expect(dialog.findAll('pre').map(pre => pre.text())).toEqual(errors)
    expect(dialog.text()).toContain('42 statements imported')
    await dialog.findAll('button').find(button => button.text() === 'Copy all errors')!.trigger('click')
    expect(native.copy).toHaveBeenCalledWith(expect.stringContaining('File: dump.sql'))
    for (const error of errors) expect(native.copy).toHaveBeenCalledWith(expect.stringContaining(error))
    await dialog.findAll('button').find(button => button.text() === 'Close')!.trigger('click')
    expect(progress.importErrorsOpen).toBe(false)
    expect(progress.importReport.errors).toEqual(errors)
  })

  it('shows compact export progress and lets the footer reopen a hidden card without cancelling', async () => {
    const progress = startExport()
    const wrapper = render()
    expect(region('Exporting database').text()).toContain('app')
    expect(region('Exporting database').text()).toContain('1 / 2 tables')
    expect(region('Exporting database').get('[role="progressbar"]').attributes('aria-valuenow')).toBe('50')
    await region('Exporting database').get('[aria-label="Hide export progress"]').trigger('click')
    expect(document.querySelector('[aria-label="Exporting database"]')).toBeNull()
    expect(progress.isExporting).toBe(true)
    expect(native.invoke).not.toHaveBeenCalled()
    await wrapper.findAll('button').find(button => button.text().includes('Exporting'))!.trigger('click')
    expect(region('Exporting database').exists()).toBe(true)
    progress.isExporting = false; await flushPromises()
    expect(document.querySelector('[aria-label="Exporting database"]')).toBeNull()
  })

  it('stacks import, export and update cards without a modal overlay and supports indeterminate imports', async () => {
    const progress = startExport()
    progress.isImporting = true; progress.importExpanded = true; progress.importDatabase = 'archive'
    progress.importFileName = 'dump.sql'; progress.importProgress = { current: 0, total: 0, status: 'Reading file…' }
    render(true)
    expect(document.querySelectorAll('[data-notice-stack] [role="region"]')).toHaveLength(3)
    expect(document.querySelector('[data-slot="dialog-overlay"]')).toBeNull()
    expect(region('Importing database').text()).toContain('dump.sql')
    expect(region('Importing database').get('[role="progressbar"]').attributes('aria-valuenow')).toBeUndefined()
    progress.importProgress = { current: 512, total: 1024, status: 'Executing statements…' }
    await flushPromises()
    expect(region('Importing database').get('[role="progressbar"]').attributes('aria-valuenow')).toBe('50')
    expect(region('Importing database').text()).toContain('512 B / 1.0 KB')
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }))
    document.body.dispatchEvent(new MouseEvent('click', { bubbles: true }))
    expect(progress.importExpanded).toBe(true); expect(progress.exportExpanded).toBe(true)
  })

  it('sends cancellation to the correct transfer and restores retry after an error', async () => {
    const progress = startExport()
    render()
    native.invoke.mockRejectedValueOnce(new Error('Cancellation failed'))
    await region('Exporting database').findAll('button').find(button => button.text() === 'Cancel')!.trigger('click')
    await flushPromises()
    expect(native.invoke).toHaveBeenCalledWith('cancel_export', { exportId: 'export-1' })
    expect(progress.isCancellingExport).toBe(false)
    expect(native.error).toHaveBeenCalledWith('Failed to cancel export', 'Error: Cancellation failed')
    progress.isExporting = false; progress.isImporting = true; progress.importExpanded = true
    progress.importConnectionId = 'other'; progress.importId = 'import-2'
    await flushPromises()
    await region('Importing database').findAll('button').find(button => button.text() === 'Cancel')!.trigger('click')
    await flushPromises()
    expect(native.invoke).toHaveBeenCalledWith('cancel_import', { connectionId: 'other', importId: 'import-2' })
    expect(region('Cancelling import…').findAll('button').find(button => button.text() === 'Cancelling…')!.attributes('disabled')).toBeDefined()
    await region('Cancelling import…').get('[aria-label="Hide import progress"]').trigger('click')
    expect(progress.isImporting).toBe(true)
  })
})
