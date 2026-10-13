<template>
  <Dialog :open="open" @update:open="emit('update:open', $event)">
    <DialogContent class="sm:max-w-2xl gap-3 select-none" @open-auto-focus.prevent="input?.focus()">
      <DialogHeader><DialogTitle>Quick search</DialogTitle><DialogDescription>Find connections, databases, loaded tables and saved queries.</DialogDescription></DialogHeader>
      <input ref="input" v-model="search" type="search" aria-label="Quick search" placeholder="Search by name or database…" class="w-full h-10 border rounded px-3 bg-background text-sm select-text focus:outline-none focus:ring-1 focus:ring-primary" @keydown="onKeydown" />
      <div ref="list" class="max-h-80 overflow-auto" role="listbox" aria-label="Search results">
        <button v-for="(item, index) in filtered" :key="item.id" :data-index="index" role="option" :aria-selected="index === highlighted" :disabled="busy" tabindex="-1" @mousedown.prevent
          class="w-full flex items-center gap-3 text-left px-3 py-2 rounded hover:bg-muted disabled:opacity-50" :class="index === highlighted ? 'bg-muted' : ''" @click="emit('select', item)">
          <span class="w-20 shrink-0 text-[10px] text-muted-foreground uppercase">{{ item.kind }}</span><div class="min-w-0"><div class="text-sm truncate">{{ item.label }}</div><div class="text-xs text-muted-foreground truncate">{{ item.detail }}</div></div>
        </button>
        <p v-if="!filtered.length" class="p-4 text-sm text-muted-foreground">No matches. Select a database to load its tables.</p>
      </div>
      <p class="text-[11px] text-muted-foreground">↑ ↓ to navigate · Enter to open · Escape to close{{ busy ? ' · Opening…' : '' }}</p>
    </DialogContent>
  </Dialog>
</template>
<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription } from '@/components/ui/dialog'
export interface QuickSearchItem { id: string; kind: string; label: string; detail: string; connectionId: string; database?: string | null; table?: string; sql?: string }
const props = defineProps<{ open: boolean; items: QuickSearchItem[]; busy: boolean }>()
const emit = defineEmits<{ 'update:open': [value: boolean]; select: [item: QuickSearchItem] }>()
const input = ref<HTMLInputElement | null>(null), list = ref<HTMLElement | null>(null)
const search = ref(''), highlighted = ref(0)
const filtered = computed(() => {
  const words = search.value.trim().toLowerCase().split(/\s+/)
  return props.items.filter(item => words.every(word => `${item.label} ${item.detail} ${item.kind}`.toLowerCase().includes(word))).slice(0, 100)
})
watch(search, () => { highlighted.value = 0 })
watch(() => props.open, async open => { if (open) { search.value = ''; highlighted.value = 0; await nextTick(); input.value?.focus() } })
function onKeydown(event: KeyboardEvent) {
  if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
    event.preventDefault(); highlighted.value = Math.max(0, Math.min(filtered.value.length - 1, highlighted.value + (event.key === 'ArrowDown' ? 1 : -1)))
    void nextTick(() => list.value?.querySelector(`[data-index="${highlighted.value}"]`)?.scrollIntoView?.({ block: 'nearest' }))
  } else if (event.key === 'Enter' && !props.busy) {
    event.preventDefault(); const item = filtered.value[highlighted.value]; if (item) emit('select', item)
  }
}
</script>
