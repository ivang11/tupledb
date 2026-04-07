<script setup lang="ts">
import {
  Trash2Icon,
  TerminalIcon,
  UploadIcon,
  DownloadIcon,
} from "lucide-vue-next";

defineProps<{
  show: boolean;
  x: number;
  y: number;
  databaseName: string;
}>();

const emit = defineEmits<{
  "open-query": [];
  "import-sql": [];
  "export-database": [];
  drop: [];
}>();
</script>

<template>
  <div
    v-if="show"
    class="fixed z-[100] min-w-[160px] bg-background/95 backdrop-blur-md border rounded-lg shadow-xl p-1 animate-in fade-in zoom-in-95 duration-100"
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
    <div class="border-t my-1" />
    <button
      class="w-full flex items-center gap-2 px-3 py-2 text-xs font-medium rounded-md hover:bg-destructive/10 text-destructive transition-colors text-left"
      @click="emit('drop')"
    >
      <Trash2Icon class="size-3.5" /> Drop Database
    </button>
  </div>
</template>
