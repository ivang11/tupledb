<template>
  <ExportDialog
    v-if="showTableSelector"
    :open="showTableSelector"
    :database="exportContext?.database ?? ''"
    :tables="exportContextTables"
    :supports-sql="!!exportContext && store.openConnections[exportContext.connectionId]?.capabilities.exportSql"
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
  <ConnectionEditorHost
    v-if="showNewConnDialog"
    :open="showNewConnDialog"
    :connection="newConn"
    :is-saving="isSavingConn"
    :show-connect-button="true"
    @update:open="(val) => { if (!val) showNewConnDialog = false }"
    :save-connection="saveNewConn"
  />
  <NewDatabaseDialog
    v-if="!!showNewDb"
    :open="!!showNewDb"
    :connection-name="showNewDb ? (store.openConnections[showNewDb]?.connection.name ?? '') : ''"
    :supports-collations="!!showNewDb && store.openConnections[showNewDb]?.capabilities.databaseCollations"
    :encoding-label="showNewDb && store.openConnections[showNewDb]?.connection.database.engine === 'postgresql' ? 'Encoding' : 'Character Set'"
    :name="newDbName"
    :character-set="newDbCharacterSet"
    :collation="newDbCollation"
    :options="newDbOptions"
    :is-loading-options="isLoadingNewDbOptions"
    :options-error="newDbOptionsError"
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
    @update:character-set="updateNewDbCharacterSet"
    @update:collation="newDbCollation = $event"
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
    @dismiss="sidebarContextMenu.show = false"
    v-if="sidebarContextMenu.show"
    :show="sidebarContextMenu.show"
    :x="sidebarContextMenu.x"
    :y="sidebarContextMenu.y"
    :connection="sidebarContextMenu.connection"
    :can-create-database="!!sidebarContextMenu.connection && store.openConnections[sidebarContextMenu.connection.id]?.capabilities.createDatabase"
    :is-connected="!!sidebarContextMenu.connection && !!store.openConnections[sidebarContextMenu.connection.id]"
    :variant="sidebarContextMenu.source"
    :open-connection-count="Object.keys(store.openConnections).length"
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
    @disconnect-others="
      (id) => {
        for (const other of Object.keys(store.openConnections)) if (other !== id) disconnectConn(other);
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
    @dismiss="sidebarTableContextMenu.show = false"
    v-if="sidebarTableContextMenu.show"
    :show="sidebarTableContextMenu.show"
    :x="sidebarTableContextMenu.x"
    :y="sidebarTableContextMenu.y"
    :table-name="sidebarContextTableLabel"
    @open="loadTableData(sidebarTableContextMenu.tableName, sidebarTableContextMenu.connectionId, sidebarTableContextMenu.database)"
    @copy-name="copyText(sidebarContextTableLabel)"
    :selected-count="sidebarTableContextMenu.selectedCount"
    :can-write="store.openConnections[sidebarTableContextMenu.connectionId]?.connection.allow_writes !== false"
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
    @dismiss="sidebarDatabaseContextMenu.show = false"
    v-if="sidebarDatabaseContextMenu.show"
    :show="sidebarDatabaseContextMenu.show"
    :x="sidebarDatabaseContextMenu.x"
    :y="sidebarDatabaseContextMenu.y"
    :database-name="sidebarDatabaseContextMenu.databaseName"
    :can-import="store.openConnections[sidebarDatabaseContextMenu.connectionId]?.capabilities.importSql"
    :can-drop-database="store.openConnections[sidebarDatabaseContextMenu.connectionId]?.capabilities.createDatabase"
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
    :supports-fk-checks="!!deleteTablesContext && store.openConnections[deleteTablesContext.connectionId]?.capabilities.disableForeignKeyChecks"
    :can-drop-database="!!deleteTablesContext && store.openConnections[deleteTablesContext.connectionId]?.capabilities.createDatabase"
    :loading-tables="isLoadingDeleteTables"
    :is-executing="isExecutingDeleteTables"
    :error="deleteTablesError"
    @update:open="(val) => { if (!val) showDeleteTablesDialog = false }"
    @delete-tables="(tables, disableFk) => executeDeleteTablesFromDialog(tables, disableFk)"
    @drop-database="executeDropDatabaseFromDeleteDialog"
  />

  <RowContextMenu
    @dismiss="rowContextMenu.show = false"
    v-if="rowContextMenu.show"
    :show="rowContextMenu.show"
    :x="rowContextMenu.x"
    :y="rowContextMenu.y"
    :has-primary-key="!!rowContextMenu.pane && !!getPrimaryKey(rowContextMenu.pane)"
    :selected-count="rowContextMenu.pane ? (getPaneTab(rowContextMenu.pane)?.selectedRowPks?.length ?? 1) : 1"
    @delete="handleRowContextDelete"
    @duplicate="handleRowContextDuplicate"
    @copy-json="copyContextRow('json')"
    @copy-tsv="copyContextRow('tsv')"
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
    :show-fk-option="!!deleteRowTarget && !!store.openConnections[getPaneTab(deleteRowTarget.pane)?.connectionId ?? '']?.capabilities.disableForeignKeyChecks"
    :disable-fk-checks="disableFkChecks"
    @update:open="(val) => { if (!val) showDeleteRowDialog = false }"
    @update:disable-fk-checks="disableFkChecks = $event"
    @confirm="confirmDeleteRow"
  />
</template>

<script setup lang="ts">
import { computed, defineAsyncComponent } from "vue";
import { rowRecord } from "@/lib/rowAccess";
import { copyText, rowTsv } from "@/lib/clipboard";
import { tableLabel } from "@/lib/tableReference";
import { useConnectionStore } from "@/stores/connections";
import { useWorkspaceDialogsContext } from "@/composables/useWorkspaceDialogsContext";

const ConnectionContextMenu = defineAsyncComponent(() => import("@/components/ConnectionContextMenu.vue"));
const TableContextMenu = defineAsyncComponent(() => import("@/components/TableContextMenu.vue"));
const DatabaseContextMenu = defineAsyncComponent(() => import("@/components/DatabaseContextMenu.vue"));
const RowContextMenu = defineAsyncComponent(() => import("@/components/RowContextMenu.vue"));
const ConnectionEditorHost = defineAsyncComponent(() => import("@/components/connections/ConnectionEditorHost.vue"));
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
  newDbCharacterSet,
  newDbCollation,
  newDbOptions,
  isLoadingNewDbOptions,
  newDbOptionsError,
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
  updateNewDbCharacterSet,
  deleteConn,
  executeDatabaseAction,
  openEditConnDialog,
  handleDuplicateConnection,
  disconnectConn,
  confirmDeleteConn,
  stageSidebarTableAction,
  loadTableData,
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
const sidebarContextTableLabel = computed(() => {
  const menu = sidebarTableContextMenu.value;
  return tableLabel(store.tableReference(menu.connectionId, menu.database, menu.tableName));
});
function copyContextRow(format: 'json' | 'tsv') {
  const menu = rowContextMenu.value;
  if (!menu.pane || !menu.row) return;
  const columns = getPaneTab(menu.pane)?.queryResult?.columns ?? [];
  const record = rowRecord(menu.row, columns);
  void copyText(format === 'json' ? JSON.stringify(record, null, 2) : rowTsv(record));
}
</script>
