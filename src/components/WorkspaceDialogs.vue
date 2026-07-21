<template>
  <ExportDialog
    v-if="showTableSelector"
    :open="showTableSelector"
    :database="exportContext?.database ?? ''"
    :tables="exportContextTables"
    :loading-tables="isLoadingExportTables"
    :selected-tables="selectedExportTables"
    :current-mode="currentExportMode"
    @update:open="(val) => { if (!val) showTableSelector = false }"
    @update:selected-tables="selectedExportTables = $event"
    @update:current-mode="currentExportMode = $event"
    @start="(payload) => startExport(payload)"
  />
  <ImportDialog
    v-if="showImportDialog"
    :open="showImportDialog"
    :database="importContext?.database ?? ''"
    @update:open="(val) => { if (!val) showImportDialog = false }"
    @start-file="() => confirmImportFromBrowse()"
    @start-file-path="(path) => confirmImportFromFilePath(path)"
  />
  <ConnectionDialog
    v-if="showNewConnDialog"
    :open="showNewConnDialog"
    :connection="newConn"
    :is-saving="isSavingConn"
    :show-connect-button="true"
    @update:open="(val) => { if (!val) showNewConnDialog = false }"
    @save="saveNewConn"
  />
  <NewDatabaseDialog
    v-if="!!showNewDb"
    :open="!!showNewDb"
    :connection-name="showNewDb ? (store.openConnections[showNewDb]?.connection.name ?? '') : ''"
    :name="newDbName"
    :is-creating="isCreatingDb"
    @update:open="
      (val) => {
        if (!val && !isCreatingDb) {
          showNewDb = null;
          newDbName = '';
        }
      }
    "
    @update:name="newDbName = $event"
    @create="showNewDb && createDatabase(showNewDb)"
  />
  <DeleteConfirmDialog
    v-if="showDeleteConnDialog"
    :open="showDeleteConnDialog"
    title="Delete Connection"
    description="Are you sure you want to delete this connection? This action cannot be undone."
    @update:open="(val) => { if (!val) showDeleteConnDialog = false }"
    @confirm="deleteConn"
  />
  <DatabaseActionDialog
    v-if="databaseActionData"
    :open="showDatabaseActionDialog"
    :database-name="databaseActionData.databaseName"
    :is-executing="isExecutingDatabaseAction"
    @update:open="(val) => { if (!val) showDatabaseActionDialog = false }"
    @confirm="executeDatabaseAction"
  />

  <ConnectionContextMenu
    v-if="sidebarContextMenu.show"
    :show="sidebarContextMenu.show"
    :x="sidebarContextMenu.x"
    :y="sidebarContextMenu.y"
    :connection="sidebarContextMenu.connection"
    :is-connected="!!sidebarContextMenu.connection && !!store.openConnections[sidebarContextMenu.connection.id]"
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
        selectedSidebarConnectionId = id;
        showNewDb = id;
        newDbName = '';
        sidebarContextMenu.show = false;
      }
    "
    @delete="confirmDeleteConn"
  />
  <TableContextMenu
    v-if="sidebarTableContextMenu.show"
    :show="sidebarTableContextMenu.show"
    :x="sidebarTableContextMenu.x"
    :y="sidebarTableContextMenu.y"
    :table-name="sidebarTableContextMenu.tableName"
    :selected-count="sidebarTableContextMenu.selectedCount"
    @truncate="
      stageSidebarTableAction(
        'truncate',
        sidebarTableContextMenu.connectionId,
        sidebarTableContextMenu.database,
        sidebarTableContextMenu.tableName,
      );
      sidebarTableContextMenu.show = false
    "
    @drop="
      stageSidebarTableAction(
        'drop',
        sidebarTableContextMenu.connectionId,
        sidebarTableContextMenu.database,
        sidebarTableContextMenu.tableName,
      );
      sidebarTableContextMenu.show = false
    "
    @truncate-selected="
      stageSelectedTableTruncation();
      sidebarTableContextMenu.show = false
    "
    @drop-selected="
      stageSelectedTableDeletion();
      sidebarTableContextMenu.show = false
    "
  />
  <DatabaseContextMenu
    v-if="sidebarDatabaseContextMenu.show"
    :show="sidebarDatabaseContextMenu.show"
    :x="sidebarDatabaseContextMenu.x"
    :y="sidebarDatabaseContextMenu.y"
    :database-name="sidebarDatabaseContextMenu.databaseName"
    @open-query="openQueryTab(sidebarDatabaseContextMenu.connectionId, sidebarDatabaseContextMenu.databaseName)"
    @open-in-split="openDatabaseInSplit(sidebarDatabaseContextMenu.connectionId, sidebarDatabaseContextMenu.databaseName)"
    @refresh-schema="refreshDatabaseSchema(sidebarDatabaseContextMenu.connectionId, sidebarDatabaseContextMenu.databaseName)"
    @import-sql="openImportSelector(sidebarDatabaseContextMenu.connectionId, sidebarDatabaseContextMenu.databaseName)"
    @export-database="openExportSelector(sidebarDatabaseContextMenu.connectionId, sidebarDatabaseContextMenu.databaseName)"
    @close-database="handleCloseDatabase(sidebarDatabaseContextMenu.connectionId, sidebarDatabaseContextMenu.databaseName)"
    @delete-tables="openDeleteTablesDialog(sidebarDatabaseContextMenu.connectionId, sidebarDatabaseContextMenu.databaseName)"
    @drop="confirmSidebarDatabaseAction(sidebarDatabaseContextMenu.connectionId, sidebarDatabaseContextMenu.databaseName)"
  />
  <DeleteTablesDialog
    v-if="showDeleteTablesDialog"
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

  <RowContextMenu
    v-if="rowContextMenu.show"
    :show="rowContextMenu.show"
    :x="rowContextMenu.x"
    :y="rowContextMenu.y"
    :has-primary-key="!!rowContextMenu.pane && !!getPrimaryKey(rowContextMenu.pane)"
    :selected-count="rowContextMenu.pane ? (getPaneTab(rowContextMenu.pane)?.selectedRowPks?.length ?? 1) : 1"
    @delete="handleRowContextDelete"
    @duplicate="handleRowContextDuplicate"
  />

  <DeleteConfirmDialog
    v-if="showDeleteRowDialog"
    :open="showDeleteRowDialog"
    :title="deleteRowTarget && (getPaneTab(deleteRowTarget.pane)?.selectedRowPks?.length ?? 0) > 1
      ? `Delete ${getPaneTab(deleteRowTarget.pane)?.selectedRowPks?.length} rows`
      : 'Delete Row'"
    :description="deleteRowTarget && (getPaneTab(deleteRowTarget.pane)?.selectedRowPks?.length ?? 0) > 1
      ? `Mark ${getPaneTab(deleteRowTarget.pane)?.selectedRowPks?.length} rows for deletion? They will not be deleted until you apply changes.`
      : 'Mark this row for deletion? It will not be deleted until you apply changes.'"
    :show-fk-option="true"
    :disable-fk-checks="disableFkChecks"
    @update:open="(val) => { if (!val) showDeleteRowDialog = false }"
    @update:disable-fk-checks="disableFkChecks = $event"
    @confirm="confirmDeleteRow"
  />
