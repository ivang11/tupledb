<template>
  <div class="h-full flex overflow-hidden bg-background">
    <WorkspaceConnectionPanel />

    <!-- Workspace container -->
    <div
      v-if="hasOpenConnections"
      v-show="!showConnectionManager"
      class="flex-1 flex flex-col min-w-0 overflow-hidden"
      :class="{ 'select-none': draggingPaneIdx !== null }"
    >
      <div
        ref="panesContainer"
        class="flex-1 flex min-h-0 min-w-0 overflow-hidden"
      >
        <template v-for="(pane, paneIdx) in panes" :key="pane.id">
          <PaneResizer
            v-if="paneIdx > 0 && !focusedPaneId"
            :pane-idx="paneIdx"
            :dragging-pane-idx="draggingPaneIdx"
            @resize-start="startPaneResize"
          />

          <WorkspacePane
            :pane="pane"
            :pane-idx="paneIdx"
          />
        </template>

</div>

      <StatusBar
        :active-connection-id="activeLogConnectionId"
        :active-database="activeLogDatabase"
        :active-connection-name="activeLogConnectionName"
        :view-mode="activePanePagination.viewMode"
        :page="activePanePagination.page"
        :page-size="activePanePagination.pageSize"
        :row-count="activePanePagination.rowCount"
        :total-count="activePanePagination.totalCount"
        :total-count-approximate="activePanePagination.totalCountApproximate"
        :exact-count-loading="activePanePagination.exactCountLoading"
        :is-inserting-row="activePanePagination.isInsertingRow"
        :insert-row-error="insertRowError"
        @change-page="(delta) => changePage(activePane, delta)"
        @change-limit="(limit) => changeLimit(activePane, limit)"
        @goto-offset="(offset) => gotoOffset(activePane, offset)"
        @request-exact-count="refreshExactCount(activePane)"
      />
    </div>

    <WorkspaceDialogs />
  </div>
</template>

<script setup lang="ts">
import { defineAsyncComponent, ref, watch } from "vue";
import { useKeybindings } from "@/composables/useKeybindings";
import { useSidebarState } from "@/composables/useSidebarState";
import type Sidebar from "@/components/Sidebar.vue";
import WorkspaceConnectionPanel from "@/components/WorkspaceConnectionPanel.vue";
import WorkspaceDialogs from "@/components/WorkspaceDialogs.vue";
import { useWorkspace } from "@/composables/useWorkspace";
import { usePanelResizing } from "@/composables/usePanelResizing";
import { useTableTabs } from "@/composables/useTableTabs";
import { useRowEditing } from "@/composables/useRowEditing";
import { useSidebarManager } from "@/composables/useSidebarManager";
import { useWorkspaceCoordinator } from "@/composables/useWorkspaceCoordinator";
import { useWorkspaceTableUi } from "@/composables/useWorkspaceTableUi";
import { useWorkspaceConnectionState } from "@/composables/useWorkspaceConnectionState";
import { useRowContextMenu } from "@/composables/useRowContextMenu";
import { useWorkspaceViewContexts } from "@/composables/useWorkspaceViewContexts";
import { useWorkspaceShortcuts } from "@/composables/useWorkspaceShortcuts";

const PaneResizer = defineAsyncComponent(() => import("@/components/PaneResizer.vue"));
const StatusBar = defineAsyncComponent(() => import("@/components/StatusBar.vue"));
const WorkspacePane = defineAsyncComponent(() => import("@/components/WorkspacePane.vue"));

// ── Workspace & resizing ──────────────────────────────────────────────────────

const panesContainer = ref<HTMLElement | null>(null);
const workspace = useWorkspace(panesContainer);
const {
  panes,
  activePaneId,
  draggingPaneIdx,
  focusedPaneId,
  getPane,
  addPane,
  removePane,
  toggleFocusPane,
  startPaneResize,
  getPaneTab,
  getPrimaryKey,
  getPaneConnection,
} = workspace;

const panelResizing = usePanelResizing();
const { resizeAllPanelTextareas } = panelResizing;

// ── Composables ───────────────────────────────────────────────────────────────

const tableTabs = useTableTabs({
  panes,
  activePaneId,
  focusedPaneId,
  getPane,
  getPaneTab,
  getPrimaryKey,
  getPaneConnection,
  addPane,
  removePane,
});
const {
  openQueryTab,
  switchToTab,
  closeTab,
  loadTableData,
  refreshActiveTab,
  refreshExactCount,
  sortPayload,
  changePage,
  changeLimit,
  gotoOffset,
} = tableTabs;

const rowEditing = useRowEditing({
  panes,
  getPaneTab,
  getPrimaryKey,
  getPaneConnection,
  refreshActiveTab,
  loadTableData,
});
const {
  insertingRowTabId,
  insertRowError,
  duplicateRow,
  duplicateSelectedRows,
  toggleDeletion,
  toggleDeletionSelected,
} = rowEditing;

const rowContext = useRowContextMenu({
  getPaneTab,
  duplicateRow,
  duplicateSelectedRows,
  toggleDeletion,
  toggleDeletionSelected,
});
const sidebarManager = useSidebarManager({
  panes,
  activePaneId,
  getPane,
  getPaneTab,
  switchToTab,
  closeTab,
  refreshActiveTab,
  loadTableData,
  openQueryTab,
});
const {
  selectedSidebarConnectionId,
  connectSaved,
  selectDatabase,
  disconnectConn,
  sidebarDatabaseContextMenu,
} = sidebarManager;

const tableUi = useWorkspaceTableUi({
  panes,
  activePaneId,
  selectedSidebarConnectionId,
  insertingRowTabId,
  getPane,
  getPaneTab,
  sortPayload,
});
const {
  activeLogConnectionId,
  activeLogDatabase,
  activeLogConnectionName,
  activePane,
  activePanePagination,
} = tableUi;

// ── Cross-composable wiring ───────────────────────────────────────────────────

watch(
  () => panes.value.map((p) => getPaneTab(p)?.selectedRowPk).join(","),
  () => resizeAllPanelTextareas(),
);

// ── Sidebar visibility ────────────────────────────────────────────────────────

const sidebarState = useSidebarState();
const { sidebarVisible, sidebarToggleVisible } = sidebarState;
useKeybindings();

// ── Keyboard shortcuts ────────────────────────────────────────────────────────

const sidebarRef = ref<InstanceType<typeof Sidebar> | null>(null);

const coordinator = useWorkspaceCoordinator({
  panes,
  activePaneId,
  getPane,
  getPaneTab,
  addPane,
  switchToTab,
  selectDatabase,
  disconnectConn,
  selectedSidebarConnectionId,
  sidebarDatabaseContextMenu,
  sidebarRef,
});
const {
  resetWorkspaceState,
} = coordinator;

const connectionState = useWorkspaceConnectionState({
  selectedSidebarConnectionId,
  sidebarToggleVisible,
  connectSaved,
  resetWorkspaceState,
});
const {
  hasOpenConnections,
  showConnectionManager,
} = connectionState;

useWorkspaceViewContexts({
  workspace,
  panelResizing,
  tableTabs,
  rowEditing,
  rowContext,
  sidebarManager,
  tableUi,
  coordinator,
  connectionState,
  sidebarState,
  sidebarRef,
});

useWorkspaceShortcuts({
  sidebarVisible,
  activePaneId,
  sidebarRef,
  getPane,
  toggleFocusPane,
  closeTab,
  refreshActiveTab,
});

</script>
