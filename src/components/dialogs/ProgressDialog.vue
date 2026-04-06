<script setup lang="ts">
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
} from '@/components/ui/dialog'

defineProps<{
  open: boolean
  title: string
  description: string
  progress: { current: number; total: number; status: string }
}>()
</script>

<template>
  <Dialog :open="open">
    <DialogContent class="sm:max-w-md" :hide-close="true">
      <DialogHeader>
        <DialogTitle>{{ title }}</DialogTitle>
        <DialogDescription>{{ description }}</DialogDescription>
      </DialogHeader>
      <div class="py-6">
        <div class="flex items-center justify-between mb-2 text-xs font-bold uppercase tracking-widest text-muted-foreground">
          <span>{{ progress.status }}</span>
          <span v-if="progress.total">{{ Math.round((progress.current / progress.total) * 100) }}%</span>
        </div>
        <div class="h-2 w-full bg-muted rounded-full overflow-hidden">
          <div
            class="h-full bg-primary transition-all duration-300 ease-out"
            :style="{ width: `${progress.total ? (progress.current / progress.total) * 100 : 0}%` }"
          />
        </div>
        <div class="mt-2 text-[10px] text-muted-foreground text-center">
          {{ progress.current }} / {{ progress.total }} statements
        </div>
      </div>
    </DialogContent>
  </Dialog>
</template>
