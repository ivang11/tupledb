<template>
  <Dialog :open="open" @update:open="emit('update:open', $event)">
    <DialogContent class="sm:max-w-2xl"><KeyboardShortcutsPanel /></DialogContent>
  </Dialog>
</template>
<script setup lang="ts">
import { Dialog, DialogContent } from '@/components/ui/dialog'
import KeyboardShortcutsPanel from '@/components/KeyboardShortcutsPanel.vue'
defineProps<{ open: boolean }>()
const emit = defineEmits<{ 'update:open': [value: boolean] }>()
</script>
