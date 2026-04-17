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
  showFkOption?: boolean
  disableFkChecks?: boolean
}>()

const emit = defineEmits<{
  'update:open': [val: boolean]
  'update:disableFkChecks': [val: boolean]
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

      <div v-if="showFkOption" class="px-1 py-2">
        <label class="flex items-center gap-2.5 cursor-pointer group">
          <input
            type="checkbox"
            :checked="disableFkChecks"
            @change="emit('update:disableFkChecks', ($event.target as HTMLInputElement).checked)"
            class="size-3.5 rounded border-input accent-primary"
          />
          <span class="text-xs text-muted-foreground group-hover:text-foreground transition-colors">
            Disable foreign key checks
          </span>
        </label>
        <p class="text-[10px] text-muted-foreground/50 mt-1.5 ml-6">
          Use this if the delete fails due to FK constraints
        </p>
      </div>

      <DialogFooter class="gap-2 sm:gap-0">
        <Button variant="ghost" @click="emit('update:open', false)">Cancel</Button>
        <Button variant="destructive" @click="emit('confirm')">Delete</Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
