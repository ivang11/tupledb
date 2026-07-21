<template>
  <div
    v-if="tabs.length > 0 || hasOpenConnections"
    class="flex items-center bg-(--bg-0) overflow-hidden shrink-0 px-1.5 pt-1 border-b border-(--line-2)"
    style="height: 38px"
  >
    <div
      v-if="workspaceLabel"
      class="mr-2 hidden max-w-52 shrink-0 items-center gap-1.5 rounded-md border border-(--line-2) bg-(--bg-1) px-2.5 h-7 text-[11px] font-semibold text-(--fg-3) lg:flex"
      :title="workspaceLabel"
    >
      <DatabaseIcon class="size-3 shrink-0 opacity-60" />
      <span class="truncate">{{ workspaceLabel }}</span>
    </div>

    <div
      class="flex min-w-0 flex-1 items-center gap-0.5 overflow-x-auto overflow-y-hidden pr-1"
      @dragover.prevent
      @drop="emit('tab-drop', null, $event)"
    >
      <!-- Tabs (Brave-style pill) -->
      <button
        v-for="tab in tabs"
        :key="tab.id"
        draggable="true"
        @click="emit('switch-tab', tab.id)"
        @mousedown="(e) => { if (e.button === 1) { e.preventDefault(); e.stopPropagation(); emit('close-tab', tab.id, e); } }"
        @dragstart="emit('tab-drag-start', tab.id, $event)"
        @dragover.prevent
        @drop.stop="emit('tab-drop', tab.id, $event)"
        :class="[
          'relative flex items-center gap-2 px-2.5 h-7 rounded-lg transition-colors min-w-30 max-w-50 shrink-0 group/tab font-mono text-[11.5px]',
          tab.id === activeTabId
            ? 'bg-(--bg-2) text-(--fg-1)'
            : 'text-(--fg-3) hover:bg-(--bg-2)/50 hover:text-(--fg-2)',
        ]"
        :title="
          tab.type === 'query'
            ? `Query · ${connectionNames[tab.connectionId] ?? ''}`
            : `${tab.tableName} · ${connectionNames[tab.connectionId] ?? ''} · ${tab.database}`
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
          {{ tab.type === "query" ? "Query" : tab.tableName }}
        </span>

        <!-- Close -->
        <span
          @click.stop="emit('close-tab', tab.id, $event)"
          :class="[
            'shrink-0 size-4 flex items-center justify-center rounded transition-all text-(--fg-4) hover:text-(--fg-1) hover:bg-(--bg-3)',
            tab.id === activeTabId
              ? 'opacity-100'
              : 'opacity-0 group-hover/tab:opacity-100',
          ]"
        >
          <XIcon class="size-2.5" />
        </span>
      </button>
    </div>

    <div class="flex shrink-0 items-center gap-0.5 border-l border-(--line-2) ml-1 pl-1.5">
    <!-- New query -->
    <button
      v-if="hasOpenConnections && firstConnectionId && canOpenQuery"
      @click="emit('new-query', firstConnectionId)"
      class="flex items-center gap-1 px-2 h-7 rounded-md text-[11px] text-(--fg-1) hover:bg-(--bg-2) transition-colors shrink-0"
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
            'size-7 flex items-center justify-center rounded-md transition-colors',
            showFilters
              ? 'bg-(--acc-soft) text-(--acc)'
              : 'text-(--fg-1) hover:bg-(--bg-2)',
          ]"
          title="Toggle Filters"
          @click="emit('toggle-filters')"
        >
          <FilterIcon class="size-4" />
        </button>
        <button
          type="button"
          class="size-7 flex items-center justify-center rounded-md text-(--fg-1) hover:bg-(--bg-2) transition-colors"
          title="Refresh"
          @click="emit('refresh')"
        >
          <RefreshCwIcon class="size-4" />
        </button>
      </div>
    </template>

<!-- Pin / focus pane -->
    <button
      v-if="showFocusButton"
      @click="emit('toggle-focus')"
      :class="[
        'flex items-center justify-center size-7 rounded-md transition-colors shrink-0',
        isFocused
          ? 'text-(--acc) bg-(--acc-soft) hover:brightness-110'
          : 'text-(--fg-1) hover:bg-(--bg-2)',
      ]"
      :title="isFocused ? `Unpin pane (${focusPaneKey})` : `Pin pane — focus (${focusPaneKey})`"
    >
      <PinOffIcon v-if="isFocused" class="size-4" />
      <PinIcon v-else class="size-4" />
    </button>

    <!-- Split pane -->
    <button
      v-if="isLastPane && !isFocused && canSplitPane"
      @click="emit('add-pane')"
      class="flex items-center justify-center size-7 rounded-md text-(--fg-1) hover:bg-(--bg-2) transition-colors shrink-0"
      title="Split pane"
    >
      <PanelRightOpenIcon class="size-4" />
    </button>

    <!-- Close pane -->
    <button
      v-if="showClosePaneButton && !isFocused"
      @click="emit('remove-pane')"
      class="flex items-center justify-center size-7 rounded-md text-(--fg-4) hover:text-destructive hover:bg-destructive/10 transition-colors shrink-0"
      title="Close pane"
    >
      <XIcon class="size-4" />
    </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
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
}

defineProps<{
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
