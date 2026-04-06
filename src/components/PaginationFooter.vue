<script setup lang="ts">
import { LayoutListIcon, TablePropertiesIcon, PlusIcon, ChevronLeftIcon, ChevronRightIcon } from 'lucide-vue-next'
import { Button } from '@/components/ui/button'

defineProps<{
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
</script>

<template>
  <footer class="h-12 border-t flex items-center justify-between px-6 bg-background shrink-0">
    <!-- Left: view mode + insert row toggle -->
    <div class="flex items-center gap-2">
      <div class="flex items-center rounded border bg-muted/30 p-0.5 gap-0.5">
        <button
          type="button"
          @click="emit('set-view-mode', 'content')"
          :class="['flex items-center gap-1 px-2 h-6 rounded text-[10px] font-bold uppercase tracking-wider transition-all', viewMode === 'content' ? 'bg-background text-foreground shadow-sm' : 'text-muted-foreground hover:text-foreground']"
        >
          <LayoutListIcon class="size-3" /> Data
        </button>
        <button
          type="button"
          @click="emit('set-view-mode', 'structure')"
          :class="['flex items-center gap-1 px-2 h-6 rounded text-[10px] font-bold uppercase tracking-wider transition-all', viewMode === 'structure' ? 'bg-background text-foreground shadow-sm' : 'text-muted-foreground hover:text-foreground']"
        >
          <TablePropertiesIcon class="size-3" /> Structure
        </button>
      </div>
      <button
        type="button"
        :class="['size-6 flex items-center justify-center rounded border transition-colors', isInsertingRow ? 'bg-emerald-500/15 border-emerald-500/40 text-emerald-500' : 'border-transparent text-muted-foreground hover:border-border hover:bg-muted/30 hover:text-foreground']"
        title="Insert new row"
        @click="emit('toggle-insert-row')"
      >
        <PlusIcon class="size-3.5" />
      </button>
    </div>

    <!-- Right: insert hints OR pagination -->
    <div v-if="isInsertingRow" class="flex items-center gap-2">
      <span v-if="insertRowError" class="text-[10px] text-destructive max-w-xs truncate cursor-help" :title="insertRowError ?? ''">{{ insertRowError }}</span>
      <span v-else class="text-[10px] text-muted-foreground">Enter · Esc to cancel</span>
      <Button size="sm" class="h-6 text-[10px] px-3 bg-emerald-600 hover:bg-emerald-700" :disabled="insertRowLoading" @click="emit('submit-insert-row')">
        {{ insertRowLoading ? '...' : 'Insert' }}
      </Button>
      <Button size="sm" variant="ghost" class="h-6 text-[10px] px-2" @click="emit('cancel-insert-row')">Cancel</Button>
    </div>

    <div v-else-if="viewMode === 'content'" class="flex items-center gap-3">
      <div class="text-[11px] font-bold text-muted-foreground uppercase tracking-wider">
        {{ page * pageSize + 1 }} - {{ Math.min((page + 1) * pageSize, totalCount) }} of {{ totalCount }} rows
      </div>
      <div class="flex items-center gap-3">
        <div class="flex items-center gap-1.5">
          <span class="text-[10px] font-bold text-muted-foreground uppercase tracking-wider">Limit</span>
          <input
            type="number"
            :value="pageSize"
            min="1"
            class="h-6 w-16 rounded border border-input bg-transparent px-2 text-[11px] font-bold text-center focus:outline-none focus:ring-1 focus:ring-ring [appearance:textfield] [&::-webkit-outer-spin-button]:appearance-none [&::-webkit-inner-spin-button]:appearance-none"
            @change="emit('change-limit', +($event.target as HTMLInputElement).value)"
          />
        </div>
        <div class="flex items-center gap-1.5">
          <span class="text-[10px] font-bold text-muted-foreground uppercase tracking-wider">Offset</span>
          <input
            type="number"
            :value="page * pageSize"
            min="0"
            class="h-6 w-16 rounded border border-input bg-transparent px-2 text-[11px] font-bold text-center focus:outline-none focus:ring-1 focus:ring-ring [appearance:textfield] [&::-webkit-outer-spin-button]:appearance-none [&::-webkit-inner-spin-button]:appearance-none"
            @change="emit('goto-offset', +($event.target as HTMLInputElement).value)"
          />
        </div>
        <div class="flex items-center gap-1">
          <Button variant="ghost" size="sm" class="h-7 w-7 p-0" :disabled="page === 0" @click="emit('change-page', -1)">
            <ChevronLeftIcon class="size-4" />
          </Button>
          <div class="text-[10px] font-bold px-1">{{ page + 1 }}</div>
          <Button variant="ghost" size="sm" class="h-7 w-7 p-0" :disabled="(page + 1) * pageSize >= totalCount" @click="emit('change-page', 1)">
            <ChevronRightIcon class="size-4" />
          </Button>
        </div>
      </div>
    </div>
  </footer>
</template>
