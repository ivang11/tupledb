import { beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises, mount } from '@vue/test-utils'
import { createPinia, setActivePinia } from 'pinia'
import ConnectionEditorView from '@/views/ConnectionEditorView.vue'
import ConnectionForm from '@/components/connections/ConnectionForm.vue'
import { editorEvents } from '@/lib/connectionEditorWindow'

const transport = vi.hoisted(() => ({
  listeners: new Map<string, (event: any) => any>(),
  emitTo: vi.fn(), close: vi.fn(), setFocus: vi.fn(),
  innerSize: vi.fn(), scaleFactor: vi.fn(), setSize: vi.fn(),
  outerPosition: vi.fn(), setPosition: vi.fn(), currentMonitor: vi.fn(),
}))
vi.mock('@tauri-apps/api/webviewWindow', () => ({
  WebviewWindow: class {},
  getCurrentWebviewWindow: () => ({
    label: 'connection-editor-fixture',
    emitTo: transport.emitTo, close: transport.close, setFocus: transport.setFocus,
    innerSize: transport.innerSize, scaleFactor: transport.scaleFactor, setSize: transport.setSize,
    outerPosition: transport.outerPosition, setPosition: transport.setPosition,
    listen: async (name: string, handler: (event: any) => any) => {
      transport.listeners.set(name, handler)
      return () => transport.listeners.delete(name)
    },
    onCloseRequested: async (handler: (event: any) => any) => {
      transport.listeners.set('close-requested', handler)
      return () => transport.listeners.delete('close-requested')
    },
  }),
}))
vi.mock('@tauri-apps/api/window', () => ({ currentMonitor: transport.currentMonitor }))

beforeEach(() => {
  setActivePinia(createPinia())
  transport.listeners.clear()
  vi.clearAllMocks()
  transport.emitTo.mockResolvedValue(undefined)
  transport.close.mockResolvedValue(undefined)
  transport.setFocus.mockResolvedValue(undefined)
  transport.innerSize.mockResolvedValue({ width: 1280, height: 1120 })
  transport.scaleFactor.mockResolvedValue(2)
  transport.setSize.mockResolvedValue(undefined)
  transport.outerPosition.mockResolvedValue({ x: 400, y: 200 })
  transport.setPosition.mockResolvedValue(undefined)
  transport.currentMonitor.mockResolvedValue({ workArea: { position: { x: 0, y: 0 }, size: { width: 3840, height: 2160 } } })
})

async function setup(ssh = false) {
  const wrapper = mount(ConnectionEditorView, { attachTo: document.body, global: { stubs: { WindowResizeHandles: true } } })
  await flushPromises()
  await transport.listeners.get(editorEvents.initialize)!({ payload: {
    connection: { id: 'fixture', name: 'Test connection', environment: 'LOCAL', database: {
      engine: 'postgresql', settings: { host: 'localhost', port: 5432, user: 'postgres', ssl_mode: 'verify_ca' },
    }, ssh: ssh ? { host: 'existing-bastion', port: 22, user: 'dev', auth: { type: 'password', password: 'fixture-password' } } : undefined },
    editing: false, drivers: [{ engine: 'postgresql', label: 'PostgreSQL', defaultPort: 5432 }], showConnectButton: true,
  } })
  await flushPromises()
  return wrapper
}

