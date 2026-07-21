<template>
  <div class="flex flex-col flex-1 min-h-0">
    <ScrollArea class="flex-1 min-h-0">
      <div class="px-5 py-4">
        <div
          class="text-[10px] tracking-[0.14em] uppercase text-(--fg-5) font-semibold mb-3"
        >Columns · {{ tableStructure.length }}</div>

        <table v-if="tableStructure.length" class="w-full border-collapse text-[12px]">
          <thead>
            <tr class="text-(--fg-5)">
              <th
                v-for="h in ['', 'Name', 'Type', 'Nullable', 'Default', 'Extra', 'Foreign key']"
                :key="h"
                class="text-left px-2.5 py-1.5 text-[10px] tracking-widest uppercase font-semibold border-b border-(--line-2)"
              >{{ h }}</th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="col in tableStructure"
              :key="col.field"
              class="border-b border-(--line-faint) hover:bg-(--bg-1) transition-colors"
            >
              <td class="px-2.5 py-2 w-7">
                <span
                  v-if="col.key === 'PRI'"
                  class="inline-flex items-center px-1.5 py-0.5 rounded text-[9px] font-bold uppercase text-(--pk) bg-[oklch(0.78_0.14_80/0.18)]"
                >PK</span>
                <span
                  v-else-if="col.key === 'UNI'"
                  class="inline-flex items-center px-1.5 py-0.5 rounded text-[9px] font-bold uppercase text-(--env-staging) bg-[oklch(0.75_0.14_250/0.18)]"
                >UNI</span>
                <span
                  v-else-if="col.key === 'MUL'"
                  class="inline-flex items-center px-1.5 py-0.5 rounded text-[9px] font-bold uppercase text-(--fg-4) bg-(--bg-2)"
                >IDX</span>
              </td>
              <td class="px-2.5 py-2 font-mono text-(--fg-1)">{{ col.field }}</td>
              <td class="px-2.5 py-2 font-mono text-(--fg-3)">{{ col.field_type }}</td>
              <td class="px-2.5 py-2 font-mono">
                <span
                  v-if="col.nullable"
                  class="text-[10px] font-bold uppercase text-(--fg-2)"
                >YES</span>
                <span v-else class="text-[10px] font-bold uppercase text-(--fg-5)">NO</span>
              </td>
              <td class="px-2.5 py-2 font-mono text-(--fg-4)">
                <span v-if="col.default_value === null" class="italic">NULL</span>
                <span v-else>{{ col.default_value }}</span>
              </td>
              <td class="px-2.5 py-2 font-mono text-(--fg-4)">{{ col.extra || "—" }}</td>
              <td class="px-2.5 py-2 font-mono">
                <span
                  v-if="fkMap[col.field]"
                  class="inline-flex items-center gap-1 text-(--acc)"
                >
                  <ArrowRightIcon class="size-3 shrink-0" />
                  {{ fkMap[col.field].table }}.{{ fkMap[col.field].column }}
                </span>
                <span v-else class="text-(--fg-5)">—</span>
              </td>
            </tr>
          </tbody>
        </table>
        <div
          v-else-if="metadataLoading"
          class="px-2 py-4 text-xs text-(--fg-5) italic"
        >Loading metadata…</div>

        <div v-if="ddl" class="mt-8">
          <div
            class="text-[10px] tracking-[0.14em] uppercase text-(--fg-5) font-semibold mb-3 flex items-center gap-2"
          >
            <CodeIcon class="size-3" />
            DDL
          </div>
          <pre
            class="font-mono text-[11.5px] leading-relaxed text-(--fg-2) bg-(--bg-1) border border-(--line-2) rounded-md px-3.5 py-3 overflow-auto whitespace-pre"
          >{{ ddl }}</pre>
        </div>
        <div
          v-else-if="metadataLoading"
          class="mt-6 text-xs text-(--fg-5) italic"
        >Loading DDL…</div>
      </div>
    </ScrollArea>
  </div>
</template>

<script setup lang="ts">
import { ArrowRightIcon, CodeIcon } from "lucide-vue-next";
import { ScrollArea } from "@/components/ui/scroll-area";

defineProps<{
  tableStructure: any[];
  tableIndexes: any[];
  fkMap: Record<string, { table: string; column: string }>;
  ddl: string | null;
  metadataLoading?: boolean;
  metadataLoaded?: boolean;
  paneId: string;
  indexPanelHeight: number | undefined;
}>();

const emit = defineEmits<{
  "start-index-resize": [e: MouseEvent, paneId: string];
}>();

// emit is exported so existing parent bindings still work even though
// indexes have moved to their own tab.
void emit;
</script>
