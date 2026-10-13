<template>
  <div class="shrink-0 bg-(--bg-0) border-b border-(--line-2) flex items-center px-1.5 h-7 gap-0.5">
    <button
      v-for="tab in SUB_TABS"
      :key="tab.id"
      class="relative h-full px-2 text-[11.5px] font-medium transition-colors cursor-pointer"
      :class="activeMode === tab.id ? 'text-(--fg-1)' : 'text-(--fg-3) hover:text-(--fg-1)'"
      @click="emit('set-mode', tab.id)"
    >
      <span class="inline-flex items-center gap-1">
        {{ tab.label }}
        <span
          v-if="badgeCount(tab.id) != null"
          class="tabular-nums text-[10px] text-(--fg-4)"
        >{{ badgeCount(tab.id) }}</span>
      </span>
      <span
        v-if="activeMode === tab.id"
        class="absolute inset-x-1.5 -bottom-px h-[2px] bg-(--acc)"
      />
    </button>

    <div class="flex-1" />

    <button
      v-if="showRowDetailToggle"
      class="h-[22px] px-1.5 inline-flex items-center gap-1 rounded-[4px] text-[11px] transition-colors"
      :class="rowDetailOnClick ? 'text-(--acc) bg-(--acc-soft)' : 'text-(--fg-2) hover:bg-(--bg-2) hover:text-(--fg-1)'"
      :title="rowDetailOnClick ? 'Disable row detail on click' : 'Enable row detail on click'"
      @click="emit('toggle-row-detail')"
    >
      <PanelRightIcon class="size-3.5" /> Detail
    </button>

    <button
      v-if="canInsertRow"
      class="h-[22px] px-1.5 inline-flex items-center gap-1 rounded-[4px] text-[11px] text-(--fg-2) hover:bg-(--bg-2) hover:text-(--fg-1) transition-colors"
      title="Insert new row"
      @click="emit('insert-row')"
    >
      <PlusIcon class="size-3.5" /> Row
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
