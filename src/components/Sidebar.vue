<script setup lang="ts">
import { ref } from "vue";
import {
  SearchIcon,
  DatabaseIcon,
  ServerIcon,
  ChevronDownIcon,
  ChevronRightIcon,
  TableIcon,
  PlusIcon,
  CheckIcon,
  XIcon,
  PlugZapIcon,
} from "lucide-vue-next";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import type { Connection, Environment } from "@/types/connection";

const props = defineProps<{
  width: number;
  search: string;
  openConnections: Record<
    string,
    {
      connection: Connection;
      databases: string[];
      tables: Record<string, any[]>;
    }
  >;
  closedConnections: Connection[];
  expandedConnections: Set<string>;
  expandedDatabases: Set<string>;
  connectingId: string | null;
  showNewDb: string | null;
  newDbName: string;
  isCreatingDb: boolean;
  isTableActive: (name: string, db: string, connId: string) => boolean;
  isTableOpen: (name: string, db: string, connId: string) => boolean;
  filteredTables: (connId: string, db: string) => any[];
  isTableSelected: (connId: string, db: string, tableName: string) => boolean;
}>();

const emit = defineEmits<{
  "resize-start": [e: MouseEvent];
  "update:search": [val: string];
  "update:showNewDb": [val: string | null];
  "update:newDbName": [val: string];
  "new-connection": [];
  "connect-saved": [conn: Connection];
  "toggle-connection": [connId: string];
  "toggle-database": [connId: string, db: string];
  "load-table": [tableName: string, connId: string, db: string];
  "create-database": [connId: string];
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
}>();

const getEnvColor = (env: Environment): string => {
  switch (env) {
    case "PRODUCTION":
      return "bg-red-500/10 text-red-500";
    case "STAGING":
      return "bg-orange-500/10 text-orange-500";
    case "DEV":
      return "bg-blue-500/10 text-blue-500";
    default:
      return "bg-green-500/10 text-green-500";
  }
};

