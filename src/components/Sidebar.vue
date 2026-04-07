<script setup lang="ts">
import {
  SearchIcon,
  DatabaseIcon,
  ServerIcon,
  ChevronDownIcon,
  ChevronRightIcon,
  TableIcon,
  TerminalIcon,
  UploadIcon,
  DownloadIcon,
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
}>();

const emit = defineEmits<{
  "update:search": [val: string];
  "update:showNewDb": [val: string | null];
  "update:newDbName": [val: string];
  "new-connection": [];
  "connect-saved": [conn: Connection];
  "toggle-connection": [connId: string];
  "toggle-database": [connId: string, db: string];
  "load-table": [tableName: string, connId: string, db: string];
  "open-query": [connId: string, db: string];
  "import-sql": [connId: string, db: string];
  "export-database": [connId: string, db: string];
  "create-database": [connId: string];
  "context-menu-connection": [e: MouseEvent, conn: Connection];
  "context-menu-table": [
    e: MouseEvent,
    connId: string,
    db: string,
    tableName: string,
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

const dbKey = (connId: string, db: string) => `${connId}:${db}`;
</script>

<template>
  <aside class="w-72 flex flex-col border-r bg-muted/10">
    <!-- Header -->
    <div class="p-4 border-b bg-background/50 backdrop-blur-sm">
      <div class="flex items-center justify-between mb-3">
        <div class="flex items-center gap-2">
          <div
            class="size-8 rounded bg-primary/10 flex items-center justify-center text-primary shrink-0"
          >
            <ServerIcon class="size-4" />
          </div>
          <div>
            <h2 class="text-sm font-bold">Explorer</h2>
            <span class="text-[10px] text-muted-foreground">
              {{ Object.keys(openConnections).length }} connection{{
                Object.keys(openConnections).length !== 1 ? "s" : ""
              }}
            </span>
          </div>
        </div>
        <button
          @click="emit('new-connection')"
          class="size-7 flex items-center justify-center rounded-md text-muted-foreground hover:text-primary hover:bg-primary/10 transition-colors"
          title="New Connection"
        >
          <PlusIcon class="size-4" />
        </button>
      </div>
      <div v-if="Object.keys(openConnections).length > 0" class="relative">
        <SearchIcon
          class="absolute left-2.5 top-1/2 -translate-y-1/2 size-3.5 text-muted-foreground"
        />
        <Input
          :value="search"
          @input="
            emit('update:search', ($event.target as HTMLInputElement).value)
          "
          placeholder="Filter tables..."
          class="h-8 pl-8 text-xs bg-muted/50 border-none rounded-lg"
        />
      </div>
    </div>

    <!-- Tree -->
    <ScrollArea class="flex-1 p-2">
      <!-- Empty state -->
      <div
        v-if="
          Object.keys(openConnections).length === 0 &&
          closedConnections.length === 0
        "
        class="flex flex-col items-center justify-center py-12 px-4 text-center"
      >
        <PlugZapIcon class="size-8 text-muted-foreground/30 mb-3" />
        <p class="text-xs text-muted-foreground/60 font-medium">
          No connections saved
        </p>
      </div>

      <!-- Connected section -->
      <template v-if="Object.keys(openConnections).length > 0">
        <div class="px-2 py-1.5 mb-0.5">
          <span
            class="text-[10px] font-bold text-muted-foreground/50 uppercase tracking-widest"
            >Connected</span
          >
        </div>
      </template>

      <div
        v-for="(connState, connId) in openConnections"
        :key="connId"
        class="mb-1"
      >
        <!-- Connection header -->
        <div
          class="flex items-center gap-1 group rounded-md overflow-hidden hover:bg-muted/40 transition-colors select-none"
        >
          <span
            :class="[
              'w-1.5 h-full rounded-l-md',
              getEnvBorderColor(connState.connection.environment),
            ]"
          ></span>
          <button
            class="flex-1 flex items-center gap-2 px-2 py-1.5 min-w-0"
            @click="emit('toggle-connection', connId as string)"
            @contextmenu="
              emit('context-menu-connection', $event, connState.connection)
            "
          >
            <ChevronDownIcon
              v-if="expandedConnections.has(connId as string)"
              class="size-3.5 text-muted-foreground shrink-0 transition-transform"
            />
            <ChevronRightIcon
              v-else
              class="size-3.5 text-muted-foreground shrink-0 transition-transform"
            />
            <DatabaseIcon class="size-3.5 shrink-0 text-primary/70" />
            <span class="text-xs font-bold truncate flex-1 text-left">{{
              connState.connection.name
            }}</span>
            <Badge
              variant="outline"
              class="text-[10px] uppercase py-0.5 px-2 h-5 shrink-0"
            >
              {{ connState.connection.environment }}
            </Badge>
          </button>
        </div>

        <!-- Databases -->
        <div
          v-if="expandedConnections.has(connId as string)"
          class="ml-4 mt-0.5 space-y-0.5"
        >
          <!-- New DB form -->
          <div v-if="showNewDb === connId" class="px-1 pb-1">
            <div class="flex items-center gap-1">
              <Input
                :value="newDbName"
                @input="
                  emit(
                    'update:newDbName',
                    ($event.target as HTMLInputElement).value,
                  )
                "
                placeholder="database_name"
                class="h-7 text-xs bg-muted/50 border-none flex-1"
                autofocus
                @keyup.enter="emit('create-database', connId as string)"
                @keyup.escape="
                  emit('update:showNewDb', null);
                  emit('update:newDbName', '');
                "
              />
              <button
                class="flex items-center justify-center size-7 rounded-md bg-primary text-primary-foreground hover:bg-primary/90 transition-colors shrink-0"
                :disabled="isCreatingDb || !newDbName.trim()"
                @click="emit('create-database', connId as string)"
              >
                <CheckIcon class="size-3.5" />
              </button>
              <button
                class="flex items-center justify-center size-7 rounded-md hover:bg-muted/60 transition-colors text-muted-foreground shrink-0"
                @click="
                  emit('update:showNewDb', null);
                  emit('update:newDbName', '');
                "
              >
                <XIcon class="size-3.5" />
              </button>
            </div>
          </div>

          <!-- Add DB button -->
          <button
            v-if="showNewDb !== connId"
            class="w-full flex items-center gap-2 px-2 py-1 rounded text-[10px] text-muted-foreground/50 hover:text-muted-foreground transition-colors"
            @click="
              emit('update:showNewDb', connId as string);
              emit('update:newDbName', '');
            "
          >
            <PlusIcon class="size-3" /> New database
          </button>

          <!-- Databases -->
          <div v-for="db in connState.databases" :key="db">
            <div
              class="flex items-center gap-1 group/db rounded-md hover:bg-muted/30 transition-colors"
            >
              <button
                class="flex-1 flex items-center gap-2 px-2 py-1.5 min-w-0"
                @click="emit('toggle-database', connId as string, db)"
              >
                <ChevronDownIcon
                  v-if="expandedDatabases.has(dbKey(connId as string, db))"
                  class="size-3 text-muted-foreground shrink-0"
                />
                <ChevronRightIcon
                  v-else
                  class="size-3 text-muted-foreground/40 shrink-0"
                />
                <DatabaseIcon
                  class="size-3 shrink-0 text-muted-foreground/60"
                />
                <span class="text-xs truncate flex-1 text-left font-medium">{{
                  db
                }}</span>
              </button>
              <!-- DB action icons -->
              <div
                class="flex items-center gap-0.5 opacity-0 group-hover/db:opacity-100 transition-opacity shrink-0 mr-1"
              >
                <button
                  class="size-5 flex items-center justify-center rounded text-muted-foreground/50 hover:text-primary hover:bg-primary/10 transition-colors"
                  title="New Query"
                  @click.stop="emit('open-query', connId as string, db)"
                >
                  <TerminalIcon class="size-3" />
                </button>
                <button
                  class="size-5 flex items-center justify-center rounded text-muted-foreground/50 hover:text-foreground hover:bg-muted/60 transition-colors"
                  title="Import SQL"
                  @click.stop="emit('import-sql', connId as string, db)"
                >
                  <UploadIcon class="size-3" />
                </button>
                <button
                  class="size-5 flex items-center justify-center rounded text-muted-foreground/50 hover:text-foreground hover:bg-muted/60 transition-colors"
                  title="Export database"
                  @click.stop="emit('export-database', connId as string, db)"
                >
                  <DownloadIcon class="size-3" />
                </button>
              </div>
            </div>

            <!-- Tables -->
            <div
              v-if="expandedDatabases.has(dbKey(connId as string, db))"
              class="ml-4 space-y-0.5 mt-0.5"
            >
              <button
                v-for="table in filteredTables(connId as string, db)"
                :key="table.name"
                @click="emit('load-table', table.name, connId as string, db)"
                @contextmenu="
                  emit(
                    'context-menu-table',
                    $event,
                    connId as string,
                    db,
                    table.name,
                  )
                "
                :class="[
                  'w-full flex items-center gap-2 px-2 py-1.5 rounded-md text-xs transition-all text-left group/tbl',
                  isTableActive(table.name, db, connId as string)
                    ? 'bg-primary text-primary-foreground shadow-sm'
                    : 'hover:bg-primary/5 text-foreground',
                ]"
              >
                <TableIcon
                  :class="[
                    'size-3 shrink-0',
                    isTableActive(table.name, db, connId as string)
                      ? 'text-primary-foreground/70'
                      : 'text-muted-foreground group-hover/tbl:text-primary',
                  ]"
                />
                <span class="flex-1 truncate">{{ table.name }}</span>
                <span
                  v-if="isTableOpen(table.name, db, connId as string)"
                  class="size-1.5 rounded-full shrink-0"
                  :class="
                    isTableActive(table.name, db, connId as string)
                      ? 'bg-primary-foreground/60'
                      : 'bg-primary/40'
                  "
                />
              </button>

              <div
                v-if="
                  filteredTables(connId as string, db).length === 0 && search
                "
                class="px-2 py-1 text-[10px] text-muted-foreground/40 italic"
              >
                No matches
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- Saved (closed) connections -->
      <template v-if="closedConnections.length > 0">
        <div
          class="border-t border-muted/30 my-2"
          v-if="Object.keys(openConnections).length > 0"
        />
        <div class="px-2 py-1.5 mb-0.5">
          <span
            class="text-[10px] font-bold text-muted-foreground/50 uppercase tracking-widest"
            >Saved</span
          >
        </div>
        <div
          v-for="conn in closedConnections"
          :key="conn.id"
          class="flex items-center gap-1 group rounded-md hover:bg-muted/30 transition-colors mb-0.5 pr-1 select-none"
          @dblclick="emit('connect-saved', conn)"
          @contextmenu="emit('context-menu-connection', $event, conn)"
        >
          <div class="flex-1 flex items-center gap-2 px-2 py-1.5 min-w-0">
            <div
              class="size-1.5 rounded-full bg-muted-foreground/20 shrink-0"
            ></div>
            <DatabaseIcon class="size-3.5 shrink-0 text-muted-foreground/40" />
            <span class="text-xs truncate flex-1 text-muted-foreground/70">{{
              conn.name
            }}</span>
            <Badge
              variant="outline"
              :class="[
                getEnvColor(conn.environment),
                'text-[9px] uppercase py-0 px-1 h-3.5 shrink-0',
              ]"
            >
              {{ conn.environment }}
            </Badge>
          </div>
        </div>
      </template>
    </ScrollArea>
  </aside>
</template>
