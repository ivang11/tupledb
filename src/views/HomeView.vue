<script setup lang="ts">
import { ref, watch, nextTick, defineAsyncComponent } from "vue";
import type { TableTab } from "@/types/workspace";
import { useConnectionStore } from "@/stores/connections";
const QueryEditor = defineAsyncComponent(() => import("@/components/QueryEditor.vue"));
import FilterBar from "@/components/FilterBar.vue";
import Sidebar from "@/components/Sidebar.vue";
import TabBar from "@/components/TabBar.vue";
import DataGrid from "@/components/DataGrid.vue";
import StructureView from "@/components/StructureView.vue";
import RowDetailPanel from "@/components/RowDetailPanel.vue";
import PaginationFooter from "@/components/PaginationFooter.vue";
import PendingChangesBar from "@/components/PendingChangesBar.vue";
import TableExplorerEmptyState from "@/components/TableExplorerEmptyState.vue";
import PaneResizer from "@/components/PaneResizer.vue";
import ConnectionDialog from "@/components/dialogs/ConnectionDialog.vue";
import DeleteConfirmDialog from "@/components/dialogs/DeleteConfirmDialog.vue";
import ExportDialog from "@/components/dialogs/ExportDialog.vue";
import TableActionDialog from "@/components/dialogs/TableActionDialog.vue";
import BulkTableActionDialog from "@/components/dialogs/BulkTableActionDialog.vue";
import DatabaseActionDialog from "@/components/dialogs/DatabaseActionDialog.vue";
import DeleteTablesDialog from "@/components/dialogs/DeleteTablesDialog.vue";
import ConnectionContextMenu from "@/components/ConnectionContextMenu.vue";
import TableContextMenu from "@/components/TableContextMenu.vue";
import DatabaseContextMenu from "@/components/DatabaseContextMenu.vue";
import { useWorkspace } from "@/composables/useWorkspace";
import { usePanelResizing } from "@/composables/usePanelResizing";
import { useTableTabs } from "@/composables/useTableTabs";
import { useRowEditing } from "@/composables/useRowEditing";
import { useSidebarManager } from "@/composables/useSidebarManager";
import { useActionShortcut } from "@/composables/useKeyboardShortcut";

const store = useConnectionStore();

// ── Workspace & resizing ──────────────────────────────────────────────────────

const panesContainer = ref<HTMLElement | null>(null);
const {
  panes,
  activePaneId,
  paneWidths,
  draggingPaneIdx,
  getPane,
  addPane,
  removePane,
  startPaneResize,
  getPaneTab,
  isPaneActiveTabQuery,
  getPaneQueryTabs,
  getPrimaryKey,
  hasPendingChangesInPane,
  getFkMap,
  getPaneConnection,
  isTableOpenInAnyPane,
  isTableActiveInAnyPane,
} = useWorkspace(panesContainer);

const {
  sidebarWidth,
  startSidebarResize,
  sidePanelWidths,
  startSidePanelResize,
  startColResize,
  getColumnWidths,
  structureIndexHeights,
  startStructureResize,
  resizeAllPanelTextareas,
} = usePanelResizing();

// ── Composables ───────────────────────────────────────────────────────────────

const {
  openQueryTab,
  switchToTab,
  closeTab,
  loadTableData,
  refreshActiveTab,
  sortPayload,
  changePage,
  changeLimit,
  gotoOffset,
  onSortColumn,
  connectionNames,
  getAvailableDatabases,
} = useTableTabs({
  panes,
  activePaneId,
  getPane,
  getPaneTab,
  getPrimaryKey,
  getPaneConnection,
});

const {
  isSaving,
  disableFkChecks,
  insertingRowPaneId,
  insertRowValues,
  insertRowLoading,
  insertRowError,
  isColAutoIncrement,
  isBooleanCol,
  openInsertRowDialog,
  cancelInsertRow,
  submitInsertRow,
  updatePendingChange,
  discardChanges,
  clearRowSelection,
  getSelectedRow,
  onTableRowClick,
  onCellDblclick,
  onCellBlur,
  cellEditValue,
  setViewMode,
  applyChanges,
  navigateToRelated,
} = useRowEditing({
  panes,
  getPaneTab,
  getPrimaryKey,
  getPaneConnection,
  refreshActiveTab,
  loadTableData,
});