const getEnvBorderColor = (env: Environment): string => {
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
</script>

<template>
  <aside
    class="shrink-0 flex flex-col border-r border-sidebar-border bg-sidebar relative select-none"
    :style="{ width: width + 'px' }"
  >
    <!-- Header -->
    <div class="shrink-0 border-b border-sidebar-border">
      <!-- Top row: title + add button -->
      <div class="flex items-center gap-2 px-3 h-9">
        <div class="flex items-center gap-2 min-w-0 flex-1">
          <ServerIcon class="size-3.5 text-primary/60 shrink-0" />
          <span class="text-xs font-bold text-sidebar-foreground/70 uppercase tracking-widest truncate">
            Connections
          </span>
          <span
            v-if="Object.keys(openConnections).length > 0"
            class="text-[11px] font-bold text-primary/60 tabular-nums"
          >
            {{ Object.keys(openConnections).length }}
          </span>
        </div>
        <button
          @click="emit('new-connection')"
          class="size-6 flex items-center justify-center rounded text-sidebar-foreground/40 hover:text-primary hover:bg-primary/10 transition-colors shrink-0"
          title="New Connection"
        >
          <PlusIcon class="size-3.5" />
        </button>
      </div>

      <!-- Search bar -->
      <div
        v-if="Object.keys(openConnections).length > 0"
        ref="searchContainerRef"
        class="px-2 pb-2"
      >
        <div class="relative">
          <SearchIcon
            class="absolute left-2.5 top-1/2 -translate-y-1/2 size-3 text-sidebar-foreground/30"
          />
          <Input
            :value="search"
            @input="emit('update:search', ($event.target as HTMLInputElement).value)"
            placeholder="Filter tables..."
            class="h-7 pl-8 text-xs bg-sidebar-accent/50 border-none rounded focus-visible:ring-1 focus-visible:ring-primary/30"
          />
        </div>
      </div>
    </div>

    <!-- Tree -->
    <ScrollArea class="flex-1 py-1">
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

      <!-- Connected section -->
      <template v-if="Object.keys(openConnections).length > 0">
        <div class="px-3 pt-1 pb-0.5">
          <span class="text-[10px] font-bold text-sidebar-foreground/30 uppercase tracking-widest">
            Connected
          </span>
        </div>
      </template>

      <div
        v-for="(connState, connId) in openConnections"
        :key="connId"
        class="mb-0.5 px-1"
      >
        <!-- Connection header -->
        <div
          class="flex items-center gap-0 group rounded-md overflow-hidden hover:bg-sidebar-accent/60 transition-colors"
        >
          <span
            :class="['w-1 self-stretch rounded-l shrink-0', getEnvBorderColor(connState.connection.environment)]"
          />
          <button
            class="flex-1 flex items-center gap-1.5 px-2 py-1.5 min-w-0"
            @click="emit('toggle-connection', connId as string)"
            @contextmenu="emit('context-menu-connection', $event, connState.connection)"
          >
            <ChevronDownIcon
              v-if="expandedConnections.has(connId as string)"
              class="size-3 text-sidebar-foreground/40 shrink-0"
            />
            <ChevronRightIcon
              v-else
              class="size-3 text-sidebar-foreground/30 shrink-0"
            />
            <DatabaseIcon class="size-3 shrink-0 text-primary/60" />
            <span class="text-sm font-semibold truncate flex-1 text-left text-sidebar-foreground">
              {{ connState.connection.name }}
            </span>
            <Badge
              variant="outline"
              class="text-[9px] uppercase py-0 px-1.5 h-4 shrink-0 font-bold"
              :class="getEnvColor(connState.connection.environment)"
            >
              {{ connState.connection.environment }}
            </Badge>
          </button>
        </div>

        <!-- Databases -->
        <div
          v-if="expandedConnections.has(connId as string)"
          class="ml-3 mt-0.5 space-y-px"
        >
          <!-- New DB form -->
          <div v-if="showNewDb === connId" class="px-1 py-1">
            <div class="flex items-center gap-1">
              <Input
                :value="newDbName"
                @input="emit('update:newDbName', ($event.target as HTMLInputElement).value)"
                placeholder="database_name"
                class="h-7 text-xs bg-sidebar-accent/50 border-none flex-1"
                autofocus
                @keyup.enter="emit('create-database', connId as string)"
                @keyup.escape="emit('update:showNewDb', null); emit('update:newDbName', '')"
              />
              <button
                class="flex items-center justify-center size-6 rounded bg-primary text-primary-foreground hover:bg-primary/90 transition-colors shrink-0"
                :disabled="isCreatingDb || !newDbName.trim()"
                @click="emit('create-database', connId as string)"
              >
                <CheckIcon class="size-3" />
              </button>
              <button
                class="flex items-center justify-center size-6 rounded hover:bg-sidebar-accent transition-colors text-sidebar-foreground/50 shrink-0"
                @click="emit('update:showNewDb', null); emit('update:newDbName', '')"
              >
                <XIcon class="size-3" />
              </button>
            </div>
          </div>

          <!-- Databases -->
          <div v-for="db in connState.databases" :key="db">
            <div class="flex items-center group/db rounded hover:bg-sidebar-accent/40 transition-colors">
              <button
                class="flex-1 flex items-center gap-1.5 px-2 py-1 min-w-0"
                @click="emit('toggle-database', connId as string, db)"
                @contextmenu="emit('context-menu-database', $event, connId as string, db)"
              >
                <ChevronDownIcon
                  v-if="expandedDatabases.has(dbKey(connId as string, db))"
                  class="size-2.5 text-sidebar-foreground/40 shrink-0"
                />
                <ChevronRightIcon
                  v-else
                  class="size-2.5 text-sidebar-foreground/20 shrink-0"
                />
                <DatabaseIcon class="size-2.5 shrink-0 text-sidebar-foreground/40" />
                <span class="text-sm truncate flex-1 text-left font-medium text-sidebar-foreground/80">
                  {{ db }}
                </span>
              </button>
            </div>

            <!-- Tables -->
            <div
              v-if="expandedDatabases.has(dbKey(connId as string, db))"
              class="ml-6 space-y-px mt-0.5 mb-1"
            >
              <button
                v-for="table in filteredTables(connId as string, db)"
                :key="table.name"
                :ref="(el) => setTableRef(el as HTMLElement | null, table.name, db, connId as string)"
                @click="handleTableClick($event, connId as string, db, table.name)"
                @contextmenu="emit('context-menu-table', $event, connId as string, db, table.name)"
                :class="[
                  'w-full flex items-center gap-1.5 px-2 py-1 rounded text-xs transition-all text-left group/tbl',
                  isTableSelected(connId as string, db, table.name)
                    ? 'bg-primary/20 text-primary ring-1 ring-inset ring-primary/30'
                    : isTableActive(table.name, db, connId as string)
                      ? 'bg-primary text-primary-foreground'
                      : 'hover:bg-sidebar-accent/50 text-sidebar-foreground/70 hover:text-sidebar-foreground',
                ]"
              >
                <TableIcon
                  :class="[
                    'size-2.5 shrink-0',
                    isTableActive(table.name, db, connId as string)
                      ? 'text-primary-foreground/70'
                      : isTableSelected(connId as string, db, table.name)
                        ? 'text-primary'
                        : 'text-sidebar-foreground/30 group-hover/tbl:text-sidebar-foreground/60',
                  ]"
                />
                <span class="flex-1 truncate text-xs">{{ table.name }}</span>
                <!-- Open indicator -->
                <span
                  v-if="isTableOpen(table.name, db, connId as string) && !isTableActive(table.name, db, connId as string)"
                  class="size-1.5 rounded-full shrink-0 bg-primary/30"
                />
              </button>

              <div
                v-if="filteredTables(connId as string, db).length === 0 && search"
                class="px-2 py-1 text-[10px] text-sidebar-foreground/30 italic"
              >
                No matches
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- Saved (closed) connections -->
      <template v-if="closedConnections.length > 0">
        <div class="border-t border-sidebar-border/50 my-1.5 mx-2" v-if="Object.keys(openConnections).length > 0" />
        <div class="px-3 pb-0.5">
          <span class="text-[9px] font-bold text-sidebar-foreground/25 uppercase tracking-widest">
            Saved
          </span>
        </div>
        <div class="px-1 space-y-px">
          <div
            v-for="conn in closedConnections"
            :key="conn.id"
            class="flex items-center gap-0 group rounded overflow-hidden hover:bg-sidebar-accent/40 transition-colors"
            @dblclick="emit('connect-saved', conn)"
            @contextmenu="emit('context-menu-connection', $event, conn)"
          >
            <div class="flex-1 flex items-center gap-1.5 px-3 py-1.5 min-w-0">
              <div class="size-1.5 rounded-full bg-sidebar-foreground/15 shrink-0" />
              <DatabaseIcon class="size-3 shrink-0 text-sidebar-foreground/25" />
              <span class="text-sm truncate flex-1 text-sidebar-foreground/40 font-medium">
                {{ conn.name }}
              </span>
              <Badge
                variant="outline"
                :class="[getEnvColor(conn.environment), 'text-[9px] uppercase py-0 px-1 h-3.5 shrink-0 opacity-60']"
              >
                {{ conn.environment }}
              </Badge>
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
