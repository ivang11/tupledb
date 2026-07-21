<template>
  <Dialog :open="open" @update:open="(val: boolean) => emit('update:open', val)">
    <DialogContent
      style="width: 780px; max-width: 780px; height: 620px"
      :show-close-button="false"
      class="flex flex-col p-0 overflow-hidden bg-(--bg-1) border-(--line-1) rounded-xl"
    >
      <!-- Header -->
      <div class="flex items-start gap-3 px-6 pt-4 pb-3 border-b border-(--line-2)">
        <div class="size-9 rounded-lg bg-(--acc-soft) text-(--acc) grid place-items-center shrink-0">
          <DownloadIcon class="size-4" />
        </div>
        <div class="flex-1 min-w-0">
          <DialogTitle class="text-[17px] font-semibold tracking-tight text-(--fg-1)">
            Export database
          </DialogTitle>
          <DialogDescription class="text-[13px] text-(--fg-3) mt-1 truncate">
            <span class="font-mono text-(--fg-1)">{{ database }}</span>
            <span class="mx-2 text-(--fg-4)">·</span>
            <span>Choose tables + format</span>
          </DialogDescription>
        </div>
        <button
          class="size-7 rounded-md grid place-items-center text-(--fg-3) hover:text-(--fg-1) hover:bg-(--bg-2) transition-colors"
          @click="close"
        >
          <XIcon class="size-4" />
        </button>
      </div>

      <!-- Body: two columns -->
      <div class="flex flex-1 min-h-0">

        <!-- Left: flat tables list -->
        <div class="flex flex-col min-h-0 flex-[1.4] border-r border-(--line-2)">
          <!-- Toolbar -->
          <div class="flex items-center gap-2 px-4 pt-3 pb-2">
            <span class="text-[11px] tracking-widest uppercase text-(--fg-3) font-semibold">Tables</span>
            <span class="font-mono text-[11px] text-(--fg-3)">
              {{ totalCount }} total · {{ selectedCount }} selected
            </span>
            <div class="flex-1" />
            <button class="h-6 px-2 text-[12px] rounded text-(--fg-2) hover:text-(--fg-1) hover:bg-(--bg-2) transition-colors" @click="selectAll">All</button>
            <button class="h-6 px-2 text-[12px] rounded text-(--fg-2) hover:text-(--fg-1) hover:bg-(--bg-2) transition-colors" @click="selectNone">None</button>
            <button class="h-6 px-2 text-[12px] rounded text-(--fg-2) hover:text-(--fg-1) hover:bg-(--bg-2) transition-colors" @click="selectInvert">Invert</button>
          </div>

          <!-- Search -->
          <div class="px-4 pb-2">
            <div class="relative">
              <input
                v-model="search"
                class="w-full h-8 pl-8 pr-3 rounded-md bg-(--bg-0) border border-(--line-2) text-[13px] text-(--fg-1) outline-none focus:border-(--acc-line) placeholder:text-(--fg-4)"
                placeholder="Filter tables…"
              />
              <SearchIcon class="size-3.5 text-(--fg-4) absolute left-2.5 top-2.5" />
            </div>
          </div>

          <!-- Flat list -->
          <div class="flex-1 overflow-auto px-2 pb-3 custom-scrollbar">
            <div v-if="loadingTables" class="px-3 py-4 text-sm text-(--fg-3) italic">
              Loading tables…
            </div>
            <div v-else-if="filteredTables.length === 0" class="px-3 py-4 text-sm text-(--fg-3) italic">
              No tables match "{{ search }}".
            </div>

            <label
              v-for="t in filteredTables"
              :key="t.name"
              class="grid grid-cols-[16px_1fr_auto_auto] items-center gap-2.5 px-3 py-1.5 text-[13px] cursor-pointer rounded hover:bg-(--bg-2)/50 transition-colors"
            >
              <input
                type="checkbox"
                :checked="isSelected(t.name)"
                class="size-3.5 accent-(--acc)"
                @change="toggleTable(t.name)"
              />
              <span class="font-mono text-(--fg-1) truncate">{{ t.name }}</span>
              <span class="font-mono text-[11px] text-(--fg-3) tabular-nums">
                {{ t.rows != null ? t.rows.toLocaleString() : "" }}
              </span>
              <span class="font-mono text-[11px] text-(--fg-3) w-14 text-right">
                {{ t.size ?? "" }}
              </span>
            </label>
          </div>
        </div>

        <!-- Right: format / content / options -->
        <div class="flex-1 overflow-auto px-5 py-4 custom-scrollbar">

          <!-- Format -->
          <div class="text-[11px] tracking-widest uppercase text-(--fg-3) font-semibold mb-2">Format</div>
          <div class="grid grid-cols-3 gap-1.5 mb-5">
            <button
              v-for="f in FORMATS"
              :key="f.id"
              type="button"
              class="px-3 py-3 h-16 rounded-md text-left transition-all flex flex-col justify-center border"
              :class="format === f.id
                ? 'bg-(--acc-soft) border-(--acc-line)'
                : 'bg-(--bg-2) border-(--line-2) hover:border-(--fg-4)'"
              @click="format = f.id"
            >
              <div
                class="text-[14px] font-semibold leading-none mb-1"
                :class="format === f.id ? 'text-(--acc)' : 'text-(--fg-1)'"
              >{{ f.name }}</div>
              <div
                class="font-mono text-[11px] leading-none"
                :class="format === f.id ? 'text-(--acc)/70' : 'text-(--fg-3)'"
              >{{ f.sub }}</div>
            </button>
          </div>

          <!-- Content -->
          <div class="text-[11px] tracking-widest uppercase text-(--fg-3) font-semibold mb-2">Content</div>
          <div class="flex gap-1.5 mb-5">
            <button
              v-for="m in CONTENT_MODES"
              :key="m.id"
              type="button"
              class="flex-1 px-2.5 py-2 rounded text-[12px] text-center transition-colors border"
              :class="currentMode === m.id
                ? 'bg-(--bg-3) text-(--fg-1) border-(--line-2)'
                : 'text-(--fg-2) border-(--line-2) hover:bg-(--bg-2) hover:text-(--fg-1)'"
              @click="emit('update:currentMode', m.id)"
            >{{ m.label }}</button>
          </div>

          <!-- Options -->
          <div class="text-[11px] tracking-widest uppercase text-(--fg-3) font-semibold mb-2">Options</div>
          <label
            v-for="(o, i) in [
              { key: 'dropIfExists',    label: 'Drop table if exists' },
              { key: 'includeViews',    label: 'Include views' },
              { key: 'useTransactions', label: 'Use transactions' },
              { key: 'compressGzip',    label: 'Compress (gzip)' },
            ]"
            :key="i"
            class="flex items-center gap-2.5 py-1.5 text-[13px] text-(--fg-1) cursor-pointer hover:text-(--fg-1) transition-colors"
          >
            <input
              type="checkbox"
              class="size-3.5 accent-(--acc)"
              :checked="(options as any)[o.key]"
              @change="(e) => ((options as any)[o.key] = (e.target as HTMLInputElement).checked)"
            />
            {{ o.label }}
          </label>

        </div>
      </div>

      <!-- Footer -->
      <div class="flex items-center gap-2 px-5 py-2.5 border-t border-(--line-2) bg-(--bg-0)">
        <span class="font-mono text-[12px] text-(--fg-3) truncate">{{ filename }}</span>
        <div class="flex-1" />
        <button
          class="h-8 px-3 rounded-md text-[13px] text-(--fg-2) hover:text-(--fg-1) hover:bg-(--bg-2) transition-colors"
          @click="close"
        >Cancel</button>
        <button
          class="h-8 px-4 rounded-md bg-(--acc) text-(--acc-fg) text-[13px] font-semibold hover:brightness-110 transition-all disabled:opacity-50"
          :disabled="loadingTables || selectedTables.length === 0"
          @click="handleExportStart"
        >Start export →</button>
      </div>
    </DialogContent>
  </Dialog>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import {
  Dialog,
  DialogContent,
  DialogTitle,
  DialogDescription,
} from "@/components/ui/dialog";
import {
  DownloadIcon,
  SearchIcon,
  XIcon,
} from "lucide-vue-next";

