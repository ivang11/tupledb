<template>
  <aside
    class="shrink-0 flex flex-col bg-sidebar relative select-none"
    :style="{ width: width + 'px' }"
  >
    <!-- Active connection: compact identity row -->
    <div
      v-if="selectedConnectionId && openConnections[selectedConnectionId]"
      class="shrink-0 px-2 pt-2 pb-1"
    >
      <div
        class="flex h-7 items-center gap-2 rounded-md px-2 transition-colors hover:bg-(--bg-1)"
        :title="`${openConnections[selectedConnectionId].connection.name} · ${activeConnectionStatus} · ${activeConnectionDetail}`"
        @contextmenu="emit('context-menu-connection', $event, openConnections[selectedConnectionId].connection)"
      >
        <span
          class="size-2 shrink-0 rounded-full"
          :class="activeConnection?.status === 'error' ? 'bg-destructive' : getEnvDotColor(openConnections[selectedConnectionId].connection.environment)"
          :title="activeConnectionStatusTitle"
        />
        <span class="min-w-0 flex-1 truncate text-[12px] font-semibold text-(--fg-1)">
          {{ openConnections[selectedConnectionId].connection.name }}
        </span>
        <span class="truncate text-[11px] text-(--fg-3)">{{ activeConnectionDetail }}</span>
        <span
          class="inline-flex shrink-0 items-center rounded-[3px] px-1 py-px text-[9px] font-semibold uppercase tracking-[0.06em] ring-1 ring-inset"
          :class="getEnvColor(openConnections[selectedConnectionId].connection.environment)"
        >
          {{ openConnections[selectedConnectionId].connection.environment }}
        </span>
      </div>
    </div>

    <div
      v-if="selectedConnectionId && openConnections[selectedConnectionId]"
      ref="dbDropdownRef"
      class="shrink-0 px-2 pb-1 relative"
    >
      <div class="flex items-center gap-1">
        <button
          type="button"
          class="min-w-0 flex-1 flex items-center gap-1.5 h-7 px-2 rounded-md bg-(--bg-1) ring-1 ring-inset ring-(--line-2) hover:bg-(--bg-2) transition-colors text-left"
          @click="toggleDbDropdown"
          @contextmenu="activeDb && emit('context-menu-database', $event, selectedConnectionId, activeDb)"
        >
          <DatabaseIcon class="size-3 shrink-0 text-(--fg-3)" />
          <span class="flex-1 truncate text-[12px] font-medium" :class="activeDb ? 'text-(--fg-1)' : 'text-(--fg-4)'">
            {{ activeDb || 'Select database' }}
          </span>
          <ChevronDownIcon
            class="size-3 shrink-0 text-(--fg-4) transition-transform duration-150"
            :class="{ 'rotate-180': dbDropdownOpen }"
          />
        </button>
        <button
          type="button"
          class="size-7 shrink-0 rounded-md bg-(--bg-1) ring-1 ring-inset ring-(--line-2) hover:bg-(--bg-2) text-(--fg-3) hover:text-(--fg-1) transition-colors flex items-center justify-center"
          title="New Database"
          v-if="activeConnection?.capabilities?.createDatabase"
          @click="emit('new-database', selectedConnectionId)"
        >
          <PlusIcon class="size-3.5" :stroke-width="2.25" />
        </button>
      </div>

      <!-- Dropdown list -->
      <div
        v-if="dbDropdownOpen"
        class="absolute left-2 right-2 top-full mt-1 z-50 rounded-md border border-(--line-1) bg-popover shadow-xl overflow-hidden"
      >
        <!-- Search input -->
        <div class="p-1.5 border-b border-(--line-2)">
          <div class="relative">
            <SearchIcon class="absolute left-2 top-1/2 -translate-y-1/2 size-3 text-(--fg-4)" />
            <input
              ref="dbSearchInput"
              v-model="dbSearch"
              type="text"
              placeholder="Search..."
              class="w-full h-6 bg-(--bg-0) rounded-[4px] pl-6 pr-2 text-[11.5px] text-foreground placeholder:text-(--fg-4) focus:outline-none ring-1 ring-inset ring-(--line-2) focus:ring-(--acc-line) transition-colors"
              @keydown="onDbSearchKeydown"
              aria-label="Search databases"
            />
          </div>
        </div>

        <!-- Results -->
        <div class="max-h-52 overflow-y-auto py-1 custom-scrollbar">
          <button
            v-for="(db, index) in filteredDatabases"
            :key="db"
            data-db-option
            type="button"
            class="w-full flex items-center gap-2 px-2.5 h-6 text-[11.5px] transition-colors text-left"
            :class="index === highlightedDbIndex
              ? 'text-(--fg-1) bg-(--bg-3)'
              : db === activeDb ? 'text-(--fg-1) bg-(--bg-2)'
              : 'text-(--fg-2) hover:text-(--fg-1) hover:bg-(--bg-2)'"
            @click="selectDb(db)"
          >
            <span
              class="size-1.5 rounded-full shrink-0"
              :class="db === activeDb ? 'bg-primary' : ''"
            />
            {{ db }}
          </button>
          <p v-if="filteredDatabases.length === 0" class="px-3 py-2 text-[11.5px] text-(--fg-4) italic">
            No matches
          </p>
        </div>
      </div>
    </div>

    <!-- Search bar (for selected connection's tables) -->
    <div
      v-if="selectedConnectionId && openConnections[selectedConnectionId]"
      ref="searchContainerRef"
      class="shrink-0 px-2 pb-1"
    >
      <div class="relative">
        <SearchIcon
          class="absolute left-2 top-1/2 -translate-y-1/2 size-3 text-(--fg-4)"
        />
        <Input
          :model-value="search"
          @update:model-value="emit('update:search', String($event))"
          placeholder="Filter tables"
          aria-label="Search tables and views"
          @keydown="onTableSearchKeydown"
          class="h-7 pl-6.5 pr-7 text-[11.5px] md:text-[11.5px] text-foreground placeholder:text-(--fg-4) bg-(--bg-0) border border-(--line-2) rounded-md shadow-none focus-visible:ring-0 focus-visible:border-(--acc-line)"
        />
        <button
          v-if="search"
          class="absolute right-1.5 top-1/2 -translate-y-1/2 size-4 rounded flex items-center justify-center text-(--fg-4) hover:text-foreground transition-colors"
          title="Clear search"
          @click="emit('update:search', '')"
        >
          <XIcon class="size-3" />
        </button>
      </div>
    </div>

    <!-- Tables for selected database -->
    <ScrollArea data-table-list class="flex-1 py-0.5" @contextmenu="onTableAreaContextMenu">
      <template v-if="selectedConnectionId && openConnections[selectedConnectionId] && activeDatabase">
        <div class="px-2 pb-2">
          <div class="flex items-center justify-between pl-2 pr-1 h-6 text-[10.5px] font-medium uppercase tracking-[0.06em] text-(--fg-4)">
            <span aria-live="polite">{{ objectSummary }}</span>
            <button type="button" title="Refresh schema" aria-label="Refresh schema"
              class="size-5 rounded flex items-center justify-center hover:bg-(--bg-2) hover:text-foreground focus-visible:outline-2 focus-visible:outline-primary"
              @click="emit('refresh-schema', activeConnId, activeDb)">
              <RefreshCwIcon class="size-2.5" />
            </button>
          </div>
          <section v-for="group in tableGroups" :key="group.key" class="mb-0.5">
            <button v-if="showTableGroups" type="button" :aria-expanded="isGroupExpanded(group.key)"
              class="w-full flex items-center gap-1 px-1.5 h-6 text-[11px] font-medium text-(--fg-3) hover:text-foreground focus-visible:outline-2 focus-visible:outline-primary rounded"
              @click="toggleGroup(group.key)">
              <ChevronDownIcon class="size-3 transition-transform" :class="{ '-rotate-90': !isGroupExpanded(group.key) }" />
              <span class="flex-1 truncate text-left">{{ group.label }}</span>
              <span class="tabular-nums text-(--fg-4)">{{ group.tables.length }}</span>
            </button>
            <div v-if="isGroupExpanded(group.key)">
              <button v-for="table in group.tables" :key="tableHandle(table)"
                :ref="(el) => setTableRef(el as HTMLElement | null, tableHandle(table), activeDb, activeConnId)"
                type="button" :title="tableLabel(table.reference ?? table.name)"
                :aria-current="isTableActive(tableHandle(table), activeDb, activeConnId) ? 'page' : undefined"
                @click="handleTableClick($event, activeConnId, activeDb, tableHandle(table))"
                @keydown="onTableKeydown($event, table)"
                @contextmenu.prevent.stop="emit('context-menu-table', $event, activeConnId, activeDb, tableHandle(table))"
                :class="tableButtonClasses(tableHandle(table), activeDb, activeConnId)">
                <EyeIcon v-if="isView(table)" :class="tableIconClasses(tableHandle(table), activeDb, activeConnId)" />
                <TableIcon v-else :class="tableIconClasses(tableHandle(table), activeDb, activeConnId)" />
                <span class="flex-1 truncate text-[11.5px]">{{ table.name }}</span>
                <span v-if="isTableOpen(tableHandle(table), activeDb, activeConnId) && !isTableActive(tableHandle(table), activeDb, activeConnId)"
                  class="size-1.5 rounded-full shrink-0 bg-primary/60" />
              </button>
            </div>
          </section>
          <p v-if="visibleTables.length === 0" class="px-2 py-3 text-[11.5px] text-(--fg-4) leading-relaxed">
            {{ search ? 'No matching tables or views. Try another search.' : 'No tables or views in this database.' }}
          </p>
        </div>
      </template>

<div
        v-else-if="selectedConnectionId && openConnections[selectedConnectionId]"
        class="px-4 py-6 text-center"
      >
        <DatabaseIcon class="size-6 mx-auto text-(--fg-5) mb-2" />
        <p class="text-[11.5px] text-(--fg-4)">Select a database to browse tables.</p>
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
import { groupSidebarTables } from '@/lib/sidebarTables'
import { tableHandle, tableLabel, tableSelectionKey } from '@/lib/tableReference'

import { databaseEngines } from "@/lib/databaseEngines";
import { ref, computed, nextTick, onMounted, onUnmounted, watch } from "vue";
import {
  SearchIcon,
  DatabaseIcon,
  ChevronDownIcon,
  TableIcon,
  EyeIcon,
  XIcon,
  PlusIcon,
  RefreshCwIcon,
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
      capabilities?: import("@/types/connection").DatabaseCapabilities;
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
  "refresh-schema": [connId: string, db: string];
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
  searchContainerRef.value?.querySelector("input")?.select();
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
  const key = tableSelectionKey(connId, db, tableName);
  if (el) tableRefs.value[key] = el;
  else delete tableRefs.value[key];
}

function scrollToTable(tableName: string, db: string, connId: string) {
  const key = tableSelectionKey(connId, db, tableName);
  const el = tableRefs.value[key];
  el?.scrollIntoView({ block: "nearest", behavior: "smooth" });
}

function isView(table: any) {
  return String(table?.table_type ?? '').toUpperCase().includes('VIEW')
}

function tableButtonClasses(tableName: string, db: string, connId: string) {
  const base = 'w-full flex items-center gap-1.5 pl-2 pr-1.5 h-6 rounded-[4px] transition-colors text-left group/tbl focus-visible:outline-2 focus-visible:outline-primary focus-visible:outline-offset-[-2px]'
  const action = props.pendingTableAction(tableName, db, connId)

  if (action === 'drop') {
    return `${base} bg-destructive/12 text-destructive line-through ring-1 ring-inset ring-destructive/20`
  }
  if (action === 'truncate') {
    return `${base} bg-amber-500/14 text-amber-200 line-through ring-1 ring-inset ring-amber-500/25`
  }
  if (props.isTableSelected(connId, db, tableName)) return `${base} bg-primary/15 text-primary`
  if (props.isTableActive(tableName, db, connId)) return `${base} bg-(--bg-3) text-(--fg-1) shadow-none`
  return `${base} hover:bg-(--bg-2) text-(--fg-2) hover:text-(--fg-1)`
}

function tableIconClasses(tableName: string, db: string, connId: string) {
  const action = props.pendingTableAction(tableName, db, connId)

  if (action === 'drop') return 'size-3 shrink-0 text-destructive/85'
  if (action === 'truncate') return 'size-3 shrink-0 text-amber-300/90'
  if (props.isTableActive(tableName, db, connId)) return 'size-3 shrink-0 text-(--fg-2)'
  if (props.isTableSelected(connId, db, tableName)) return 'size-3 shrink-0 text-primary/85'
  return 'size-3 shrink-0 text-(--fg-4) group-hover/tbl:text-(--fg-3)'
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

const getEnvDotColor = (env: Environment): string => {
  switch (env) {
    case "PRODUCTION": return "bg-(--env-prod)";
    case "STAGING": return "bg-(--env-staging)";
    case "DEV": return "bg-(--env-dev)";
    default: return "bg-(--env-local)";
  }
};

// Non-null alias used inside the v-if="selectedConnectionId" template blocks
const activeConnId = computed(() => props.selectedConnectionId ?? '')
const activeConnection = computed(() =>
  props.selectedConnectionId ? props.openConnections[props.selectedConnectionId] : null,
)
const activeDatabase = computed(() => activeConnection.value?.selectedDatabase ?? null)
const activeDb = computed(() => activeDatabase.value ?? '')
function onTableAreaContextMenu(event: MouseEvent) {
  if (!activeConnection.value || !activeDatabase.value) return
  event.preventDefault()
  emit('context-menu-database', event, activeConnId.value, activeDb.value)
}
const visibleTables = computed(() => props.filteredTables(activeConnId.value, activeDb.value))
const collapsedGroups = ref(new Set<string>())
const tableGroups = computed(() => groupSidebarTables(visibleTables.value))
const showTableGroups = computed(() => tableGroups.value.length > 1)
const objectSummary = computed(() => {
  if (props.search) return `${visibleTables.value.length} matches`
  if (tableGroups.value.length === 1) return `${tableGroups.value[0].label} · ${visibleTables.value.length}`
  return `${visibleTables.value.length} objects`
})
function isGroupExpanded(key: string) {
  return !showTableGroups.value || !collapsedGroups.value.has(key)
}
watch(() => props.search, () => collapsedGroups.value.clear())
function toggleGroup(key: string) {
  if (collapsedGroups.value.has(key)) collapsedGroups.value.delete(key)
  else collapsedGroups.value.add(key)
}
const navigableTables = computed(() => tableGroups.value
  .filter(group => isGroupExpanded(group.key)).flatMap(group => group.tables))
function focusTable(table: any) {
  const el = tableRefs.value[tableSelectionKey(activeConnId.value, activeDb.value, tableHandle(table))]
  el?.focus()
  el?.scrollIntoView?.({ block: 'nearest' })
}
function onTableSearchKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape') { event.preventDefault(); emit('update:search', '') }
  if (event.key === 'ArrowDown' && navigableTables.value.length) {
    event.preventDefault(); focusTable(navigableTables.value[0])
  }
  if (event.key === 'Enter' && navigableTables.value.length) {
    event.preventDefault()
    emit('load-table', tableHandle(navigableTables.value[0]), activeConnId.value, activeDb.value)
  }
}
function onTableKeydown(event: KeyboardEvent, table: any) {
  if (event.key === 'Escape') { event.preventDefault(); focusSearch(); return }
  const tables = navigableTables.value
  const index = tables.findIndex(item => tableHandle(item) === tableHandle(table))
  let next = index
  if (event.key === 'ArrowDown') next = Math.min(index + 1, tables.length - 1)
  else if (event.key === 'ArrowUp') next = Math.max(index - 1, 0)
  else if (event.key === 'Home') next = 0
  else if (event.key === 'End') next = tables.length - 1
  else return
  event.preventDefault()
  if (tables[next]) focusTable(tables[next])
}
watch([activeConnId, activeDb], () => {
  collapsedGroups.value.clear()
  dbDropdownOpen.value = false
  dbSearch.value = ''
})

// ── Database dropdown ─────────────────────────────────────────────────────────
const dbDropdownOpen = ref(false)
const dbDropdownRef = ref<HTMLElement | null>(null)
const dbSearch = ref('')
const highlightedDbIndex = ref(0)
watch(dbSearch, () => { highlightedDbIndex.value = 0 })
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
    highlightedDbIndex.value = Math.max(0, filteredDatabases.value.indexOf(activeDb.value))
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

function onDbSearchKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape') { event.preventDefault(); dbDropdownOpen.value = false; dbDropdownRef.value?.querySelector('button')?.focus() }
  else if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
    event.preventDefault()
    const delta = event.key === 'ArrowDown' ? 1 : -1
    highlightedDbIndex.value = Math.max(0, Math.min(filteredDatabases.value.length - 1, highlightedDbIndex.value + delta))
    nextTick(() => dbDropdownRef.value?.querySelectorAll('[data-db-option]')[highlightedDbIndex.value]?.scrollIntoView?.({ block: 'nearest' }))
  } else if (event.key === 'Enter') {
    event.preventDefault()
    const db = filteredDatabases.value[highlightedDbIndex.value]
    if (db) selectDb(db)
  }
}

function onClickOutsideDb(e: MouseEvent) {
  if (dbDropdownRef.value && !dbDropdownRef.value.contains(e.target as Node)) {
    dbDropdownOpen.value = false
  }
}

onMounted(() => document.addEventListener('mousedown', onClickOutsideDb))
onUnmounted(() => document.removeEventListener('mousedown', onClickOutsideDb))
const activeConnectionDetail = computed(() => {
  const engine = activeConnection.value?.connection.database.engine
  const label = engine ? databaseEngines[engine].label : ''
  return activeConnection.value?.serverVersion ? `${label} ${activeConnection.value.serverVersion}` : label
})
const activeConnectionStatus = computed(() =>
  activeConnection.value?.status === "error" ? "Connection error" : "Connected",
)
const activeConnectionStatusTitle = computed(() =>
  activeConnection.value?.statusMessage ?? activeConnectionStatus.value,
)
</script>
