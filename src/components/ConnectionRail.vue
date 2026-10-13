<template>
  <aside class="w-12 shrink-0 bg-(--rail) border-r border-(--line-2) flex flex-col items-center py-2 select-none">
    <div class="flex flex-col gap-1 mb-2">
      <button
        class="size-8 rounded-md flex items-center justify-center text-(--fg-3) hover:text-(--fg-1) hover:bg-(--bg-2) transition-colors"
        title="Workspace"
        @click="emit('home')"
      >
        <HomeIcon class="size-3.5" />
      </button>
    </div>

    <div class="flex flex-col gap-1">
      <button
        v-for="item in railItems"
        :key="item.database === null ? `connection:${item.connectionId}` : `database:${item.connectionId}:${item.database}`"
        class="relative size-8 rounded-md flex items-center justify-center transition-colors ring-1 ring-inset"
        :class="[
          getEnvRailColor(item.connection.environment, item.active),
          item.active ? 'ring-white/15' : 'ring-white/5',
        ]"
        :title="`${item.connection.name} / ${item.database ?? 'Choose database'}`"
        @click="selectItem(item)"
        @contextmenu="openItemContextMenu($event, item)"
      >
        <span class="text-[10.5px] font-bold uppercase">{{ initial(item.database ?? item.connection.name) }}</span>
        <component :is="item.database === null ? ServerIcon : DatabaseIcon" class="absolute right-1 bottom-1 size-2 opacity-60" />
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
        ? "bg-red-600 text-white"
        : "bg-red-500/14 text-red-200/80 hover:bg-red-500/24 hover:text-red-50";
    case "STAGING":
      return active
        ? "bg-orange-600 text-white"
        : "bg-orange-500/14 text-orange-200/80 hover:bg-orange-500/24 hover:text-orange-50";
    case "DEV":
      return active
        ? "bg-blue-600 text-white"
        : "bg-blue-500/14 text-blue-200/80 hover:bg-blue-500/24 hover:text-blue-50";
    default:
      return active
        ? "bg-(--acc) text-white"
        : "bg-green-500/14 text-green-200/80 hover:bg-green-500/24 hover:text-green-50";
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
