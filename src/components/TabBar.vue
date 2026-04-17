<script setup lang="ts">
import { computed } from "vue";
import { useKeybindings, formatKeybinding } from "@/composables/useKeybindings";
import {
  TerminalIcon,
  TableIcon,
  XIcon,
  PlusIcon,
  FilterIcon,
  RefreshCwIcon,
  PanelRightOpenIcon,
  ChevronRightIcon,
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

const props = defineProps<{
  paneId: string;
  tabs: AnyTab[];
  activeTabId: string | null;
  connectionNames: Record<string, string>;
  connectionEnvironments: Record<string, Environment>;
  hasOpenConnections: boolean;
  firstConnectionId: string | null;
  showFilters: boolean;
  hasActiveTableTab: boolean;
  isLastPane: boolean;
  showClosePaneButton: boolean;
  isFocused: boolean;
  showFocusButton: boolean;
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
}>();

const getEnvTopColor = (env: Environment): string => {
  switch (env) {
    case "PRODUCTION": return "bg-red-500";
    case "STAGING":    return "bg-orange-500";
    case "DEV":        return "bg-blue-500";
    default:           return "bg-green-500";
  }
};

const getEnvDotColor = (env: Environment): string => {
  switch (env) {
    case "PRODUCTION": return "bg-red-400";
    case "STAGING":    return "bg-orange-400";
    case "DEV":        return "bg-blue-400";
    default:           return "bg-green-400";
  }
};

const { getBinding } = useKeybindings();
const focusPaneKey = computed(() => formatKeybinding(getBinding('focusPane')));

const activeTab = computed(() => props.tabs.find(t => t.id === props.activeTabId));
const paneConnectionName = computed(() =>
  activeTab.value ? (props.connectionNames[activeTab.value.connectionId] ?? null) : null
);
const paneDatabase = computed(() => activeTab.value?.database ?? null);
const paneEnv = computed(() =>
  activeTab.value ? (props.connectionEnvironments[activeTab.value.connectionId] ?? null) : null
);
</script>

<template>
  <div
    v-if="tabs.length > 0 || hasOpenConnections"
    class="flex items-stretch border-b border-border bg-muted/5 overflow-x-auto shrink-0"
    style="height: 36px"
  >
    <!-- Pane connection identity -->
    <div
      v-if="paneConnectionName"
      class="flex items-center gap-1.5 px-2.5 border-r border-border shrink-0"
    >
      <span :class="['size-1.5 rounded-full shrink-0', paneEnv ? getEnvDotColor(paneEnv) : 'bg-muted-foreground/30']" />
      <span class="text-[11px] font-semibold text-foreground/60 whitespace-nowrap">{{ paneConnectionName }}</span>
      <ChevronRightIcon class="size-2.5 text-muted-foreground/30 shrink-0" />
      <span class="text-[11px] text-muted-foreground/50 whitespace-nowrap">{{ paneDatabase }}</span>
    </div>

    <!-- Tabs -->
    <button
      v-for="tab in tabs"
      :key="tab.id"
      @click="emit('switch-tab', tab.id)"
      @mousedown="(e) => { if (e.button === 1) { e.preventDefault(); e.stopPropagation(); emit('close-tab', tab.id, e); } }"
      :class="[
        'relative flex items-center gap-1.5 px-3 border-r border-border transition-colors min-w-0 max-w-48 shrink-0 group/tab',
        tab.id === activeTabId
          ? 'bg-background text-foreground'
          : 'text-muted-foreground hover:bg-muted/20 hover:text-foreground',
      ]"
      :title="
        tab.type === 'query'
          ? `Query · ${connectionNames[tab.connectionId] ?? ''}`
          : `${tab.tableName} · ${connectionNames[tab.connectionId] ?? ''} · ${tab.database}`
      "
    >
      <!-- Env top indicator (active tab only) -->
      <span
        v-if="tab.id === activeTabId"
        :class="['absolute inset-x-0 top-0 h-0.5', getEnvTopColor(connectionEnvironments[tab.connectionId])]"
      />

      <!-- Icon -->
      <TerminalIcon v-if="tab.type === 'query'" class="size-3 shrink-0 opacity-50" />
      <TableIcon v-else class="size-3 shrink-0 opacity-50" />

      <!-- Label -->
      <div class="flex items-center gap-1.5 min-w-0 flex-1">
        <span
          :class="['size-1.5 rounded-full shrink-0', getEnvDotColor(connectionEnvironments[tab.connectionId])]"
        />
        <span class="text-[11px] font-semibold truncate">
          {{ tab.type === "query" ? "Query" : tab.tableName }}
        </span>
        <span class="text-[9px] opacity-40 truncate hidden">
          {{ connectionNames[tab.connectionId] ?? "" }}
        </span>
      </div>

      <!-- Close -->
      <span
        @click.stop="emit('close-tab', tab.id, $event)"
        :class="[
          'shrink-0 size-4 flex items-center justify-center rounded transition-all hover:text-destructive hover:bg-destructive/10',
          tab.id === activeTabId
            ? 'opacity-30 hover:opacity-100'
            : 'opacity-0 group-hover/tab:opacity-30 group-hover/tab:hover:opacity-100',
        ]"
      >
        <XIcon class="size-2.5" />
      </span>
    </button>

    <!-- New Query -->
    <button
      v-if="hasOpenConnections && firstConnectionId"
      @click="emit('new-query', firstConnectionId)"
      class="flex items-center gap-1 px-2.5 text-muted-foreground/40 hover:text-muted-foreground hover:bg-muted/20 transition-colors shrink-0 border-r border-border"
      title="New Query (SQL)"
    >
      <TerminalIcon class="size-3" />
      <PlusIcon class="size-2.5" />
    </button>

    <div class="flex-1" />

    <!-- Table controls -->
    <template v-if="hasActiveTableTab">
      <div class="flex items-center gap-0.5 px-1.5 border-l border-border">
        <button
          type="button"
          :class="[
            'size-6 flex items-center justify-center rounded transition-colors',
            showFilters
              ? 'bg-primary/15 text-primary'
              : 'text-muted-foreground/50 hover:text-muted-foreground hover:bg-muted/30',
          ]"
          title="Toggle Filters"
          @click="emit('toggle-filters')"
        >
          <FilterIcon class="size-3" />
        </button>
        <button
          type="button"
          class="size-6 flex items-center justify-center rounded text-muted-foreground/50 hover:text-muted-foreground hover:bg-muted/30 transition-colors"
          title="Refresh"
          @click="emit('refresh')"
        >
          <RefreshCwIcon class="size-3" />
        </button>
      </div>
    </template>

    <!-- Pin / focus pane -->
    <button
      v-if="showFocusButton"
      @click="emit('toggle-focus')"
      :class="[
        'flex items-center gap-1 px-2.5 transition-colors shrink-0 border-l border-border',
        isFocused
          ? 'text-primary bg-primary/10 hover:bg-primary/20'
          : 'text-muted-foreground/30 hover:text-muted-foreground hover:bg-muted/20',
      ]"
      :title="isFocused ? `Unpin pane (${focusPaneKey})` : `Pin pane — focus (${focusPaneKey})`"
    >
      <PinOffIcon v-if="isFocused" class="size-3" />
      <PinIcon v-else class="size-3" />
    </button>

    <!-- Split pane -->
    <button
      v-if="isLastPane && !isFocused"
      @click="emit('add-pane')"
      class="flex items-center gap-1 px-2.5 text-muted-foreground/30 hover:text-muted-foreground hover:bg-muted/20 transition-colors shrink-0 border-l border-border"
      title="Split pane"
    >
      <PanelRightOpenIcon class="size-3" />
    </button>

    <!-- Close pane -->
    <button
      v-if="showClosePaneButton && !isFocused"
      @click="emit('remove-pane')"
      class="flex items-center gap-1 px-2.5 text-muted-foreground/30 hover:text-destructive hover:bg-destructive/10 transition-colors shrink-0 border-l border-border"
      title="Close pane"
    >
      <XIcon class="size-3" />
    </button>
  </div>
</template>
