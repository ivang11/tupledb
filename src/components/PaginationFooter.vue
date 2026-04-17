<script setup lang="ts">
import { computed } from 'vue'
import { LayoutListIcon, TablePropertiesIcon, PlusIcon, ChevronLeftIcon, ChevronRightIcon } from 'lucide-vue-next'

const props = defineProps<{
  viewMode: 'content' | 'structure'
  page: number
  pageSize: number
  totalCount: number
  isInsertingRow: boolean
  insertRowError: string | null
  insertRowLoading: boolean
}>()

const emit = defineEmits<{
  'set-view-mode': [mode: 'content' | 'structure']
  'toggle-insert-row': []
  'submit-insert-row': []
  'cancel-insert-row': []
  'change-page': [delta: number]
  'change-limit': [newLimit: number]
  'goto-offset': [offset: number]
}>()

const rowRangeLabel = computed(() => {
  if (props.totalCount === 0) return '0 rows'
  const from = props.page * props.pageSize + 1
  const to = Math.min((props.page + 1) * props.pageSize, props.totalCount)
  return `${from.toLocaleString()} – ${to.toLocaleString()} of ${props.totalCount.toLocaleString()}`
})

const canPrevPage = computed(() => props.page > 0)
const canNextPage = computed(() => (props.page + 1) * props.pageSize < props.totalCount)
</script>

<template>
  <footer
    class="shrink-0 border-t border-border flex items-center justify-between px-3 bg-background"
    style="height: 36px"
  >
    <!-- Left: view mode toggle + insert -->
    <div class="flex items-center gap-1.5">
      <div class="flex items-center rounded border border-border bg-muted/20 p-0.5 gap-px">
        <button
          type="button"
          @click="emit('set-view-mode', 'content')"
          :class="[
            'flex items-center gap-1 px-2 h-5 rounded text-[10px] font-bold uppercase tracking-wider transition-all',
            viewMode === 'content'
              ? 'bg-background text-foreground shadow-sm'
              : 'text-muted-foreground hover:text-foreground',
          ]"
        >
          <LayoutListIcon class="size-2.5" />
          Data
        </button>
        <button
          type="button"
          @click="emit('set-view-mode', 'structure')"
          :class="[
            'flex items-center gap-1 px-2 h-5 rounded text-[10px] font-bold uppercase tracking-wider transition-all',
            viewMode === 'structure'
              ? 'bg-background text-foreground shadow-sm'
              : 'text-muted-foreground hover:text-foreground',
          ]"
        >
          <TablePropertiesIcon class="size-2.5" />
          Structure
        </button>
      </div>

      <button
        v-if="viewMode === 'content'"
        type="button"
        :class="[
          'size-6 flex items-center justify-center rounded border transition-colors',
          isInsertingRow
            ? 'bg-emerald-500/15 border-emerald-500/30 text-emerald-500'
            : 'border-transparent text-muted-foreground/50 hover:border-border hover:bg-muted/30 hover:text-foreground',
        ]"
        title="Insert new row"
        @click="emit('toggle-insert-row')"
      >
        <PlusIcon class="size-3" />
      </button>
    </div>

    <!-- Right: insert hints or pagination -->
    <div v-if="isInsertingRow" class="flex items-center gap-2">
      <span
        v-if="insertRowError"
        class="text-[10px] text-destructive max-w-xs truncate cursor-help"
        :title="insertRowError ?? ''"
      >
        {{ insertRowError }}
      </span>
      <span v-else class="text-[10px] text-muted-foreground/50">
        Enter to confirm · Esc to cancel
      </span>
      <button
        class="h-6 px-3 rounded bg-emerald-600 hover:bg-emerald-700 text-white text-[10px] font-semibold transition-colors disabled:opacity-50"
        :disabled="insertRowLoading"
        @click="emit('submit-insert-row')"
      >
        {{ insertRowLoading ? '…' : 'Insert' }}
      </button>
      <button
        class="h-6 px-2 rounded text-[10px] text-muted-foreground hover:bg-muted/40 transition-colors"
        @click="emit('cancel-insert-row')"
      >
        Cancel
      </button>
    </div>

    <div v-else-if="viewMode === 'content'" class="flex items-center gap-3">
      <!-- Row count -->
      <span class="text-[11px] font-semibold text-muted-foreground tabular-nums">
        {{ rowRangeLabel }}
      </span>

      <!-- Limit + Offset controls -->
      <div class="flex items-center gap-2 border-l border-border pl-3">
        <div class="flex items-center gap-1">
          <span class="text-[10px] font-bold text-muted-foreground/50 uppercase tracking-wider">Limit</span>
          <input
            type="number"
            :value="pageSize"
            min="1"
            class="h-5 w-14 rounded border border-border/60 bg-transparent px-1.5 text-[10px] font-bold text-center focus:outline-none focus:ring-1 focus:ring-ring [appearance:textfield] [&::-webkit-outer-spin-button]:appearance-none [&::-webkit-inner-spin-button]:appearance-none"
            @change="emit('change-limit', +($event.target as HTMLInputElement).value)"
          />
        </div>
        <div class="flex items-center gap-1">
          <span class="text-[10px] font-bold text-muted-foreground/50 uppercase tracking-wider">Offset</span>
          <input
            type="number"
            :value="page * pageSize"
            min="0"
            class="h-5 w-14 rounded border border-border/60 bg-transparent px-1.5 text-[10px] font-bold text-center focus:outline-none focus:ring-1 focus:ring-ring [appearance:textfield] [&::-webkit-outer-spin-button]:appearance-none [&::-webkit-inner-spin-button]:appearance-none"
            @change="emit('goto-offset', +($event.target as HTMLInputElement).value)"
          />
        </div>
      </div>

      <!-- Page arrows -->
      <div class="flex items-center gap-1 border-l border-border pl-2">
        <button
          class="size-5 flex items-center justify-center rounded transition-colors disabled:opacity-20 text-muted-foreground hover:bg-muted/40 hover:text-foreground disabled:cursor-default"
          :disabled="!canPrevPage"
          @click="emit('change-page', -1)"
        >
          <ChevronLeftIcon class="size-3.5" />
        </button>
        <span class="text-[10px] font-bold text-muted-foreground/60 tabular-nums w-6 text-center">
          {{ page + 1 }}
        </span>
        <button
          class="size-5 flex items-center justify-center rounded transition-colors disabled:opacity-20 text-muted-foreground hover:bg-muted/40 hover:text-foreground disabled:cursor-default"
          :disabled="!canNextPage"
          @click="emit('change-page', 1)"
        >
          <ChevronRightIcon class="size-3.5" />
        </button>
      </div>
    </div>
  </footer>
</template>
