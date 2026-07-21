<template>
  <div class="shrink-0 bg-(--bg-0) border-b border-(--line-2) flex items-center px-2 h-9">
    <button
      v-for="tab in SUB_TABS"
      :key="tab.id"
      class="relative px-3 h-full text-[12px] font-medium text-(--fg-1) transition-colors cursor-pointer"
      @click="emit('set-mode', tab.id)"
    >
      <span class="inline-flex items-center gap-1.5">
        {{ tab.label }}
        <span
          v-if="badgeCount(tab.id) != null"
          class="font-mono text-[10px] text-(--fg-1)"
        >{{ badgeCount(tab.id) }}</span>
      </span>
      <span
        v-if="activeMode === tab.id"
        class="absolute inset-x-2 bottom-0 h-0.5 bg-(--acc) rounded-full"
      />
    </button>

    <div class="flex-1" />

    <button
      v-if="showRowDetailToggle"
      class="h-6 px-2 inline-flex items-center gap-1 rounded text-[11px] transition-colors"
      :class="rowDetailOnClick ? 'text-(--acc) bg-(--acc-soft)' : 'text-(--fg-1) hover:bg-(--bg-2)'"
      :title="rowDetailOnClick ? 'Disable row detail on click' : 'Enable row detail on click'"
      @click="emit('toggle-row-detail')"
    >
      <PanelRightIcon class="size-4" /> Detail
    </button>

    <button
      v-if="canInsertRow"
      class="h-6 px-2 inline-flex items-center gap-1 rounded text-[11px] text-(--fg-1) hover:bg-(--bg-2) transition-colors"
      title="Insert new row"
      @click="emit('insert-row')"
    >
      <PlusIcon class="size-4" /> Row
    </button>
  </div>
</template>

<script setup lang="ts">
import { PanelRightIcon, PlusIcon } from "lucide-vue-next";
import type { TableViewMode } from "@/types/workspace";

const props = defineProps<{
  activeMode: TableViewMode;
  indexCount?: number | null;
  canInsertRow?: boolean;
  showRowDetailToggle?: boolean;
  rowDetailOnClick?: boolean;
}>();

const emit = defineEmits<{
  "set-mode": [mode: TableViewMode];
  "insert-row": [];
  "toggle-row-detail": [];
}>();

const SUB_TABS: { id: TableViewMode; label: string }[] = [
  { id: "content", label: "Data" },
  { id: "structure", label: "Structure" },
  { id: "indexes", label: "Indexes" },
];


function badgeCount(mode: TableViewMode): number | null {
  if (mode === "indexes") return props.indexCount ?? null;
  return null;
}
</script>
