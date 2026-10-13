<template>
  <main
    :class="[
      'min-w-0 overflow-hidden bg-background',
      overlay ? 'absolute inset-0 z-40' : 'relative flex-1 min-h-0',
    ]"
  >
    <button
      v-if="overlay"
      class="absolute right-3 top-3 z-50 flex size-7 items-center justify-center rounded-md text-(--fg-3) transition-colors hover:bg-(--bg-2) hover:text-(--fg-1)"
      title="Close"
      aria-label="Close"
      @click="emit('close')"
    ><XIcon class="size-3.5" /></button>

    <div class="custom-scrollbar h-full overflow-y-auto">
      <div class="mx-auto flex min-h-full w-full max-w-[640px] flex-col px-6 pb-10 pt-14">
        <!-- Header -->
        <div class="flex items-center justify-between gap-4">
          <div class="flex items-baseline gap-2.5">
            <span class="wordmark text-[24px] leading-none">tuple<span class="wordmark-accent">db</span></span>
            <span class="text-[11.5px] tabular-nums text-(--fg-4)">v{{ APP_VERSION }}</span>
          </div>
          <div class="flex items-center gap-1">
            <button
              class="h-8 rounded-lg px-2.5 text-[12.5px] text-(--fg-3) transition-colors hover:bg-(--bg-1) hover:text-(--fg-1)"
              @click="emit('import-connections')"
            >Import</button>
            <button
              class="h-8 rounded-lg px-2.5 text-[12.5px] text-(--fg-3) transition-colors hover:bg-(--bg-1) hover:text-(--fg-1)"
              @click="emit('export-connections')"
            >Export</button>
            <button
              class="ml-1 flex h-8 items-center gap-1.5 rounded-lg bg-(--acc) pl-2.5 pr-3 text-[12.5px] font-medium text-(--acc-fg) shadow-[inset_0_1px_0_rgba(255,255,255,0.14)] transition-[filter] hover:brightness-110"
              @click="emit('new-connection')"
            >
              <PlusIcon class="size-3.5" />
              New connection
            </button>
          </div>
        </div>

        <!-- Search -->
        <div
          class="mt-6 flex h-10 items-center gap-2.5 rounded-xl bg-(--bg-1) px-3.5 ring-1 ring-inset ring-(--line-faint) transition-shadow focus-within:ring-(--acc-line)"
        >
          <SearchIcon class="size-4 shrink-0 text-(--fg-4)" />
          <input
            v-model="search"
            class="min-w-0 flex-1 border-0 bg-transparent text-[13.5px] text-(--fg-1) outline-none placeholder:text-(--fg-4)"
            placeholder="Search connections"
          />
          <kbd class="rounded-md bg-(--bg-2) px-1.5 py-0.5 font-mono text-[10.5px] text-(--fg-4)">⌘K</kbd>
        </div>

        <!-- Filters + sort -->
        <div class="mt-3 flex items-center gap-1">
          <button
            v-for="chip in envChips"
            :key="chip.value ?? 'all'"
            class="h-6 rounded-full px-2.5 text-[11.5px] font-medium transition-colors"
            :class="envFilter === chip.value
              ? 'bg-(--fg-1) text-(--bg-0)'
              : 'text-(--fg-3) hover:bg-(--bg-1) hover:text-(--fg-1)'"
            @click="envFilter = chip.value"
          >{{ chip.label }}</button>

          <div class="flex-1" />

          <span v-if="canReorder && visibleConnections.length > 1" class="mr-2 text-[11px] text-(--fg-5)">Drag to reorder</span>
          <button
            ref="sortButton"
            aria-label="Sort connections"
            aria-haspopup="menu"
            :aria-expanded="sortMenuOpen"
            class="flex h-6 items-center gap-1 rounded-full pl-2.5 pr-1.5 text-[11.5px] text-(--fg-3) transition-colors hover:bg-(--bg-1) hover:text-(--fg-1)"
            @click="openSortMenu"
          >
            {{ SORT_OPTIONS.find((opt) => opt.value === sortMode)?.label }}
            <ChevronDownIcon class="size-3.5" />
          </button>
          <ContextMenuSurface :show="sortMenuOpen" :x="sortMenuPosition.x" :y="sortMenuPosition.y" @dismiss="sortMenuOpen = false">
            <button
              v-for="opt in SORT_OPTIONS"
              :key="opt.value"
              role="menuitemradio"
              :aria-checked="sortMode === opt.value"
              class="sort-item"
              @click="sortMode = opt.value"
            >
              <CheckIcon class="size-3.5" :class="sortMode === opt.value ? 'opacity-100' : 'opacity-0'" />
              {{ opt.label }}
            </button>
          </ContextMenuSurface>
        </div>

        <!-- List -->
        <div
          v-if="visibleConnections.length"
          ref="listRoot"
          class="relative mt-4 rounded-xl bg-(--bg-1) ring-1 ring-inset ring-(--line-faint)"
          :class="drag ? 'select-none' : ''"
          @pointerdown="onPointerDown"
        >
          <button
            v-for="(conn, index) in visibleConnections"
            :key="conn.id"
            type="button"
            :data-connection-id="conn.id"
            :title="hostOf(conn)"
            :class="[
              rowClass,
              index === 0 ? 'rounded-t-xl' : 'border-t border-(--line-faint)',
              index === visibleConnections.length - 1 ? 'rounded-b-xl' : '',
              activeConnectionId === conn.id && !drag ? 'bg-(--bg-2)' : '',
              drag && drag.id === conn.id
                ? 'drag-lifted z-10 rounded-lg bg-(--bg-2) ring-1 ring-(--acc-line) shadow-[0_12px_32px_rgba(0,0,0,0.45)]'
                : drag ? 'drag-shifted' : '',
              canReorder ? 'cursor-grab active:cursor-grabbing' : '',
            ]"
            :style="rowStyle(index)"
            :tabindex="isConnectionTabbable(conn) ? 0 : -1"
            @focus="activeConnectionId = conn.id"
            @keydown="onConnectionKeydown($event, conn)"
            @dblclick="handleConnect(conn)"
            @contextmenu="emit('context-menu-connection', $event, conn)"
          >
            <GripVerticalIcon
              v-if="canReorder"
              class="absolute left-1.5 size-3.5 transition-opacity"
              :class="drag && drag.id === conn.id ? 'text-(--acc) opacity-100' : 'text-(--fg-5) opacity-0 group-hover:opacity-100'"
            />
            <span class="eng-tile" :data-engine="engineOf(conn)">{{ ENG_ABBR[engineOf(conn)] }}</span>
            <span class="min-w-0 flex-1">
              <span class="block truncate text-[13px] font-medium leading-5 text-(--fg-1)">{{ conn.name }}</span>
              <span class="block truncate text-[11.5px] leading-4 text-(--fg-3)">
                {{ engineLabel(conn) }} <span class="text-(--fg-5)">·</span> {{ hostOf(conn) }}
              </span>
            </span>
            <span v-if="statusText(conn)" class="shrink-0 text-[11.5px] text-(--acc)">{{ statusText(conn) }}</span>
            <span class="flex w-24 shrink-0 items-center justify-end gap-1.5 text-[11.5px] text-(--fg-3)">
              <span class="size-1.5 rounded-full" :class="envDot(conn.environment)" />
              {{ envLabel(conn.environment) }}
            </span>
            <ChevronRightIcon class="size-3.5 shrink-0 text-(--fg-5) opacity-0 transition-opacity group-hover:opacity-100" />
          </button>
        </div>

        <p
          v-else-if="connections.length > 0"
          class="mt-4 rounded-xl bg-(--bg-1) px-4 py-6 text-center text-[12.5px] text-(--fg-4) ring-1 ring-inset ring-(--line-faint)"
        >
          No connections match your search.
        </p>

        <div
          v-if="connections.length === 0"
          class="mt-4 flex flex-col items-center rounded-xl bg-(--bg-1) px-6 py-14 text-center ring-1 ring-inset ring-(--line-faint)"
        >
          <span class="eng-tile mb-3 opacity-70"><PlugZapIcon class="size-4" /></span>
          <p class="text-[13.5px] font-medium text-(--fg-1)">No connections yet</p>
          <p class="mt-1 max-w-xs text-[12.5px] leading-5 text-(--fg-3)">
            Add your first database to start browsing tables and running queries.
          </p>
          <button
            class="mt-4 h-8 rounded-lg bg-(--acc) px-3 text-[12.5px] font-medium text-(--acc-fg) hover:brightness-110"
            @click="emit('new-connection')"
          >
            New connection
          </button>
        </div>
      </div>
    </div>
  </main>