describe('connection editor form in a separate window', () => {
  it('shows connection, SSH and TLS settings together without a title bar or tabs', async () => {
    const wrapper = await setup()
    expect(wrapper.find('[data-slot="dialog-content"]').exists()).toBe(false)
    expect(wrapper.find('[role="tablist"]').exists()).toBe(false)
    expect(wrapper.find('h1, h2').exists()).toBe(false)
    expect(wrapper.find('[data-tauri-drag-region]').exists()).toBe(true)
    expect(wrapper.get('#connection-host').isVisible()).toBe(true)
    expect(wrapper.get('#connection-ssh-enabled').isVisible()).toBe(true)
    expect(wrapper.get('#connection-ssh-host').isVisible()).toBe(false)
    wrapper.findComponent(ConnectionForm).vm.$emit('height-change', 450)
    await flushPromises()
    expect(transport.setSize).toHaveBeenLastCalledWith(expect.objectContaining({ width: 640, height: 480 }))
    expect(wrapper.get('fieldset').attributes('disabled')).toBeDefined()
    expect(wrapper.get('#connection-timeout').isVisible()).toBe(true)
    expect(document.activeElement?.id).toBe('connection-name')
    expect(wrapper.get('#connection-ca').isVisible()).toBe(true)
    expect(wrapper.findAll('button').find(button => button.text() === 'Save & Connect')!.isVisible()).toBe(true)
    await wrapper.get('#connection-ssh-enabled').trigger('click')
    await flushPromises()
    wrapper.findComponent(ConnectionForm).vm.$emit('height-change', 710)
    await flushPromises()
    expect(transport.setSize).toHaveBeenLastCalledWith(expect.objectContaining({ width: 640, height: 740 }))
    expect(wrapper.get('fieldset').attributes('disabled')).toBeUndefined()
    expect(wrapper.get('#connection-ssh-host').isVisible()).toBe(true)
    expect(wrapper.get('#connection-host').isVisible()).toBe(true)
    expect(wrapper.get('#connection-ca').isVisible()).toBe(true)
    await wrapper.get('#connection-ssh-host').setValue('changed-bastion')
    await wrapper.get('#connection-ssh-enabled').trigger('click')
    await flushPromises()
    expect(wrapper.get('#connection-ssh-host').isVisible()).toBe(false)
    wrapper.findComponent(ConnectionForm).vm.$emit('height-change', 450)
    await flushPromises()
    expect(transport.setSize).toHaveBeenLastCalledWith(expect.objectContaining({ width: 640, height: 480 }))
    await wrapper.get('#connection-ssh-enabled').trigger('click')
    await flushPromises()
    expect((wrapper.get('#connection-ssh-host').element as HTMLInputElement).value).toBe('changed-bastion')
    wrapper.unmount()
    expect(transport.listeners.size).toBe(0)
  })

  it('opens an existing SSH connection with its settings already visible below the connection', async () => {
    const wrapper = await setup(true)
    expect(wrapper.get('#connection-ssh-host').isVisible()).toBe(true)
    expect((wrapper.get('#connection-ssh-host').element as HTMLInputElement).value).toBe('existing-bastion')
    wrapper.findComponent(ConnectionForm).vm.$emit('height-change', 710)
    await flushPromises()
    expect(transport.setSize).toHaveBeenLastCalledWith(expect.objectContaining({ width: 640, height: 740 }))
    wrapper.unmount()
  })

  it('keeps a tall editor inside the monitor work area', async () => {
    const wrapper = await setup()
    wrapper.findComponent(ConnectionForm).vm.$emit('height-change', 2000)
    await flushPromises()
    expect(transport.setSize).toHaveBeenLastCalledWith(expect.objectContaining({ width: 640, height: 1048 }))
    expect(transport.setPosition).toHaveBeenLastCalledWith(expect.objectContaining({ x: 400, y: 32 }))
    wrapper.unmount()
  })

  it('sends Save & Connect to the main workspace and retains the form on persistence failure', async () => {
    const wrapper = await setup()
    await wrapper.get('#connection-name').setValue('Edited draft')
    await wrapper.findAll('button').find(button => button.text() === 'Save & Connect')!.trigger('click')
    await flushPromises()
    expect(transport.emitTo).toHaveBeenCalledWith('main', editorEvents.save, expect.objectContaining({
      label: 'connection-editor-fixture', andConnect: true, connection: expect.objectContaining({ name: 'Edited draft' }),
    }))
    expect(transport.emitTo.mock.calls.find(([, event]) => event === editorEvents.save)![2].connection.ssh).toBeUndefined()
    const preventDefault = vi.fn()
    transport.listeners.get('close-requested')!({ preventDefault })
    expect(preventDefault).toHaveBeenCalled()
    transport.listeners.get(editorEvents.result)!({ payload: { error: 'Cannot write connection storage' } })
    await flushPromises()
    expect(wrapper.get('[role="alert"]').text()).toContain('Cannot write connection storage')
    expect((wrapper.get('#connection-name').element as HTMLInputElement).value).toBe('Edited draft')
    expect(wrapper.findAll('button').find(button => button.text() === 'Save & Connect')!.attributes('disabled')).toBeUndefined()
    expect(transport.close).not.toHaveBeenCalled()
    wrapper.unmount()
  })

  it('closes the native window when Cancel is selected', async () => {
    const wrapper = await setup()
    await wrapper.findAll('button').find(button => button.text() === 'Cancel')!.trigger('click')
    await flushPromises()
    expect(transport.close).toHaveBeenCalledTimes(1)
    wrapper.unmount()
  })
})
