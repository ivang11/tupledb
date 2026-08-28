import { ref, computed, onMounted, watch, type Ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { downloadDir } from "@tauri-apps/api/path";
import { save, open } from "@tauri-apps/plugin-dialog";
import { v4 as uuidv4 } from "uuid";
import { useConnectionStore } from "@/stores/connections";
import { useProgressStore } from "@/stores/progress";
import { useQueryLogStore, type QueryLogEntry } from "@/stores/queryLog";
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

interface DatabaseCollation {
  name: string;
  characterSet: string;
  isDefault: boolean;
}

interface DatabaseCreationOptions {
  defaultCharacterSet: string;
  defaultCollation: string;
  collations: DatabaseCollation[];
}

const DATABASE_OPTION_DEFAULT = "__server_default__";

export function useSidebarManager(ctx: SidebarContext) {
  const store = useConnectionStore();
  const progressStore = useProgressStore();
  const queryLog = useQueryLogStore();
  type QueryLogPayload = Omit<QueryLogEntry, "id">;

  function recordQueryLogEntry(entry: QueryLogPayload) {
    const addEntry = (queryLog as { addEntry?: (entry: QueryLogPayload) => void }).addEntry;
    if (typeof addEntry === "function") {
      addEntry(entry);
      return;
    }

    const entries = queryLog.entries as QueryLogEntry[];
    const nextId = entries.reduce((max, item) => Math.max(max, item.id), 0) + 1;
    entries.push({ id: nextId, ...entry });
    if (entries.length > 500) {
      entries.splice(0, entries.length - 500);
    }
  }
  const {
    panes,
    activePaneId,
    getPane,
    getPaneTab,
    switchToTab,
    closeTab,
    loadTableData,
  } = ctx;

  // ── Sidebar UI state ────────────────────────────────────────────────────────

  const search = ref("");
  const expandedConnections = ref<Set<string>>(new Set());
  const expandedDatabases = ref<Set<string>>(new Set());
  const selectedSidebarConnectionId = ref<string | null>(null);
  const showNewDb = ref<string | null>(null);
  const newDbName = ref("");
  const newDbCharacterSet = ref(DATABASE_OPTION_DEFAULT);
  const newDbCollation = ref(DATABASE_OPTION_DEFAULT);
  const newDbOptions = ref<DatabaseCreationOptions | null>(null);
  const isLoadingNewDbOptions = ref(false);
  const newDbOptionsError = ref("");
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

  watch(showNewDb, async (connectionId) => {
    newDbOptions.value = null;
    newDbOptionsError.value = "";
    newDbCharacterSet.value = DATABASE_OPTION_DEFAULT;
    newDbCollation.value = DATABASE_OPTION_DEFAULT;
    if (!connectionId) return;

    isLoadingNewDbOptions.value = true;
    try {
      const options = await invoke<DatabaseCreationOptions>(
        "get_database_creation_options",
        { connectionId },
      );
      if (showNewDb.value !== connectionId) return;
      newDbOptions.value = options;
    } catch (e) {
      if (showNewDb.value === connectionId) {
        newDbOptionsError.value = String(e);
      }
    } finally {
      if (showNewDb.value === connectionId) isLoadingNewDbOptions.value = false;
    }
  });

  function updateNewDbCharacterSet(characterSet: string) {
    newDbCharacterSet.value = characterSet;
    if (characterSet === DATABASE_OPTION_DEFAULT) {
      newDbCollation.value = DATABASE_OPTION_DEFAULT;
      return;
    }
    const collations = newDbOptions.value?.collations.filter(
      (option) => option.characterSet === characterSet,
    ) ?? [];
    if (
      newDbCollation.value !== DATABASE_OPTION_DEFAULT &&
      !collations.some((option) => option.name === newDbCollation.value)
    ) {
      newDbCollation.value = DATABASE_OPTION_DEFAULT;
    }
  }

  async function connectSaved(conn: any) {
    const existing = store.openConnections[conn.id];
    if (existing) {
      existing.selectedDatabase =
        existing.selectedDatabase ??
        existing.openedDatabases?.[0] ??
        existing.databases[0] ??
        null;
      expandedConnections.value.add(conn.id);
      selectedSidebarConnectionId.value = conn.id;
      return true;
    }

    connectingId.value = conn.id;
    try {
      await store.connect(conn);
      expandedConnections.value.add(conn.id);
      selectedSidebarConnectionId.value = conn.id;
      return true;
    } catch (e: any) {
      toastError('Failed to connect', String(e));
      return false;
    } finally {
      connectingId.value = null;
    }
  }

  async function selectDatabase(connectionId: string, database: string) {
    try {
      await store.selectDatabase(connectionId, database);
      expandedDatabases.value.add(dbKey(connectionId, database));
      selectedSidebarConnectionId.value = connectionId;
      clearTableSelection();
    } catch (e: any) {
      toastError('Failed to select database', String(e));
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
    const databaseName = newDbName.value.trim();
    if (!databaseName) return;
    isCreatingDb.value = true;
    try {
      await invoke("create_database", {
        connectionId,
        name: databaseName,
        characterSet:
          newDbCharacterSet.value === DATABASE_OPTION_DEFAULT
            ? null
            : newDbCharacterSet.value,
        collation:
          newDbCollation.value === DATABASE_OPTION_DEFAULT
            ? null
            : newDbCollation.value,
      });
      await store.fetchDatabasesForConnection(connectionId);
      await store.selectDatabase(connectionId, databaseName);
      expandedDatabases.value.add(dbKey(connectionId, databaseName));
      toastSuccess("Database created", `Created \`${databaseName}\`.`);
      newDbName.value = "";
      showNewDb.value = null;
    } catch (e: any) {
      toastError("Failed to create database", String(e));
    } finally {
      isCreatingDb.value = false;
    }
  }

  async function refreshDatabaseSchema(connectionId: string, database: string) {
    try {
      await store.fetchDatabasesForConnection(connectionId);
      const tables = await store.fetchTablesForConnection(connectionId, database);
      const currentTableNames = new Set((tables ?? []).map((t: any) => t.name));

      for (const pane of panes.value) {
        const relatedTabs = pane.tabs.filter(
          (t): t is TableTab =>
            t.type === "table" &&
            t.connectionId === connectionId &&
            t.database === database,
        );

        for (const tab of relatedTabs) {
          if (!currentTableNames.has(tab.tableName)) {
            closeTab(tab.id, pane.id);
            continue;
          }

          const [tableStructure, tableIndexes, foreignKeys, ddl] = await Promise.all([
            store.fetchTableStructure(connectionId, database, tab.tableName).catch(() => []),
            store.fetchTableIndexes(connectionId, database, tab.tableName).catch(() => []),
            store.fetchForeignKeys(connectionId, database, tab.tableName).catch(() => []),
            store.fetchTableDdl(connectionId, database, tab.tableName).catch(() => null),
          ]);

          tab.tableStructure = tableStructure;
          tab.tableIndexes = tableIndexes;
          tab.foreignKeys = foreignKeys;
          tab.ddl = ddl;

          if (pane.activeTabId === tab.id) {
            tab.queryResult = await store.fetchTableData(
              connectionId,
              database,
              tab.tableName,
              tab.page,
              tab.pageSize,
              tab.filters ?? null,
              tab.sortColumn
                ? { column: tab.sortColumn, desc: tab.sortDesc }
                : null,
            );
          }
        }
      }

      toastSuccess(
        "Schema refreshed",
        `Updated tables and open tabs for \`${database}\`.`,
      );
    } catch (e: any) {
      toastError("Failed to refresh schema", String(e));
    } finally {
      sidebarDatabaseContextMenu.value.show = false;
    }
  }

  // ── Import / Export ─────────────────────────────────────────────────────────

  interface ExportOptions {
    dropIfExists: boolean;
    includeViews: boolean;
    useTransactions: boolean;
    compressGzip: boolean;
  }

  interface ExportStartPayload {
    format: string;
    options: ExportOptions;
  }

  type ImportMetrics = {
    parsed_statements: number;
    compacted_statements: number;
    executed_batches: number;
    sql_blocks: number;
    read_ms: number;
    process_ms: number;
    execute_ms: number;
    total_ms: number;
  };

  function formatImportDuration(ms: number) {
    if (ms < 1000) return `${ms}ms`;
    const seconds = ms / 1000;
    if (seconds < 60) return `${seconds.toFixed(1)}s`;
    const minutes = Math.floor(seconds / 60);
    const remainingSeconds = Math.round(seconds % 60);
    return `${minutes}m ${remainingSeconds}s`;
  }

  function formatImportSummary(metrics: ImportMetrics) {
    const parts = [
      `read ${formatImportDuration(metrics.read_ms)}`,
      `process ${formatImportDuration(metrics.process_ms)}`,
      `execute ${formatImportDuration(metrics.execute_ms)}`,
      `${metrics.executed_batches.toLocaleString()} batches`,
    ];
    if (metrics.compacted_statements > 0) {
      parts.push(
        `${metrics.compacted_statements.toLocaleString()} compacted into ${metrics.sql_blocks.toLocaleString()} SQL blocks`,
      );
    }
    return parts.join(" · ");
  }

  function nowTimestamp() {
    return new Date().toLocaleTimeString([], {
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
    });
  }

  async function runImportFromPath(
    connectionId: string,
    database: string,
    path: string,
  ) {
    const importId = crypto.randomUUID();
    progressStore.isImporting = true;
    progressStore.importConnectionId = connectionId;
    progressStore.importId = importId;
    progressStore.isCancellingImport = false;
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
      const result = await invoke<{
        executed: number;
        errors: string[];
        metrics: ImportMetrics;
      }>(
        "import_sql",
        {
          connectionId,
          database,
          path,
          importId,
        },
      );
      await store.fetchTablesForConnection(connectionId, database);
      const tab = getPaneTab(getPane());
      if (tab && tab.connectionId === connectionId && tab.database === database)
        await loadTableData(tab.tableName, connectionId, database);
      const toastSummary = `Completed in ${formatImportDuration(result.metrics.total_ms)}. ${result.executed.toLocaleString()} statements imported.`;
      const detailedSummary = `${toastSummary} ${formatImportSummary(result.metrics)}.`;
      recordQueryLogEntry({
        connection_id: connectionId,
        database,
        sql: `-- IMPORT SQL INTO \`${database}\`\n${detailedSummary}`,
        timestamp: nowTimestamp(),
        duration_ms: result.metrics.total_ms,
        error: result.errors.length > 0 ? result.errors[result.errors.length - 1] : null,
      });
      if (result.errors.length > 0) {
        toastError(
          `Import finished with ${result.errors.length} error${result.errors.length !== 1 ? 's' : ''}`,
          toastSummary,
        );
      } else {
        toastSuccess(
          'Import complete',
          toastSummary,
        );
      }
    } catch (e: any) {
      const msg = String(e);
      if (msg.includes('Import cancelled')) {
        recordQueryLogEntry({
          connection_id: connectionId,
          database,
          sql: `-- IMPORT SQL INTO \`${database}\`\nImport cancelled by user.`,
          timestamp: nowTimestamp(),
          duration_ms: 0,
          error: "Import cancelled",
        });
        useToast().show({
          type: 'info',
          title: 'Import cancelled',
          message: 'The SQL import was stopped by the user.',
        });
      } else {
        toastError('Import failed', msg);
      }
    } finally {
      progressStore.isImporting = false;
      progressStore.importConnectionId = null;
      progressStore.importId = null;
      progressStore.isCancellingImport = false;
      if (unlisten) unlisten();
    }
  }

  // ── Import modal flow ─────────────────────────────────────────────────────
  const showImportDialog = ref(false);
  const importContext = ref<{ connectionId: string; database: string } | null>(null);

  function openImportSelector(connectionId: string, database: string) {
    importContext.value = { connectionId, database };
    showImportDialog.value = true;
  }

  async function confirmImportFromBrowse() {
    if (!importContext.value) return;
    const { connectionId, database } = importContext.value;
    showImportDialog.value = false;
    const path = await open({
      filters: [{ name: "SQL", extensions: ["sql", "gz"] }],
      multiple: false,
    });
    if (!path) return;
    await runImportFromPath(connectionId, database, path as string);
  }

  async function confirmImportFromFilePath(filePath: string) {
    if (!importContext.value) return;
    const { connectionId, database } = importContext.value;
    showImportDialog.value = false;
    await runImportFromPath(connectionId, database, filePath);
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

  async function startExport(payload: ExportStartPayload) {
    if (!exportContext.value) return;
    showTableSelector.value = false;
    if (selectedExportTables.value.length === 0) return;
    const { connectionId, database } = exportContext.value;
    const selectedTables = payload.options.includeViews
      ? selectedExportTables.value
      : selectedExportTables.value.filter((name) => {
          const table = exportContextTables.value.find((t: any) => t.name === name);
          return !String(table?.table_type ?? "").toUpperCase().includes("VIEW");
        });
    if (selectedTables.length === 0) return;
    const today = new Date().toISOString().slice(0, 10);
    const baseExt = payload.format === "csv" ? "csv" : payload.format === "json" ? "json" : "sql";
    const ext = payload.options.compressGzip ? `${baseExt}.gz` : baseExt;
    const filterName = payload.options.compressGzip
      ? `${baseExt.toUpperCase()} (gzip)`
      : baseExt.toUpperCase();
    const filterExts = payload.options.compressGzip ? ["gz", ext] : [baseExt];
    const downloadsPath = await downloadDir().catch(() => null);
    const defaultPath = downloadsPath
      ? `${downloadsPath}/${database}-${today}.${ext}`
      : `${database}-${today}.${ext}`;
    const path = await save({
      defaultPath,
      filters: [{ name: filterName, extensions: filterExts }],
    });
    if (!path) return;
    progressStore.isExporting = true;
    progressStore.exportExpanded = true;
    progressStore.exportProgress = { current: 0, total: 0, status: "" };
    progressStore.exportTables = [...selectedTables];
    progressStore.exportDoneCount = 0;
    progressStore.exportStartTime = Date.now();
    progressStore.exportConnectionId = connectionId;
    progressStore.exportId = uuidv4();
    progressStore.isCancellingExport = false;
    let unlisten: UnlistenFn | null = null;
    try {
      let lastStatus = "";
      unlisten = await listen<{
        current: number;
        total: number;
        status: string;
      }>("export-progress", (event) => {
        progressStore.exportProgress = event.payload;
        const s = event.payload.status;
        if (s && s !== lastStatus) {
          if (lastStatus) progressStore.exportDoneCount++;
          lastStatus = s;
        }
      });
      const t0 = Date.now();
      const rows = await invoke<number>("export_database", {
        connectionId,
        database,
        mode: currentExportMode.value,
        path,
        tables: selectedTables,
        exportId: progressStore.exportId,
        format: payload.format,
        dropIfExists: payload.options.dropIfExists,
        includeViews: payload.options.includeViews,
        useTransactions: payload.options.useTransactions,
        compressGzip: payload.options.compressGzip,
      });
      const durationMs = Date.now() - t0;
      const tableCount = selectedTables.length;
      const summary = `${rows.toLocaleString()} rows · ${tableCount} table${tableCount !== 1 ? 's' : ''} · ${formatImportDuration(durationMs)}`;
      recordQueryLogEntry({
        connection_id: connectionId,
        database,
        sql: `-- EXPORT \`${database}\` → ${path.split(/[\\/]/).pop()}\n-- ${summary}`,
        timestamp: nowTimestamp(),
        duration_ms: durationMs,
        error: null,
      });
      toastSuccess(
        'Export complete',
        summary,
      );
    } catch (e: any) {
      recordQueryLogEntry({
        connection_id: connectionId,
        database,
        sql: `-- EXPORT \`${database}\` failed`,
        timestamp: nowTimestamp(),
        duration_ms: 0,
        error: String(e),
      });
      toastError('Export failed', String(e));
    } finally {
      progressStore.isExporting = false;
      progressStore.exportTables = [];
      progressStore.exportDoneCount = 0;
      progressStore.exportStartTime = null;
      progressStore.exportConnectionId = null;
      progressStore.exportId = null;
      progressStore.isCancellingExport = false;
      if (unlisten) unlisten();
    }
  }

  // ── Table actions ───────────────────────────────────────────────────────────

  async function stageTableAction(
    type: "truncate" | "drop",
    connectionId: string,
    database: string,
    tableName: string,
  ) {
    await loadTableData(tableName, connectionId, database);
    const pane = getPane(activePaneId.value);
    const tab = getPaneTab(pane);
    if (
      !tab ||
      tab.connectionId !== connectionId ||
      tab.database !== database ||
      tab.tableName !== tableName
    ) {
      return;
    }

    tab.pendingChanges = {};
    tab.pendingDeletions = {};
    tab.pendingInserts = [];
    tab.selectedRowPk = null;
    tab.selectedRowPks = [];
    tab.inlineEditColumn = null;
    tab.pendingTruncate = type === "truncate";
    tab.pendingDrop = type === "drop";
  }

  async function stageSidebarTableAction(
    type: "truncate" | "drop",
    connectionId: string,
    database: string,
    tableName: string,
  ) {
    try {
      await stageTableAction(type, connectionId, database, tableName);
    } catch (e: any) {
      toastError(`Failed to ${type} table`, String(e));
    }
  }

  // ── Multiple table selection ────────────────────────────────────────────────

  const selectedTables = ref<Set<string>>(new Set());
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
    tableName: string,
  ) {
    const tables = filteredTables(connectionId, database).map((table: any) => table.name);
    const firstKey = [...selectedTables.value][0];
    const firstTableName = firstKey?.split(":")[2] ?? tableName;
    const start = tables.indexOf(firstTableName);
    const end = tables.indexOf(tableName);
    const tableNames =
      start >= 0 && end >= 0
        ? tables.slice(Math.min(start, end), Math.max(start, end) + 1)
        : [tableName];

    const firstSelection = [...selectedTables.value][0];
    if (firstSelection) {
      const [existingConn, existingDb] = firstSelection.split(":");
      if (existingConn !== connectionId || existingDb !== database) {
        selectedTables.value.clear();
      }
    }

    for (const name of tableNames) {
      selectedTables.value.add(tableSelectionKey(connectionId, database, name));
    }
  }

  async function stageSelectedTableDeletion() {
    if (selectedTables.value.size === 0) return;

    try {
      for (const key of selectedTables.value) {
        const [connectionId, database, tableName] = key.split(":");
        await stageTableAction("drop", connectionId, database, tableName);
      }

      selectedTables.value.clear();
    } catch (e: any) {
      toastError('Failed to delete tables', String(e));
    }
  }

  async function stageSelectedTableTruncation() {
    if (selectedTables.value.size === 0) return;

    try {
      for (const key of selectedTables.value) {
        const [connectionId, database, tableName] = key.split(":");
        await stageTableAction("truncate", connectionId, database, tableName);
      }

      selectedTables.value.clear();
    } catch (e: any) {
      toastError('Failed to truncate tables', String(e));
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
      await invoke("drop_tables", {
        connectionId,
        database,
        tables: tableNames,
        disableFkChecks: disableFk,
      });

      for (const pane of panes.value) {
        const related = pane.tabs.filter((t) => {
          if (
            t.type !== "table" ||
            (t as TableTab).database !== database ||
            t.connectionId !== connectionId
          ) {
            return false;
          }

          return tableNames.includes((t as TableTab).tableName);
        });
        related.forEach((t) => closeTab(t.id, pane.id));
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

  function closeSidebarContextMenus() {
    sidebarContextMenu.value.show = false;
    sidebarTableContextMenu.value.show = false;
    sidebarDatabaseContextMenu.value.show = false;
  }

  function openSidebarContextMenu(e: MouseEvent, conn: Connection) {
    e.preventDefault();
    e.stopPropagation();
    closeSidebarContextMenus();
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
    closeSidebarContextMenus();
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
    closeSidebarContextMenus();
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
      allow_writes: true,
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

  async function exportConnections() {
    const path = await save({
      defaultPath: 'connections.json',
      filters: [{ name: 'JSON', extensions: ['json'] }],
    });
    if (!path) return;
    try {
      await store.exportConnections(path);
      toastSuccess('Exportado', `${store.connections.length} conexiones guardadas.`);
    } catch (e) {
      toastError('Error al exportar', String(e));
    }
  }

  async function importConnections() {
    const path = await open({
      filters: [{ name: 'JSON', extensions: ['json'] }],
      multiple: false,
    });
    if (!path) return;
    try {
      const count = await store.importConnections(path as string);
      toastSuccess('Importado', `${count} conexiones importadas correctamente.`);
    } catch (e) {
      toastError('Error al importar', String(e));
    }
  }

  return {
    // Sidebar state
    search,
    expandedConnections,
    expandedDatabases,
    selectedSidebarConnectionId,
    showNewDb,
    newDbName,
    newDbCharacterSet,
    newDbCollation,
    newDbOptions,
    isLoadingNewDbOptions,
    newDbOptionsError,
    updateNewDbCharacterSet,
    isCreatingDb,
    connectingId,
    closedConnections,
    filteredTables,
    connectSaved,
    selectDatabase,
    toggleConnection,
    toggleDatabase,
    disconnectConn,
    createDatabase,
    refreshDatabaseSchema,
    // Import/Export
    showTableSelector,
    isLoadingExportTables,
    selectedExportTables,
    currentExportMode,
    exportContext,
    exportContextTables,
    showImportDialog,
    importContext,
    openImportSelector,
    confirmImportFromBrowse,
    confirmImportFromFilePath,
    openExportSelector,
    startExport,
    // Table actions
    stageSidebarTableAction,
    // Multiple table selection
    selectedTables,
    isTableSelected,
    toggleTableSelection,
    selectTableRange,
    clearTableSelection,
    stageSelectedTableDeletion,
    stageSelectedTableTruncation,
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
    exportConnections,
    importConnections,
  };
}
