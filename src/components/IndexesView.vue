<template>
  <div class="flex flex-col flex-1 min-h-0">
    <ScrollArea class="flex-1 min-h-0">
      <div class="px-5 py-4">
        <div
          class="text-[10px] tracking-[0.14em] uppercase text-(--fg-5) font-semibold mb-3"
        >
          Indexes · {{ groups.length }}
        </div>
        <table v-if="groups.length" class="w-full border-collapse text-[12px]">
          <thead>
            <tr class="text-(--fg-5)">
              <th
                v-for="h in ['Name', 'Algorithm', 'Unique', 'Columns']"
                :key="h"
                class="text-left px-2.5 py-1.5 text-[10px] tracking-widest uppercase font-semibold border-b border-(--line-2)"
              >
                {{ h }}
              </th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="g in groups"
              :key="g.name"
              class="border-b border-(--line-faint) hover:bg-(--bg-1) transition-colors"
            >
              <td class="px-2.5 py-2 font-mono">
                <div class="flex items-center gap-2">
                  <span
                    v-if="g.name === 'PRIMARY'"
                    class="text-[9px] font-bold uppercase text-(--pk) bg-[oklch(0.78_0.14_80/0.18)] px-1.5 py-0.5 rounded"
                  >PK</span>
                  <KeyRoundIcon v-else class="size-3 text-(--fg-5)" />
                  <span class="text-(--fg-1)">{{ g.name }}</span>
                </div>
              </td>
              <td class="px-2.5 py-2 font-mono text-(--fg-3)">{{ g.algorithm }}</td>
              <td class="px-2.5 py-2">
                <span
                  v-if="g.unique"
                  class="text-[10px] font-bold uppercase text-(--acc)"
                >YES</span>
                <span v-else class="text-[10px] font-bold uppercase text-(--fg-5)">NO</span>
              </td>
              <td class="px-2.5 py-2 font-mono text-(--fg-2)">{{ g.columns }}</td>
            </tr>
          </tbody>
        </table>
        <div
          v-else-if="metadataLoading"
          class="px-2 py-4 text-xs text-(--fg-5) italic"
        >Loading metadata…</div>
        <div
          v-else
          class="px-2 py-4 text-xs text-(--fg-5) italic"
        >No indexes found</div>
      </div>
    </ScrollArea>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { KeyRoundIcon } from "lucide-vue-next";
import { ScrollArea } from "@/components/ui/scroll-area";

const props = defineProps<{
  tableIndexes: any[];
  metadataLoading?: boolean;
}>();

interface IndexGroup {
  name: string;
  algorithm: string;
  unique: boolean;
  columns: string;
  comment: string;
}

const groups = computed<IndexGroup[]>(() => {
  const map = new Map<string, any[]>();
  for (const idx of props.tableIndexes ?? []) {
    const k = idx.key_name;
    if (!map.has(k)) map.set(k, []);
    map.get(k)!.push(idx);
  }
  return Array.from(map.entries()).map(([name, rows]) => {
    rows.sort((a: any, b: any) => a.seq_in_index - b.seq_in_index);
    return {
      name,
      algorithm: rows[0]?.index_type ?? "",
      unique: !rows[0]?.non_unique,
      columns: rows.map((r: any) => r.column_name).join(", "),
      comment: rows[0]?.comment || "",
    };
  });
});
</script>
