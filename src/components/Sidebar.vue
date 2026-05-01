<script setup lang="ts">
import { ref, computed } from "vue";
import {
  SearchIcon,
  DatabaseIcon,
  ServerIcon,
  ChevronDownIcon,
  ChevronRightIcon,
  TableIcon,
  EyeIcon,
  PlusIcon,
  PlugZapIcon,
  DownloadIcon,
  UploadIcon,
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
      tables: Record<string, any[]>;
    }
  >;
  closedConnections: Connection[];
  expandedDatabases: Set<string>;
  connectingId: string | null;
  isTableActive: (name: string, db: string, connId: string) => boolean;
  isTableOpen: (name: string, db: string, connId: string) => boolean;
  filteredTables: (connId: string, db: string) => any[];
  isTableSelected: (connId: string, db: string, tableName: string) => boolean;
}>();

const emit = defineEmits<{
  "resize-start": [e: MouseEvent];
  "update:search": [val: string];
  "update:selectedConnectionId": [id: string];
  "new-connection": [];
  "connect-saved": [conn: Connection];
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

const getEnvTextColor = (env: Environment): string => {
  switch (env) {
    case "PRODUCTION":
      return "text-red-100";
    case "STAGING":
      return "text-orange-100";
    case "DEV":
      return "text-blue-100";
    default:
      return "text-green-100";
  }
};

const getEnvAccentColor = (env: Environment): string => {
  switch (env) {
    case "PRODUCTION":
      return "bg-red-400/80";
    case "STAGING":
      return "bg-orange-400/80";
    case "DEV":
      return "bg-blue-400/80";
    default:
      return "bg-green-400/80";
  }
};

const getEnvChipColor = (env: Environment): string => {
  switch (env) {
    case "PRODUCTION":
      return "bg-red-500/18 ring-1 ring-inset ring-red-400/28";
    case "STAGING":
      return "bg-orange-500/18 ring-1 ring-inset ring-orange-400/28";
    case "DEV":
      return "bg-blue-500/18 ring-1 ring-inset ring-blue-400/28";
    default:
      return "bg-green-500/18 ring-1 ring-inset ring-green-400/28";
  }
};

const searchContainerRef = ref<HTMLElement | null>(null);

function focusSearch() {
  searchContainerRef.value?.querySelector("input")?.focus();
}

defineExpose({ focusSearch, scrollToTable });

const dbKey = (connId: string, db: string) => `${connId}:${db}`;

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

// Non-null alias used inside the v-if="selectedConnectionId" template blocks
const activeConnId = computed(() => props.selectedConnectionId ?? '')
</script>

<template>
  <aside
    class="shrink-0 flex flex-col border-r border-sidebar-border bg-sidebar relative select-none"
    :style="{ width: width + 'px' }"
  >
    <!-- Header -->
    <div class="shrink-0 border-b border-sidebar-border">
      <div class="flex items-center gap-2 px-3 h-9">
        <div class="flex items-center gap-2 min-w-0 flex-1">
          <ServerIcon class="size-3.5 text-primary/75 shrink-0" />
          <span class="text-[11px] font-black text-sidebar-foreground/88 uppercase tracking-[0.18em] truncate">
            Connections
          </span>
          <span
            v-if="Object.keys(openConnections).length > 0"
            class="text-[11px] font-bold text-primary/70 tabular-nums"
          >
            {{ Object.keys(openConnections).length }}
          </span>
        </div>
        <button
          @click="emit('import-connections')"
          class="size-6 flex items-center justify-center rounded text-sidebar-foreground/50 hover:text-primary hover:bg-primary/12 transition-colors shrink-0"
          title="Import connections"
        >
          <UploadIcon class="size-3.5" />
        </button>
        <button
          @click="emit('export-connections')"
          class="size-6 flex items-center justify-center rounded text-sidebar-foreground/50 hover:text-primary hover:bg-primary/12 transition-colors shrink-0"
          title="Export connections"
        >
          <DownloadIcon class="size-3.5" />
        </button>
        <button
          @click="emit('new-connection')"
          class="size-6 flex items-center justify-center rounded text-sidebar-foreground/50 hover:text-primary hover:bg-primary/12 transition-colors shrink-0"
          title="New Connection"
        >
          <PlusIcon class="size-3.5" />
        </button>
      </div>
    </div>

    <!-- Empty state -->
    <div
      v-if="Object.keys(openConnections).length === 0 && closedConnections.length === 0"
      class="flex flex-col items-center justify-center py-16 px-4 text-center"
    >
      <PlugZapIcon class="size-7 text-sidebar-foreground/15 mb-3" />
      <p class="text-xs text-sidebar-foreground/30 font-medium">No connections yet</p>
      <button
        @click="emit('new-connection')"
        class="mt-3 text-[11px] text-primary/70 hover:text-primary transition-colors"
      >
        Add connection
      </button>
    </div>

    <!-- Open connections list (compact, selectable) -->
    <div
      v-if="Object.keys(openConnections).length > 0"
      class="shrink-0 border-b border-sidebar-border/60"
    >
      <div
        v-for="(connState, connId) in openConnections"
        :key="connId"
        class="flex items-center gap-0 group overflow-hidden cursor-pointer transition-colors"
        :class="selectedConnectionId === connId
          ? 'bg-primary/10 shadow-[inset_0_1px_0_rgba(122,162,247,0.08)]'
          : 'hover:bg-sidebar-accent/55'"
        @click="emit('update:selectedConnectionId', connId as string)"
        @contextmenu="emit('context-menu-connection', $event, connState.connection)"
      >
        <span
          :class="['w-1 self-stretch shrink-0', getEnvAccentColor(connState.connection.environment)]"
        />
        <div class="flex-1 flex items-center gap-2 px-2.5 py-2 min-w-0">
          <DatabaseIcon
            :class="[
              'size-3.5 shrink-0',
              selectedConnectionId === connId ? 'text-primary/80' : 'text-sidebar-foreground/38'
            ]"
          />
          <span
            :class="[
              'text-sm truncate flex-1 tracking-[0.01em]',
              selectedConnectionId === connId
                ? 'font-semibold text-sidebar-foreground'
                : 'font-medium text-sidebar-foreground/82'
            ]"
          >
            {{ connState.connection.name }}
          </span>
          <span
            class="inline-flex items-center gap-1 rounded-full px-1.5 py-0.5 shrink-0"
            :class="getEnvChipColor(connState.connection.environment)"
          >
            <span
              class="text-[9px] font-bold uppercase tracking-[0.08em]"
              :class="getEnvTextColor(connState.connection.environment)"
            >
              {{ connState.connection.environment }}
            </span>
          </span>
        </div>
      </div>
    </div>

    <!-- Search bar (for selected connection's tables) -->
    <div
      v-if="selectedConnectionId && openConnections[selectedConnectionId]"
      ref="searchContainerRef"
      class="shrink-0 px-2 py-1.5 border-b border-sidebar-border/40"
    >
      <div class="relative">
        <SearchIcon
          class="absolute left-2.5 top-1/2 -translate-y-1/2 size-3 text-sidebar-foreground/42"
        />
        <Input
          :value="search"
          @input="emit('update:search', ($event.target as HTMLInputElement).value)"
          placeholder="Filter tables..."
          class="h-7 pl-8 text-xs text-sidebar-foreground/90 placeholder:text-sidebar-foreground/38 bg-sidebar-accent/65 border-none rounded focus-visible:ring-1 focus-visible:ring-primary/30"
        />
      </div>
    </div>

    <!-- Tree for selected connection -->
    <ScrollArea class="flex-1 py-1">
      <template v-if="selectedConnectionId && openConnections[selectedConnectionId]">
        <div class="px-1 mt-0.5 space-y-px">
          <!-- Databases -->
          <div
            v-for="db in openConnections[activeConnId].databases"
            :key="db"
          >
            <div class="flex items-center group/db rounded hover:bg-sidebar-accent/45 transition-colors">
              <button
                class="flex-1 flex items-center gap-1.5 px-2 py-1.5 min-w-0"
                @click="emit('toggle-database', activeConnId, db)"
                @contextmenu="emit('context-menu-database', $event, activeConnId, db)"
              >
                <ChevronDownIcon
                  v-if="expandedDatabases.has(dbKey(activeConnId, db))"
                  class="size-2.5 text-sidebar-foreground/40 shrink-0"
                />
                <ChevronRightIcon
                  v-else
                  class="size-2.5 text-sidebar-foreground/20 shrink-0"
                />
                <DatabaseIcon class="size-2.5 shrink-0 text-sidebar-foreground/48" />
                <span class="text-sm truncate flex-1 text-left font-medium text-sidebar-foreground/88">
                  {{ db }}
                </span>
              </button>
            </div>

            <!-- Tables -->
            <div
              v-if="expandedDatabases.has(dbKey(activeConnId, db))"
              class="ml-6 space-y-px mt-0.5 mb-1"
            >
              <button
                v-for="table in filteredTables(activeConnId, db)"
                :key="table.name"
                :ref="(el) => setTableRef(el as HTMLElement | null, table.name, db, activeConnId)"
                @click="handleTableClick($event, activeConnId, db, table.name)"
                @contextmenu="emit('context-menu-table', $event, activeConnId, db, table.name)"
                :class="[
                  'w-full flex items-center gap-1.5 px-2 py-1.5 rounded text-xs transition-all text-left group/tbl',
                  isTableSelected(activeConnId, db, table.name)
                    ? 'bg-primary/20 text-primary ring-1 ring-inset ring-primary/30'
                    : isTableActive(table.name, db, activeConnId)
                      ? 'bg-primary text-primary-foreground'
                      : 'hover:bg-sidebar-accent/50 text-sidebar-foreground/78 hover:text-sidebar-foreground',
                ]"
              >
                <EyeIcon
                  v-if="isView(table)"
                  :class="[
                    'size-2.5 shrink-0',
                    isTableActive(table.name, db, activeConnId)
                      ? 'text-primary-foreground/70'
                      : isTableSelected(activeConnId, db, table.name)
                        ? 'text-primary'
                        : 'text-sidebar-foreground/36 group-hover/tbl:text-sidebar-foreground/68',
                  ]"
                />
                <TableIcon
                  v-else
                  :class="[
                    'size-2.5 shrink-0',
                    isTableActive(table.name, db, activeConnId)
                      ? 'text-primary-foreground/70'
                      : isTableSelected(activeConnId, db, table.name)
                        ? 'text-primary'
                        : 'text-sidebar-foreground/36 group-hover/tbl:text-sidebar-foreground/68',
                  ]"
                />
                <span class="flex-1 truncate text-xs font-medium tracking-[0.01em]">{{ table.name }}</span>
                <!-- Open indicator -->
                <span
                  v-if="isTableOpen(table.name, db, activeConnId) && !isTableActive(table.name, db, activeConnId)"
                  class="size-1.5 rounded-full shrink-0 bg-primary/30"
                />
              </button>

              <div
                v-if="filteredTables(activeConnId, db).length === 0 && search"
                class="px-2 py-1 text-[10px] text-sidebar-foreground/30 italic"
              >
                No matches
              </div>
            </div>
          </div>
        </div>
      </template>

      <!-- Saved (closed) connections -->
      <template v-if="closedConnections.length > 0">
        <div
          class="border-t border-sidebar-border/50 my-1.5 mx-2"
          v-if="Object.keys(openConnections).length > 0"
        />
        <div class="px-3 pb-0.5">
          <span class="text-[10px] font-black text-sidebar-foreground/42 uppercase tracking-[0.18em]">
            Saved
          </span>
        </div>
        <div class="px-1 space-y-px">
          <div
            v-for="conn in closedConnections"
            :key="conn.id"
            class="flex items-center gap-0 group rounded overflow-hidden hover:bg-sidebar-accent/45 transition-colors"
            @dblclick="emit('connect-saved', conn)"
            @contextmenu="emit('context-menu-connection', $event, conn)"
          >
            <div class="flex-1 flex items-center gap-2 px-3 py-2 min-w-0">
              <DatabaseIcon class="size-3 shrink-0 text-sidebar-foreground/34" />
              <span class="text-sm truncate flex-1 text-sidebar-foreground/68 font-medium tracking-[0.01em]">
                {{ conn.name }}
              </span>
              <span
                class="inline-flex items-center gap-1 rounded-full px-1.5 py-0.5 shrink-0 opacity-95"
                :class="getEnvChipColor(conn.environment)"
              >
                <span
                  class="text-[9px] font-bold uppercase tracking-[0.08em]"
                  :class="getEnvTextColor(conn.environment)"
                >
                  {{ conn.environment }}
                </span>
              </span>
            </div>
          </div>
        </div>
      </template>
    </ScrollArea>

    <!-- Resize handle -->
    <div
      class="group absolute right-0 top-0 bottom-0 w-2 cursor-col-resize z-20 flex items-stretch justify-end"
      @mousedown.prevent="emit('resize-start', $event)"
    >
      <div class="w-px bg-sidebar-border group-hover:bg-primary/50 transition-colors" />
    </div>
  </aside>
</template>
