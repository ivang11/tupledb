<template>
  <Dialog :open="open" @update:open="emit('update:open', $event)"><DialogContent class="sm:max-w-md">
    <DialogHeader><DialogTitle>Export results</DialogTitle><DialogDescription>Export {{ rows.length.toLocaleString() }} loaded rows. Rows outside the displayed result are not included.</DialogDescription></DialogHeader>
    <label class="flex items-center justify-between text-sm">Format<select v-model="format" class="border rounded px-3 py-1.5 bg-background"><option value="csv">CSV</option><option value="json">JSON</option></select></label>
    <p v-if="error" class="text-xs text-destructive">{{ error }}</p>
    <DialogFooter><Button variant="outline" @click="emit('update:open', false)">Cancel</Button><Button :disabled="saving" @click="exportFile">{{ saving ? 'Saving…' : 'Save file' }}</Button></DialogFooter>
  </DialogContent></Dialog>
</template>
<script setup lang="ts">
import { ref } from 'vue'
import { save } from '@tauri-apps/plugin-dialog'
import { writeTextFile } from '@tauri-apps/plugin-fs'
import { useAppSettings } from '@/composables/useAppSettings'
import { serializeCsv } from '@/lib/csv'
import { rowRecord, type DataRow, type NamedColumn } from '@/lib/rowAccess'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
const props = defineProps<{ open: boolean; columns: NamedColumn[]; rows: DataRow[] }>()
const emit = defineEmits<{ 'update:open': [value: boolean] }>()
const format = ref('csv'), error = ref(''), saving = ref(false)
const { settings } = useAppSettings()
async function exportFile() {
  error.value = ''; saving.value = true
  try {
    const path = await save({ defaultPath: `results.${format.value}`, filters: [{ name: format.value.toUpperCase(), extensions: [format.value] }] })
    if (!path) return
    const options = settings.value
    const text = format.value === 'json' ? JSON.stringify(props.rows.map(row => rowRecord(row, props.columns)), null, 2)
      : serializeCsv(props.columns, props.rows, { delimiter: options.csvDelimiter, quotes: options.csvQuotes, lineBreak: options.csvLineBreak, decimal: options.csvDecimal, nullMarker: options.csvNull })
    await writeTextFile(path, text); emit('update:open', false)
  } catch (cause) { error.value = String(cause) }
  finally { saving.value = false }
}
</script>
