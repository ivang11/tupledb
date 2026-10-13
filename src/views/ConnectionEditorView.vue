<template>
  <div class="flex h-screen flex-col overflow-hidden bg-background" :class="nativeChrome ? '' : 'border border-(--line-1)'">
    <WindowResizeHandles v-if="!nativeChrome" />
    <div ref="bar" data-tauri-drag-region class="relative flex h-8 shrink-0 items-center border-b border-(--line-2) bg-(--bg-1)">
      <!-- Centered over the full bar so the traffic lights on macOS do not push it sideways. -->
      <span data-tauri-drag-region class="pointer-events-none absolute inset-x-0 text-center text-[11.5px] font-medium text-(--fg-3)">{{ draft?.editing ? 'Edit connection' : 'New connection' }}</span>
      <button v-if="!nativeChrome" type="button" class="absolute right-1.5 flex size-6 items-center justify-center rounded-md text-(--fg-3) transition-colors hover:bg-(--bg-2) hover:text-(--fg-1)"
        aria-label="Close connection window" :disabled="isSaving" @click="close"><XIcon class="pointer-events-none size-3.5" /></button>
    </div>
    <ConnectionForm v-if="draft" class="min-h-0 flex-1" :open="true" :connection="draft.connection"
      :editing="draft.editing" :is-saving="isSaving" :show-connect-button="draft.showConnectButton" :show-cancel="false"
      @update:open="close" @save="save" @height-change="resizeToFit">
      <template #error><p v-if="saveError" role="alert" class="mb-3 max-h-20 overflow-y-auto text-xs text-destructive">{{ saveError }}</p></template>
    </ConnectionForm>
    <div v-else class="flex flex-1 items-center justify-center text-[12.5px] text-(--fg-4)">Opening connection settings…</div>
  </div>
</template>

<script setup lang="ts">
import { nextTick, onMounted, onUnmounted, ref } from 'vue'
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow'
import { LogicalSize, PhysicalPosition } from '@tauri-apps/api/dpi'
import { currentMonitor } from '@tauri-apps/api/window'
import type { UnlistenFn } from '@tauri-apps/api/event'
import ConnectionForm from '@/components/connections/ConnectionForm.vue'
import WindowResizeHandles from '@/components/WindowResizeHandles.vue'
import { usesNativeMacWindowControls } from '@/lib/platform'
import { XIcon } from 'lucide-vue-next'
import { useConnectionStore } from '@/stores/connections'
import { editorEvents, type EditorDraft } from '@/lib/connectionEditorWindow'
import type { Connection } from '@/types/connection'

// macOS uses native decorations (rounded corners, traffic lights); other
// platforms draw their own close button and resize handles.
const nativeChrome = usesNativeMacWindowControls
const editor = getCurrentWebviewWindow()
const store = useConnectionStore()
const draft = ref<EditorDraft | null>(null)
const bar = ref<HTMLElement | null>(null)
const CHROME_FALLBACK_PX = 30
const isSaving = ref(false)
const saveError = ref<string | null>(null)
const unlisteners: UnlistenFn[] = []
let disposed = false
let resizeQueue = Promise.resolve()
let revealed = false
let revealTimer: ReturnType<typeof setTimeout> | undefined

// The window is created hidden. Reveal it after the form has painted and the
// window has been fitted to it; the timer guarantees it appears even if the
// host never sends the draft.
async function reveal() {
  if (revealed || disposed) return
  revealed = true
  clearTimeout(revealTimer)
  try { await editor.show() } catch (error) { console.error('Could not show connection editor:', error) }
}

function resizeToFit(formHeight: number) {
  // Fit the actual content, including test feedback and optional certificates.
  // Scroll is only needed when the content exceeds the monitor's usable area.
  resizeQueue = resizeQueue.then(async () => {
    if (disposed) return
    const [size, scale, monitor, position] = await Promise.all([
      editor.innerSize(), editor.scaleFactor(), currentMonitor(), editor.outerPosition(),
    ])
    if (disposed) return
    const usableHeight = monitor ? monitor.workArea.size.height / scale : screen.availHeight
    // Window chrome above the form: the measured bar (with its border) plus the
    // frame border on platforms that draw their own.
    const barHeight = bar.value?.getBoundingClientRect().height ?? 0
    const chrome = barHeight > 0 ? barHeight + (nativeChrome ? 0 : 2) : CHROME_FALLBACK_PX
    const height = Math.min(Math.max(480, formHeight + chrome), Math.max(480, usableHeight - 32))
    await editor.setSize(new LogicalSize(size.width / scale, height))
    if (monitor && !disposed) {
      const top = monitor.workArea.position.y + 16 * scale
      const bottom = monitor.workArea.position.y + monitor.workArea.size.height - (height + 16) * scale
      const y = Math.max(top, Math.min(position.y, bottom))
      if (y !== position.y) await editor.setPosition(new PhysicalPosition(position.x, y))
    }
  }).catch(error => console.error('Could not resize connection editor:', error))
}

async function close() {
  if (!isSaving.value) await editor.close()
}

async function save(connection: Connection, andConnect: boolean) {
  if (isSaving.value) return
  isSaving.value = true
  saveError.value = null
  try {
    await editor.emitTo('main', editorEvents.save, { label: editor.label, connection, andConnect })
  } catch (error) {
    saveError.value = String(error)
    isSaving.value = false
  }
}

function onKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape') void close()
}

onMounted(async () => {
  unlisteners.push(await editor.listen<EditorDraft>(editorEvents.initialize, async event => {
    if (draft.value) return
    store.availableDrivers = event.payload.drivers
    draft.value = event.payload
    await nextTick()
    await new Promise(resolve => requestAnimationFrame(() => resolve(undefined)))
    await resizeQueue
    await reveal()
    await editor.setFocus()
    document.getElementById('connection-name')?.focus()
  }))
  unlisteners.push(await editor.listen<{ error: string | null }>(editorEvents.result, event => {
    saveError.value = event.payload.error
    isSaving.value = false
  }))
  unlisteners.push(await editor.onCloseRequested(event => {
    if (isSaving.value) event.preventDefault()
  }))
  document.addEventListener('keydown', onKeydown)
  revealTimer = setTimeout(() => void reveal(), 1500)
  await editor.emitTo('main', editorEvents.ready, { label: editor.label })
})

onUnmounted(() => {
  disposed = true
  clearTimeout(revealTimer)
  unlisteners.forEach(unlisten => unlisten())
  document.removeEventListener('keydown', onKeydown)
})
</script>