</template>

<script setup lang="ts">
import { databaseEngines, connectionAddress } from "@/lib/databaseEngines";
import { computed, nextTick, onBeforeUnmount, ref, watch } from "vue";
import {
  CheckIcon,
  ChevronDownIcon,
  ChevronRightIcon,
  GripVerticalIcon,
  PlugZapIcon,
  PlusIcon,
  SearchIcon,
  XIcon,
} from "lucide-vue-next";
import ContextMenuSurface from "@/components/ContextMenuSurface.vue";
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

const APP_VERSION = __APP_VERSION__;

// ── Local preferences (per device) ────────────────────────────────────────────

const ORDER_KEY = "tupledb:connection-order";
const SORT_KEY = "tupledb:connections-sort";

function readJson<T>(key: string, fallback: T): T {
  try {
    const raw = localStorage.getItem(key);
    return raw ? (JSON.parse(raw) as T) : fallback;
  } catch {
    return fallback;
  }
}

function writeJson(key: string, value: unknown) {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch {}
}

function readStringList(key: string): string[] {
  const parsed = readJson<unknown>(key, []);
  return Array.isArray(parsed) ? parsed.filter((v): v is string => typeof v === "string") : [];
}

type SortMode = "custom" | "name" | "engine" | "env";

const SORT_OPTIONS: { value: SortMode; label: string }[] = [
  { value: "custom", label: "Custom order" },
  { value: "name", label: "Name" },
  { value: "engine", label: "Engine" },
  { value: "env", label: "Environment" },
];