const props = defineProps<{
  open: boolean;
  database: string;
  tables: { name: string; rows?: number; size?: string }[];
  loadingTables?: boolean;
  selectedTables: string[];
  currentMode: string;
}>();

export interface ExportStartPayload {
  format: "sql" | "csv" | "json";
  options: {
    dropIfExists: boolean;
    includeViews: boolean;
    useTransactions: boolean;
    compressGzip: boolean;
  };
}

const emit = defineEmits<{
  "update:open": [val: boolean];
  "update:selectedTables": [tables: string[]];
  "update:currentMode": [mode: string];
  "start": [payload: ExportStartPayload];
}>();

// ── Format ────────────────────────────────────────────────────────────────────

const FORMATS = [
  { id: "sql",  name: "SQL",  sub: ".sql" },
  { id: "csv",  name: "CSV",  sub: "one per table" },
  { id: "json", name: "JSON", sub: "array per table" },
] as const;

const format = ref<typeof FORMATS[number]["id"]>("sql");

const CONTENT_MODES = [
  { id: "full",      label: "Schema + data" },
  { id: "structure", label: "Schema only"   },
  { id: "data",      label: "Data only"     },
] as const;

// ── Options ───────────────────────────────────────────────────────────────────

const options = ref({
  dropIfExists: true,
  includeViews: true,
  useTransactions: true,
  compressGzip: false,
});

// ── Tables (flat) ─────────────────────────────────────────────────────────────

const search = ref("");

const filteredTables = computed(() => {
  const q = search.value.trim().toLowerCase();
  if (!q) return props.tables;
  return props.tables.filter((t) => t.name.toLowerCase().includes(q));
});

const totalCount = computed(() => props.tables.length);
const selectedCount = computed(() => props.selectedTables.length);

function isSelected(name: string) {
  return props.selectedTables.includes(name);
}

function toggleTable(name: string) {
  if (isSelected(name)) {
    emit("update:selectedTables", props.selectedTables.filter((n) => n !== name));
  } else {
    emit("update:selectedTables", [...props.selectedTables, name]);
  }
}

function selectAll() {
  emit("update:selectedTables", props.tables.map((t) => t.name));
}
function selectNone() {
  emit("update:selectedTables", []);
}
function selectInvert() {
  const set = new Set(props.selectedTables);
  emit("update:selectedTables", props.tables.filter((t) => !set.has(t.name)).map((t) => t.name));
}

// ── Footer ────────────────────────────────────────────────────────────────────

const filename = computed(() => {
  const today = new Date().toISOString().slice(0, 10);
  const ext = options.value.compressGzip ? `${format.value}.gz` : format.value;
  return `${props.database}-${today}.${ext}`;
});

function close() {
  emit("update:open", false);
}

function handleExportStart() {
  emit("start", {
    format: format.value,
    options: { ...options.value },
  });
}
</script>
