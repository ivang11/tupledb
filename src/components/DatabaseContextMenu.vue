<template>
  <div
    v-if="show"
    class="fixed z-100 min-w-40 bg-background/95 backdrop-blur-md border rounded-lg shadow-xl p-1 animate-in fade-in zoom-in-95 duration-100"
    :style="{ left: x + 'px', top: y + 'px' }"
  >
    <button
      class="w-full flex items-center gap-2 px-3 py-2 text-xs font-medium rounded-md hover:bg-primary/10 text-foreground transition-colors text-left"
      title="New Query"
      @click="emit('open-query')"
    >
      <TerminalIcon class="size-3.5" /> New Query
    </button>
    <button
      class="w-full flex items-center gap-2 px-3 py-2 text-xs font-medium rounded-md hover:bg-primary/10 text-foreground transition-colors text-left"
      title="Open in Split"
      @click="emit('open-in-split')"
    >
      <PanelRightOpenIcon class="size-3.5" /> Open in Split
    </button>
    <button
      class="w-full flex items-center gap-2 px-3 py-2 text-xs font-medium rounded-md hover:bg-primary/10 text-foreground transition-colors text-left"
      title="Refresh schema"
      @click="emit('refresh-schema')"
    >
      <RefreshCwIcon class="size-3.5" /> Refresh Schema
    </button>
    <button
      class="w-full flex items-center gap-2 px-3 py-2 text-xs font-medium rounded-md hover:bg-primary/10 text-foreground transition-colors text-left"
      title="Import SQL"
      @click="emit('import-sql')"
    >
      <UploadIcon class="size-3.5" /> Import SQL
    </button>
    <button
      class="w-full flex items-center gap-2 px-3 py-2 text-xs font-medium rounded-md hover:bg-primary/10 text-foreground transition-colors text-left"
      title="Export database"
      @click="emit('export-database')"
    >
      <DownloadIcon class="size-3.5" /> Export
    </button>
    <button
      class="w-full flex items-center gap-2 px-3 py-2 text-xs font-medium rounded-md hover:bg-primary/10 text-foreground transition-colors text-left"
      title="Close database"
      @click="emit('close-database')"
    >
      <XIcon class="size-3.5" /> Close DB
    </button>
    <div class="border-t my-1" />
    <button
      class="w-full flex items-center gap-2 px-3 py-2 text-xs font-medium rounded-md hover:bg-destructive/10 text-destructive transition-colors text-left"
      @click="emit('delete-tables')"
    >
      <TableIcon class="size-3.5" /> Delete Tables…
    </button>
    <button
      class="w-full flex items-center gap-2 px-3 py-2 text-xs font-medium rounded-md hover:bg-destructive/10 text-destructive transition-colors text-left"
      @click="emit('drop')"
    >
      <Trash2Icon class="size-3.5" /> Drop Database
    </button>
  </div>
</template>

<script setup lang="ts">
import {
  Trash2Icon,
  TerminalIcon,
  UploadIcon,
  DownloadIcon,
  TableIcon,
  RefreshCwIcon,
  PanelRightOpenIcon,
  XIcon,
} from "lucide-vue-next";

defineProps<{
  show: boolean;
  x: number;
  y: number;
  databaseName: string;
}>();

const emit = defineEmits<{
  "open-query": [];
  "open-in-split": [];
  "refresh-schema": [];
  "import-sql": [];
  "export-database": [];
  "close-database": [];
  "delete-tables": [];
  drop: [];
}>();
</script>

