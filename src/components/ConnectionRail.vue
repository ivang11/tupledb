<template>
  <aside class="w-16 shrink-0 bg-[#101820] flex flex-col items-center py-3 select-none">
    <div class="flex flex-col gap-2 mb-4">
      <button
        class="size-10 rounded-lg flex items-center justify-center text-sidebar-foreground/75 hover:text-white hover:bg-[#243241] transition-colors"
        title="Workspace"
        @click="emit('home')"
      >
        <HomeIcon class="size-4" />
      </button>
    </div>

    <div class="flex flex-col gap-2">
      <button
        v-for="item in railItems"
        :key="item.database === null ? `connection:${item.connectionId}` : `database:${item.connectionId}:${item.database}`"
        class="relative size-11 rounded-lg flex items-center justify-center transition-colors ring-1 ring-inset"
        :class="[
          getEnvRailColor(item.connection.environment, item.active),
          item.active ? 'ring-white/18' : 'ring-white/8',
        ]"
        :title="`${item.connection.name} / ${item.database ?? 'Choose database'}`"
        @click="selectItem(item)"
        @contextmenu="openItemContextMenu($event, item)"
      >
        <span class="text-[11px] font-black uppercase">{{ initial(item.database ?? item.connection.name) }}</span>
        <component :is="item.database === null ? ServerIcon : DatabaseIcon" class="absolute right-1.5 bottom-1.5 size-2.5 opacity-55" />
      </button>
    </div>
    <div class="mt-auto" />
  </aside>
</template>

<script setup lang="ts">
import { computed } from "vue";
import {
  DatabaseIcon,
  HomeIcon,
  ServerIcon,
} from "lucide-vue-next";
import type { Connection, Environment } from "@/types/connection";

const props = defineProps<{
  openConnections: Record<
    string,
    {
      connection: Connection;
      databases: string[];
      selectedDatabase: string | null;
      openedDatabases?: string[];
      tables: Record<string, any[]>;
    }
  >;
  selectedConnectionId: string | null;
}>();

const emit = defineEmits<{
  "select-connection": [connId: string];
  "select-database": [connId: string, db: string];
  home: [];
  "context-menu-connection": [e: MouseEvent, conn: Connection];
  "context-menu-database": [e: MouseEvent, connId: string, db: string];
}>();

const getEnvRailColor = (env: Environment, active: boolean): string => {
  switch (env) {
    case "PRODUCTION":
      return active
        ? "bg-red-500/90 text-white shadow-[0_0_22px_rgba(239,68,68,0.18)]"
        : "bg-red-500/16 text-red-100/70 hover:bg-red-500/28 hover:text-red-50";
    case "STAGING":
      return active
        ? "bg-orange-500/90 text-white shadow-[0_0_22px_rgba(249,115,22,0.18)]"
        : "bg-orange-500/16 text-orange-100/70 hover:bg-orange-500/28 hover:text-orange-50";
    case "DEV":
      return active
        ? "bg-blue-500/90 text-white shadow-[0_0_22px_rgba(59,130,246,0.18)]"
        : "bg-blue-500/16 text-blue-100/70 hover:bg-blue-500/28 hover:text-blue-50";
    default:
      return active
        ? "bg-[#1f6a44] text-white shadow-[0_0_22px_rgba(74,222,128,0.18)]"
        : "bg-green-500/16 text-green-100/70 hover:bg-green-500/28 hover:text-green-50";
  }
};

interface RailItem {
  connectionId: string;
  connection: Connection;
  database: string | null;
  active: boolean;
}

const railItems = computed<RailItem[]>(() =>
  Object.entries(props.openConnections).flatMap(([connectionId, state]) => {
    const databases =
      state.openedDatabases?.length
        ? state.openedDatabases
        : state.selectedDatabase
          ? [state.selectedDatabase]
          : [];

    const items: (string | null)[] = databases.length ? databases : [null];
    return items.map((database) => ({
      connectionId,
      connection: state.connection,
      database,
      active: props.selectedConnectionId === connectionId && (database === null || state.selectedDatabase === database),
    }));
  }),
);

function selectItem(item: RailItem) {
  if (item.database === null) {
    emit('select-connection', item.connectionId);
  } else {
    emit('select-database', item.connectionId, item.database);
  }
}

function openItemContextMenu(event: MouseEvent, item: RailItem) {
  if (item.database === null) {
    emit('context-menu-connection', event, item.connection);
  } else {
    emit('context-menu-database', event, item.connectionId, item.database);
  }
}

function initial(name: string) {
  return name.slice(0, 1).toUpperCase();
}
</script>