function readSortMode(): SortMode {
  const stored = readJson<unknown>(SORT_KEY, "custom");
  return SORT_OPTIONS.some((opt) => opt.value === stored) ? (stored as SortMode) : "custom";
}

const customOrder = ref<string[]>(readStringList(ORDER_KEY));
const sortMode = ref<SortMode>(readSortMode());
watch(sortMode, (value) => writeJson(SORT_KEY, value));

const sortButton = ref<HTMLButtonElement | null>(null);
const sortMenuOpen = ref(false);
const sortMenuPosition = ref({ x: 0, y: 0 });
function openSortMenu() {
  const rect = sortButton.value?.getBoundingClientRect();
  sortMenuPosition.value = { x: rect ? rect.right - 168 : 0, y: rect ? rect.bottom + 4 : 0 };
  sortMenuOpen.value = true;
}

function handleConnect(conn: Connection) {
  emit("connect-saved", conn);
}

// ── Presentation helpers ──────────────────────────────────────────────────────

const ENG_ABBR: Record<string, string> = Object.fromEntries(
  Object.entries(databaseEngines).map(([engine, info]) => [engine, info.abbreviation]),
);

function engineOf(conn: Connection): string {
  return conn.database.engine;
}

function engineLabel(conn: Connection): string {
  return databaseEngines[conn.database.engine]?.label ?? conn.database.engine;
}

function hostOf(conn: Connection): string {
  return connectionAddress(conn);
}

const ENV_ORDER: Environment[] = ["LOCAL", "DEV", "STAGING", "PRODUCTION"];

function envLabel(env: Environment): string {
  switch (env) {
    case "PRODUCTION": return "Production";
    case "STAGING": return "Staging";
    case "DEV": return "Development";
    default: return "Local";
  }
}

function envDot(env: Environment): string {
  switch (env) {
    case "PRODUCTION": return "bg-(--env-prod)";
    case "STAGING": return "bg-(--env-staging)";
    case "DEV": return "bg-(--env-dev)";
    default: return "bg-(--env-local)";
  }
}

function statusText(conn: Connection): string | null {
  if (props.connectingId === conn.id) return "connecting…";
  if (props.openConnectionIds.includes(conn.id)) return "connected";
  return null;
}

const rowClass =
  "group relative flex w-full items-center gap-3 py-2.5 pl-4 pr-3 text-left transition-colors hover:bg-(--bg-2)/70 focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-inset focus-visible:ring-(--acc)";

// ── Filtering and sorting ─────────────────────────────────────────────────────

const search = ref("");
const envFilter = ref<Environment | null>(null);
const listRoot = ref<HTMLElement | null>(null);
const activeConnectionId = ref<string | null>(props.selectedConnectionId);

