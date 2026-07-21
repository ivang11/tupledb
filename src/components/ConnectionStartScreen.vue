<template>
  <main
    :class="[
      'min-w-0 overflow-hidden bg-background',
      overlay
        ? 'absolute inset-0 z-40 bg-(--bg-0)/96 backdrop-blur-sm'
        : 'flex-1 min-h-0',
    ]"
  >
    <button
      v-if="overlay"
      class="absolute right-4 top-4 z-50 size-8 rounded-md flex items-center justify-center text-(--fg-4) hover:text-(--fg-1) hover:bg-(--bg-2) transition-colors"
      title="Close"
      @click="emit('close')"
    >
      <XIcon class="size-4" />
    </button>

    <div class="mx-auto flex h-full max-w-3xl flex-col px-8 pt-12 pb-8">
      <!-- Brand -->
      <div class="wordmark text-3xl mb-5 flex shrink-0 items-baseline gap-3">
        <span>tuple<span class="wordmark-accent">db</span></span>
        <span
          class="font-mono text-[11px] text-(--fg-3) font-normal tracking-normal"
          >v{{ APP_VERSION }} · {{ connections.length }} connection{{
            connections.length === 1 ? "" : "s"
          }}</span
        >
      </div>

      <!-- Big search -->
      <div
        class="z-10 flex shrink-0 items-center gap-3 px-4 py-3.5 rounded-xl bg-(--bg-1) border border-(--line-1) shadow-[0_6px_24px_rgba(0,0,0,0.25)]"
      >
        <SearchIcon class="size-4 text-(--fg-3) shrink-0" />
        <input
          v-model="search"
          class="flex-1 border-0 outline-none text-(--fg-1) text-[15px] placeholder:text-(--fg-3)"
          :placeholder="`Search ${connections.length} connections or paste a connection string…`"
        />
        <kbd
          class="font-mono text-[10px] text-(--fg-3) px-1.5 py-0.5 border border-(--line-1) rounded"
          >⌘K</kbd
        >
      </div>

      <!-- Quick actions -->
      <div class="mt-4 flex shrink-0 items-center gap-2">
        <button
          class="h-7 px-2.5 rounded-md flex items-center gap-1.5 text-(--fg-3) hover:text-(--fg-1) hover:bg-(--bg-2) text-[11px] transition-colors"
          title="Import connections"
          @click="emit('import-connections')"
        >
          <FileInputIcon class="size-3" />
          Import
        </button>
        <button
          class="h-7 px-2.5 rounded-md flex items-center gap-1.5 text-(--fg-3) hover:text-(--fg-1) hover:bg-(--bg-2) text-[11px] transition-colors"
          title="Export connections"
          @click="emit('export-connections')"
        >
          <FileOutputIcon class="size-3" />
          Export
        </button>
        <div class="flex-1" />
        <button
          class="h-7 px-3 rounded-md bg-(--acc) text-(--acc-fg) text-[11px] font-semibold flex items-center gap-1.5 hover:brightness-105 transition-all"
          @click="emit('new-connection')"
        >
          <PlusIcon class="size-3" />
          New connection
        </button>
      </div>

      <div ref="listRoot" class="mt-5 min-h-0 flex-1 overflow-y-auto pr-1 pb-8 custom-scrollbar">
        <!-- Recent section -->
        <template v-if="recentConnections.length > 0">
          <div class="mb-1.5 flex items-center gap-2.5">
            <span
              class="text-[10px] tracking-[0.14em] uppercase text-(--fg-3) font-semibold"
              >Recent</span
            >
            <div class="flex-1 h-px bg-(--line-2)" />
            <span class="font-mono text-[10px] text-(--fg-3)">{{
              recentConnections.length
            }}</span>
          </div>
          <div>
            <button
              v-for="conn in recentConnections"
              :key="`r-${conn.id}`"
              type="button"
              :data-connection-id="conn.id"
              :class="[
                connectionRowClass,
                activeConnectionId === conn.id ? 'bg-(--bg-2)' : '',
              ]"
              :tabindex="isConnectionTabbable(conn) ? 0 : -1"
              @focus="activeConnectionId = conn.id"
              @keydown="onConnectionKeydown($event, conn)"
              @dblclick="handleConnect(conn)"
              @contextmenu="emit('context-menu-connection', $event, conn)"
            >
              <span class="eng-glyph" :data-engine="engineOf(conn)">{{
                ENG_ABBR[engineOf(conn)]
              }}</span>
              <div class="min-w-0">
                <div class="flex items-center gap-2">
                  <span
                    class="text-(--fg-1) font-medium text-[13.5px] truncate"
                    >{{ conn.name }}</span
                  >
                  <span
                    class="size-1.5 rounded-full shrink-0"
                    :class="envDot(conn.environment)"
                  />
                </div>
                <div class="font-mono text-[10.5px] text-(--fg-3)">
                  {{ engineOf(conn) }} · {{ conn.environment.toLowerCase() }}
                </div>
              </div>
              <div class="font-mono text-[11px] text-(--fg-2) truncate">
                {{ hostOf(conn) }}
              </div>
              <div class="flex items-center gap-3 shrink-0">
                <span
                  v-if="connectingId === conn.id"
                  class="font-mono text-[10px] text-(--acc)"
                  >connecting…</span
                >
                <span
                  v-else-if="openConnectionIds.includes(conn.id)"
                  class="font-mono text-[10px] text-(--acc)"
                  >● connected</span
                >
                <span v-else class="font-mono text-[10px] text-(--fg-3)"
                  >—</span
                >
                <span class="font-mono text-[10px] text-(--fg-3)">↵</span>
              </div>
            </button>
          </div>
        </template>

