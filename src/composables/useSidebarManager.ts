import { ref, computed, onMounted, type Ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { save, open } from "@tauri-apps/plugin-dialog";
import { v4 as uuidv4 } from "uuid";
import { useConnectionStore } from "@/stores/connections";
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
  });

  async function connectSaved(conn: any) {
    connectingId.value = conn.id;
    try {
      await store.connect(conn);
      expandedConnections.value.add(conn.id);
    } catch (e: any) {
      alert(`Failed to connect: ${e}`);
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

  const isImporting = ref(false);
  const importResult = ref<{ executed: number; errors: string[] } | null>(null);
  const importProgress = ref({ current: 0, total: 0, status: "" });

  async function importSql(connectionId: string, database: string) {
    const path = await open({
      filters: [{ name: "SQL", extensions: ["sql"] }],
      multiple: false,
    });
    if (!path) return;
    isImporting.value = true;
    importResult.value = null;
    importProgress.value = { current: 0, total: 0, status: "Reading file..." };
    let unlisten: UnlistenFn | null = null;
    try {
      unlisten = await listen<{
        current: number;
        total: number;
        status: string;
      }>("import-progress", (event) => {
        importProgress.value = event.payload;
      });
      const result = await invoke<{ executed: number; errors: string[] }>(
        "import_sql",
        { connectionId, database, path },
      );
      importResult.value = result;
      await store.fetchTablesForConnection(connectionId, database);
      const tab = getPaneTab(getPane());
      if (tab && tab.connectionId === connectionId && tab.database === database)
        await loadTableData(tab.tableName, connectionId, database);
    } catch (e: any) {
      importResult.value = { executed: 0, errors: [String(e)] };
    } finally {
      isImporting.value = false;
      if (unlisten) unlisten();
    }
  }

  const showTableSelector = ref(false);
  const selectedExportTables = ref<string[]>([]);
  const currentExportMode = ref("full");
  const exportContext = ref<{ connectionId: string; database: string } | null>(
    null,
  );
  const isExportingDb = ref(false);
  const exportProgress = ref({ current: 0, total: 0, status: "" });
  const exportResult = ref<{ success: boolean; message: string } | null>(null);

  const exportContextTables = computed(() => {
    if (!exportContext.value) return [];
    return (
      store.openConnections[exportContext.value.connectionId]?.tables[
        exportContext.value.database
      ] ?? []
    );
  });

  function openExportSelector(connectionId: string, database: string) {
    exportContext.value = { connectionId, database };
    currentExportMode.value = "full";
    selectedExportTables.value = (
      store.openConnections[connectionId]?.tables[database] ?? []
    ).map((t: any) => t.name);
    showTableSelector.value = true;
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
    isExportingDb.value = true;
    exportResult.value = null;
    exportProgress.value = {
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
        exportProgress.value = event.payload;
      });
      const rows = await invoke<number>("export_database", {
        connectionId,
        database,
        mode: currentExportMode.value,
        path,
        tables: selectedExportTables.value,
      });
      exportResult.value = {
        success: true,
        message: `Database exported successfully. ${rows} rows from ${selectedExportTables.value.length} tables included.`,
      };
    } catch (e: any) {
      exportResult.value = { success: false, message: String(e) };
    } finally {
      isExportingDb.value = false;
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
      alert(`Failed to ${type} table: ${e}`);
    } finally {
      isExecutingTableAction.value = false;
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
      alert(`Failed to drop database: ${e}`);
    } finally {
      isExecutingDatabaseAction.value = false;
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
      alert(`Error: ${e}`);
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
    isImporting,
    importResult,
    importProgress,
    showTableSelector,
    selectedExportTables,
    currentExportMode,
    exportContext,
    isExportingDb,
    exportProgress,
    exportResult,
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
    // Database actions
    showDatabaseActionDialog,
    databaseActionData,
    isExecutingDatabaseAction,
    confirmSidebarDatabaseAction,
    executeDatabaseAction,
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
