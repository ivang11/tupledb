<template>
  <Dialog :open="open" @update:open="!$event && emit('cancel')">
    <DialogContent><DialogHeader><DialogTitle>Discard pending changes?</DialogTitle><DialogDescription>These changes have not been applied to the database. Keep working or discard them to continue.</DialogDescription></DialogHeader>
      <p class="text-sm break-words">{{ labels.join(' · ') }}</p>
      <DialogFooter><Button variant="outline" @click="emit('cancel')">Keep working</Button><Button variant="destructive" @click="emit('discard')">Discard and continue</Button></DialogFooter>
    </DialogContent>
  </Dialog>
</template>
<script setup lang="ts">
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
defineProps<{ open: boolean; labels: string[] }>()
const emit = defineEmits<{ cancel: []; discard: [] }>()
</script>
