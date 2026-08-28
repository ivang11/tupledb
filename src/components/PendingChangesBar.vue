<template>
  <div class="absolute bottom-4 left-1/2 -translate-x-1/2 z-50 flex items-center gap-4 bg-card border border-primary/20 shadow-2xl rounded-full px-5 py-2.5 animate-in fade-in slide-in-from-bottom-4 duration-300">
    <div class="flex items-center gap-3">
      <div class="size-2 rounded-full bg-amber-500 animate-pulse"></div>
      <span class="whitespace-nowrap text-xs font-bold uppercase tracking-wide text-foreground">
        {{ pendingLabel }}
      </span>
    </div>

    <div class="h-4 w-px bg-border"></div>

    <div class="flex items-center gap-2">
      <div v-if="hasDataChanges" class="flex items-center gap-2 mr-2">
        <label class="flex items-center gap-2 cursor-pointer group">
          <input
            type="checkbox"
            :checked="disableFkChecks"
            @change="emit('update:disableFkChecks', ($event.target as HTMLInputElement).checked)"
            class="size-3.5 rounded border-input accent-primary"
          />
          <span class="whitespace-nowrap text-[10px] font-bold text-muted-foreground group-hover:text-foreground transition-colors uppercase tracking-tight">
            FK Checks
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

<script setup lang="ts">
import { computed } from 'vue'
import { Button } from '@/components/ui/button'

const props = defineProps<{
  pendingTruncate: boolean
  pendingDrop: boolean
  pendingChangesCount: number
  pendingStructureChangesCount: number
  pendingDeletionsCount: number
  pendingInsertionsCount: number
  disableFkChecks: boolean
  isSaving: boolean
}>()

const emit = defineEmits<{
  'update:disableFkChecks': [val: boolean]
  'discard': []
  'apply': []
}>()

const pendingLabel = computed(() => {
  if (props.pendingDrop) return 'Drop pending'
  if (props.pendingTruncate) return 'Truncate pending'

  const parts: string[] = []
  if (props.pendingStructureChangesCount > 0) parts.push(`${props.pendingStructureChangesCount} schema change${props.pendingStructureChangesCount === 1 ? '' : 's'}`)
  if (props.pendingChangesCount > 0) parts.push(`${props.pendingChangesCount} update${props.pendingChangesCount === 1 ? '' : 's'}`)
  if (props.pendingDeletionsCount > 0) parts.push(`${props.pendingDeletionsCount} delete${props.pendingDeletionsCount === 1 ? '' : 's'}`)
  if (props.pendingInsertionsCount > 0) parts.push(`${props.pendingInsertionsCount} insert${props.pendingInsertionsCount === 1 ? '' : 's'}`)
  return parts.length ? `Pending: ${parts.join(' · ')}` : 'Pending changes'
})

const hasDataChanges = computed(() =>
  props.pendingTruncate ||
  props.pendingDrop ||
  props.pendingChangesCount > 0 ||
  props.pendingDeletionsCount > 0 ||
  props.pendingInsertionsCount > 0,
)
</script>
