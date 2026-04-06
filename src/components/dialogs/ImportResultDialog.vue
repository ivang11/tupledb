<script setup lang="ts">
import { Button } from '@/components/ui/button'
import { ScrollArea } from '@/components/ui/scroll-area'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
} from '@/components/ui/dialog'

const props = defineProps<{
  result: { executed: number; errors: string[] } | null
}>()

const emit = defineEmits<{
  'close': []
}>()
</script>

<template>
  <Dialog :open="!!result" @update:open="(val) => !val && emit('close')">
    <DialogContent class="sm:max-w-lg">
      <DialogHeader>
        <DialogTitle :class="result?.errors.length ? 'text-destructive' : 'text-green-500'">
          {{ result?.errors.length ? 'Import finished with errors' : 'Import successful' }}
        </DialogTitle>
        <DialogDescription>{{ result?.executed }} statements executed successfully.</DialogDescription>
      </DialogHeader>
      <ScrollArea v-if="result?.errors.length" class="mt-4 max-h-[300px] rounded-md border bg-muted/30 p-4">
        <div class="text-[10px] font-black uppercase tracking-widest text-destructive mb-2">Error Log:</div>
        <div
          v-for="(err, i) in result.errors"
          :key="i"
          class="text-xs font-mono mb-2 last:mb-0 break-all border-b border-muted last:border-0 pb-2"
        >{{ err }}</div>
      </ScrollArea>
      <div class="flex justify-end mt-4">
        <Button @click="emit('close')">Close</Button>
      </div>
    </DialogContent>
  </Dialog>
</template>
