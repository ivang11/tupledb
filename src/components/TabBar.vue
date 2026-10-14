<template>
  <div
    v-if="tabs.length > 0 || hasOpenConnections"
    class="flex items-center bg-(--bg-0) overflow-hidden shrink-0 px-1 border-b border-(--line-2)"
    style="height: 32px"
  >
    <div
      v-if="workspaceLabel"
      class="mr-1.5 hidden max-w-52 shrink-0 items-center gap-1.5 rounded-[5px] border border-(--line-2) bg-(--bg-1) px-2 h-6 text-[10.5px] font-medium text-(--fg-3) lg:flex"
      :title="workspaceLabel"
    >
      <DatabaseIcon class="size-3 shrink-0 opacity-60" />
      <span class="truncate">{{ workspaceLabel }}</span>
    </div>

    <div
      class="tab-strip flex min-w-0 flex-1 items-center gap-0.5 overflow-x-auto overflow-y-hidden pr-1"
      @dragover.prevent
      @drop="emit('tab-drop', null, $event)"
    >
      <!-- Tabs -->
      <button
        v-for="tab in tabs"
        :key="tab.id"
        draggable="true"
        @click="emit('switch-tab', tab.id)"
        @contextmenu.prevent.stop="tabMenu = { show: true, x: $event.clientX, y: $event.clientY, tab }"
        @mousedown="(e) => { if (e.button === 1) { e.preventDefault(); e.stopPropagation(); emit('close-tab', tab.id, e); } }"
        @dragstart="emit('tab-drag-start', tab.id, $event)"
        @dragover.prevent
        @drop.stop="emit('tab-drop', tab.id, $event)"
        :class="[
          'relative flex items-center gap-1.5 pl-2 pr-1.5 h-6 rounded-[5px] transition-colors min-w-24 max-w-44 shrink-0 group/tab text-[11.5px] font-medium',
          tab.id === activeTabId
            ? 'bg-(--bg-2) text-(--fg-1)'
            : 'text-(--fg-3) hover:bg-(--bg-1) hover:text-(--fg-2)',
        ]"
        :title="
          tab.type === 'query'
            ? `Query · ${connectionNames[tab.connectionId] ?? ''}`
            : `${tableLabel(tab.reference ?? tab.tableName ?? '')} · ${connectionNames[tab.connectionId] ?? ''} · ${tab.database}`
        "
      >
        <!-- Icon -->
        <TerminalIcon
          v-if="tab.type === 'query'"
          class="size-3 shrink-0"
          :class="tab.id === activeTabId ? 'text-(--fg-2)' : 'text-(--fg-4)'"
        />
        <TableIcon
          v-else
          class="size-3 shrink-0"
          :class="tab.id === activeTabId ? 'text-(--fg-2)' : 'text-(--fg-4)'"
        />

        <!-- Live (connection environment) dot -->
        <span
          v-if="tab.id === activeTabId"
          :class="['size-1.5 rounded-full shrink-0', getEnvDotColor(connectionEnvironments[tab.connectionId])]"
        />

        <!-- Label -->
        <span class="flex-1 min-w-0 truncate text-left">
          {{ tab.type === "query" ? "Query" : tableLabel(tab.reference ?? tab.tableName ?? "") }}
        </span>

        <!-- Close -->
        <span
          @click.stop="emit('close-tab', tab.id, $event)"
          :class="[
            'shrink-0 size-3.5 flex items-center justify-center rounded-[3px] transition-all text-(--fg-4) hover:text-(--fg-1) hover:bg-(--bg-3)',
            tab.id === activeTabId
              ? 'opacity-100'
              : 'opacity-0 group-hover/tab:opacity-100',
          ]"
        >
          <XIcon class="size-2.5" />
        </span>
      </button>
    </div>

    <div class="flex shrink-0 items-center gap-0.5 border-l border-(--line-2) ml-1 pl-1">
    <!-- New query -->
    <button
      v-if="hasOpenConnections && firstConnectionId && canOpenQuery"
      @click="emit('new-query', firstConnectionId)"
      class="flex items-center gap-1 px-2 h-6 rounded-[5px] text-[11px] font-medium text-(--fg-2) hover:bg-(--bg-2) hover:text-(--fg-1) transition-colors shrink-0"
      title="New Query (SQL)"
    >
      SQL
    </button>

    <!-- Table controls -->
    <template v-if="hasActiveTableTab">
      <div class="flex items-center gap-0.5">
        <button
          type="button"
          :class="[
            'size-6 flex items-center justify-center rounded-[5px] transition-colors',
            showFilters
              ? 'bg-(--acc-soft) text-(--acc)'
              : 'text-(--fg-2) hover:bg-(--bg-2) hover:text-(--fg-1)',
          ]"
          title="Toggle Filters"
          @click="emit('toggle-filters')"
        >
          <FilterIcon class="size-3.5" />
        </button>
        <button
          type="button"
          class="size-6 flex items-center justify-center rounded-[5px] text-(--fg-2) hover:bg-(--bg-2) hover:text-(--fg-1) transition-colors"
          title="Refresh"
          @click="emit('refresh')"
        >
          <RefreshCwIcon class="size-3.5" />
        </button>
      </div>
    </template>

