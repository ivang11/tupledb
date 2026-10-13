<template>
  <Dialog :open="settingsOpen" @update:open="settingsOpen = $event">
    <DialogContent class="sm:max-w-3xl max-h-[85vh] overflow-auto select-none">
      <DialogHeader><DialogTitle>Settings</DialogTitle><DialogDescription>Preferences apply immediately and are saved on this device.</DialogDescription></DialogHeader>
      <div class="flex gap-1 border-b pb-2 overflow-x-auto" role="tablist" aria-label="Settings sections">
        <button v-for="section in sections" :key="section.id" role="tab" :aria-selected="tab === section.id" class="px-3 py-1.5 rounded text-xs whitespace-nowrap hover:bg-muted" :class="tab === section.id ? 'bg-muted text-foreground' : 'text-muted-foreground'" @click="tab = section.id">{{ section.label }}</button>
      </div>
      <KeyboardShortcutsPanel v-if="tab === 'keyboard'" />
      <div v-else class="settings-fields">
        <template v-if="tab === 'appearance'">
          <label>Theme<select v-model="settings.theme"><option value="system">Follow system</option><option value="light">Light</option><option value="dark">Dark</option></select></label>
          <label>Interface font<select v-model="settings.uiFont"><option value="system">System</option><option value="Inter">Inter (if installed)</option></select></label>
          <label>Table font<select v-model="settings.tableFont"><option v-for="font in FONT_OPTIONS" :key="font.value" :value="font.value">{{ font.label }}</option></select></label>
          <label>Table font size<input type="number" min="10" max="20" v-model.number="settings.tableFontSize" /></label>
          <label>Row density<select v-model="settings.density"><option value="comfortable">Comfortable</option><option value="compact">Compact</option></select></label>
          <label>Default column width<select v-model="settings.columnWidth"><option value="narrow">Narrow · 100 px</option><option value="normal">Normal · 180 px</option><option value="wide">Wide · 260 px</option><option value="fit">Fit to content</option></select></label>
          <label>Alternate row backgrounds<input type="checkbox" v-model="settings.alternatingRows" /></label>
          <p>Manually resized columns keep their width. “Reset columns” restores the default.</p>
        </template>
        <template v-if="tab === 'editor'">
          <label>Editor font<select v-model="settings.editorFont"><option v-for="font in FONT_OPTIONS" :key="font.value" :value="font.value">{{ font.label }}</option></select></label>
          <label>Editor font size<input type="number" min="10" max="24" v-model.number="settings.editorFontSize" /></label>
          <label>Indent with<select v-model="settings.indent"><option value="spaces">Spaces</option><option value="tabs">Tabs</option></select></label>
          <label>Indent size<input type="number" min="1" max="8" v-model.number="settings.tabSize" /></label>
          <label>Accept autocomplete with<select v-model="settings.completionKey"><option value="both">Enter or Tab</option><option value="enter">Enter</option><option value="tab">Tab</option></select></label>
          <label>Close quotes and brackets automatically<input type="checkbox" v-model="settings.closeBrackets" /></label>
          <label>Complete SQL keywords in uppercase<input type="checkbox" v-model="settings.uppercaseKeywords" /></label>
          <label>Enable :named query parameters<input type="checkbox" v-model="settings.queryParameters" /></label>
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
          <label>Restore workspace on startup<input type="checkbox" v-model="settings.restoreSession" /></label>
          <p>Restored SQL drafts are never executed automatically.</p>
        </template>
      </div>
      <DialogFooter><Button v-if="tab !== 'keyboard'" variant="outline" @click="reset">Restore defaults</Button><Button @click="settingsOpen = false">Done</Button></DialogFooter>
    </DialogContent>
  </Dialog>
</template>
<script setup lang="ts">
import { ref, watch } from 'vue'
import KeyboardShortcutsPanel from '@/components/KeyboardShortcutsPanel.vue'
import { useAppSettings, FONT_OPTIONS } from '@/composables/useAppSettings'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
const { settings, settingsOpen, reset } = useAppSettings()
const sections = [{ id: 'general', label: 'General' }, { id: 'appearance', label: 'Appearance' }, { id: 'editor', label: 'Editor' }, { id: 'queries', label: 'Queries' }, { id: 'export', label: 'Export' }, { id: 'keyboard', label: 'Keyboard shortcuts' }]
const tab = ref('general')
watch(settingsOpen, open => { if (open) tab.value = 'general' })
</script>
<style scoped>
.settings-fields { display: grid; gap: 14px; padding: 10px 0; }
.settings-fields label { display: flex; align-items: center; justify-content: space-between; gap: 20px; font-size: 13px; }
.settings-fields select, .settings-fields input:not([type=checkbox]) { background: var(--background); border: 1px solid var(--border); border-radius: 5px; padding: 6px 9px; width: 230px; font-size: 12px; }
.settings-fields input[type=checkbox] { accent-color: var(--primary); }
.settings-fields p { font-size: 12px; color: var(--muted-foreground); line-height: 1.6; }
</style>