const envChips = computed(() => {
  const present = new Set(props.connections.map((c) => c.environment));
  const chips: { value: Environment | null; label: string }[] = [{ value: null, label: "All" }];
  for (const env of ENV_ORDER) if (present.has(env)) chips.push({ value: env, label: envLabel(env) });
  return chips.length > 2 ? chips : [];
});

watch(envChips, (chips) => {
  if (envFilter.value && !chips.some((chip) => chip.value === envFilter.value)) envFilter.value = null;
});

const filteredConnections = computed(() => {
  const q = search.value.trim().toLowerCase();
  return props.connections.filter((c) => {
    if (envFilter.value && c.environment !== envFilter.value) return false;
    if (!q) return true;
    return (
      c.name.toLowerCase().includes(q) ||
      connectionAddress(c).toLowerCase().includes(q) ||
      c.environment.toLowerCase().includes(q) ||
      engineOf(c).toLowerCase().includes(q) ||
      engineLabel(c).toLowerCase().includes(q)
    );
  });
});

function applyCustomOrder(list: Connection[]): Connection[] {
  const position = new Map(customOrder.value.map((id, index) => [id, index]));
  // Ids the user never placed keep their stored order after the placed ones.
  return list
    .map((conn, index) => ({ conn, rank: position.get(conn.id) ?? customOrder.value.length + index }))
    .sort((a, b) => a.rank - b.rank)
    .map((item) => item.conn);
}

const visibleConnections = computed(() => {
  const list = filteredConnections.value;
  if (sortMode.value === "custom") return applyCustomOrder(list);
  const sorted = [...list];
  if (sortMode.value === "name") {
    sorted.sort((a, b) => a.name.localeCompare(b.name));
  } else if (sortMode.value === "engine") {
    sorted.sort((a, b) => engineOf(a).localeCompare(engineOf(b)) || a.name.localeCompare(b.name));
  } else {
    sorted.sort((a, b) => ENV_ORDER.indexOf(a.environment) - ENV_ORDER.indexOf(b.environment) || a.name.localeCompare(b.name));
  }
  return sorted;
});

// ── Manual ordering (pointer drag, Alt+Arrow) ─────────────────────────────────
// Pointer events instead of HTML5 drag and drop: the Tauri window keeps native
// drag-and-drop enabled for file imports, which swallows HTML5 drag events in
// the WebView.

const DRAG_THRESHOLD_PX = 4;
const FALLBACK_ROW_HEIGHT = 56;
const canReorder = computed(() => sortMode.value === "custom" && !search.value.trim() && envFilter.value === null);

interface DragState { id: string; startIndex: number; targetIndex: number; dy: number; rowHeight: number }
const drag = ref<DragState | null>(null);
let pressed: { id: string; pointerId: number; startY: number; startIndex: number; rowHeight: number } | null = null;

function moveInCustomOrder(movingId: string, targetId: string) {
  // Start from the complete store order so ids outside the visible list keep a stable rank.
  const current = applyCustomOrder(props.connections).map((c) => c.id);
  const movingIndex = current.indexOf(movingId);
  const targetIndex = current.indexOf(targetId);
  if (movingIndex === -1 || targetIndex === -1 || movingIndex === targetIndex) return;
  // The moved item takes the target's slot: before it when moving up, after it when moving down.
  const order = current.filter((id) => id !== movingId);
  const insertAt = order.indexOf(targetId) + (movingIndex < targetIndex ? 1 : 0);
  order.splice(insertAt, 0, movingId);
  customOrder.value = order;
  writeJson(ORDER_KEY, order);
}

function rowFromEvent(event: Event): HTMLElement | null {
  const target = event.target;
  return target instanceof Element ? target.closest<HTMLElement>("[data-connection-id]") : null;
}

// The lifted row follows the pointer; every row between its origin and its
// destination slides one slot to make room, so the drop position is visible
// before releasing.
function rowStyle(index: number) {
  const state = drag.value;
  if (!state) return undefined;
  if (index === state.startIndex) return { transform: `translateY(${state.dy}px) scale(1.01)` };
  if (state.startIndex < state.targetIndex && index > state.startIndex && index <= state.targetIndex) {
    return { transform: `translateY(-${state.rowHeight}px)` };
  }
  if (state.startIndex > state.targetIndex && index >= state.targetIndex && index < state.startIndex) {
    return { transform: `translateY(${state.rowHeight}px)` };
  }
  return { transform: "translateY(0)" };
}