const showBulkTruncateDialog = ref(false);

const {
  search,
  expandedConnections,
  expandedDatabases,
  showNewDb,
  newDbName,
  isCreatingDb,
  connectingId,
  closedConnections,
  filteredTables,
  connectSaved,
  toggleConnection,
  toggleDatabase,
  disconnectConn,
  createDatabase,
  importSql,
  showTableSelector,
  isLoadingExportTables,
  selectedExportTables,
  currentExportMode,
  exportContext,
  exportContextTables,
  openExportSelector,
  startExport,
  showTableActionDialog,
  tableActionData,
  isExecutingTableAction,
  confirmSidebarTableAction,
  executeTableAction,
  // Multiple table selection
  selectedTables,
  showBulkTableActionDialog,
  isExecutingBulkTableAction,
  isTableSelected,
  toggleTableSelection,
  selectTableRange,
  clearTableSelection,
  executeBulkTableDeletion,
  executeBulkTableTruncation,
  // Database actions
  showDatabaseActionDialog,
  databaseActionData,
  isExecutingDatabaseAction,
  confirmSidebarDatabaseAction,
  executeDatabaseAction,
  // Delete tables dialog
  showDeleteTablesDialog,
  isLoadingDeleteTables,
  isExecutingDeleteTables,
  deleteTablesError,
  deleteTablesContext,
  deleteTablesDialogTables,
  openDeleteTablesDialog,
  executeDeleteTablesFromDialog,
  executeDropDatabaseFromDeleteDialog,
  // Context menus
  sidebarContextMenu,
  sidebarTableContextMenu,
  sidebarDatabaseContextMenu,
  openSidebarContextMenu,
  openSidebarTableContextMenu,
  openSidebarDatabaseContextMenu,
  showNewConnDialog,
  isSavingConn,
  newConn,
  openNewConnDialog,
  openEditConnDialog,
  handleDuplicateConnection,
  saveNewConn,
  showDeleteConnDialog,
  confirmDeleteConn,
  deleteConn,
} = useSidebarManager({
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

// ── Cross-composable wiring ───────────────────────────────────────────────────

watch(
  () => panes.value.map((p) => getPaneTab(p)?.selectedRowPk).join(","),
  () => resizeAllPanelTextareas(),
);

// ── Keyboard shortcuts ────────────────────────────────────────────────────────

const sidebarRef = ref<InstanceType<typeof Sidebar> | null>(null);

useActionShortcut("closeTab", () => {
  const activePane = getPane(activePaneId.value);
  if (activePane?.activeTabId) {
    closeTab(activePane.activeTabId, activePane.id);
  }
});

useActionShortcut("sidebarSearch", () => {
  sidebarRef.value?.focusSearch();
});

useActionShortcut("refreshTable", () => {
  refreshActiveTab(activePaneId.value);
});

// Scroll sidebar to active table when the active tab changes
watch(
  () => {
    const pane = getPane(activePaneId.value);
    const tab = getPaneTab(pane);
    return tab ? `${tab.connectionId}:${tab.database}:${tab.tableName}` : null;
  },
  (key) => {
    if (!key) return;
    const [connId, db, tableName] = key.split(":");
    nextTick(() => sidebarRef.value?.scrollToTable(tableName, db, connId));
  },
);

// ── FilterBar handlers (need store + tab context) ─────────────────────────────

async function applyFilters(pane: ReturnType<typeof getPane>, filters: any) {
  const t = getPaneTab(pane);
  if (!t) return;
  t.selectedRowPk = null;
  t.inlineEditColumn = null;
  t.filters = filters;
  t.queryResult = await store.fetchTableData(
    t.connectionId,
    t.database,
    t.tableName,
    pane.page,
    pane.pageSize,
    filters,
    sortPayload(t),
  );
}

async function clearFilters(pane: ReturnType<typeof getPane>) {
  const t = getPaneTab(pane);
  if (!t) return;
  t.selectedRowPk = null;
  t.inlineEditColumn = null;
  t.filters = null;
  t.queryResult = await store.fetchTableData(
    t.connectionId,
    t.database,
    t.tableName,
    pane.page,
    pane.pageSize,
    null,
    sortPayload(t),
  );
}

// ── Query editor schema (tables + columns already in memory) ─────────────────

function getSchema(connectionId: string, database: string | null): Record<string, string[]> {
  if (!database) return {}
  const tables = store.openConnections[connectionId]?.tables[database] ?? []
  const schema: Record<string, string[]> = {}
  for (const t of tables) {
    schema[t.name] = []
  }
  // Fill columns from any open table tabs for this connection + database
  for (const pane of panes.value) {
    for (const tab of pane.tabs) {
      if (tab.type === 'table' && tab.connectionId === connectionId && (tab as TableTab).database === database) {
        const tt = tab as TableTab
        if (tt.tableStructure?.length) {
          schema[tt.tableName] = tt.tableStructure.map((c: any) => c.field)
        }
      }
    }
  }
  return schema
}
</script>

<template>
  <div class="h-full flex overflow-hidden bg-background">
    <!-- Sidebar -->
    <Sidebar
      ref="sidebarRef"
      :width="sidebarWidth"
      :search="search"
      :open-connections="store.openConnections"
      :closed-connections="closedConnections"
      :expanded-connections="expandedConnections"
      :expanded-databases="expandedDatabases"
      :connecting-id="connectingId"
      :show-new-db="showNewDb"
      :new-db-name="newDbName"
      :is-creating-db="isCreatingDb"
      :is-table-active="isTableActiveInAnyPane"
      :is-table-open="isTableOpenInAnyPane"
      :filtered-tables="filteredTables"
      :is-table-selected="isTableSelected"
      @update:search="search = $event"
      @update:show-new-db="showNewDb = $event"
      @update:new-db-name="newDbName = $event"
      @new-connection="openNewConnDialog"
      @connect-saved="connectSaved"
      @toggle-connection="toggleConnection"
      @toggle-database="toggleDatabase"
      @load-table="loadTableData"
      @toggle-table-selection="toggleTableSelection"
      @select-table-range="selectTableRange"
      @clear-table-selection="clearTableSelection"
      @open-query="openQueryTab"
      @import-sql="importSql"
      @export-database="openExportSelector"
      @create-database="createDatabase"
      @context-menu-connection="openSidebarContextMenu"
      @context-menu-table="openSidebarTableContextMenu"
      @context-menu-database="openSidebarDatabaseContextMenu"
      @resize-start="startSidebarResize"
    />

    <!-- Panes container -->
    <div
      ref="panesContainer"
      class="flex-1 flex min-w-0 overflow-hidden"
      :class="{ 'select-none': draggingPaneIdx !== null }"
    >
      <template v-for="(pane, paneIdx) in panes" :key="pane.id">
        <PaneResizer
          v-if="paneIdx > 0"
          :pane-idx="paneIdx"
          :dragging-pane-idx="draggingPaneIdx"
          @resize-start="startPaneResize"
        />

        <div
          class="flex flex-col min-h-0 min-w-0 overflow-hidden bg-background"
          :style="{ flex: paneWidths[paneIdx] }"
          :class="
            panes.length > 1 && pane.id === activePaneId
              ? 'ring-1 ring-inset ring-primary/10'
              : ''
          "
          @mousedown.capture="activePaneId = pane.id"
        >
          <!-- Tab bar -->
          <TabBar
            :pane-id="pane.id"
            :tabs="pane.tabs as any[]"
            :active-tab-id="pane.activeTabId"
            :connection-names="connectionNames()"
            :connection-environments="
              Object.fromEntries(
                Object.entries(store.openConnections).map(([id, state]) => [
                  id,
                  state.connection.environment,
                ]),
              )
            "
            :has-open-connections="
              Object.keys(store.openConnections).length > 0
            "
            :first-connection-id="Object.keys(store.openConnections)[0] ?? null"
            :show-filters="pane.showFilters"
            :has-active-table-tab="!!getPaneTab(pane)"
            :is-last-pane="paneIdx === panes.length - 1"
            :show-close-pane-button="panes.length > 1"
            @switch-tab="(id) => switchToTab(id, pane.id)"
            @close-tab="(id, e) => closeTab(id, pane.id, e)"
            @new-query="(connId) => openQueryTab(connId, null, pane.id)"
            @toggle-filters="pane.showFilters = !pane.showFilters"
            @refresh="refreshActiveTab(pane.id)"
            @add-pane="addPane"
            @remove-pane="removePane(pane.id)"
          />

          <!-- Query Editors -->
          <div
            v-if="isPaneActiveTabQuery(pane)"
            class="flex-1 min-h-0 flex flex-col overflow-hidden"
          >
            <QueryEditor
              v-for="qTab in getPaneQueryTabs(pane)"
              v-show="qTab.id === pane.activeTabId"
              :key="qTab.id"
              :connection-id="qTab.connectionId"
              :database="qTab.database"
              :available-databases="getAvailableDatabases(qTab.connectionId)"
              :initial-sql="qTab.sql"
              :open-tabs-schema="getSchema(qTab.connectionId, qTab.database)"
              class="flex-1 min-h-0"
              style="display: flex"
              @update:sql="qTab.sql = $event"
            />
          </div>

          <!-- Table content -->
          <template v-if="getPaneTab(pane) && !isPaneActiveTabQuery(pane)">
            <!-- Filters -->
            <FilterBar
              v-show="pane.viewMode === 'content' && pane.showFilters"
              :key="pane.activeTabId"
              :columns="getPaneTab(pane)?.queryResult?.columns ?? []"
              :initial-filter="getPaneTab(pane)?.filters"
              @apply="(filters) => applyFilters(pane, filters)"
              @clear="() => clearFilters(pane)"
            />

            <!-- Structure View -->
            <StructureView
              v-if="pane.viewMode === 'structure'"
              :table-structure="getPaneTab(pane)?.tableStructure ?? []"
              :table-indexes="getPaneTab(pane)?.tableIndexes ?? []"
              :fk-map="getFkMap(pane)"
              :ddl="getPaneTab(pane)?.ddl ?? null"
              :pane-id="pane.id"
              :index-panel-height="structureIndexHeights[pane.id]"
              @start-index-resize="startStructureResize"
            />

            <!-- Data table + row detail panel -->
            <div
              v-if="pane.viewMode === 'content'"
              class="flex flex-1 min-h-0 min-w-0 flex-row"
            >
              <DataGrid
                :columns="getPaneTab(pane)?.queryResult?.columns ?? []"
                :rows="getPaneTab(pane)?.queryResult?.rows ?? []"
                :primary-key="getPrimaryKey(pane)"
                :total-count="getPaneTab(pane)?.queryResult?.total_count ?? 0"
                :pending-changes="getPaneTab(pane)?.pendingChanges ?? {}"
                :pending-deletions="getPaneTab(pane)?.pendingDeletions ?? {}"
                :pending-truncate="getPaneTab(pane)?.pendingTruncate ?? false"
                :selected-row-pk="getPaneTab(pane)?.selectedRowPk ?? null"
                :inline-edit-column="getPaneTab(pane)?.inlineEditColumn ?? null"
                :sort-column="getPaneTab(pane)?.sortColumn ?? null"
                :sort-desc="getPaneTab(pane)?.sortDesc ?? false"
                :inserting-row="insertingRowPaneId === pane.id"
                :insert-row-values="insertRowValues"
                :column-widths="getColumnWidths(getPaneTab(pane))"
                :fk-map="getFkMap(pane)"
                :is-col-auto-increment="(col) => isColAutoIncrement(pane, col)"
                :is-boolean-col="(col) => isBooleanCol(pane, col)"
                :get-cell-value="(row, col) => cellEditValue(pane, row, col)"
                @row-click="(row, e) => onTableRowClick(pane, row, e)"
                @cell-dblclick="(row, col) => onCellDblclick(pane, row, col)"
                @cell-blur="onCellBlur(pane)"
                @cell-input="
                  (row, col, val) => updatePendingChange(pane, row, col, val)
                "
                @sort="(col) => onSortColumn(pane, col)"
                @start-col-resize="
                  (e, col) => startColResize(e, getPaneTab(pane), col)
                "
                @navigate-related="
                  (table, col, val) => navigateToRelated(pane, table, col, val)
                "
                @insert-row-input="
                  (col, val) => {
                    insertRowValues[col] = val;
                  }
                "
                @insert-row-submit="submitInsertRow(pane)"
                @insert-row-cancel="cancelInsertRow"
              />

              <RowDetailPanel
                v-if="getPrimaryKey(pane) && getSelectedRow(pane)"
                :pane-id="pane.id"
                :row="getSelectedRow(pane)!"
                :columns="getPaneTab(pane)?.queryResult?.columns ?? []"
                :primary-key="getPrimaryKey(pane)"
                :fk-map="getFkMap(pane)"
                :pending-deletions="getPaneTab(pane)?.pendingDeletions ?? {}"
                :width="sidePanelWidths[pane.id] ?? 320"
                :get-cell-value="(row, col) => cellEditValue(pane, row, col)"
                @close="clearRowSelection(pane)"
                @cell-input="
                  (col, val) =>
                    updatePendingChange(pane, getSelectedRow(pane)!, col, val)
                "
                @navigate-related="
                  (table, col, val) => navigateToRelated(pane, table, col, val)
                "
                @start-resize="(e) => startSidePanelResize(e, pane.id)"
              />
            </div>

            <!-- Pending Changes Bar -->
            <PendingChangesBar
              v-if="
                hasPendingChangesInPane(pane) &&
                (panes.length === 1 || pane.id === activePaneId)
              "
              :pending-truncate="getPaneTab(pane)?.pendingTruncate ?? false"
              :pending-changes-count="
                Object.keys(getPaneTab(pane)?.pendingChanges || {}).length
              "
              :pending-deletions-count="
                Object.keys(getPaneTab(pane)?.pendingDeletions || {}).length
              "
              :disable-fk-checks="disableFkChecks"
              :is-saving="isSaving"
              @update:disable-fk-checks="disableFkChecks = $event"
              @discard="discardChanges(pane)"
              @apply="applyChanges(pane)"
            />

            <!-- Pagination Footer -->
            <PaginationFooter
              :view-mode="pane.viewMode"
              :page="pane.page"
              :page-size="pane.pageSize"
              :total-count="getPaneTab(pane)?.queryResult?.total_count ?? 0"
              :is-inserting-row="insertingRowPaneId === pane.id"
              :insert-row-error="insertRowError"
              :insert-row-loading="insertRowLoading"
              @set-view-mode="(mode) => setViewMode(pane, mode)"
              @toggle-insert-row="openInsertRowDialog(pane)"
              @submit-insert-row="submitInsertRow(pane)"
              @cancel-insert-row="cancelInsertRow"
              @change-page="(delta) => changePage(pane, delta)"
              @change-limit="(limit) => changeLimit(pane, limit)"
              @goto-offset="(offset) => gotoOffset(pane, offset)"
            />
          </template>

          <!-- Empty State -->
          <TableExplorerEmptyState
            v-else-if="!isPaneActiveTabQuery(pane)"
            :has-connections="Object.keys(store.openConnections).length > 0"
          />
        </div>
      </template>
    </div>

    <!-- Dialogs -->
    <ExportDialog
      :open="showTableSelector"
      :database="exportContext?.database ?? ''"
      :tables="exportContextTables"
      :loading-tables="isLoadingExportTables"
      :selected-tables="selectedExportTables"
      :current-mode="currentExportMode"
      @update:open="
        (val) => {
          if (!val) showTableSelector = false;
        }
      "
      @update:selected-tables="selectedExportTables = $event"
      @update:current-mode="currentExportMode = $event"
      @start="startExport"
    />
    <ConnectionDialog
      :open="showNewConnDialog"
      :connection="newConn"
      :is-saving="isSavingConn"
      :show-connect-button="true"
      @update:open="
        (val) => {
          if (!val) showNewConnDialog = false;
        }
      "
      @save="saveNewConn"
    />
    <DeleteConfirmDialog
      :open="showDeleteConnDialog"
      title="Delete Connection"
      description="Are you sure you want to delete this connection? This action cannot be undone."
      @update:open="
        (val) => {
          if (!val) showDeleteConnDialog = false;
        }
      "
      @confirm="deleteConn"
    />
    <TableActionDialog
      v-if="tableActionData"
      :open="showTableActionDialog"
      :type="tableActionData.type"
      :table-name="tableActionData.tableName"
      :is-executing="isExecutingTableAction"
      @update:open="
        (val) => {
          if (!val) showTableActionDialog = false;
        }
      "
      @confirm="executeTableAction"
    />
    <BulkTableActionDialog
      :open="showBulkTableActionDialog"
      :count="selectedTables.size"
      :is-executing="isExecutingBulkTableAction"
      type="drop"
      @update:open="
        (val) => {
          if (!val) showBulkTableActionDialog = false;
        }
      "
      @confirm="executeBulkTableDeletion"
    />
    <BulkTableActionDialog
      :open="showBulkTruncateDialog"
      :count="selectedTables.size"
      :is-executing="isExecutingBulkTableAction"
      type="truncate"
      @update:open="
        (val) => {
          if (!val) showBulkTruncateDialog = false;
        }
      "
      @confirm="executeBulkTableTruncation"
    />
    <DatabaseActionDialog
      v-if="databaseActionData"
      :open="showDatabaseActionDialog"
      :database-name="databaseActionData.databaseName"
      :is-executing="isExecutingDatabaseAction"
      @update:open="
        (val) => {
          if (!val) showDatabaseActionDialog = false;
        }
      "
      @confirm="executeDatabaseAction"
    />

    <!-- Context Menus -->
    <ConnectionContextMenu
      :show="sidebarContextMenu.show"
      :x="sidebarContextMenu.x"
      :y="sidebarContextMenu.y"
      :connection="sidebarContextMenu.connection"
      :is-connected="
        !!sidebarContextMenu.connection &&
        !!store.openConnections[sidebarContextMenu.connection.id]
      "
      @edit="
        (conn) => {
          openEditConnDialog(conn);
          sidebarContextMenu.show = false;
        }
      "
      @duplicate="handleDuplicateConnection"
      @disconnect="
        (id) => {
          disconnectConn(id);
          sidebarContextMenu.show = false;
        }
      "
      @new-database="
        (id) => {
          if (!expandedConnections.has(id)) expandedConnections.add(id);
          showNewDb = id;
          newDbName = '';
          sidebarContextMenu.show = false;
        }
      "
      @delete="confirmDeleteConn"
    />
    <TableContextMenu
      :show="sidebarTableContextMenu.show"
      :x="sidebarTableContextMenu.x"
      :y="sidebarTableContextMenu.y"
      :table-name="sidebarTableContextMenu.tableName"
      :selected-count="sidebarTableContextMenu.selectedCount"
      @truncate="
        confirmSidebarTableAction(
          'truncate',
          sidebarTableContextMenu.connectionId,
          sidebarTableContextMenu.database,
          sidebarTableContextMenu.tableName,
        )
      "
      @drop="
        confirmSidebarTableAction(
          'drop',
          sidebarTableContextMenu.connectionId,
          sidebarTableContextMenu.database,
          sidebarTableContextMenu.tableName,
        )
      "
      @truncate-selected="showBulkTruncateDialog = true"
      @drop-selected="showBulkTableActionDialog = true"
    />
    <DatabaseContextMenu
      :show="sidebarDatabaseContextMenu.show"
      :x="sidebarDatabaseContextMenu.x"
      :y="sidebarDatabaseContextMenu.y"
      :database-name="sidebarDatabaseContextMenu.databaseName"
      @open-query="
        openQueryTab(
          sidebarDatabaseContextMenu.connectionId,
          sidebarDatabaseContextMenu.databaseName,
        )
      "
      @import-sql="
        importSql(
          sidebarDatabaseContextMenu.connectionId,
          sidebarDatabaseContextMenu.databaseName,
        )
      "
      @export-database="
        openExportSelector(
          sidebarDatabaseContextMenu.connectionId,
          sidebarDatabaseContextMenu.databaseName,
        )
      "
      @delete-tables="
        openDeleteTablesDialog(
          sidebarDatabaseContextMenu.connectionId,
          sidebarDatabaseContextMenu.databaseName,
        )
      "
      @drop="
        confirmSidebarDatabaseAction(
          sidebarDatabaseContextMenu.connectionId,
          sidebarDatabaseContextMenu.databaseName,
        )
      "
    />
    <DeleteTablesDialog
      :open="showDeleteTablesDialog"
      :database="deleteTablesContext?.database ?? ''"
      :tables="deleteTablesDialogTables"
      :loading-tables="isLoadingDeleteTables"
      :is-executing="isExecutingDeleteTables"
      :error="deleteTablesError"
      @update:open="(val) => { if (!val) showDeleteTablesDialog = false }"
      @delete-tables="(tables, disableFk) => executeDeleteTablesFromDialog(tables, disableFk)"
      @drop-database="executeDropDatabaseFromDeleteDialog"
    />
  </div>
</template>
