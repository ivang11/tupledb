<script setup lang="ts">
import { Trash2Icon } from "lucide-vue-next";

defineProps<{
  show: boolean;
  x: number;
  y: number;
  databaseName: string;
}>();

const emit = defineEmits<{
  drop: [];
}>();
</script>

<template>
  <div
    v-if="show"
    class="fixed z-[100] min-w-[160px] bg-background/95 backdrop-blur-md border rounded-lg shadow-xl p-1 animate-in fade-in zoom-in-95 duration-100"
    :style="{ left: x + 'px', top: y + 'px' }"
  >
    <div class="px-2 py-1 mb-1 border-b">
      <span
        class="text-[10px] font-semibold font-mono tracking-normal text-muted-foreground truncate block"
      >
        {{ databaseName }}
      </span>
    </div>
    <button
      class="w-full flex items-center gap-2 px-3 py-2 text-xs font-medium rounded-md hover:bg-destructive/10 text-destructive transition-colors text-left"
      @click="emit('drop')"
    >
      <Trash2Icon class="size-3.5" /> Drop Database
    </button>
  </div>
</template>