function onPointerDown(event: PointerEvent) {
  if (!canReorder.value || event.button !== 0 || event.ctrlKey || event.metaKey) return;
  const row = rowFromEvent(event);
  const id = row?.dataset.connectionId;
  if (!row || !id) return;
  const startIndex = visibleConnections.value.findIndex((conn) => conn.id === id);
  if (startIndex === -1) return;
  pressed = { id, pointerId: event.pointerId, startY: event.clientY, startIndex, rowHeight: row.offsetHeight || FALLBACK_ROW_HEIGHT };
  window.addEventListener("pointermove", onPointerMove);
  window.addEventListener("pointerup", onPointerUp);
  window.addEventListener("pointercancel", onPointerUp);
}

function onPointerMove(event: PointerEvent) {
  if (!pressed || event.pointerId !== pressed.pointerId) return;
  const dy = event.clientY - pressed.startY;
  if (!drag.value && Math.abs(dy) < DRAG_THRESHOLD_PX) return;
  const last = visibleConnections.value.length - 1;
  const targetIndex = Math.max(0, Math.min(last, pressed.startIndex + Math.round(dy / pressed.rowHeight)));
  drag.value = { id: pressed.id, startIndex: pressed.startIndex, targetIndex, dy, rowHeight: pressed.rowHeight };
}

function onPointerUp() {
  window.removeEventListener("pointermove", onPointerMove);
  window.removeEventListener("pointerup", onPointerUp);
  window.removeEventListener("pointercancel", onPointerUp);
  const state = drag.value;
  pressed = null;
  drag.value = null;
  if (!state || state.targetIndex === state.startIndex) return;
  const target = visibleConnections.value[state.targetIndex];
  if (target) {
    moveInCustomOrder(state.id, target.id);
    focusConnection(state.id);
  }
}

onBeforeUnmount(() => {
  window.removeEventListener("pointermove", onPointerMove);
  window.removeEventListener("pointerup", onPointerUp);
  window.removeEventListener("pointercancel", onPointerUp);
});

function nudgeInCustomOrder(conn: Connection, delta: number) {
  const list = visibleConnections.value;
  const index = list.findIndex((c) => c.id === conn.id);
  const target = list[index + delta];
  if (index === -1 || !target) return;
  moveInCustomOrder(conn.id, target.id);
  focusConnection(conn.id);
}

// ── Keyboard navigation ───────────────────────────────────────────────────────

const activeConnectionIsVisible = computed(
  () => activeConnectionId.value !== null && visibleConnections.value.some((conn) => conn.id === activeConnectionId.value),
);

function focusConnection(id: string) {
  activeConnectionId.value = id;
  nextTick(() => {
    const buttons = Array.from(listRoot.value?.querySelectorAll<HTMLButtonElement>("[data-connection-id]") ?? []);
    buttons.find((button) => button.dataset.connectionId === id)?.focus();
  });
}

function moveConnectionFocus(currentId: string, delta: number) {
  const connections = visibleConnections.value;
  if (connections.length === 0) return;
  const currentIndex = connections.findIndex((conn) => conn.id === currentId);
  const startIndex = currentIndex === -1 ? 0 : currentIndex;
  const nextIndex = Math.max(0, Math.min(connections.length - 1, startIndex + delta));
  focusConnection(connections[nextIndex].id);
}

function onConnectionKeydown(e: KeyboardEvent, conn: Connection) {
  if (e.key === "Enter" || e.key === " ") {
    e.preventDefault();
    handleConnect(conn);
    return;
  }
  const isArrowDown = e.key === "ArrowDown";
  const isArrowUp = e.key === "ArrowUp";
  if (e.altKey && canReorder.value && (isArrowDown || isArrowUp)) {
    e.preventDefault();
    nudgeInCustomOrder(conn, isArrowDown ? 1 : -1);
  } else if (isArrowDown) {
    e.preventDefault();
    moveConnectionFocus(conn.id, 1);
  } else if (isArrowUp) {
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

<style scoped>
.sort-item {
  display: flex;
  width: 100%;
  align-items: center;
  gap: 8px;
  padding: 6px 10px 6px 8px;
  border-radius: 4px;
  font-size: 12px;
  text-align: left;
  color: var(--fg-1);
}
.sort-item:hover {
  background: var(--bg-2);
}
.drag-shifted {
  transition: transform 160ms ease;
}
.drag-lifted {
  transition: none;
  will-change: transform;
}
</style>
