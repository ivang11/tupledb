<template>
  <div class="absolute bottom-3 left-1/2 -translate-x-1/2 z-50 flex items-center gap-3 bg-popover border border-(--line-1) shadow-xl rounded-lg px-3 py-1.5 animate-in fade-in slide-in-from-bottom-2 duration-200">
    <div class="flex items-center gap-2">
      <div class="size-1.5 rounded-full bg-amber-400"></div>
      <span class="whitespace-nowrap text-[11.5px] font-medium text-foreground">
        {{ pendingLabel }}
      </span>
    </div>

    <div class="h-3.5 w-px bg-(--line-2)"></div>

    <div class="flex items-center gap-1.5">
      <div v-if="hasDataChanges" class="flex items-center gap-2 mr-1">
        <label v-if="supportsFkChecks !== false" class="flex items-center gap-1.5 cursor-pointer group">
          <input
            type="checkbox"
            :checked="disableFkChecks"
            @change="emit('update:disableFkChecks', ($event.target as HTMLInputElement).checked)"
            class="size-3 rounded border-input accent-primary"
          />
          <span class="whitespace-nowrap text-[11px] text-muted-foreground group-hover:text-foreground transition-colors">
            Skip FK checks
          </span>
        </label>
      </div>
      <Button variant="outline" size="sm" class="h-7 px-2.5 text-[11.5px]" @click="emit('preview')">Review SQL</Button>
      <Button variant="ghost" size="sm" class="h-7 px-2.5 text-[11.5px]" @click="emit('discard')">Discard</Button>
      <Button size="sm" class="h-7 px-3 text-[11.5px] font-semibold" :disabled="isSaving" @click="emit('apply')">
        {{ isSaving ? 'Saving…' : 'Apply' }}
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
  supportsFkChecks?: boolean
  isSaving: boolean
}>()

const emit = defineEmits<{
  'update:disableFkChecks': [val: boolean]
  'discard': []
  'apply': []
  'preview': []
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
