import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { mount } from '@vue/test-utils'
import { defineComponent, ref } from 'vue'
import { save } from '@tauri-apps/plugin-dialog'
import { useSidebarManager } from '@/composables/useSidebarManager'
import { useConnectionStore } from '@/stores/connections'
import { useProgressStore } from '@/stores/progress'
import { useToast } from '@/composables/useToast'
import { useQueryLogStore } from '@/stores/queryLog'

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({ invoke }))
let wrapper: ReturnType<typeof mount>
beforeEach(() => { setActivePinia(createPinia()); invoke.mockReset(); useToast().toasts.value = [] })
afterEach(() => wrapper?.unmount())
function setup() {
  const store = useConnectionStore()
  vi.spyOn(store, 'fetchConnections').mockResolvedValue(undefined)
  const refresh = vi.spyOn(store, 'fetchTablesForConnection').mockResolvedValue([])
  store.openConnections.local = {
    connection: { id: 'local', name: 'Local', environment: 'LOCAL', database: { engine: 'mysql', settings: { host: 'localhost', port: 3306, user: 'root' } } },
    databases: ['app'], selectedDatabase: 'app', openedDatabases: ['app'], serverVersion: '8.4', status: 'connected', statusMessage: null, tables: {},
    capabilities: { schemas: false, createDatabase: true, databaseCollations: true, editRows: true, alterColumns: true, truncateTable: true, disableForeignKeyChecks: true, cancelQuery: true, importSql: true, exportSql: true, estimatedRowCount: true },
  }
  let sidebar!: ReturnType<typeof useSidebarManager>
  wrapper = mount(defineComponent({ setup() {
    sidebar = useSidebarManager({ panes: ref([]), activePaneId: ref('pane'), getPane: vi.fn(), getPaneTab: vi.fn(), switchToTab: vi.fn(), closeTab: vi.fn(), refreshActiveTab: vi.fn(), loadTableData: vi.fn(), openQueryTab: vi.fn() })
    return () => null
  } }))
  sidebar.openImportSelector('local', 'app')
  return { sidebar, refresh, progress: useProgressStore(), log: useQueryLogStore() }
}
const metrics = { parsed_statements: 10, compacted_statements: 0, executed_batches: 10, sql_blocks: 10, read_ms: 1, process_ms: 2, execute_ms: 3, total_ms: 6 }
describe('import error reporting', () => {
  it('retains every backend error and logs them even when refreshing the database fails', async () => {
    const { sidebar, refresh, progress, log } = setup()
    const errors = ['CREATE TABLE users: Table already exists', 'INSERT INTO orders: Duplicate entry', 'ALTER TABLE users: Unknown column']
    invoke.mockResolvedValue({ executed: 7, errors, metrics })
    refresh.mockRejectedValueOnce('Connection lost during refresh')
    const pending = sidebar.confirmImportFromFilePath('/backups/dump.sql')
    progress.importExpanded = false
    await pending
    expect(useToast().toasts.value.filter(toast => toast.type === 'error')).toEqual([])
    expect(progress.isImporting).toBe(false)
    expect(progress.importReport).toEqual({ database: 'app', fileName: 'dump.sql', executed: 7, errors })
    expect(progress.importExpanded).toBe(true)
    for (const error of errors) expect(log.entries.at(-1)?.error).toContain(error)
  })
  it('makes a fatal import failure inspectable after progress has been cleared', async () => {
    const { sidebar, progress } = setup()
    invoke.mockRejectedValueOnce('Permission denied: dump.sql')
    const pending = sidebar.confirmImportFromFilePath('/backups/dump.sql')
    progress.importExpanded = false
    await pending
    expect(progress.importExpanded).toBe(true)
    expect(useToast().toasts.value.filter(toast => toast.type === 'error')).toEqual([])
    expect(progress.isImporting).toBe(false)
    expect(progress.importId).toBeNull()
    expect(progress.importReport).toEqual({ database: 'app', fileName: 'dump.sql', executed: null, errors: ['Permission denied: dump.sql'] })
  })
  it('does not show cancellation or a successful import as an error', async () => {
    const { sidebar, progress } = setup()
    invoke.mockRejectedValueOnce('Import cancelled')
    await sidebar.confirmImportFromFilePath('/backups/dump.sql')
    expect(progress.importReport).toBeNull()
    invoke.mockResolvedValueOnce({ executed: 10, errors: [], metrics })
    await sidebar.confirmImportFromFilePath('/backups/dump.sql')
    expect(progress.importReport?.errors).toEqual([])
  })
})

describe('export concurrency', () => {
  const payload = { format: 'sql', options: { dropIfExists: true, includeViews: true, useTransactions: true, compressGzip: false } }
  it('reopens the active export instead of starting another one', async () => {
    const { sidebar, progress } = setup()
    progress.isExporting = true
    progress.exportExpanded = false
    await sidebar.openExportSelector('local', 'app')
    expect(progress.exportExpanded).toBe(true)
    expect(sidebar.showTableSelector.value).toBe(false)
    await sidebar.startExport(payload)
    expect(invoke).not.toHaveBeenCalledWith('export_database', expect.anything())
  })

  it('checks again after the save dialog in case another export started meanwhile', async () => {
    const { sidebar, progress } = setup()
    await sidebar.openExportSelector('local', 'app')
    sidebar.selectedExportTables.value = ['users']
    let selectPath!: (path: string) => void
    vi.mocked(save).mockReturnValueOnce(new Promise(resolve => { selectPath = resolve }))
    const pending = sidebar.startExport(payload)
    // downloadDir resolves before the native save dialog opens.
    await new Promise(resolve => setTimeout(resolve, 0))
    progress.isExporting = true
    progress.exportId = 'existing-export'
    progress.exportExpanded = false
    selectPath('/tmp/dump.sql')
    await pending
    expect(progress.exportId).toBe('existing-export')
    expect(progress.exportExpanded).toBe(true)
    expect(invoke).not.toHaveBeenCalledWith('export_database', expect.anything())
  })
})
