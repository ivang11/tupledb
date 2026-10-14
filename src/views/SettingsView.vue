<template>
  <div class="flex h-screen flex-col overflow-hidden bg-background" :class="nativeChrome ? '' : 'border border-(--line-1)'">
    <WindowResizeHandles v-if="!nativeChrome" />
    <div data-tauri-drag-region class="relative flex h-8 shrink-0 items-center border-b border-(--line-2) bg-(--bg-1)">
      <span data-tauri-drag-region class="pointer-events-none absolute inset-x-0 text-center text-[11.5px] font-medium text-(--fg-3)">Settings</span>
      <button v-if="!nativeChrome" type="button" class="absolute right-1.5 flex size-6 items-center justify-center rounded-md text-(--fg-3) transition-colors hover:bg-(--bg-2) hover:text-(--fg-1)"
        aria-label="Close settings window" @click="close"><XIcon class="pointer-events-none size-3.5" /></button>
    </div>

    <div class="flex min-h-0 flex-1 select-none">
      <nav class="w-44 shrink-0 space-y-0.5 border-r border-(--line-2) bg-(--bg-1)/50 p-2" role="tablist" aria-label="Settings sections">
        <button
          v-for="section in sections"
          :key="section.id"
          role="tab"
          :aria-selected="tab === section.id"
          class="flex h-8 w-full items-center gap-2 rounded-md px-2.5 text-left text-[12.5px] transition-colors"
          :class="tab === section.id ? 'bg-(--bg-2) text-(--fg-1)' : 'text-(--fg-3) hover:bg-(--bg-1) hover:text-(--fg-1)'"
          @click="tab = section.id"
        >
          <component :is="section.icon" class="size-3.5 shrink-0" :class="tab === section.id ? 'text-(--fg-2)' : 'text-(--fg-4)'" />
          {{ section.label }}
        </button>
      </nav>

      <div class="custom-scrollbar min-w-0 flex-1 overflow-y-auto px-6 py-5">
        <h2 class="mb-4 text-[15px] font-semibold tracking-[-0.01em] text-(--fg-1)">{{ activeSection.label }}</h2>
        <KeyboardShortcutsPanel v-if="tab === 'keyboard'" />
        <div v-else class="settings-fields">
          <template v-if="tab === 'appearance'">
            <label>Theme<select v-model="settings.theme"><option value="system">Follow system</option><option value="light">Light</option><option value="dark">Dark</option></select></label>
            <label>Interface font<select v-model="settings.uiFont"><option value="system">System</option><option value="Inter">Inter (if installed)</option></select></label>
            <label>Table font<select v-model="settings.tableFont"><option v-for="font in FONT_OPTIONS" :key="font.value" :value="font.value">{{ font.label }}</option></select></label>
            <label>Table font size<input type="number" min="10" max="20" v-model.number="settings.tableFontSize" /></label>
            <label>Row density<select v-model="settings.density"><option value="comfortable">Comfortable</option><option value="compact">Compact</option></select></label>
            <label>Default column width<select v-model="settings.columnWidth"><option value="narrow">Narrow · 100 px</option><option value="normal">Normal · 180 px</option><option value="wide">Wide · 260 px</option><option value="fit">Fit to content</option></select></label>
            <label>Alternate row backgrounds<ToggleSwitch v-model="settings.alternatingRows" /></label>
            <p>Manually resized columns keep their width. “Reset columns” restores the default.</p>
          </template>
          <template v-if="tab === 'editor'">
            <label>Editor font<select v-model="settings.editorFont"><option v-for="font in FONT_OPTIONS" :key="font.value" :value="font.value">{{ font.label }}</option></select></label>
            <label>Editor font size<input type="number" min="10" max="24" v-model.number="settings.editorFontSize" /></label>
            <label>Indent with<select v-model="settings.indent"><option value="spaces">Spaces</option><option value="tabs">Tabs</option></select></label>
            <label>Indent size<input type="number" min="1" max="8" v-model.number="settings.tabSize" /></label>
            <label>Accept autocomplete with<select v-model="settings.completionKey"><option value="both">Enter or Tab</option><option value="enter">Enter</option><option value="tab">Tab</option></select></label>
            <label>Close quotes and brackets automatically<ToggleSwitch v-model="settings.closeBrackets" /></label>
            <label>Complete SQL keywords in uppercase<ToggleSwitch v-model="settings.uppercaseKeywords" /></label>
            <label>Enable :named query parameters<ToggleSwitch v-model="settings.queryParameters" /></label>
            <p>Run executes the selection, or the statement under the cursor. Run all executes all statements in order.</p>
          </template>
          <template v-if="tab === 'queries'">
            <label>Query timeout (seconds)<input type="number" min="1" max="3600" v-model.number="settings.queryTimeout" /></label>
            <label>Maximum displayed rows per result<input type="number" min="100" max="50000" step="100" v-model.number="settings.resultRowLimit" /></label>
            <p>Results also have a cell limit to keep wide queries responsive.</p>
          </template>
          <template v-if="tab === 'export'">
            <label>CSV delimiter<select v-model="settings.csvDelimiter"><option value=",">Comma</option><option value=";">Semicolon</option><option :value="'\t'">Tab</option><option value="|">Pipe</option></select></label>
            <label>CSV quoting<select v-model="settings.csvQuotes"><option value="needed">Quote when needed</option><option value="always">Always quote</option></select></label>
            <label>Line breaks<select v-model="settings.csvLineBreak"><option :value="'\n'">LF</option><option :value="'\r\n'">CRLF</option></select></label>
            <label>Decimal separator<select v-model="settings.csvDecimal"><option value=".">Dot</option><option value=",">Comma</option></select></label>
            <label>SQL NULL marker<input v-model="settings.csvNull" class="select-text" /></label>
            <p>These defaults apply to CSV result export.</p>
          </template>
          <template v-if="tab === 'general'">
            <label>Restore workspace on startup<ToggleSwitch v-model="settings.restoreSession" /></label>
            <p>Restored SQL drafts are never executed automatically.</p>
            <SettingsUpdates />
            <div class="flex items-center justify-between gap-5 border-t border-(--line-faint) pt-4">
              <span class="text-[12.5px] text-(--fg-1)">Source code</span>
              <button type="button" class="flex h-8 items-center gap-1.5 rounded-lg px-3 text-[12.5px] text-(--fg-2) transition-colors hover:bg-(--bg-1) hover:text-(--fg-1)" @click="openRepository">
                <GithubIcon class="size-3.5" /> github.com/ivang11/tupledb
              </button>
            </div>
            <div class="flex items-center justify-between gap-5 border-t border-(--line-faint) pt-4">
              <span class="text-[12.5px] text-(--fg-1)">Preferences</span>
              <button type="button" class="h-8 rounded-lg px-3 text-[12.5px] text-(--fg-3) transition-colors hover:bg-(--bg-1) hover:text-(--fg-1)" @click="reset">
                Restore defaults
              </button>
            </div>
          </template>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref } from 'vue'
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow'
import {
  CodeIcon,
  FileOutputIcon,
  GithubIcon,
  KeyboardIcon,
  PaletteIcon,
  SlidersHorizontalIcon,
  TerminalIcon,
  XIcon,
} from 'lucide-vue-next'
import KeyboardShortcutsPanel from '@/components/KeyboardShortcutsPanel.vue'
import ToggleSwitch from '@/components/ui/ToggleSwitch.vue'
import SettingsUpdates from '@/components/SettingsUpdates.vue'
import { openUrl } from '@tauri-apps/plugin-opener'
import { REPOSITORY_URL } from '@/lib/links'
import WindowResizeHandles from '@/components/WindowResizeHandles.vue'
import { useAppSettings, FONT_OPTIONS } from '@/composables/useAppSettings'
import { usesNativeMacWindowControls } from '@/lib/platform'
import { SETTINGS_TAB_EVENT } from '@/lib/settingsWindow'
import type { UnlistenFn } from '@tauri-apps/api/event'

