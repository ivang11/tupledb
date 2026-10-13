<template>
  <div class="flex flex-col flex-1 min-h-0">
    <div class="flex h-7 shrink-0 items-center border-b border-(--line-2) bg-(--bg-1) px-3 text-[11px]">
      <span class="font-medium text-(--fg-2)">
        Indexes <span class="tabular-nums text-(--fg-4)">{{ groups.length }}</span>
      </span>
    </div>
    <ScrollArea class="flex-1 min-h-0">
      <table v-if="groups.length" class="structure-table w-full border-collapse text-[11.5px]">
        <thead>
          <tr>
            <th class="w-9 text-right">#</th>
            <th>Name</th>
            <th>Algorithm</th>
            <th class="w-16">Unique</th>
            <th>Columns</th>
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="(g, index) in groups"
            :key="g.name"
            class="hover:bg-(--bg-1) transition-colors"
          >
            <td class="gutter text-right tabular-nums">{{ index + 1 }}</td>
            <td class="font-mono text-(--fg-1)">
              <span class="inline-flex items-center gap-2">
                {{ g.name }}
                <span v-if="g.name === 'PRIMARY'" class="key-badge text-(--pk)" title="Primary key">PK</span>
                <KeyRoundIcon v-else class="size-3 text-(--fg-5)" />
              </span>
            </td>
            <td class="font-mono text-(--fg-3)">{{ g.algorithm }}</td>
            <td class="text-center">
              <CheckIcon v-if="g.unique" class="inline size-3 text-(--acc)" aria-label="Unique" />
              <span v-else class="text-(--fg-5)" aria-label="Not unique">—</span>
            </td>
            <td class="font-mono text-(--fg-2)">{{ g.columns }}</td>
          </tr>
        </tbody>
      </table>
      <div
        v-else-if="metadataLoading"
        class="px-3 py-4 text-[11.5px] text-(--fg-4)"
      >Loading metadata…</div>
      <div
        v-else
        class="px-3 py-4 text-[11.5px] text-(--fg-4)"
      >No indexes found</div>
    </ScrollArea>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { CheckIcon, KeyRoundIcon } from "lucide-vue-next";
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

<style scoped>
.structure-table th,
.structure-table td {
  height: 26px;
  padding: 0 10px;
  white-space: nowrap;
  border-bottom: 1px solid var(--line-faint);
  border-right: 1px solid var(--line-faint);
  text-align: left;
  vertical-align: middle;
}
.structure-table th:last-child,
.structure-table td:last-child {
  border-right: 0;
}
.structure-table th {
  position: sticky;
  top: 0;
  z-index: 1;
  height: 28px;
  font-size: 10.5px;
  font-weight: 500;
  color: var(--fg-3);
  background: var(--bg-1);
  border-bottom-color: var(--line-2);
}
.structure-table .gutter {
  font-size: 10.5px;
  color: var(--fg-4);
  background: var(--bg-1);
  border-right-color: var(--line-2);
}
.key-badge {
  font-family: var(--font-mono);
  font-size: 8px;
  font-weight: 700;
  letter-spacing: 0.04em;
}
</style>
