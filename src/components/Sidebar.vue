<template>
  <aside
    class="shrink-0 flex flex-col bg-sidebar relative select-none"
    :style="{ width: width + 'px' }"
  >
    <!-- Database selector for the active connection -->
    <div
      v-if="selectedConnectionId && openConnections[selectedConnectionId]"
      class="shrink-0 p-4 pb-2"
    >
      <div
        class="flex items-center gap-0 group overflow-hidden transition-colors rounded-lg ring-1 ring-inset"
        :class="getConnectionCardColor(openConnections[selectedConnectionId].connection.environment)"
        @contextmenu="emit('context-menu-connection', $event, openConnections[selectedConnectionId].connection)"
      >
        <div class="flex-1 px-3 py-2.5 min-w-0">
          <div class="flex items-center gap-2">
            <span class="text-[13px] truncate flex-1 font-bold text-white">
              {{ openConnections[selectedConnectionId].connection.name }}
            </span>
            <span
              class="inline-flex items-center rounded-md px-1.5 py-0.5 text-[9px] font-black uppercase tracking-[0.08em] ring-1 ring-inset"
              :class="getEnvColor(openConnections[selectedConnectionId].connection.environment)"
            >
              {{ openConnections[selectedConnectionId].connection.environment }}
            </span>
          </div>
          <div class="mt-1 flex items-center gap-1.5 text-[11px] text-white/72">
            <ServerIcon class="size-3" />
            <span
              class="font-semibold"
              :class="activeConnection?.status === 'error' ? 'text-red-100' : ''"
              :title="activeConnectionStatusTitle"
            >
              {{ activeConnectionStatus }}
            </span>
            <span class="text-white/35">|</span>
            <span class="truncate text-white/62">{{ activeConnectionDetail }}</span>
          </div>
        </div>
      </div>
    </div>

    <!-- Search bar (for selected connection's tables) -->
    <div
      v-if="selectedConnectionId && openConnections[selectedConnectionId]"
      ref="searchContainerRef"
      class="shrink-0 px-4 pb-3"
    >
      <div class="relative">
        <SearchIcon
          class="absolute left-3 top-1/2 -translate-y-1/2 size-3.5 text-sidebar-foreground/62"
        />
        <Input
          :model-value="search"
          @update:model-value="emit('update:search', String($event))"
          placeholder="Search"
          class="h-9 pl-9 pr-9 text-sm text-white placeholder:text-sidebar-foreground/58 bg-accent border border-white/10 rounded-lg focus-visible:ring-0 focus-visible:border-white/10"
        />
        <button
          v-if="search"
          class="absolute right-2 top-1/2 -translate-y-1/2 size-5 rounded flex items-center justify-center text-sidebar-foreground/55 hover:text-white transition-colors"
          title="Clear search"
          @click="emit('update:search', '')"
        >
          <XIcon class="size-3" />
        </button>
      </div>
    </div>

    <div
      v-if="selectedConnectionId && openConnections[selectedConnectionId]"
      ref="dbDropdownRef"
      class="shrink-0 px-3 pb-2 relative"
    >
      <div class="flex items-center gap-1.5">
        <button
          type="button"
          class="min-w-0 flex-1 flex items-center gap-2 px-3 py-2 rounded-lg bg-black/20 hover:bg-black/30 transition-colors text-left"
          @click="toggleDbDropdown"
          @contextmenu="activeDb && emit('context-menu-database', $event, selectedConnectionId, activeDb)"
        >
          <DatabaseIcon class="size-3.5 shrink-0 text-white/50" />
          <span class="flex-1 truncate text-[13px] font-semibold" :class="activeDb ? 'text-white' : 'text-white/40'">
            {{ activeDb || 'Select database' }}
          </span>
          <ChevronDownIcon
            class="size-3.5 shrink-0 text-white/40 transition-transform duration-150"
            :class="{ 'rotate-180': dbDropdownOpen }"
          />
        </button>
        <button
          type="button"
          class="size-9 shrink-0 rounded-lg bg-black/20 hover:bg-black/30 text-white/80 hover:text-white transition-colors flex items-center justify-center"
          title="New Database"
          @click="emit('new-database', selectedConnectionId)"
        >
          <PlusIcon class="size-4" :stroke-width="2.5" />
        </button>
      </div>

      <!-- Dropdown list -->
      <div
        v-if="dbDropdownOpen"
        class="absolute left-3 right-3 top-full mt-1 z-50 rounded-lg border border-white/10 bg-[#0f1117] shadow-2xl overflow-hidden"
      >
        <!-- Search input -->
        <div class="p-2 border-b border-white/8">
          <div class="relative">
            <SearchIcon class="absolute left-2.5 top-1/2 -translate-y-1/2 size-3 text-white/30" />
            <input
              ref="dbSearchInput"
              v-model="dbSearch"
              type="text"
              placeholder="Search..."
              class="w-full bg-white/5 rounded-md pl-7 pr-3 py-1.5 text-[12px] text-white placeholder:text-white/30 focus:outline-none focus:bg-white/8 transition-colors"
              @keydown.escape="dbDropdownOpen = false"
              @keydown.enter="onDbSearchEnter"
            />
          </div>
        </div>

        <!-- Results -->
        <div class="max-h-52 overflow-y-auto py-1 custom-scrollbar">
          <button
            v-for="db in filteredDatabases"
            :key="db"
            type="button"
            class="w-full flex items-center gap-2.5 px-3 py-2 text-[12.5px] font-medium transition-colors text-left"
            :class="db === activeDb
              ? 'text-white bg-white/8'
              : 'text-white/70 hover:text-white hover:bg-white/5'"
            @click="selectDb(db)"
          >
            <span
              class="size-1.5 rounded-full shrink-0"
              :class="db === activeDb ? 'bg-primary' : ''"
            />
            {{ db }}
          </button>
          <p v-if="filteredDatabases.length === 0" class="px-3 py-3 text-[12px] text-white/30 italic">
            No matches
          </p>
        </div>
      </div>
    </div>

    <!-- Tables for selected database -->
    <ScrollArea class="flex-1 py-1">
      <template v-if="selectedConnectionId && openConnections[selectedConnectionId] && activeDatabase">
        <div class="px-4 mt-1 pb-3">
          <div class="space-y-0.5">
              <button
                v-for="table in filteredTables(activeConnId, activeDb)"
                :key="table.name"
                :ref="(el) => setTableRef(el as HTMLElement | null, table.name, activeDb, activeConnId)"
                @click="handleTableClick($event, activeConnId, activeDb, table.name)"
                @contextmenu="emit('context-menu-table', $event, activeConnId, activeDb, table.name)"
	                :class="tableButtonClasses(table.name, activeDb, activeConnId)"
              >
                <EyeIcon
                  v-if="isView(table)"
                  :class="tableIconClasses(table.name, activeDb, activeConnId)"
                />
                <TableIcon
                  v-else
                  :class="tableIconClasses(table.name, activeDb, activeConnId)"
                />
                <span class="flex-1 truncate text-sm font-semibold">{{ table.name }}</span>
                <!-- Open indicator -->
                <span
                  v-if="isTableOpen(table.name, activeDb, activeConnId) && !isTableActive(table.name, activeDb, activeConnId)"
                  class="size-1.5 rounded-full shrink-0 bg-primary/30"
                />
              </button>

              <div
                v-if="filteredTables(activeConnId, activeDb).length === 0 && search"
                class="px-2 py-1 text-[10px] text-sidebar-foreground/30 italic"
              >
                No matches
              </div>
          </div>
        </div>
      </template>

<div
        v-else-if="selectedConnectionId && openConnections[selectedConnectionId]"
        class="px-4 py-8 text-center"
      >
        <DatabaseIcon class="size-7 mx-auto text-sidebar-foreground/18 mb-3" />
        <p class="text-xs font-medium text-sidebar-foreground/45">Select a database to browse tables.</p>
      </div>
    </ScrollArea>

    <!-- Resize handle -->
    <div
      class="group absolute right-0 top-0 bottom-0 w-2 cursor-col-resize z-20 flex items-stretch justify-end"
      @mousedown.prevent="emit('resize-start', $event)"
    >
      <div class="w-px group-hover:bg-primary/35 transition-colors" />
    </div>
  </aside>
</template>

<script setup lang="ts">
import { ref, computed, nextTick, onMounted, onUnmounted } from "vue";
import {
  SearchIcon,
  DatabaseIcon,
  ServerIcon,
  ChevronDownIcon,
  TableIcon,
  EyeIcon,
  XIcon,
  PlusIcon,
} from "lucide-vue-next";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import type { Connection, Environment } from "@/types/connection";

const props = defineProps<{
  width: number;
  search: string;
  selectedConnectionId: string | null;
  openConnections: Record<
    string,
    {
      connection: Connection;
      databases: string[];
      selectedDatabase: string | null;
      serverVersion?: string | null;
      status?: "connected" | "error";
      statusMessage?: string | null;
      tables: Record<string, any[]>;
    }
  >;
  isTableActive: (name: string, db: string, connId: string) => boolean;
  isTableOpen: (name: string, db: string, connId: string) => boolean;
  pendingTableAction: (name: string, db: string, connId: string) => "drop" | "truncate" | null;
  filteredTables: (connId: string, db: string) => any[];
  isTableSelected: (connId: string, db: string, tableName: string) => boolean;
}>();

const emit = defineEmits<{
  "resize-start": [e: MouseEvent];
  "update:search": [val: string];
  "update:selectedConnectionId": [id: string];
  "new-connection": [];
  "new-database": [connId: string];
  "connect-saved": [conn: Connection];
  "select-database": [connId: string, db: string];
  "toggle-database": [connId: string, db: string];
  "load-table": [tableName: string, connId: string, db: string];
  "toggle-table-selection": [connId: string, db: string, tableName: string];
  "select-table-range": [connId: string, db: string, tableName: string];
  "clear-table-selection": [];
  "import-sql": [connId: string, db: string];
  "export-database": [connId: string, db: string];
  "open-query": [connId: string, db: string | null];
  "delete-selected-tables": [];
  "context-menu-connection": [e: MouseEvent, conn: Connection];
  "context-menu-table": [
    e: MouseEvent,
    connId: string,
    db: string,
    tableName: string,
  ];
  "context-menu-database": [
    e: MouseEvent,
    connId: string,
    databaseName: string,
  ];
  "export-connections": [];
  "import-connections": [];
}>();

const searchContainerRef = ref<HTMLElement | null>(null);

function focusSearch() {
  searchContainerRef.value?.querySelector("input")?.focus();
}

defineExpose({ focusSearch, scrollToTable });

function handleTableClick(
  e: MouseEvent,
  connId: string,
  db: string,
  tableName: string,
) {
  if (e.shiftKey) {
    e.preventDefault();
    emit("select-table-range", connId, db, tableName);
  } else if (e.ctrlKey || e.metaKey) {
    e.preventDefault();
    emit("toggle-table-selection", connId, db, tableName);
  } else {
    emit("load-table", tableName, connId, db);
  }
}

// ── Scroll to table ───────────────────────────────────────────────────────────

const tableRefs = ref<Record<string, HTMLElement>>({});

function setTableRef(el: HTMLElement | null, tableName: string, db: string, connId: string) {
  const key = `${connId}:${db}:${tableName}`;
  if (el) tableRefs.value[key] = el;
  else delete tableRefs.value[key];
}

function scrollToTable(tableName: string, db: string, connId: string) {
  const key = `${connId}:${db}:${tableName}`;
  const el = tableRefs.value[key];
  el?.scrollIntoView({ block: "nearest", behavior: "smooth" });
}

function isView(table: any) {
  return String(table?.table_type ?? '').toUpperCase().includes('VIEW')
}

function tableButtonClasses(tableName: string, db: string, connId: string) {
  const base = 'w-full flex items-center gap-2.5 px-2.5 py-2 rounded-md text-sm transition-all text-left group/tbl'
  const action = props.pendingTableAction(tableName, db, connId)

  if (action === 'drop') {
    return `${base} bg-destructive/12 text-destructive line-through ring-1 ring-inset ring-destructive/20`
  }
  if (action === 'truncate') {
    return `${base} bg-amber-500/14 text-amber-200 line-through ring-1 ring-inset ring-amber-500/25`
  }
  if (props.isTableSelected(connId, db, tableName)) return `${base} bg-primary/15 text-primary`
  if (props.isTableActive(tableName, db, connId)) return `${base} bg-accent text-white shadow-none`
  return `${base} hover:bg-accent/70 text-sidebar-foreground/90 hover:text-white`
}

function tableIconClasses(tableName: string, db: string, connId: string) {
  const action = props.pendingTableAction(tableName, db, connId)

  if (action === 'drop') return 'size-2.5 shrink-0 text-destructive/85'
  if (action === 'truncate') return 'size-2.5 shrink-0 text-amber-300/90'
  if (props.isTableActive(tableName, db, connId)) return 'size-2.5 shrink-0 text-white/70'
  if (props.isTableSelected(connId, db, tableName)) return 'size-2.5 shrink-0 text-primary/85'
  return 'size-2.5 shrink-0 text-sidebar-foreground/68 group-hover/tbl:text-sidebar-foreground/90'
}

const getEnvColor = (env: Environment): string => {
  switch (env) {
    case "PRODUCTION":
      return "bg-red-500/12 text-red-200 ring-red-400/25";
    case "STAGING":
      return "bg-orange-500/12 text-orange-200 ring-orange-400/25";
    case "DEV":
      return "bg-blue-500/12 text-blue-200 ring-blue-400/25";
    default:
      return "bg-green-500/12 text-green-200 ring-green-400/25";
  }
};

const getConnectionCardColor = (env: Environment): string => {
  switch (env) {
    case "PRODUCTION":
      return "bg-red-900/80 ring-red-400/20";
    case "STAGING":
      return "bg-orange-800/80 ring-orange-300/20";
    case "DEV":
      return "bg-blue-800/80 ring-blue-300/20";
    default:
      return "bg-[#1f6a44] ring-green-300/20";
  }
};

// Non-null alias used inside the v-if="selectedConnectionId" template blocks
const activeConnId = computed(() => props.selectedConnectionId ?? '')
const activeConnection = computed(() =>
  props.selectedConnectionId ? props.openConnections[props.selectedConnectionId] : null,
)
const activeDatabase = computed(() => activeConnection.value?.selectedDatabase ?? null)
const activeDb = computed(() => activeDatabase.value ?? '')

// ── Database dropdown ─────────────────────────────────────────────────────────
const dbDropdownOpen = ref(false)
const dbDropdownRef = ref<HTMLElement | null>(null)
const dbSearch = ref('')
const dbSearchInput = ref<HTMLInputElement | null>(null)

const filteredDatabases = computed(() => {
  if (!props.selectedConnectionId) return []
  const dbs = props.openConnections[props.selectedConnectionId]?.databases ?? []
  if (!dbSearch.value) return dbs
  return dbs.filter(db => db.toLowerCase().includes(dbSearch.value.toLowerCase()))
})

async function toggleDbDropdown() {
  dbDropdownOpen.value = !dbDropdownOpen.value
  if (dbDropdownOpen.value) {
    dbSearch.value = ''
    await nextTick()
    dbSearchInput.value?.focus()
  }
}

function selectDb(db: string) {
  if (!props.selectedConnectionId) return
  emit('select-database', props.selectedConnectionId, db)
  dbDropdownOpen.value = false
  dbSearch.value = ''
}

function onDbSearchEnter() {
  if (filteredDatabases.value.length === 1) selectDb(filteredDatabases.value[0])
}

function onClickOutsideDb(e: MouseEvent) {
  if (dbDropdownRef.value && !dbDropdownRef.value.contains(e.target as Node)) {
    dbDropdownOpen.value = false
  }
}

onMounted(() => document.addEventListener('mousedown', onClickOutsideDb))
onUnmounted(() => document.removeEventListener('mousedown', onClickOutsideDb))
const activeConnectionDetail = computed(() => {
  return activeConnection.value?.serverVersion
    ? `MySQL ${activeConnection.value.serverVersion}`
    : 'MySQL'
})
const activeConnectionStatus = computed(() =>
  activeConnection.value?.status === "error" ? "Connection error" : "Connected",
)
const activeConnectionStatusTitle = computed(() =>
  activeConnection.value?.statusMessage ?? activeConnectionStatus.value,
)
</script>
