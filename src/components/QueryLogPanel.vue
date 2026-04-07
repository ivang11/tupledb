<script setup lang="ts">
import { ref, watch, nextTick } from 'vue'
import { TerminalIcon, TrashIcon, ChevronUpIcon, ChevronDownIcon } from 'lucide-vue-next'
import { useQueryLogStore } from '@/stores/queryLog'

const store = useQueryLogStore()

const scrollEl = ref<HTMLElement | null>(null)
const autoScroll = ref(true)

watch(
  () => store.entries.length,
  async () => {
    if (autoScroll.value && store.isOpen) {
      await nextTick()
      scrollEl.value?.scrollTo({ top: scrollEl.value.scrollHeight, behavior: 'instant' })
    }
  }
)

watch(
  () => store.isOpen,
  async (open) => {
    if (open) {
      await nextTick()
      scrollEl.value?.scrollTo({ top: scrollEl.value.scrollHeight, behavior: 'instant' })
    }
  }
)

function onScroll() {
  if (!scrollEl.value) return
  const { scrollTop, scrollHeight, clientHeight } = scrollEl.value
  autoScroll.value = scrollHeight - scrollTop - clientHeight < 40
}
</script>

<template>
  <div class="flex flex-col border-t border-border bg-background" :class="store.isOpen ? 'h-48' : 'h-7'">
    <!-- Header bar (always visible) -->
    <div
      class="flex items-center gap-2 px-3 h-7 shrink-0 cursor-pointer select-none bg-muted/40 hover:bg-muted/60 transition-colors"
      @click="store.toggle()"
    >
      <TerminalIcon class="w-3.5 h-3.5 text-muted-foreground shrink-0" />
      <span class="text-xs font-medium text-muted-foreground flex-1">Query Log</span>
      <span class="text-xs text-muted-foreground/60 tabular-nums">{{ store.entries.length }}</span>
      <button
        v-if="store.isOpen && store.entries.length > 0"
        class="p-0.5 rounded hover:bg-accent text-muted-foreground hover:text-foreground"
        title="Clear log"
        @click.stop="store.clear()"
      >
        <TrashIcon class="w-3 h-3" />
      </button>
      <ChevronUpIcon v-if="store.isOpen" class="w-3.5 h-3.5 text-muted-foreground" />
      <ChevronDownIcon v-else class="w-3.5 h-3.5 text-muted-foreground" />
    </div>

    <!-- Log entries -->
    <div
      v-if="store.isOpen"
      ref="scrollEl"
      class="flex-1 overflow-y-auto font-mono text-xs leading-relaxed px-3 py-1.5 space-y-2 bg-[#0d1117]"
      @scroll="onScroll"
    >
      <div v-if="store.entries.length === 0" class="text-muted-foreground/50 italic py-2">
        No queries yet...
      </div>
      <div v-for="entry in store.entries" :key="entry.id" class="group">
        <div class="text-[#58a6ff] text-[10px] mb-0.5 tabular-nums">
          --{{ entry.timestamp }}
          <span class="text-muted-foreground/50 ml-2">({{ entry.duration_ms }}ms)</span>
        </div>
        <div :class="entry.error ? 'text-red-400' : 'text-[#79c0ff]'" class="whitespace-pre-wrap break-all">{{ entry.sql }}</div>
        <div v-if="entry.error" class="text-red-400/70 text-[10px] mt-0.5">{{ entry.error }}</div>
      </div>
    </div>
  </div>
</template>
