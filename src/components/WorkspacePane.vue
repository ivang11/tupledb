<template>
  <div
    v-show="!focusedPaneId || focusedPaneId === pane.id"
    class="relative flex flex-col min-h-0 min-w-0 overflow-hidden bg-background"
    :style="{ flex: focusedPaneId === pane.id ? 1 : paneWidths[paneIdx] }"
    :class="panes.length > 1 && pane.id === activePaneId ? 'ring-1 ring-inset ring-primary/10' : ''"
    @mousedown.capture="activatePane(pane.id)"
  >
    <TabBar
      :pane-id="pane.id"
      :tabs="visiblePaneTabs(pane) as any[]"
      :active-tab-id="pane.activeTabId"
      :connection-names="connectionNames()"
      :connection-environments="connectionEnvironments"
      :has-open-connections="hasOpenConnections"
      :first-connection-id="selectedSidebarConnectionId ?? openConnectionIds[0] ?? null"
      :can-open-query="!!(selectedSidebarConnectionId && activeConnectionState?.selectedDatabase)"
      :show-filters="pane.showFilters"
      :has-active-table-tab="!!getVisiblePaneTab(pane)"
      :is-last-pane="paneIdx === panes.length - 1"
      :show-close-pane-button="panes.length > 1"
      :is-focused="focusedPaneId === pane.id"
      :show-focus-button="panes.length > 1"
      :workspace-label="isComparingDatabases ? paneWorkspaceLabel(pane) : ''"
      :can-split-pane="!isComparingDatabases"
      @switch-tab="(id) => switchToTab(id, pane.id)"
      @close-tab="(id, e) => closeTab(id, pane.id, e)"
      @new-query="(connId) => openQueryTab(connId, selectedDatabaseForConnection(connId), pane.id)"
      @toggle-filters="pane.showFilters = !pane.showFilters"
      @refresh="refreshActiveTab(pane.id)"
      @add-pane="splitActivePane"
      @remove-pane="removePane(pane.id)"
      @toggle-focus="toggleFocusPane(pane.id)"
      @tab-drag-start="(tabId, event) => handleTabDragStart(pane.id, tabId, event)"
      @tab-drop="(targetTabId, event) => handleTabDrop(pane.id, targetTabId, event)"
    />

    <div
      v-if="isVisiblePaneActiveTabQuery(pane)"
      class="flex-1 min-h-0 flex flex-col overflow-hidden"
    >
      <QueryEditor
        v-if="getVisiblePaneActiveQueryTab(pane)"
        :key="getVisiblePaneActiveQueryTab(pane)!.id"
        :connection-id="getVisiblePaneActiveQueryTab(pane)!.connectionId"
        :database="getVisiblePaneActiveQueryTab(pane)!.database"
        :initial-sql="getVisiblePaneActiveQueryTab(pane)!.sql"
        :initial-result="getVisiblePaneActiveQueryTab(pane)!.queryResult ?? null"
        :initial-error="getVisiblePaneActiveQueryTab(pane)!.queryError ?? null"
        :initial-execution-time="getVisiblePaneActiveQueryTab(pane)!.executionTime ?? null"
        :initial-rows-limited="getVisiblePaneActiveQueryTab(pane)!.resultRowsLimited ?? false"
        :initial-total-rows="getVisiblePaneActiveQueryTab(pane)!.resultTotalRows ?? null"
        :open-tabs-schema="getSchema(getVisiblePaneActiveQueryTab(pane)!.connectionId, getVisiblePaneActiveQueryTab(pane)!.database)"
        class="flex-1 min-h-0"
        @update:sql="getVisiblePaneActiveQueryTab(pane)!.sql = $event"
        @update:result="getVisiblePaneActiveQueryTab(pane)!.queryResult = $event"
        @update:error="getVisiblePaneActiveQueryTab(pane)!.queryError = $event"
        @update:execution-time="getVisiblePaneActiveQueryTab(pane)!.executionTime = $event"
        @update:rows-limited="getVisiblePaneActiveQueryTab(pane)!.resultRowsLimited = $event"
        @update:total-rows="getVisiblePaneActiveQueryTab(pane)!.resultTotalRows = $event"
      />
    </div>

    <template v-if="getVisiblePaneTab(pane) && !isVisiblePaneActiveTabQuery(pane)">
      <TableSubTabs
        :active-mode="pane.viewMode"
        :index-count="(getPaneTab(pane)?.tableIndexes ?? []).length || null"
        :can-insert-row="pane.viewMode === 'content' && !!getPrimaryKey(pane)"
        :show-row-detail-toggle="pane.viewMode === 'content'"
        :row-detail-on-click="rowDetailOnClick"
        @set-mode="
          (mode) => {
            setViewMode(pane, mode);
            if (mode !== 'content') loadStructureViewMetadata(pane);
          }
        "
        @insert-row="openInsertRowDialog(pane)"
        @toggle-row-detail="toggleRowDetailOnClick"
      />

      <FilterBar
        v-show="pane.viewMode === 'content' && pane.showFilters"
        :key="pane.activeTabId ?? pane.id"
        :columns="(getPaneTab(pane)?.tableStructure ?? []).map((c: any) => ({ name: c.field, type_name: c.type }))"
        :initial-filter="getPaneTab(pane)?.filters"
        @apply="(filters) => applyFilters(pane, filters)"
        @clear="() => clearFilters(pane)"
      />

      <StructureView
        v-if="pane.viewMode === 'structure'"
        :table-structure="getPaneTab(pane)?.tableStructure ?? []"
        :table-indexes="getPaneTab(pane)?.tableIndexes ?? []"
        :fk-map="getFkMap(pane)"
        :ddl="getPaneTab(pane)?.ddl ?? null"
        :metadata-loading="getPaneTab(pane)?.metadataLoading ?? false"
        :metadata-loaded="getPaneTab(pane)?.metadataLoaded ?? false"
        :pane-id="pane.id"
        :index-panel-height="structureIndexHeights[pane.id]"
        :can-edit="getPaneConnection(pane)?.allow_writes !== false"
        :edit-disabled-reason="'This connection is read-only'"
        :pending-column-changes="getPaneTab(pane)?.pendingStructureChanges ?? {}"
        :has-pending-changes="hasPendingChangesInPane(pane)"
        :update-column="(oldName, newName, newType) => updatePendingStructureColumn(pane, oldName, newName, newType)"
        @start-index-resize="startStructureResize"
      />

      <IndexesView
        v-if="pane.viewMode === 'indexes'"
        :table-indexes="getPaneTab(pane)?.tableIndexes ?? []"
        :metadata-loading="getPaneTab(pane)?.metadataLoading ?? false"
      />

      <div
        v-if="pane.viewMode === 'content'"
        class="flex flex-1 min-h-0 min-w-0 flex-row"
      >
        <DataGrid
          :key="pane.activeTabId ?? pane.id"
          :columns="tableGridColumns(pane)"
          :rows="getPaneTab(pane)?.queryResult?.rows ?? []"
          :primary-key="getPrimaryKey(pane)"
          :total-count="getPaneTab(pane)?.queryResult?.total_count ?? 0"
          :pending-changes="getPaneTab(pane)?.pendingChanges ?? {}"
          :pending-deletions="getPaneTab(pane)?.pendingDeletions ?? {}"
          :pending-truncate="getPaneTab(pane)?.pendingTruncate ?? false"
          :pending-drop="getPaneTab(pane)?.pendingDrop ?? false"
          :selected-row-pk="getPaneTab(pane)?.selectedRowPk ?? null"
          :selected-row-pks="getPaneTab(pane)?.selectedRowPks ?? []"
          :inline-edit-column="getPaneTab(pane)?.inlineEditColumn ?? null"
          :sort-column="getPaneTab(pane)?.sortColumn ?? null"
          :sort-desc="getPaneTab(pane)?.sortDesc ?? false"
          :inserting-row="insertingRowTabId !== null && insertingRowTabId === pane.activeTabId"
          :insert-row-values="insertRowValues"
          :pending-inserts="getPaneTab(pane)?.pendingInserts ?? []"
          :column-widths="getColumnWidths(getPaneTab(pane))"
          :fk-map="getFkMap(pane)"
          :bottom-inset="hasPendingChangesInPane(pane) ? 128 : 0"
          :is-col-auto-increment="(col) => isColAutoIncrement(pane, col)"
          :is-boolean-col="(col) => isBooleanCol(pane, col)"
          :get-cell-value="(row, col) => cellEditValue(pane, row, col)"
          @row-click="(row, e, index) => onTableRowClick(pane, row, e, index)"
          @cell-dblclick="(row, col) => onCellDblclick(pane, row, col)"
          @cell-blur="onCellBlur(pane)"
          @cell-input="(row, col, val) => updatePendingChange(pane, row, col, val)"
          @sort="(col) => onSortColumn(pane, col)"
          @start-col-resize="(e, col) => startColResize(e, getPaneTab(pane), col)"
          @navigate-related="(table, col, val) => navigateToRelated(pane, table, col, val)"
          @insert-row-input="(col, val) => updateInsertRowValue(pane, col, val)"
          @insert-row-cancel="cancelInsertRow"
          @pending-insert-input="(index, col, val) => updatePendingInsertValue(pane, index, col, val)"
          @pending-insert-cancel="(index) => removePendingInsert(pane, index)"
          @row-contextmenu="(row, x, y) => openRowContextMenu(pane, row, x, y)"
          @delete-key-pressed="toggleDeletionSelected(pane)"
        />

        <RowDetailPanel
          v-if="rowDetailOnClick && getSelectedRow(pane) && (getPaneTab(pane)?.selectedRowPks?.length ?? 0) <= 1"
          :pane-id="pane.id"
          :row="getSelectedRow(pane)!"
          :columns="getPaneTab(pane)?.queryResult?.columns ?? []"
          :primary-key="getPrimaryKey(pane)"
          :fk-map="getFkMap(pane)"
          :pending-deletions="getPaneTab(pane)?.pendingDeletions ?? {}"
          :width="sidePanelWidths[pane.id] ?? 320"
          :get-cell-value="(row, col) => cellEditValue(pane, row, col)"
          @close="clearRowSelection(pane)"
          @cell-input="(col, val) => updatePendingChange(pane, getSelectedRow(pane)!, col, val)"
          @navigate-related="(table, col, val) => navigateToRelated(pane, table, col, val)"
          @start-resize="(e) => startSidePanelResize(e, pane.id)"
        />
      </div>

      <PendingChangesBar
        v-if="hasPendingChangesInPane(pane) && (panes.length === 1 || pane.id === activePaneId)"
        :pending-truncate="globalPendingSummary.pendingTruncate"
        :pending-drop="globalPendingSummary.pendingDrop"
        :pending-changes-count="globalPendingSummary.pendingChangesCount"
        :pending-structure-changes-count="globalPendingSummary.pendingStructureChangesCount"
        :pending-deletions-count="globalPendingSummary.pendingDeletionsCount"
        :pending-insertions-count="globalPendingSummary.pendingInsertionsCount"
        :disable-fk-checks="disableFkChecks"
        :is-saving="isSaving"
        @update:disable-fk-checks="disableFkChecks = $event"
        @discard="discardChanges(pane)"
        @apply="applyChanges(pane)"
      />
    </template>