const nativeChrome = usesNativeMacWindowControls
const win = getCurrentWebviewWindow()
const { settings, reset } = useAppSettings()

const sections = [
  { id: 'general', label: 'General', icon: SlidersHorizontalIcon },
  { id: 'appearance', label: 'Appearance', icon: PaletteIcon },
  { id: 'editor', label: 'Editor', icon: CodeIcon },
  { id: 'queries', label: 'Queries', icon: TerminalIcon },
  { id: 'export', label: 'Export', icon: FileOutputIcon },
  { id: 'keyboard', label: 'Keyboard shortcuts', icon: KeyboardIcon },
]
function requestedTab(value: string | null | undefined) {
  return sections.some((section) => section.id === value) ? (value as string) : null
}
const tab = ref(requestedTab(new URLSearchParams(window.location.search).get('tab')) ?? 'general')
const activeSection = computed(() => sections.find((section) => section.id === tab.value) ?? sections[0])

async function close() {
  await win.close()
}

function openRepository() {
  void openUrl(REPOSITORY_URL).catch(error => console.error('Could not open repository:', error))
}

function onKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape') void close()
}

let stopTabRequests: UnlistenFn | undefined
let revealed = false
async function reveal() {
  if (revealed) return
  revealed = true
  try { await win.show() } catch (error) { console.error('Could not show settings window:', error) }
}

onMounted(async () => {
  document.addEventListener('keydown', onKeydown)
  stopTabRequests = await win.listen<string>(SETTINGS_TAB_EVENT, event => {
    const next = requestedTab(event.payload)
    if (next) tab.value = next
  })
  await nextTick()
  await new Promise(resolve => requestAnimationFrame(() => resolve(undefined)))
  await reveal()
  await win.setFocus()
})

onUnmounted(() => {
  document.removeEventListener('keydown', onKeydown)
  stopTabRequests?.()
})
</script>

<style scoped>
.settings-fields { display: grid; gap: 14px; }
.settings-fields label { display: flex; align-items: center; justify-content: space-between; gap: 20px; font-size: 12.5px; color: var(--fg-1); }
.settings-fields select,
.settings-fields input:not([type=checkbox]) {
  height: 32px;
  width: 220px;
  border-radius: 6px;
  border: 1px solid var(--line-faint);
  background: var(--bg-1);
  padding: 0 10px;
  font-size: 12.5px;
  color: var(--fg-1);
  transition: border-color 120ms ease;
}
.settings-fields select:focus-visible,
.settings-fields input:not([type=checkbox]):focus-visible { outline: none; border-color: var(--acc-line); }
.settings-fields select {
  appearance: none;
  padding-right: 28px;
  background-image: url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='12' height='12' viewBox='0 0 24 24' fill='none' stroke='%23888' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'><path d='m6 9 6 6 6-6'/></svg>");
  background-repeat: no-repeat;
  background-position: right 10px center;
}
.settings-fields input[type=checkbox] { width: 15px; height: 15px; accent-color: var(--acc); }
.settings-fields p { font-size: 11.5px; line-height: 1.6; color: var(--fg-4); }
</style>
