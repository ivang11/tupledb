import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { flushPromises, mount } from '@vue/test-utils'
import { reactive } from 'vue'
import ConnectionDialog from '@/components/dialogs/ConnectionDialog.vue'
import { useConnectionStore } from '@/stores/connections'
import type { Connection, ConnectionTestProgress } from '@/types/connection'

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({
  invoke,
  Channel: class { onmessage = (_message: ConnectionTestProgress) => {} },
}))

const dialogStubs = Object.fromEntries(
  ['Dialog', 'DialogContent', 'DialogHeader', 'DialogTitle', 'DialogDescription']
    .map(name => [name, { template: '<div><slot /></div>' }]),
)

beforeEach(() => {
  setActivePinia(createPinia())
  invoke.mockReset()
  useConnectionStore().availableDrivers = [{ engine: 'mysql', label: 'MySQL', defaultPort: 3306 }]
})

function setup(ssh = false) {
  const connection = reactive<Connection>({
    id: 'test', name: 'Local', environment: 'LOCAL', timeout_secs: 5,
    database: { engine: 'mysql', settings: { host: 'localhost', port: 3306, user: 'root', password: 'test-password' } },
    ssh: ssh ? { host: 'bastion', port: 22, user: 'dev', auth: { type: 'password', password: 'ssh-password' } } : undefined,
  })
  const wrapper = mount(ConnectionDialog, {
    props: { open: true, connection, showConnectButton: true }, global: { stubs: dialogStubs },
  })
  return { wrapper, connection }
}

function deferredTest() {
  let resolve!: (value: string) => void
  let reject!: (error: string) => void
  const promise = new Promise<string>((yes, no) => { resolve = yes; reject = no })
  let send!: (progress: ConnectionTestProgress) => void
  invoke.mockImplementation((command, args) => {
    if (command === 'test_connection') {
      send = args.onProgress.onmessage
      return promise
    }
  })
  return { resolve, reject, send: (progress: ConnectionTestProgress) => send(progress) }
}

