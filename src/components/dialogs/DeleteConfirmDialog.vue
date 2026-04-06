<script setup lang="ts">
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from '@/components/ui/dialog'

defineProps<{
  open: boolean
  title?: string
  description?: string
}>()

const emit = defineEmits<{
  'update:open': [val: boolean]
  'confirm': []
}>()
</script>

<template>
  <Dialog :open="open" @update:open="(val) => emit('update:open', val)">
    <DialogContent class="sm:max-w-[400px]">
      <DialogHeader>
        <DialogTitle>{{ title ?? 'Delete' }}</DialogTitle>
        <DialogDescription>
          {{ description ?? 'Are you sure? This action cannot be undone.' }}
        </DialogDescription>
      </DialogHeader>
      <DialogFooter class="gap-2 sm:gap-0">
        <Button variant="ghost" @click="emit('update:open', false)">Cancel</Button>
        <Button variant="destructive" @click="emit('confirm')">Delete</Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
