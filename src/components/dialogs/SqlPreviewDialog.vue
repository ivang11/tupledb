<template>
  <Dialog :open="open" @update:open="emit('update:open', $event)">
    <DialogContent class="sm:max-w-3xl max-h-[85vh] flex flex-col">
      <DialogHeader><DialogTitle>Review pending SQL</DialogTitle><DialogDescription>Preview of pending operations. Values are shown separately as parameters. The database driver validates types and manages transactions when you apply changes.</DialogDescription></DialogHeader>
      <pre class="overflow-auto whitespace-pre-wrap break-words rounded border p-4 text-xs font-mono select-text">{{ sql }}</pre>
      <DialogFooter><Button variant="outline" @click="copyText(sql)">Copy preview</Button><Button variant="outline" @click="emit('update:open', false)">Keep editing</Button><Button :disabled="saving" @click="emit('apply')">Apply changes</Button></DialogFooter>
    </DialogContent>
  </Dialog>
</template>
<script setup lang="ts">
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { copyText } from '@/lib/clipboard'
defineProps<{ open: boolean; sql: string; saving: boolean }>()
const emit = defineEmits<{ 'update:open': [value: boolean]; apply: [] }>()
</script>
