<script setup lang="ts">
import {
  TerminalIcon,
  TableIcon,
  XIcon,
  PlusIcon,
  FilterIcon,
  RefreshCwIcon,
  PanelRightOpenIcon,
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
}>();

const emit = defineEmits<{
  "switch-tab": [tabId: string];
  "close-tab": [tabId: string, e: MouseEvent];
  "new-query": [connectionId: string];
  "toggle-filters": [];
  refresh: [];
  "add-pane": [];
  "remove-pane": [];
}>();

const getEnvBorderColor = (env: Environment): string => {
  switch (env) {
    case "PRODUCTION":
      return "border-red-500";
    case "STAGING":
      return "border-orange-500";
    case "DEV":
      return "border-blue-500";
    default:
      return "border-green-500";
  }
};

const getEnvDotColor = (env: Environment): string => {
  switch (env) {
    case "PRODUCTION":
      return "bg-red-500";
    case "STAGING":
      return "bg-orange-500";
    case "DEV":
      return "bg-blue-500";
    default:
      return "bg-green-500";
  }
};
</script>

<template>
  <div
    v-if="tabs.length > 0 || hasOpenConnections"
    class="flex items-end border-b bg-muted/5 overflow-x-auto shrink-0 h-10"
  >
    <!-- Tabs -->
    <button
      v-for="tab in tabs"
      :key="tab.id"
      @click="emit('switch-tab', tab.id)"
      :class="[
        'relative flex items-center gap-2 px-3 h-full border-r transition-colors min-w-0 max-w-55 group/tab shrink-0',
        tab.id === activeTabId
          ? 'bg-background text-foreground shadow-[inset_0_2px_0_0] shadow-primary'
          : 'bg-transparent text-muted-foreground hover:bg-muted/30 hover:text-foreground',
      ]"
      :title="
        tab.type === 'query'
          ? `Query · ${connectionNames[tab.connectionId] ?? ''}`
          : `${tab.tableName} · ${connectionNames[tab.connectionId] ?? ''} · ${tab.database}`
      "
    >
      <span
        v-if="tab.id === activeTabId"
        :class="[
          'absolute inset-x-0 top-0 h-2',
          getEnvBorderColor(connectionEnvironments[tab.connectionId]),
        ]"
      />
      <TerminalIcon
        v-if="tab.type === 'query'"
        class="size-3 shrink-0 opacity-60 mt-0.5"
      />
      <TableIcon v-else class="size-3 shrink-0 opacity-60 mt-0.5" />
      <div class="flex flex-col items-start min-w-0 flex-1">
        <div class="flex items-center gap-2">
          <span
            :class="[
              'size-2 rounded-full shrink-0',
              getEnvDotColor(connectionEnvironments[tab.connectionId]),
            ]"
          ></span>
          <span class="text-sm font-semibold truncate leading-tight">
            {{ tab.type === "query" ? "Query" : tab.tableName }}
          </span>
        </div>
        <span class="text-[9px] font-medium truncate leading-tight opacity-50">
          {{ connectionNames[tab.connectionId] ?? ""
          }}<template v-if="tab.database"> · {{ tab.database }}</template>
        </span>
      </div>
      <span
        @click.stop="emit('close-tab', tab.id, $event)"
        :class="[
          'shrink-0 size-3.5 flex items-center justify-center rounded transition-all hover:text-destructive',
          tab.id === activeTabId
            ? 'opacity-40 hover:opacity-100'
            : 'opacity-0 group-hover/tab:opacity-40 group-hover/tab:hover:opacity-100',
        ]"
      >
        <XIcon class="size-3" />
      </span>
    </button>

    <!-- New Query button -->
    <button
      v-if="hasOpenConnections && firstConnectionId"
      @click="emit('new-query', firstConnectionId)"
      class="flex items-center gap-1 px-3 h-full text-[11px] text-muted-foreground/50 hover:text-muted-foreground hover:bg-muted/20 transition-colors shrink-0 border-r"
      title="New Query"
    >
      <TerminalIcon class="size-3" />
      <PlusIcon class="size-2.5" />
    </button>

    <div class="flex-1" />

    <!-- Table controls -->
    <template v-if="hasActiveTableTab">
      <div class="flex items-center gap-1 px-2 border-r h-full">
        <button
          type="button"
          class="size-6 flex items-center justify-center rounded border transition-colors"
          :class="
            showFilters
              ? 'bg-primary/10 text-primary border-primary/20'
              : 'text-muted-foreground border-transparent hover:border-border hover:bg-muted/30'
          "
          title="Toggle Filters"
          @click="emit('toggle-filters')"
        >
          <FilterIcon class="size-3" />
        </button>
        <button
          type="button"
          class="size-6 flex items-center justify-center rounded border border-transparent text-muted-foreground hover:border-border hover:bg-muted/30 transition-colors"
          title="Refresh"
          @click="emit('refresh')"
        >
          <RefreshCwIcon class="size-3" />
        </button>
      </div>
    </template>

    <!-- Split pane -->
    <button
      v-if="isLastPane"
      @click="emit('add-pane')"
      class="flex items-center gap-1 px-3 h-full text-[11px] text-muted-foreground/40 hover:text-muted-foreground hover:bg-muted/20 transition-colors shrink-0"
      title="Split pane"
    >
      <PanelRightOpenIcon class="size-3.5" />
    </button>

    <!-- Close pane -->
    <button
      v-if="showClosePaneButton"
      @click="emit('remove-pane')"
      class="flex items-center gap-1 px-3 h-full text-[11px] text-muted-foreground/40 hover:text-destructive hover:bg-muted/20 transition-colors shrink-0 border-l"
      title="Close pane"
    >
      <XIcon class="size-3.5" />
    </button>
  </div>
</template>