<TableExplorerEmptyState
      v-else-if="!isVisiblePaneActiveTabQuery(pane)"
      :has-connections="hasOpenConnections"
      :has-selected-database="!!activeConnectionState?.selectedDatabase"
    />
  </div>
</template>

<script setup lang="ts">
import { defineAsyncComponent } from "vue";
import type { PaneState } from "@/types/workspace";
import { useWorkspacePaneContext } from "@/composables/useWorkspacePaneContext";
import TabBar from "@/components/TabBar.vue";
import DataGrid from "@/components/DataGrid.vue";
import FilterBar from "@/components/FilterBar.vue";
import IndexesView from "@/components/IndexesView.vue";
import PendingChangesBar from "@/components/PendingChangesBar.vue";
import RowDetailPanel from "@/components/RowDetailPanel.vue";
import StructureView from "@/components/StructureView.vue";
import TableExplorerEmptyState from "@/components/TableExplorerEmptyState.vue";
import TableSubTabs from "@/components/TableSubTabs.vue";

const QueryEditor = defineAsyncComponent(() => import("@/components/QueryEditor.vue"));

defineProps<{
  pane: PaneState;
  paneIdx: number;
}>();

const {
  panes,
  activePaneId,
  focusedPaneId,
  paneWidths,
  visiblePaneTabs,
  getVisiblePaneTab,
  isVisiblePaneActiveTabQuery,
  getVisiblePaneActiveQueryTab,
  connectionNames,
  connectionEnvironments,
  hasOpenConnections,
  openConnectionIds,
  selectedSidebarConnectionId,
  activeConnectionState,
  isComparingDatabases,
  paneWorkspaceLabel,
  getSchema,
  getPaneTab,
  getPaneConnection,
  tableGridColumns,
  getPrimaryKey,
  getFkMap,
  hasPendingChangesInPane,
  rowDetailOnClick,
  structureIndexHeights,
  sidePanelWidths,
  insertingRowTabId,
  insertRowValues,
  globalPendingSummary,
  disableFkChecks,
  isSaving,
  switchToTab,
  closeTab,
  openQueryTab,
  selectedDatabaseForConnection,
  refreshActiveTab,
  splitActivePane,
  removePane,
  toggleFocusPane,
  handleTabDragStart,
  handleTabDrop,
  activatePane,
  setViewMode,
  loadStructureViewMetadata,
  updatePendingStructureColumn,
  openInsertRowDialog,
  toggleRowDetailOnClick,
  applyFilters,
  clearFilters,
  startStructureResize,
  getColumnWidths,
  isColAutoIncrement,
  isBooleanCol,
  cellEditValue,
  onTableRowClick,
  onCellDblclick,
  onCellBlur,
  updatePendingChange,
  onSortColumn,
  startColResize,
  navigateToRelated,
  updateInsertRowValue,
  cancelInsertRow,
  updatePendingInsertValue,
  removePendingInsert,
  openRowContextMenu,
  toggleDeletionSelected,
  getSelectedRow,
  clearRowSelection,
  startSidePanelResize,
  discardChanges,
  applyChanges,
} = useWorkspacePaneContext();
</script>
