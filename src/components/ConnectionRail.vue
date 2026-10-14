<template>
  <aside class="w-20 shrink-0 bg-(--rail) border-r border-(--line-2) flex flex-col items-center select-none">
    <button
      class="flex h-11 w-full shrink-0 items-center justify-center border-b border-(--line-2) text-(--fg-3) hover:text-(--fg-1) hover:bg-(--bg-2) transition-colors"
      title="Workspace"
      @click="emit('home')"
    >
      <HomeIcon class="size-4" />
    </button>

    <div class="rail-items flex min-h-0 w-full flex-1 flex-col overflow-y-auto">
      <button
        v-for="item in railItems"
        :key="item.database === null ? `connection:${item.connectionId}` : `database:${item.connectionId}:${item.database}`"
        class="flex min-h-20 w-full shrink-0 flex-col items-center justify-center gap-2 border-b border-(--line-2) px-1.5 py-3 transition-colors hover:bg-(--bg-2) focus-visible:outline-2 focus-visible:outline-(--acc) focus-visible:outline-offset-[-2px]"
        :class="item.active ? 'bg-(--bg-2)' : ''"
        :aria-current="item.active ? 'page' : undefined"
        :title="`${item.connection.name} / ${item.database ?? 'Choose database'}`"
        @click="selectItem(item)"
        @contextmenu="openItemContextMenu($event, item)"
      >
        <component
          :is="item.database === null ? ServerIcon : DatabaseIcon"
          class="size-5 shrink-0"
          :class="getEnvRailColor(item.connection.environment)"
          :stroke-width="1.6"
        />
        <span class="rail-label w-full text-center text-[10.5px] font-medium leading-[13px]"
          :class="item.active ? 'text-(--fg-1)' : 'text-(--fg-2)'">
          {{ item.database ?? item.connection.name }}
        </span>
      </button>
    </div>
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

const getEnvRailColor = (env: Environment): string => {
  switch (env) {
    case "PRODUCTION": return "text-(--env-prod)";
    case "STAGING": return "text-(--env-staging)";
    case "DEV": return "text-(--env-dev)";
    default: return "text-(--acc)";
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

</script>

<style scoped>
.rail-items { scrollbar-width: none; }
.rail-items::-webkit-scrollbar { display: none; }
.rail-label { display: -webkit-box; -webkit-box-orient: vertical; -webkit-line-clamp: 2; overflow: hidden; overflow-wrap: anywhere; }
</style>