<!-- Pin / focus pane -->
    <button
      v-if="showFocusButton"
      @click="emit('toggle-focus')"
      :class="[
        'flex items-center justify-center size-6 rounded-[5px] transition-colors shrink-0',
        isFocused
          ? 'text-(--acc) bg-(--acc-soft) hover:brightness-110'
          : 'text-(--fg-2) hover:bg-(--bg-2) hover:text-(--fg-1)',
      ]"
      :title="isFocused ? `Unpin pane (${focusPaneKey})` : `Pin pane — focus (${focusPaneKey})`"
    >
      <PinOffIcon v-if="isFocused" class="size-3.5" />
      <PinIcon v-else class="size-3.5" />
    </button>

    <!-- Split pane -->
    <button
      v-if="isLastPane && !isFocused && canSplitPane"
      @click="emit('add-pane')"
      class="flex items-center justify-center size-6 rounded-[5px] text-(--fg-2) hover:bg-(--bg-2) hover:text-(--fg-1) transition-colors shrink-0"
      title="Split pane"
    >
      <PanelRightOpenIcon class="size-3.5" />
    </button>

    <!-- Close pane -->
    <button
      v-if="showClosePaneButton && !isFocused"
      @click="emit('remove-pane')"
      class="flex items-center justify-center size-6 rounded-[5px] text-(--fg-4) hover:text-destructive hover:bg-destructive/10 transition-colors shrink-0"
      title="Close pane"
    >
      <XIcon class="size-3.5" />
    </button>
    </div>
    <ContextMenuSurface :show="tabMenu.show" :x="tabMenu.x" :y="tabMenu.y" @dismiss="tabMenu.show = false">
      <template v-if="tabMenu.tab">
        <button class="tab-menu-item" @click="emit('switch-tab', tabMenu.tab.id)">Select tab</button>
        <button v-if="tabMenu.tab.type === 'table'" class="tab-menu-item" @click="refreshContextTab">Refresh table</button>
        <button class="tab-menu-item" @click="copyTabName">Copy {{ tabMenu.tab.type === 'table' ? 'table name' : 'tab name' }}</button>
        <div class="border-t my-1" />
        <button class="tab-menu-item" @click="emit('close-tab', tabMenu.tab.id, $event)">Close tab</button>
        <button :disabled="tabs.length < 2" class="tab-menu-item" @click="closeContextTabs('others', $event)">Close other tabs</button>
        <button :disabled="contextTabIndex === tabs.length - 1" class="tab-menu-item" @click="closeContextTabs('right', $event)">Close tabs to the right</button>
        <button class="tab-menu-item" @click="closeContextTabs('all', $event)">Close all tabs</button>
      </template>
    </ContextMenuSurface>
  </div>
</template>

<script setup lang="ts">
import { tableLabel } from '@/lib/tableReference'

import { computed, ref } from "vue";
import ContextMenuSurface from "./ContextMenuSurface.vue";
import { copyText } from "@/lib/clipboard";
import { useKeybindings, formatKeybinding } from "@/composables/useKeybindings";
import {
  TerminalIcon,
  TableIcon,
  DatabaseIcon,
  XIcon,
  FilterIcon,
  RefreshCwIcon,
  PanelRightOpenIcon,
  PinIcon,
  PinOffIcon,
} from "lucide-vue-next";
import type { Environment } from "@/types/connection";

interface AnyTab {
  id: string;
  type: "table" | "query";
  connectionId: string;
  database: string | null;
  tableName?: string;
  reference?: import('@/types/database').TableRef;
}

const props = defineProps<{
  paneId: string;
  tabs: AnyTab[];
  activeTabId: string | null;
  connectionNames: Record<string, string>;
  connectionEnvironments: Record<string, Environment>;
  hasOpenConnections: boolean;
  firstConnectionId: string | null;
  canOpenQuery: boolean;
  showFilters: boolean;
  hasActiveTableTab: boolean;
  isLastPane: boolean;
  showClosePaneButton: boolean;
  isFocused: boolean;
  showFocusButton: boolean;
  workspaceLabel?: string;
  canSplitPane: boolean;
}>();

const emit = defineEmits<{
  "switch-tab": [tabId: string];
  "close-tab": [tabId: string, e: MouseEvent];
  "new-query": [connectionId: string];
  "toggle-filters": [];
  refresh: [];
  "add-pane": [];
  "remove-pane": [];
  "toggle-focus": [];
  "tab-drag-start": [tabId: string, e: DragEvent];
  "tab-drop": [targetTabId: string | null, e: DragEvent];
}>();

const tabMenu = ref<{ show: boolean; x: number; y: number; tab: AnyTab | null }>({ show: false, x: 0, y: 0, tab: null });
const contextTabIndex = computed(() => props.tabs.findIndex(tab => tab.id === tabMenu.value.tab?.id));
function refreshContextTab() {
  if (!tabMenu.value.tab) return;
  emit('switch-tab', tabMenu.value.tab.id);
  emit('refresh');
}
function copyTabName() {
  const tab = tabMenu.value.tab;
  if (tab) void copyText(tab.type === 'table' ? tableLabel(tab.reference ?? tab.tableName ?? '') : 'Query');
}
function closeContextTabs(mode: 'others' | 'right' | 'all', event: MouseEvent) {
  const tabs = props.tabs.filter((tab, index) => mode === 'all' || (mode === 'others' ? tab.id !== tabMenu.value.tab?.id : index > contextTabIndex.value));
  for (const tab of tabs) emit('close-tab', tab.id, event);
}

const getEnvDotColor = (env: Environment): string => {
  switch (env) {
    case "PRODUCTION": return "bg-(--env-prod)";
    case "STAGING":    return "bg-(--env-staging)";
    case "DEV":        return "bg-(--env-dev)";
    default:           return "bg-(--acc)";
  }
};

const { getBinding } = useKeybindings();
const focusPaneKey = computed(() => formatKeybinding(getBinding('focusPane')));

</script>

<style scoped>
.tab-strip { scrollbar-width: none; }
.tab-strip::-webkit-scrollbar { display: none; width: 0; height: 0; }
.tab-menu-item { display: block; width: 100%; text-align: left; padding: 5px 10px; border-radius: 4px; font-size: 12px; }
.tab-menu-item:hover:not(:disabled) { background: var(--muted); }
</style>
