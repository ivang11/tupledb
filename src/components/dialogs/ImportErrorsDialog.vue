<template>
  <Dialog :open="open" @update:open="emit('update:open', $event)">
    <DialogContent overlay-class="z-190" class="z-200 flex max-h-[85vh] flex-col sm:max-w-3xl">
      <DialogHeader>
        <DialogTitle class="text-sm">Import errors ({{ report.errors.length }})</DialogTitle>
        <DialogDescription class="text-xs break-all">{{ report.database }} · {{ report.fileName }}<span v-if="report.executed !== null"> · {{ report.executed.toLocaleString() }} statements imported</span></DialogDescription>
      </DialogHeader>
      <p class="text-xs text-(--fg-3)">Errors returned by the database, including SQL previews when available.</p>
      <ol class="min-h-0 space-y-3 overflow-y-auto custom-scrollbar">
        <li v-for="(error, index) in report.errors" :key="index" class="rounded-md border border-(--line-2) bg-(--bg-2) p-3">
          <p class="mb-2 text-[11px] font-medium text-destructive">Error {{ index + 1 }}</p>
          <pre class="select-text whitespace-pre-wrap break-words text-xs font-mono text-(--fg-1)">{{ error }}</pre>
        </li>
      </ol>
      <DialogFooter>
        <Button variant="outline" size="sm" @click="copyErrors">Copy all errors</Button>
        <Button variant="outline" size="sm" @click="emit('update:open', false)">Close</Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
<script setup lang="ts">
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { copyText } from '@/lib/clipboard'
import type { ImportReport } from '@/stores/progress'
const props = defineProps<{ open: boolean; report: ImportReport }>()
const emit = defineEmits<{ 'update:open': [value: boolean] }>()
function copyErrors() {
  const { database, fileName, executed, errors } = props.report
  void copyText([`Import: ${database}`, `File: ${fileName}`, ...(executed !== null ? [`Statements imported: ${executed}`] : []), '', ...errors.map((error, index) => `Error ${index + 1}\n${error}\n`)].join('\n'))
}
</script>
