<script setup lang="ts">
import { ArrowRightIcon, KeyRoundIcon, CodeIcon } from 'lucide-vue-next'
import { ScrollArea } from '@/components/ui/scroll-area'

defineProps<{
  tableStructure: any[]
  tableIndexes: any[]
  fkMap: Record<string, { table: string; column: string }>
  ddl: string | null
  paneId: string
  indexPanelHeight: number | undefined
}>()

const emit = defineEmits<{
  'start-index-resize': [e: MouseEvent, paneId: string]
}>()
</script>

<template>
  <div class="flex flex-col flex-1 min-h-0">
    <!-- Columns -->
    <ScrollArea class="flex-1 min-h-0 bg-muted/5">
      <table class="w-max min-w-full border-collapse">
        <thead>
          <tr>
            <th class="sticky top-0 z-20 bg-background/95 backdrop-blur-md px-4 py-3 border-b border-r text-left text-[10px] font-black uppercase tracking-widest whitespace-nowrap">Field</th>
            <th class="sticky top-0 z-20 bg-background/95 backdrop-blur-md px-4 py-3 border-b border-r text-left text-[10px] font-black uppercase tracking-widest whitespace-nowrap">Type</th>
            <th class="sticky top-0 z-20 bg-background/95 backdrop-blur-md px-4 py-3 border-b border-r text-left text-[10px] font-black uppercase tracking-widest whitespace-nowrap">Nullable</th>
            <th class="sticky top-0 z-20 bg-background/95 backdrop-blur-md px-4 py-3 border-b border-r text-left text-[10px] font-black uppercase tracking-widest whitespace-nowrap">Key</th>
            <th class="sticky top-0 z-20 bg-background/95 backdrop-blur-md px-4 py-3 border-b border-r text-left text-[10px] font-black uppercase tracking-widest whitespace-nowrap">Default</th>
            <th class="sticky top-0 z-20 bg-background/95 backdrop-blur-md px-4 py-3 border-b border-r text-left text-[10px] font-black uppercase tracking-widest whitespace-nowrap">Extra</th>
            <th class="sticky top-0 z-20 bg-background/95 backdrop-blur-md px-4 py-3 border-b text-left text-[10px] font-black uppercase tracking-widest whitespace-nowrap">Relations</th>
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="(col, idx) in tableStructure"
            :key="col.field"
            class="hover:bg-primary/5 transition-colors"
            :class="idx % 2 === 0 ? 'bg-background/30' : 'bg-transparent'"
          >
            <td class="px-4 py-3 border-b border-r font-mono text-sm font-semibold text-foreground whitespace-nowrap">
              <div class="flex items-center gap-2">
                <span v-if="col.key === 'PRI'" class="text-[9px] font-black uppercase text-amber-400 bg-amber-400/10 px-1.5 py-0.5 rounded">PK</span>
                <span v-else-if="col.key === 'UNI'" class="text-[9px] font-black uppercase text-blue-400 bg-blue-400/10 px-1.5 py-0.5 rounded">UNI</span>
                <span v-else-if="col.key === 'MUL'" class="text-[9px] font-black uppercase text-purple-400 bg-purple-400/10 px-1.5 py-0.5 rounded">IDX</span>
                {{ col.field }}
              </div>
            </td>
            <td class="px-4 py-3 border-b border-r font-mono text-xs text-primary whitespace-nowrap">{{ col.field_type }}</td>
            <td class="px-4 py-3 border-b border-r text-sm">
              <span v-if="col.nullable" class="text-[10px] font-bold text-green-400 uppercase">YES</span>
              <span v-else class="text-[10px] font-bold text-muted-foreground/50 uppercase">NO</span>
            </td>
            <td class="px-4 py-3 border-b border-r text-xs text-muted-foreground whitespace-nowrap">{{ col.key || '—' }}</td>
            <td class="px-4 py-3 border-b border-r text-xs font-mono text-muted-foreground whitespace-nowrap">
              <span v-if="col.default_value === null" class="italic opacity-40">NULL</span>
              <span v-else>{{ col.default_value }}</span>
            </td>
            <td class="px-4 py-3 border-b border-r text-xs text-muted-foreground whitespace-nowrap">{{ col.extra || '—' }}</td>
            <td class="px-4 py-3 border-b text-xs whitespace-nowrap">
              <span v-if="fkMap[col.field]" class="flex items-center gap-1 text-primary/70 font-mono">
                <ArrowRightIcon class="size-3 shrink-0" />
                {{ fkMap[col.field].table }}.{{ fkMap[col.field].column }}
              </span>
              <span v-else class="text-muted-foreground/30">—</span>
            </td>
          </tr>
        </tbody>
      </table>
    </ScrollArea>

    <!-- Resize handle -->
    <div
      class="shrink-0 h-1 border-t cursor-row-resize hover:bg-primary/40 transition-colors bg-transparent"
      @mousedown="emit('start-index-resize', $event, paneId)"
    />

    <!-- Indexes -->
    <div
      class="shrink-0 flex flex-col"
      :style="{ height: (indexPanelHeight ?? 0) > 0 ? indexPanelHeight + 'px' : '40%' }"
    >
      <div class="px-4 py-2 bg-muted/20 border-b flex items-center gap-2 shrink-0">
        <KeyRoundIcon class="size-3.5 text-muted-foreground/60" />
        <span class="text-[10px] font-black uppercase tracking-widest text-muted-foreground/60">Indexes</span>
      </div>
      <div class="flex-1 min-h-0 overflow-auto">
        <table v-if="tableIndexes?.length" class="w-max min-w-full border-collapse">
          <thead>
            <tr>
              <th class="sticky top-0 z-20 bg-background/95 backdrop-blur-md px-4 py-3 border-b border-r text-left text-[10px] font-black uppercase tracking-widest whitespace-nowrap">Name</th>
              <th class="sticky top-0 z-20 bg-background/95 backdrop-blur-md px-4 py-3 border-b border-r text-left text-[10px] font-black uppercase tracking-widest whitespace-nowrap">Algorithm</th>
              <th class="sticky top-0 z-20 bg-background/95 backdrop-blur-md px-4 py-3 border-b border-r text-left text-[10px] font-black uppercase tracking-widest whitespace-nowrap">Unique</th>
              <th class="sticky top-0 z-20 bg-background/95 backdrop-blur-md px-4 py-3 border-b border-r text-left text-[10px] font-black uppercase tracking-widest whitespace-nowrap">Columns</th>
              <th class="sticky top-0 z-20 bg-background/95 backdrop-blur-md px-4 py-3 border-b text-left text-[10px] font-black uppercase tracking-widest whitespace-nowrap">Comment</th>
            </tr>
          </thead>
          <tbody>
            <template
              v-for="(group, keyName) in Object.groupBy(tableIndexes, (i: any) => i.key_name)"
              :key="keyName"
            >
              <tr class="hover:bg-primary/5 transition-colors">
                <td class="px-4 py-3 border-b border-r font-mono text-sm font-semibold text-foreground whitespace-nowrap">
                  <div class="flex items-center gap-2">
                    <span v-if="keyName === 'PRIMARY'" class="text-[9px] font-black uppercase text-amber-400 bg-amber-400/10 px-1.5 py-0.5 rounded">PK</span>
                    <KeyRoundIcon v-else class="size-3 text-muted-foreground/50" />
                    {{ keyName }}
                  </div>
                </td>
                <td class="px-4 py-3 border-b border-r text-xs font-mono text-primary whitespace-nowrap">
                  {{ (group as any[])[0]?.index_type }}
                </td>
                <td class="px-4 py-3 border-b border-r text-sm whitespace-nowrap">
                  <span v-if="!(group as any[])[0]?.non_unique" class="text-[10px] font-bold text-green-400 uppercase">YES</span>
                  <span v-else class="text-[10px] font-bold text-muted-foreground/50 uppercase">NO</span>
                </td>
                <td class="px-4 py-3 border-b border-r text-xs font-mono text-foreground/80 whitespace-nowrap">
                  {{ (group as any[]).sort((a: any, b: any) => a.seq_in_index - b.seq_in_index).map((i: any) => i.column_name).join(', ') }}
                </td>
                <td class="px-4 py-3 border-b text-xs text-muted-foreground whitespace-nowrap">
                  {{ (group as any[])[0]?.comment || '—' }}
                </td>
              </tr>
            </template>
          </tbody>
        </table>
        <div v-else class="px-4 py-6 text-xs text-muted-foreground/40 italic">No indexes found</div>
      </div>
    </div>

    <!-- DDL -->
    <div v-if="ddl" class="shrink-0 flex flex-col border-t" style="height: 200px">
      <div class="px-4 py-2 bg-muted/20 border-b flex items-center gap-2 shrink-0">
        <CodeIcon class="size-3.5 text-muted-foreground/60" />
        <span class="text-[10px] font-black uppercase tracking-widest text-muted-foreground/60">DDL</span>
      </div>
      <div class="flex-1 min-h-0 overflow-auto">
        <pre class="px-4 py-3 text-xs font-mono text-foreground/80 whitespace-pre leading-relaxed">{{ ddl }}</pre>
      </div>
    </div>
  </div>
</template>
