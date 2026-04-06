<script setup lang="ts">
import { Button } from '@/components/ui/button'

defineProps<{
  pendingTruncate: boolean
  pendingChangesCount: number
  pendingDeletionsCount: number
  disableFkChecks: boolean
  isSaving: boolean
}>()

const emit = defineEmits<{
  'update:disableFkChecks': [val: boolean]
  'discard': []
  'apply': []
}>()
</script>

<template>
  <div class="fixed bottom-6 left-1/2 -translate-x-1/2 z-50 flex items-center gap-6 bg-card border border-primary/20 shadow-2xl rounded-full px-6 py-3 animate-in fade-in slide-in-from-bottom-4 duration-300">
    <div class="flex items-center gap-3">
      <div class="size-2 rounded-full bg-amber-500 animate-pulse"></div>
      <span class="text-xs font-bold uppercase tracking-widest text-foreground">
        <template v-if="pendingTruncate">Entire Table marked for Truncate</template>
        <template v-else>{{ pendingChangesCount }} Updates &amp; {{ pendingDeletionsCount }} Deletions Pending</template>
      </span>
    </div>

    <div class="h-4 w-px bg-border"></div>

    <div class="flex items-center gap-2">
      <div class="flex items-center gap-2 mr-2">
        <label class="flex items-center gap-2 cursor-pointer group">
          <input
            type="checkbox"
            :checked="disableFkChecks"
            @change="emit('update:disableFkChecks', ($event.target as HTMLInputElement).checked)"
            class="size-3.5 rounded border-input accent-primary"
          />
          <span class="text-[10px] font-bold text-muted-foreground group-hover:text-foreground transition-colors uppercase tracking-tight">
            Disable FK Checks
          </span>
        </label>
      </div>
      <Button variant="ghost" size="sm" class="h-8 text-xs font-bold uppercase tracking-tight" @click="emit('discard')">Discard</Button>
      <Button size="sm" class="h-8 px-4 text-xs font-bold uppercase tracking-tight shadow-lg" :disabled="isSaving" @click="emit('apply')">
        {{ isSaving ? 'Saving...' : 'Apply Changes' }}
      </Button>
    </div>
  </div>
</template>