</template>

<script setup lang="ts">
import { defineAsyncComponent } from "vue";
import { useConnectionStore } from "@/stores/connections";
import { useWorkspaceDialogsContext } from "@/composables/useWorkspaceDialogsContext";

const ConnectionContextMenu = defineAsyncComponent(() => import("@/components/ConnectionContextMenu.vue"));
const TableContextMenu = defineAsyncComponent(() => import("@/components/TableContextMenu.vue"));
const DatabaseContextMenu = defineAsyncComponent(() => import("@/components/DatabaseContextMenu.vue"));
const RowContextMenu = defineAsyncComponent(() => import("@/components/RowContextMenu.vue"));
const ConnectionDialog = defineAsyncComponent(() => import("@/components/dialogs/ConnectionDialog.vue"));
const DatabaseActionDialog = defineAsyncComponent(() => import("@/components/dialogs/DatabaseActionDialog.vue"));
const DeleteConfirmDialog = defineAsyncComponent(() => import("@/components/dialogs/DeleteConfirmDialog.vue"));
const DeleteTablesDialog = defineAsyncComponent(() => import("@/components/dialogs/DeleteTablesDialog.vue"));
const ExportDialog = defineAsyncComponent(() => import("@/components/dialogs/ExportDialog.vue"));
const ImportDialog = defineAsyncComponent(() => import("@/components/dialogs/ImportDialog.vue"));
const NewDatabaseDialog = defineAsyncComponent(() => import("@/components/dialogs/NewDatabaseDialog.vue"));

const store = useConnectionStore();

const {
  showTableSelector,
  exportContext,
  exportContextTables,
  isLoadingExportTables,
  selectedExportTables,
  currentExportMode,
  showImportDialog,
  importContext,
  showNewConnDialog,
  newConn,
  isSavingConn,
  showNewDb,
  newDbName,
  isCreatingDb,
  showDeleteConnDialog,
  databaseActionData,
  showDatabaseActionDialog,
  isExecutingDatabaseAction,
  sidebarContextMenu,
  sidebarTableContextMenu,
  sidebarDatabaseContextMenu,
  expandedConnections,
  selectedSidebarConnectionId,
  showDeleteTablesDialog,
  deleteTablesContext,
  deleteTablesDialogTables,
  isLoadingDeleteTables,
  isExecutingDeleteTables,
  deleteTablesError,
  rowContextMenu,
  showDeleteRowDialog,
  deleteRowTarget,
  disableFkChecks,
  getPrimaryKey,
  getPaneTab,
  startExport,
  confirmImportFromBrowse,
  confirmImportFromFilePath,
  saveNewConn,
  createDatabase,
  deleteConn,
  executeDatabaseAction,
  openEditConnDialog,
  handleDuplicateConnection,
  disconnectConn,
  confirmDeleteConn,
  stageSidebarTableAction,
  stageSelectedTableTruncation,
  stageSelectedTableDeletion,
  openQueryTab,
  openDatabaseInSplit,
  refreshDatabaseSchema,
  openImportSelector,
  openExportSelector,
  handleCloseDatabase,
  openDeleteTablesDialog,
  confirmSidebarDatabaseAction,
  executeDeleteTablesFromDialog,
  executeDropDatabaseFromDeleteDialog,
  handleRowContextDelete,
  handleRowContextDuplicate,
  confirmDeleteRow,
} = useWorkspaceDialogsContext();
</script>
