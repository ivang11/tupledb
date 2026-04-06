<script setup lang="ts">
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
} from '@/components/ui/dialog'

defineProps<{
  result: { success: boolean; message: string } | null
}>()

const emit = defineEmits<{
  'close': []
}>()
</script>

<template>
  <Dialog :open="!!result" @update:open="(val) => !val && emit('close')">
    <DialogContent class="sm:max-w-md">
      <DialogHeader>
        <DialogTitle :class="result?.success ? 'text-green-500' : 'text-destructive'">
          {{ result?.success ? 'Export successful' : 'Export failed' }}
        </DialogTitle>
        <DialogDescription>{{ result?.message }}</DialogDescription>
      </DialogHeader>
      <div class="flex justify-end mt-4">
        <Button @click="emit('close')">Close</Button>
      </div>
    </DialogContent>
  </Dialog>
</template>
