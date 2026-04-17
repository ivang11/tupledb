import { ref, computed, onMounted, type Ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { save, open } from "@tauri-apps/plugin-dialog";
import { v4 as uuidv4 } from "uuid";
import { useConnectionStore } from "@/stores/connections";
import { useProgressStore } from "@/stores/progress";
import { useToast } from "@/composables/useToast";
import type { Connection } from "@/types/connection";
import type { PaneState, TableTab } from "@/types/workspace";

interface SidebarContext {
  panes: Ref<PaneState[]>;
  activePaneId: Ref<string>;
  getPane: (paneId?: string) => PaneState;
  getPaneTab: (pane: PaneState) => TableTab | null;
  switchToTab: (tabId: string, paneId?: string) => void;
  closeTab: (tabId: string, paneId?: string, event?: MouseEvent) => void;
  refreshActiveTab: (paneId?: string) => Promise<void>;
  loadTableData: (
    tableName: string,
    connectionId: string,
    database: string,
    filter?: any,
    paneId?: string,
  ) => Promise<void>;
  openQueryTab: (
    connectionId: string,
    database: string | null,
    paneId?: string,
  ) => void;
}

export function useSidebarManager(ctx: SidebarContext) {
  const store = useConnectionStore();
  const progressStore = useProgressStore();
  const {
    panes,
    getPane,
    getPaneTab,
    switchToTab,
    closeTab,
    refreshActiveTab,
    loadTableData,
  } = ctx;

  // ── Sidebar UI state ────────────────────────────────────────────────────────

  const search = ref("");
  const expandedConnections = ref<Set<string>>(new Set());
  const expandedDatabases = ref<Set<string>>(new Set());
  const selectedSidebarConnectionId = ref<string | null>(null);
  const showNewDb = ref<string | null>(null);
  const newDbName = ref("");
  const isCreatingDb = ref(false);
  const connectingId = ref<string | null>(null);

  function dbKey(connectionId: string, db: string) {
    return `${connectionId}:${db}`;
  }

  onMounted(async () => {
    await store.fetchConnections();
    for (const id of Object.keys(store.openConnections))
      expandedConnections.value.add(id);
    if (!selectedSidebarConnectionId.value) {
      selectedSidebarConnectionId.value = Object.keys(store.openConnections)[0] ?? null;
    }
  });

  async function connectSaved(conn: any) {
    connectingId.value = conn.id;
    try {
      await store.connect(conn);
      expandedConnections.value.add(conn.id);
      selectedSidebarConnectionId.value = conn.id;
    } catch (e: any) {
      toastError('Failed to connect', String(e));
    } finally {
      connectingId.value = null;
    }
  }

  function toggleConnection(connectionId: string) {
    if (expandedConnections.value.has(connectionId))
      expandedConnections.value.delete(connectionId);
    else expandedConnections.value.add(connectionId);
  }

  async function toggleDatabase(connectionId: string, db: string) {
    const key = dbKey(connectionId, db);
    if (expandedDatabases.value.has(key)) {
      expandedDatabases.value.delete(key);
    } else {
      expandedDatabases.value.add(key);
      const connState = store.openConnections[connectionId];
      if (connState && !connState.tables[db])
        await store.fetchTablesForConnection(connectionId, db);
    }
  }

  function disconnectConn(id: string) {
    for (const pane of panes.value) {
      pane.tabs = pane.tabs.filter((t) => t.connectionId !== id);
      if (
        pane.activeTabId &&
        !pane.tabs.find((t) => t.id === pane.activeTabId)
      ) {
        if (pane.tabs.length > 0) {
          switchToTab(pane.tabs[0].id, pane.id);
        } else {
          pane.activeTabId = null;
          // tab closed, nothing to sync
        }
      }
    }
    store.disconnectConnection(id);
  }

  function filteredTables(connectionId: string, db: string) {
    const tbls = store.openConnections[connectionId]?.tables[db] ?? [];
    if (!search.value) return tbls;
    return tbls.filter((t: any) =>
      t.name.toLowerCase().includes(search.value.toLowerCase()),
    );
  }

  const closedConnections = computed(() =>
    store.connections.filter((c) => !store.openConnections[c.id]),
  );

  async function createDatabase(connectionId: string) {
    if (!newDbName.value.trim()) return;
    isCreatingDb.value = true;
    try {
      await invoke("create_database", {
        connectionId,
        name: newDbName.value.trim(),
      });
      await store.fetchDatabasesForConnection(connectionId);
      newDbName.value = "";
      showNewDb.value = null;
    } finally {
      isCreatingDb.value = false;
    }
  }

  // ── Import / Export ─────────────────────────────────────────────────────────

  async function importSql(connectionId: string, database: string) {
    const path = await open({
      filters: [{ name: "SQL", extensions: ["sql"] }],
      multiple: false,
    });
    if (!path) return;
    progressStore.isImporting = true;
    progressStore.importExpanded = true;
    progressStore.importProgress = { current: 0, total: 0, status: "Reading file..." };
    let unlisten: UnlistenFn | null = null;
    try {
      unlisten = await listen<{
        current: number;
        total: number;
        status: string;
      }>("import-progress", (event) => {
        progressStore.importProgress = event.payload;
      });
      const result = await invoke<{ executed: number; errors: string[] }>(
        "import_sql",
        { connectionId, database, path },
      );
      await store.fetchTablesForConnection(connectionId, database);
      const tab = getPaneTab(getPane());
      if (tab && tab.connectionId === connectionId && tab.database === database)
        await loadTableData(tab.tableName, connectionId, database);
      if (result.errors.length > 0) {
        toastError(
          `Import finished with ${result.errors.length} error${result.errors.length !== 1 ? 's' : ''}`,
          `${result.executed.toLocaleString()} statements executed. Last error: ${result.errors[result.errors.length - 1]}`,
        );
      } else {
        toastSuccess(
          'Import complete',
          `${result.executed.toLocaleString()} statements executed successfully.`,
        );
      }
    } catch (e: any) {
      toastError('Import failed', String(e));
    } finally {
      progressStore.isImporting = false;
      if (unlisten) unlisten();
    }
  }

  const showTableSelector = ref(false);
  const isLoadingExportTables = ref(false);
  const selectedExportTables = ref<string[]>([]);
  const currentExportMode = ref("full");
  const exportContext = ref<{ connectionId: string; database: string } | null>(
    null,
  );
  const { success: toastSuccess, error: toastError } = useToast();

  const exportContextTables = computed(() => {
    if (!exportContext.value) return [];
    return (
      store.openConnections[exportContext.value.connectionId]?.tables[
        exportContext.value.database
      ] ?? []
    );
  });

  async function openExportSelector(connectionId: string, database: string) {
    exportContext.value = { connectionId, database };
    currentExportMode.value = "full";
    selectedExportTables.value = [];
    showTableSelector.value = true;

    // Fetch tables if not yet loaded (e.g. database was never opened in the UI)
    if (!store.openConnections[connectionId]?.tables[database]?.length) {
      isLoadingExportTables.value = true;
      try {
        await store.fetchTablesForConnection(connectionId, database);
      } finally {
        isLoadingExportTables.value = false;
      }
    }

    selectedExportTables.value = (
      store.openConnections[connectionId]?.tables[database] ?? []
    ).map((t: any) => t.name);
  }

  async function startExport() {
    if (!exportContext.value) return;
    showTableSelector.value = false;
    if (selectedExportTables.value.length === 0) return;
    const { connectionId, database } = exportContext.value;
    const path = await save({
      defaultPath: `${database}_${currentExportMode.value}.sql`,
      filters: [{ name: "SQL", extensions: ["sql"] }],
    });
    if (!path) return;
    progressStore.isExporting = true;
    progressStore.exportExpanded = true;
    progressStore.exportProgress = {
      current: 0,
      total: 0,
      status: "Initializing export...",
    };
    let unlisten: UnlistenFn | null = null;
    try {
      unlisten = await listen<{
        current: number;
        total: number;
        status: string;
      }>("export-progress", (event) => {
        progressStore.exportProgress = event.payload;
      });
      const rows = await invoke<number>("export_database", {
        connectionId,
        database,
        mode: currentExportMode.value,
        path,
        tables: selectedExportTables.value,
      });
      toastSuccess(
        'Export complete',
        `${rows.toLocaleString()} rows from ${selectedExportTables.value.length} table${selectedExportTables.value.length !== 1 ? 's' : ''} exported.`,
      );
    } catch (e: any) {
      toastError('Export failed', String(e));
    } finally {
      progressStore.isExporting = false;
      if (unlisten) unlisten();
    }
  }

  // ── Table actions ───────────────────────────────────────────────────────────

  const showTableActionDialog = ref(false);
  const tableActionData = ref<{
    type: "truncate" | "drop";
    connectionId: string;
    database: string;
    tableName: string;
  } | null>(null);
  const isExecutingTableAction = ref(false);

  function confirmSidebarTableAction(
    type: "truncate" | "drop",
    connectionId: string,
    database: string,
    tableName: string,
  ) {
    tableActionData.value = { type, connectionId, database, tableName };
    showTableActionDialog.value = true;
  }

  async function executeTableAction(disableFk: boolean) {
    if (!tableActionData.value) return;
    const { type, connectionId, database, tableName } = tableActionData.value;
    isExecutingTableAction.value = true;
    try {
      if (type === "drop") {
        await invoke("drop_table", {
          connectionId,
          database,
          table: tableName,
          disableFkChecks: disableFk,
        });
        for (const pane of panes.value) {
          const related = pane.tabs.filter(
            (t) =>
              t.type === "table" &&
              (t as TableTab).tableName === tableName &&
              (t as TableTab).database === database &&
              t.connectionId === connectionId,
          );
          related.forEach((t) => closeTab(t.id, pane.id));
        }
        await store.fetchTablesForConnection(connectionId, database);
      } else {
        await invoke("truncate_table", {
          connectionId,
          database,
          table: tableName,
          disableFkChecks: disableFk,
        });
        for (const pane of panes.value) {
          const tab = pane.tabs.find(
            (t) =>
              t.type === "table" &&
              (t as TableTab).tableName === tableName &&
              (t as TableTab).database === database &&
              t.connectionId === connectionId,
          ) as TableTab | undefined;
          if (tab) {
            tab.pendingTruncate = false;
            tab.pendingChanges = {};
            tab.pendingDeletions = {};
            if (tab.id === pane.activeTabId) await refreshActiveTab(pane.id);
          }
        }
      }
      showTableActionDialog.value = false;
      tableActionData.value = null;
    } catch (e: any) {
      toastError(`Failed to ${type} table`, String(e));
    } finally {
      isExecutingTableAction.value = false;
    }
  }

  // ── Multiple table selection ────────────────────────────────────────────────

  const selectedTables = ref<Set<string>>(new Set());
  const showBulkTableActionDialog = ref(false);
  const isExecutingBulkTableAction = ref(false);

  function tableSelectionKey(
    connectionId: string,
    database: string,
    tableName: string,
  ): string {
    return `${connectionId}:${database}:${tableName}`;
  }

  function isTableSelected(
    connectionId: string,
    database: string,
    tableName: string,
  ): boolean {
    return selectedTables.value.has(
      tableSelectionKey(connectionId, database, tableName),
    );
  }

  function toggleTableSelection(
    connectionId: string,
    database: string,
    tableName: string,
  ) {
    const key = tableSelectionKey(connectionId, database, tableName);

    // If there's an existing selection from a different db/connection, clear it first
    const firstKey = [...selectedTables.value][0];
    if (firstKey) {
      const [existingConn, existingDb] = firstKey.split(':');
      if (existingConn !== connectionId || existingDb !== database) {
        selectedTables.value.clear();
      }
    }

    if (selectedTables.value.has(key)) {
      selectedTables.value.delete(key);
    } else {
      selectedTables.value.add(key);
    }
  }

  function clearTableSelection() {
    selectedTables.value.clear();
  }

  function selectTableRange(
    connectionId: string,
    database: string,
    tableNames: string[],
  ) {
    for (const name of tableNames) {
      selectedTables.value.add(tableSelectionKey(connectionId, database, name));
    }
  }

  async function executeBulkTableDeletion(disableFk: boolean) {
    if (selectedTables.value.size === 0) return;

    isExecutingBulkTableAction.value = true;
    try {
      const toDelete: Array<{
        connectionId: string;
        database: string;
        table: string;
      }> = [];

      for (const key of selectedTables.value) {
        const [connectionId, database, tableName] = key.split(":");
        toDelete.push({ connectionId, database, table: tableName });
      }

      // Execute all deletes
      for (const item of toDelete) {
        await invoke("drop_table", {
          connectionId: item.connectionId,
          database: item.database,
          table: item.table,
          disableFkChecks: disableFk,
        });

        // Close related tabs
        for (const pane of panes.value) {
          const related = pane.tabs.filter(
            (t) =>
              t.type === "table" &&
              (t as TableTab).tableName === item.table &&
              (t as TableTab).database === item.database &&
              t.connectionId === item.connectionId,
          );
          related.forEach((t) => closeTab(t.id, pane.id));
        }
      }

      // Refresh the databases that were affected
      const affectedDbs = new Set<string>();
      for (const key of selectedTables.value) {
        const [connectionId, database] = key.split(":");
        affectedDbs.add(`${connectionId}:${database}`);
      }

      for (const dbKey of affectedDbs) {
        const [connectionId, database] = dbKey.split(":");
        await store.fetchTablesForConnection(connectionId, database);
      }

      selectedTables.value.clear();
      showBulkTableActionDialog.value = false;
      // Note: showBulkDeleteDialog is handled in HomeView.vue
    } catch (e: any) {
      toastError('Failed to delete tables', String(e));
    } finally {
      isExecutingBulkTableAction.value = false;
    }
  }

  async function executeBulkTableTruncation(disableFk: boolean) {
    if (selectedTables.value.size === 0) return;

    isExecutingBulkTableAction.value = true;
    try {
      const toTruncate: Array<{
        connectionId: string;
        database: string;
        table: string;
      }> = [];

      for (const key of selectedTables.value) {
        const [connectionId, database, tableName] = key.split(":");
        toTruncate.push({ connectionId, database, table: tableName });
      }

      // Execute all truncates
      for (const item of toTruncate) {
        await invoke("truncate_table", {
          connectionId: item.connectionId,
          database: item.database,
          table: item.table,
          disableFkChecks: disableFk,
        });

        // Reset pending changes for truncated tables
        for (const pane of panes.value) {
          const tab = pane.tabs.find(
            (t) =>
              t.type === "table" &&
              (t as TableTab).tableName === item.table &&
              (t as TableTab).database === item.database &&
              t.connectionId === item.connectionId,
          ) as TableTab | undefined;
          if (tab) {
            tab.pendingTruncate = false;
            tab.pendingChanges = {};
            tab.pendingDeletions = {};
            if (tab.id === pane.activeTabId) await refreshActiveTab(pane.id);
          }
        }
      }

      selectedTables.value.clear();
    } catch (e: any) {
      toastError('Failed to truncate tables', String(e));
    } finally {
      isExecutingBulkTableAction.value = false;
    }
  }

  // ── Database actions ────────────────────────────────────────────────────────

  const showDatabaseActionDialog = ref(false);
  const databaseActionData = ref<{
    connectionId: string;
    databaseName: string;
  } | null>(null);
  const isExecutingDatabaseAction = ref(false);

  function confirmSidebarDatabaseAction(
    connectionId: string,
    databaseName: string,
  ) {
    databaseActionData.value = { connectionId, databaseName };
    showDatabaseActionDialog.value = true;
  }

  async function executeDatabaseAction() {
    if (!databaseActionData.value) return;
    const { connectionId, databaseName } = databaseActionData.value;
    isExecutingDatabaseAction.value = true;
    try {
      await invoke("drop_database", {
        connectionId,
        name: databaseName,
      });
      // Close all tabs related to this database
      for (const pane of panes.value) {
        const related = pane.tabs.filter(
          (t) =>
            t.connectionId === connectionId &&
            (t as TableTab).database === databaseName,
        );
        related.forEach((t) => closeTab(t.id, pane.id));
      }
      // Refresh the database list
      await store.fetchDatabasesForConnection(connectionId);
      showDatabaseActionDialog.value = false;
      databaseActionData.value = null;
    } catch (e: any) {
      toastError('Failed to drop database', String(e));
    } finally {
      isExecutingDatabaseAction.value = false;
    }
  }

  // ── Delete tables dialog ────────────────────────────────────────────────────

  const showDeleteTablesDialog = ref(false);
  const isLoadingDeleteTables = ref(false);
  const isExecutingDeleteTables = ref(false);
  const deleteTablesError = ref<string | null>(null);
  const deleteTablesContext = ref<{
    connectionId: string;
    database: string;
  } | null>(null);

  const deleteTablesDialogTables = computed(() => {
    if (!deleteTablesContext.value) return [];
    return (
      store.openConnections[deleteTablesContext.value.connectionId]?.tables[
        deleteTablesContext.value.database
      ] ?? []
    );
  });

  async function openDeleteTablesDialog(
    connectionId: string,
    database: string,
  ) {
    deleteTablesContext.value = { connectionId, database };
    deleteTablesError.value = null;
    showDeleteTablesDialog.value = true;

    if (!store.openConnections[connectionId]?.tables[database]?.length) {
      isLoadingDeleteTables.value = true;
      try {
        await store.fetchTablesForConnection(connectionId, database);
      } finally {
        isLoadingDeleteTables.value = false;
      }
    }
  }

  async function executeDeleteTablesFromDialog(
    tableNames: string[],
    disableFk: boolean,
  ) {
    if (!deleteTablesContext.value || tableNames.length === 0) return;
    const { connectionId, database } = deleteTablesContext.value;
    deleteTablesError.value = null;
    isExecutingDeleteTables.value = true;
    try {
      for (const table of tableNames) {
        await invoke("drop_table", {
          connectionId,
          database,
          table,
          disableFkChecks: disableFk,
        });
        for (const pane of panes.value) {
          const related = pane.tabs.filter(
            (t) =>
              t.type === "table" &&
              (t as TableTab).tableName === table &&
              (t as TableTab).database === database &&
              t.connectionId === connectionId,
          );
          related.forEach((t) => closeTab(t.id, pane.id));
        }
      }
      await store.fetchTablesForConnection(connectionId, database);
      showDeleteTablesDialog.value = false;
      deleteTablesContext.value = null;
    } catch (e: any) {
      const msg = String(e);
      const isFkError = msg.includes("3730") || msg.toLowerCase().includes("foreign key constraint");
      if (isFkError && !disableFk) {
        deleteTablesError.value = `Cannot delete table: it is referenced by a foreign key.\nEnable "Disable Foreign Key Checks" and try again.`;
      } else {
        deleteTablesError.value = `Failed to delete tables: ${e}`;
      }
    } finally {
      isExecutingDeleteTables.value = false;
    }
  }

  async function executeDropDatabaseFromDeleteDialog() {
    if (!deleteTablesContext.value) return;
    const { connectionId, database } = deleteTablesContext.value;
    isExecutingDeleteTables.value = true;
    try {
      await invoke("drop_database", { connectionId, name: database });
      for (const pane of panes.value) {
        const related = pane.tabs.filter(
          (t) =>
            t.connectionId === connectionId &&
            (t as TableTab).database === database,
        );
        related.forEach((t) => closeTab(t.id, pane.id));
      }
      await store.fetchDatabasesForConnection(connectionId);
      showDeleteTablesDialog.value = false;
      deleteTablesContext.value = null;
    } catch (e: any) {
      toastError('Failed to drop database', String(e));
    } finally {
      isExecutingDeleteTables.value = false;
    }
  }

  // ── Context menus ───────────────────────────────────────────────────────────

  const sidebarContextMenu = ref({
    show: false,
    x: 0,
    y: 0,
    connection: null as Connection | null,
  });
  const sidebarTableContextMenu = ref({
    show: false,
    x: 0,
    y: 0,
    connectionId: "",
    database: "",
    tableName: "",
    selectedCount: 0,
  });
  const sidebarDatabaseContextMenu = ref({
    show: false,
    x: 0,
    y: 0,
    connectionId: "",
    databaseName: "",
  });

  function openSidebarContextMenu(e: MouseEvent, conn: Connection) {
    e.preventDefault();
    sidebarContextMenu.value = {
      show: true,
      x: e.clientX,
      y: e.clientY,
      connection: conn,
    };
    const close = () => {
      sidebarContextMenu.value.show = false;
      window.removeEventListener("click", close);
    };
    window.addEventListener("click", close);
  }

  function openSidebarTableContextMenu(
    e: MouseEvent,
    connectionId: string,
    database: string,
    tableName: string,
  ) {
    e.preventDefault();
    e.stopPropagation();
    sidebarTableContextMenu.value = {
      show: true,
      x: e.clientX,
      y: e.clientY,
      connectionId,
      database,
      tableName,
      selectedCount: selectedTables.value.size,
    };
    const close = () => {
      sidebarTableContextMenu.value.show = false;
      window.removeEventListener("click", close);
    };
    setTimeout(() => window.addEventListener("click", close), 0);
  }

  function openSidebarDatabaseContextMenu(
    e: MouseEvent,
    connectionId: string,
    databaseName: string,
  ) {
    e.preventDefault();
    e.stopPropagation();
    sidebarDatabaseContextMenu.value = {
      show: true,
      x: e.clientX,
      y: e.clientY,
      connectionId,
      databaseName,
    };
    const close = () => {
      sidebarDatabaseContextMenu.value.show = false;
      window.removeEventListener("click", close);
    };
    setTimeout(() => window.addEventListener("click", close), 0);
  }

  // ── Connection dialog ───────────────────────────────────────────────────────

  const showNewConnDialog = ref(false);
  const isSavingConn = ref(false);

  function blankConn(): Connection {
    return {
      id: uuidv4(),
      name: "",
      environment: "LOCAL",
      mysql: {
        host: "127.0.0.1",
        port: 3306,
        user: "root",
        password: "",
        database: "",
      },
    };
  }

  const newConn = ref<Connection>(blankConn());

  function openNewConnDialog() {
    newConn.value = blankConn();
    showNewConnDialog.value = true;
  }
  function openEditConnDialog(conn: Connection) {
    newConn.value = JSON.parse(JSON.stringify(conn));
    showNewConnDialog.value = true;
  }

  function handleDuplicateConnection(conn: Connection) {
    const dup = JSON.parse(JSON.stringify(conn));
    dup.id = uuidv4();
    dup.name = `${conn.name} (Copy)`;
    newConn.value = dup;
    showNewConnDialog.value = true;
    sidebarContextMenu.value.show = false;
  }

  async function saveNewConn(conn: Connection, andConnect: boolean) {
    if (!conn.name) return;
    isSavingConn.value = true;
    try {
      await store.addConnection(conn);
      if (andConnect) {
        await store.connect(conn);
        expandedConnections.value.add(conn.id);
      }
      showNewConnDialog.value = false;
    } catch (e: any) {
      toastError('Error', String(e));
    } finally {
      isSavingConn.value = false;
    }
  }

  const showDeleteConnDialog = ref(false);
  const connToDelete = ref<string | null>(null);

  function confirmDeleteConn(id: string) {
    connToDelete.value = id;
    showDeleteConnDialog.value = true;
    sidebarContextMenu.value.show = false;
  }

  async function deleteConn() {
    if (!connToDelete.value) return;
    const id = connToDelete.value;
    for (const pane of panes.value) {
      pane.tabs = pane.tabs.filter((t) => t.connectionId !== id);
      if (pane.activeTabId && !pane.tabs.find((t) => t.id === pane.activeTabId))
        pane.activeTabId = pane.tabs[0]?.id ?? null;
    }
    await store.removeConnection(id);
    showDeleteConnDialog.value = false;
    connToDelete.value = null;
  }

  return {
    // Sidebar state
    search,
    expandedConnections,
    expandedDatabases,
    selectedSidebarConnectionId,
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
    // Import/Export
    showTableSelector,
    isLoadingExportTables,
    selectedExportTables,
    currentExportMode,
    exportContext,
    exportContextTables,
    importSql,
    openExportSelector,
    startExport,
    // Table actions
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
    // Connection dialogs
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
  };
}