<!-- All section -->
      <div
        v-if="restConnections.length > 0 || filteredConnections.length === 0"
        class="mt-5 mb-1.5 flex items-center gap-2.5"
      >
        <span
          class="text-[10px] tracking-[0.14em] uppercase text-(--fg-3) font-semibold"
          >All connections</span
        >
        <div class="flex-1 h-px bg-(--line-2)" />
        <span class="font-mono text-[10px] text-(--fg-3)">{{
          restConnections.length
        }}</span>
        <div class="flex items-center gap-px bg-(--bg-1) rounded p-0.5">
          <button
            v-for="opt in ['name', 'engine', 'env'] as const"
            :key="opt"
            class="text-[10px] px-1.5 py-0.5 rounded transition-colors cursor-pointer"
            :class="
              sortMode === opt
                ? 'bg-(--bg-3) text-(--fg-1)'
                : 'text-(--fg-3) hover:text-(--fg-1)'
            "
            @click="sortMode = opt"
          >
            {{ opt === "name" ? "Name" : opt === "engine" ? "Engine" : "Env" }}
          </button>
        </div>
      </div>

      <button
        v-for="conn in restConnections"
        :key="conn.id"
        type="button"
        :data-connection-id="conn.id"
        :class="[
          connectionRowClass,
          activeConnectionId === conn.id ? 'bg-(--bg-2)' : '',
        ]"
        :tabindex="isConnectionTabbable(conn) ? 0 : -1"
        @focus="activeConnectionId = conn.id"
        @keydown="onConnectionKeydown($event, conn)"
        @dblclick="handleConnect(conn)"
        @contextmenu="emit('context-menu-connection', $event, conn)"
      >
        <span class="eng-glyph" :data-engine="engineOf(conn)">{{
          ENG_ABBR[engineOf(conn)]
        }}</span>
        <div class="min-w-0">
          <div class="flex items-center gap-2">
            <span
              class="text-(--fg-1) font-medium text-[13.5px] truncate"
              >{{ conn.name }}</span
            >
            <span
              class="size-1.5 rounded-full shrink-0"
              :class="envDot(conn.environment)"
            />
          </div>
          <div class="font-mono text-[10.5px] text-(--fg-3)">
            {{ engineOf(conn) }} · {{ conn.environment.toLowerCase() }}
          </div>
        </div>
        <div class="font-mono text-[11px] text-(--fg-2) truncate">
          {{ hostOf(conn) }}
        </div>
        <div class="flex items-center gap-3 shrink-0">
          <span
            v-if="connectingId === conn.id"
            class="font-mono text-[10px] text-(--acc)"
            >connecting…</span
          >
          <span
            v-else-if="openConnectionIds.includes(conn.id)"
            class="font-mono text-[10px] text-(--acc)"
            >● connected</span
          >
          <span v-else class="font-mono text-[10px] text-(--fg-3)">—</span>
        </div>
      </button>

      <div
        v-if="filteredConnections.length === 0 && connections.length > 0"
        class="mt-8 text-center text-xs text-(--fg-3)"
      >
        No connections match "{{ search }}".
      </div>

        <div
          v-if="connections.length === 0"
          class="py-16 flex flex-col items-center justify-center text-center text-(--fg-4) bg-(--bg-1) rounded-lg mt-6"
        >
          <PlugZapIcon class="size-9 opacity-35 mb-3" />
          <p class="text-sm font-medium">No connections yet</p>
          <button
            class="mt-3 text-xs text-(--acc) hover:brightness-110"
            @click="emit('new-connection')"
          >
            Add connection
          </button>
        </div>
      </div>
    </div>
  </main>