describe('live connection test feedback', () => {
  it('marks verified host and port green while authentication is still waiting', async () => {
    const pending = deferredTest()
    const { wrapper } = setup()
    await wrapper.findAll('button').find(button => button.text() === 'Test')!.trigger('click')
    expect(wrapper.get('[role="status"]').text()).toContain('Starting connection test')
    pending.send({ fields: ['host'], status: 'success', message: 'Host resolved' })
    pending.send({ fields: ['port'], status: 'success', message: 'Server port reachable' })
    pending.send({ fields: ['user', 'password'], status: 'checking', message: 'Authenticating…' })
    await flushPromises()
    expect(wrapper.get('input[placeholder="127.0.0.1"]').attributes('data-test-state')).toBe('success')
    expect(wrapper.get('input[placeholder="root"]').attributes('data-test-state')).toBe('checking')
    expect(wrapper.get('[role="status"]').text()).toContain('Authenticating…')
    expect(wrapper.findAll('button').find(button => button.text() === 'Save & Connect')!.attributes('disabled')).toBeDefined()
    pending.send({ fields: ['user', 'password'], status: 'error', message: 'Access denied' })
    pending.reject('Access denied')
    await flushPromises()
    expect(wrapper.get('input[placeholder="127.0.0.1"]').attributes('data-test-state')).toBe('success')
    expect(wrapper.get('input[placeholder="root"]').attributes('aria-invalid')).toBe('true')
    expect(wrapper.get('[role="status"]').text()).toContain('Access denied')
    expect(wrapper.findAll('button').some(button => button.text() === 'Test')).toBe(true)
    wrapper.unmount()
  })

  it('retains successful settings while server information is loading and leaves an unspecified database unmarked', async () => {
    const pending = deferredTest()
    const { wrapper } = setup()
    await wrapper.findAll('button').find(button => button.text() === 'Test')!.trigger('click')
    pending.send({ fields: ['host', 'port', 'user', 'password'], status: 'success', message: 'Connection settings accepted' })
    pending.send({ fields: [], status: 'checking', message: 'Reading server information…' })
    await flushPromises()
    expect(wrapper.get('input[placeholder="root"]').attributes('data-test-state')).toBe('success')
    expect(wrapper.get('input[placeholder="Leave blank to pick after connecting"]').attributes('data-test-state')).toBeUndefined()
    expect(wrapper.get('[role="status"]').text()).toContain('Reading server information')
    pending.resolve('Connected successfully')
    await flushPromises()
    expect(wrapper.get('[role="status"]').text()).toBe('Connected successfully')
    wrapper.unmount()
  })

  it('clears obsolete validation when settings change and ignores late events and results', async () => {
    const pending = deferredTest()
    const { wrapper } = setup()
    await wrapper.findAll('button').find(button => button.text() === 'Test')!.trigger('click')
    pending.send({ fields: ['host'], status: 'success', message: 'Host resolved' })
    await flushPromises()
    await wrapper.get('input[placeholder="127.0.0.1"]').setValue('another-host')
    expect(wrapper.get('input[placeholder="127.0.0.1"]').attributes('data-test-state')).toBeUndefined()
    pending.send({ fields: ['host', 'port'], status: 'success', message: 'Old server reachable' })
    pending.resolve('Old connection successful')
    await flushPromises()
    expect(wrapper.find('[role="status"]').exists()).toBe(false)
    expect(wrapper.get('input[placeholder="127.0.0.1"]').attributes('data-test-state')).toBeUndefined()
    wrapper.unmount()
  })

  it('keeps database fields pending until the remote server is reached through SSH', async () => {
    const pending = deferredTest()
    const { wrapper } = setup(true)
    await wrapper.findAll('button').find(button => button.text() === 'Test')!.trigger('click')
    pending.send({ fields: ['sshHost', 'sshPort', 'sshUser', 'sshPassword'], status: 'success', message: 'SSH tunnel ready' })
    pending.send({ fields: ['host', 'port', 'user', 'password'], status: 'checking', message: 'Connecting through SSH…' })
    await flushPromises()
    expect(wrapper.get('input[placeholder="bastion.example.com"]').attributes('data-test-state')).toBe('success')
    expect(wrapper.get('input[placeholder="127.0.0.1"]').attributes('data-test-state')).toBe('checking')
    pending.reject('Remote database unavailable')
    await flushPromises()
    wrapper.unmount()
  })

  it('discards feedback after closing and reopening the modal', async () => {
    const pending = deferredTest()
    const { wrapper } = setup()
    await wrapper.findAll('button').find(button => button.text() === 'Test')!.trigger('click')
    await wrapper.setProps({ open: false })
    pending.send({ fields: ['host'], status: 'success', message: 'Late progress' })
    pending.resolve('Late success')
    await flushPromises()
    await wrapper.setProps({ open: true })
    expect(wrapper.find('[role="status"]').exists()).toBe(false)
    expect(wrapper.get('input[placeholder="127.0.0.1"]').attributes('data-test-state')).toBeUndefined()
    wrapper.unmount()
  })

  it('marks an inaccessible database as failed without blaming unverified credentials', async () => {
    const pending = deferredTest()
    const { wrapper, connection } = setup()
    if (connection.database.engine !== 'mysql') throw new Error('Expected MySQL')
    connection.database.settings.database = 'missing'
    await flushPromises()
    await wrapper.findAll('button').find(button => button.text() === 'Test')!.trigger('click')
    pending.send({ fields: ['user', 'password', 'database'], status: 'checking', message: 'Authenticating and opening database…' })
    pending.send({ fields: ['database'], status: 'error', message: 'Unknown database' })
    pending.reject('Unknown database')
    await flushPromises()
    expect(wrapper.get('input[placeholder="Leave blank to pick after connecting"]').attributes('aria-invalid')).toBe('true')
    expect(wrapper.get('input[placeholder="root"]').attributes('data-test-state')).toBeUndefined()
    expect(wrapper.get('input[type="password"]').attributes('data-test-state')).toBeUndefined()
    wrapper.unmount()
  })
})
