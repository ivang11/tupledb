<template>
  <Dialog :open="open" @update:open="(val: boolean) => emit('update:open', val)">
    <DialogContent
      style="width: 540px; max-width: 540px"
      :show-close-button="false"
      class="flex flex-col p-0 overflow-hidden bg-(--bg-1) border-(--line-1) rounded-xl"
    >
      <!-- Header -->
      <div class="flex items-start gap-3 px-5 pt-4 pb-3 border-b border-(--line-2)">
        <div class="size-9 rounded-lg bg-(--acc-soft) text-(--acc) grid place-items-center shrink-0">
          <UploadIcon class="size-4" />
        </div>
        <div class="flex-1 min-w-0">
          <DialogTitle class="text-[17px] font-semibold tracking-tight text-(--fg-1)">
            Import database
          </DialogTitle>
          <DialogDescription class="text-[13px] text-(--fg-3) mt-1 truncate">
            <span class="font-mono text-(--fg-1)">{{ database }}</span>
            <span class="mx-2 text-(--fg-4)">·</span>
            <span>Pick a file to import</span>
          </DialogDescription>
        </div>
        <button
          class="size-7 rounded-md grid place-items-center text-(--fg-3) hover:text-(--fg-1) hover:bg-(--bg-2) transition-colors"
          @click="close"
        >
          <XIcon class="size-4" />
        </button>
      </div>

      <!-- Body: single column -->
      <div class="flex flex-col px-5 py-4 gap-4 overflow-auto custom-scrollbar">

        <!-- Drop zone -->
        <div
          class="rounded-lg px-5 py-6 flex flex-col items-center justify-center text-center transition-colors cursor-pointer border-2"
          :class="isDragging
            ? 'bg-(--acc-soft) border-(--acc-line)'
            : 'bg-[oklch(0.82_0.155_158/0.04)] border-dashed border-(--acc-line) hover:bg-[oklch(0.82_0.155_158/0.07)]'"
          @dragover="onDragOver"
          @dragleave="onDragLeave"
          @drop="onDrop"
          @click="emit('start-file')"
        >
          <UploadIcon
            class="size-7 text-(--acc) mb-2"
            :class="{ 'animate-pulse': isDragging }"
          />
          <div class="text-[14px] font-medium text-(--fg-1)">Drop a file here</div>
          <div class="text-[13px] text-(--fg-3) mt-0.5">
            or <span class="text-(--acc) underline">browse…</span>
          </div>
        </div>

        <!-- File picked -->
        <div
          v-if="fileSummary"
          class="px-3 py-2 rounded-md bg-(--bg-2) text-[13px] flex items-center gap-2 -mt-2"
        >
          <FileIcon class="size-3.5 text-(--acc) shrink-0" />
          <span class="font-mono text-(--fg-1) truncate">{{ fileSummary }}</span>
          <button
            class="ml-auto text-(--fg-3) hover:text-(--fg-1) transition-colors"
            @click.stop="pickedFile = null"
          >
            <XIcon class="size-3.5" />
          </button>
        </div>



      </div>

      <!-- Footer -->
      <div class="flex items-center gap-2 px-5 py-2.5 border-t border-(--line-2) bg-(--bg-0)">
        <div class="flex-1" />
        <button
          class="h-8 px-3 rounded-md text-[13px] text-(--fg-2) hover:text-(--fg-1) hover:bg-(--bg-2) transition-colors"
          @click="close"
        >Cancel</button>
        <button
          class="h-8 px-4 rounded-md bg-(--acc) text-(--acc-fg) text-[13px] font-semibold hover:brightness-110 transition-all"
          @click="handleStart"
        >Start import →</button>
      </div>
    </DialogContent>
  </Dialog>
</template>

<script setup lang="ts">
import { computed, ref, watch, onUnmounted } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
} from "@/components/ui/dialog";
import { UploadIcon, XIcon, FileIcon } from "lucide-vue-next";

const props = defineProps<{
  open: boolean;
  database: string;
}>();

const emit = defineEmits<{
  "update:open": [val: boolean];
  "start-file": [];
  "start-file-path": [path: string];
}>();

const pickedFile = ref<{ name: string; size: number; path?: string } | null>(null);

function close() {
  emit("update:open", false);
}

function handleStart() {
  if (pickedFile.value?.path) {
    emit("start-file-path", pickedFile.value.path);
  } else {
    emit("start-file");
  }
}

// ── Drag-drop ─────────────────────────────────────────────────────────────────

const isDragging = ref(false);
let unlistenDrop: (() => void) | null = null;

watch(() => props.open, async (open) => {
  if (open) {
    try {
      unlistenDrop = await getCurrentWindow().onDragDropEvent((event) => {
        const type = event.payload.type;
        if (type === "enter" || type === "over") {
          isDragging.value = true;
        } else if (type === "leave") {
          isDragging.value = false;
        } else if (type === "drop") {
          isDragging.value = false;
          const paths = (event.payload as any).paths as string[] | undefined;
          if (paths && paths.length > 0) {
            const p = paths[0];
            pickedFile.value = { name: p.split(/[\\/]/).pop() ?? p, size: 0, path: p };
          }
        }
      });
    } catch {
      // Not running in Tauri (e.g. browser dev mode)
    }
  } else {
    unlistenDrop?.();
    unlistenDrop = null;
    isDragging.value = false;
  }
}, { immediate: true });

onUnmounted(() => {
  unlistenDrop?.();
});

function onDragOver(e: DragEvent) {
  e.preventDefault();
  isDragging.value = true;
}

function onDragLeave() {
  isDragging.value = false;
}

function onDrop(e: DragEvent) {
  e.preventDefault();
  isDragging.value = false;
  const file = e.dataTransfer?.files?.[0];
  if (file && !pickedFile.value?.path) {
    pickedFile.value = { name: file.name, size: file.size };
  }
}

const fileSummary = computed(() => {
  if (!pickedFile.value) return null;
  if (pickedFile.value.size > 0) {
    const mb = pickedFile.value.size / 1024 / 1024;
    return `${pickedFile.value.name} — ${mb.toFixed(1)} MB`;
  }
  return pickedFile.value.name;
});
</script>