</template>

<script setup lang="ts">
import { computed, nextTick, ref } from "vue";
import {
  FileInputIcon,
  FileOutputIcon,
  XIcon,
  PlugZapIcon,
  PlusIcon,
  SearchIcon,
} from "lucide-vue-next";
import type { Connection, Environment } from "@/types/connection";

const props = defineProps<{
  connections: Connection[];
  openConnectionIds: string[];
  selectedConnectionId: string | null;
  connectingId: string | null;
  overlay?: boolean;
}>();

const emit = defineEmits<{
  close: [];
  "connect-saved": [conn: Connection];
  "new-connection": [];
  "context-menu-connection": [e: MouseEvent, conn: Connection];
  "export-connections": [];
  "import-connections": [];
}>();

const RECENT_KEY = "tupledb:recent-connection-ids";
const MAX_RECENT = 4;
const APP_VERSION = __APP_VERSION__;

function loadRecentIds(): string[] {
  try {
    const raw = localStorage.getItem(RECENT_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    return Array.isArray(parsed)
      ? parsed.filter((v) => typeof v === "string")
      : [];
  } catch {
    return [];
  }
}

const recentIds = ref<string[]>(loadRecentIds());

function recordRecent(conn: Connection) {
  const next = [
    conn.id,
    ...recentIds.value.filter((id) => id !== conn.id),
  ].slice(0, MAX_RECENT);
  recentIds.value = next;
  try {
    localStorage.setItem(RECENT_KEY, JSON.stringify(next));
  } catch {}
}

function handleConnect(conn: Connection) {
  recordRecent(conn);
  emit("connect-saved", conn);
}

const search = ref("");
const listRoot = ref<HTMLElement | null>(null);
const activeConnectionId = ref<string | null>(props.selectedConnectionId);

const sortMode = ref<"name" | "engine" | "env">("name");

function envDot(env: Environment): string {
  switch (env) {
    case "PRODUCTION":
      return "bg-(--env-prod)";
    case "STAGING":
      return "bg-(--env-staging)";
    case "DEV":
      return "bg-(--env-dev)";
    default:
      return "bg-(--env-local)";
  }
}

const ENG_ABBR: Record<string, string> = {
  mysql: "My",
  postgres: "Pg",
  sqlite: "Sl",
  mssql: "Ms",
  mongo: "Mo",
};

function engineOf(_conn: Connection): string {
  // For now only MySQL is supported, but the design wants engine glyphs ready for multi-DB.
  return "mysql";
}

function hostOf(conn: Connection): string {
  return `${conn.mysql.user}@${conn.mysql.host}:${conn.mysql.port}`;
}

const filteredConnections = computed(() => {
  const q = search.value.trim().toLowerCase();
  if (!q) return props.connections;
  return props.connections.filter(
    (c) =>
      c.name.toLowerCase().includes(q) ||
      c.mysql.host.toLowerCase().includes(q) ||
      c.mysql.user.toLowerCase().includes(q) ||
      c.environment.toLowerCase().includes(q) ||
      engineOf(c).toLowerCase().includes(q),
  );
});

const recentConnections = computed(() => {
  const visible = new Map(filteredConnections.value.map((c) => [c.id, c]));
  const recents: Connection[] = [];
  for (const id of recentIds.value) {
    const conn = visible.get(id);
    if (conn) recents.push(conn);
    if (recents.length >= MAX_RECENT) break;
  }
  return recents;
});

const restConnections = computed(() => {
  const recentSet = new Set(recentConnections.value.map((c) => c.id));
  const rest = filteredConnections.value.filter((c) => !recentSet.has(c.id));
  const sorted = [...rest];
  if (sortMode.value === "name") {
    sorted.sort((a, b) => a.name.localeCompare(b.name));
  } else if (sortMode.value === "engine") {
    sorted.sort(
      (a, b) =>
        engineOf(a).localeCompare(engineOf(b)) || a.name.localeCompare(b.name),
    );
  } else if (sortMode.value === "env") {
    sorted.sort(
      (a, b) =>
        a.environment.localeCompare(b.environment) ||
        a.name.localeCompare(b.name),
    );
  }
  return sorted;
});

const visibleConnections = computed(() => [
  ...recentConnections.value,
  ...restConnections.value,
]);

const activeConnectionIsVisible = computed(() =>
  activeConnectionId.value !== null &&
  visibleConnections.value.some((conn) => conn.id === activeConnectionId.value),
);

const connectionRowClass =
  "w-full grid grid-cols-[28px_1.2fr_1.6fr_auto] gap-3.5 items-center text-left px-2.5 py-2 rounded-lg transition-colors hover:bg-(--bg-2) focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-inset focus-visible:ring-(--acc)";

function focusConnection(id: string) {
  activeConnectionId.value = id;
  nextTick(() => {
    const buttons = Array.from(
      listRoot.value?.querySelectorAll<HTMLButtonElement>("[data-connection-id]") ?? [],
    );
    buttons.find((button) => button.dataset.connectionId === id)?.focus();
  });
}

function moveConnectionFocus(currentId: string, delta: number) {
  const connections = visibleConnections.value;
  if (connections.length === 0) return;

  const currentIndex = connections.findIndex((conn) => conn.id === currentId);
  const startIndex = currentIndex === -1 ? 0 : currentIndex;
  const nextIndex = (startIndex + delta + connections.length) % connections.length;
  focusConnection(connections[nextIndex].id);
}

function onConnectionKeydown(e: KeyboardEvent, conn: Connection) {
  if (e.key === "Enter" || e.key === " ") {
    e.preventDefault();
    handleConnect(conn);
    return;
  }

  if (e.key === "ArrowDown") {
    e.preventDefault();
    moveConnectionFocus(conn.id, 1);
  } else if (e.key === "ArrowUp") {
    e.preventDefault();
    moveConnectionFocus(conn.id, -1);
  } else if (e.key === "Home" && visibleConnections.value.length > 0) {
    e.preventDefault();
    focusConnection(visibleConnections.value[0].id);
  } else if (e.key === "End" && visibleConnections.value.length > 0) {
    e.preventDefault();
    focusConnection(visibleConnections.value[visibleConnections.value.length - 1].id);
  }
}

function isConnectionTabbable(conn: Connection) {
  return !activeConnectionIsVisible.value || activeConnectionId.value === conn.id;
}
</script>
