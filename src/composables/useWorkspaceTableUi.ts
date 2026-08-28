import { computed, ref, watch, type Ref } from "vue";
import { useConnectionStore } from "@/stores/connections";
import type { AnyTab, PaneState, TableTab } from "@/types/workspace";
import { pendingTabsForDatabase, summarizePendingTabs } from "@/lib/pendingChanges";

interface WorkspaceTableUiContext {
  panes: Ref<PaneState[]>;
  activePaneId: Ref<string>;
  selectedSidebarConnectionId: Ref<string | null>;
  insertingRowTabId: Ref<string | null>;
  getPane: (paneId?: string) => PaneState;
  getPaneTab: (pane: PaneState) => TableTab | null;
  sortPayload: (tab: TableTab) => { column: string; desc: boolean } | null;
}

export function useWorkspaceTableUi(ctx: WorkspaceTableUiContext) {
  const store = useConnectionStore();
  const rowDetailOnClick = ref(localStorage.getItem("tupledb.rowDetailOnClick") !== "false");

  watch(rowDetailOnClick, (enabled) => {
    localStorage.setItem("tupledb.rowDetailOnClick", enabled ? "true" : "false");
  });

  const activeLogTab = computed<AnyTab | null>(() => {
    const pane = ctx.getPane(ctx.activePaneId.value);
    return pane.tabs.find((tab) => tab.id === pane.activeTabId) ?? null;
  });

  const activeLogConnectionId = computed(() => activeLogTab.value?.connectionId ?? ctx.selectedSidebarConnectionId.value ?? null);
  const activeLogDatabase = computed(() => {
    if (activeLogTab.value) return activeLogTab.value.database;
    const id = activeLogConnectionId.value;
    return id ? store.openConnections[id]?.selectedDatabase ?? null : null;
  });
  const activeLogConnectionName = computed(() => {
    const id = activeLogConnectionId.value;
    return id ? store.openConnections[id]?.connection.name ?? null : null;
  });

  const activePane = computed(() => ctx.getPane(ctx.activePaneId.value));
  const activePaneTab = computed(() => ctx.getPaneTab(activePane.value));
  const activePanePagination = computed(() => ({
    viewMode: activePane.value?.viewMode ?? null,
    page: activePane.value?.page ?? 0,
    pageSize: activePane.value?.pageSize ?? 300,
    rowCount: activePaneTab.value?.queryResult?.rows?.length ?? 0,
    totalCount: activePaneTab.value?.queryResult?.total_count ?? 0,
    totalCountApproximate: activePaneTab.value?.queryResult?.total_count_is_estimate ?? false,
    exactCountLoading: activePaneTab.value?.exactCountLoading ?? false,
    isInsertingRow: ctx.insertingRowTabId.value !== null && ctx.insertingRowTabId.value === activePane.value?.activeTabId,
  }));

  function pendingTableAction(tableName: string, database: string, connectionId: string): "drop" | "truncate" | null {
    for (const pane of ctx.panes.value) {
      const tab = pane.tabs.find((t): t is TableTab =>
        t.type === "table" &&
        t.connectionId === connectionId &&
        t.database === database &&
        t.tableName === tableName,
      );
      if (tab?.pendingDrop) return "drop";
      if (tab?.pendingTruncate) return "truncate";
    }
    return null;
  }

  function tableGridColumns(pane: PaneState) {
    const tab = ctx.getPaneTab(pane);
    const resultColumns = tab?.queryResult?.columns ?? [];
    if (resultColumns.length > 0) return resultColumns;
    return (tab?.tableStructure ?? []).map((col: any) => ({
      name: col.field,
      type_name: col.type,
    }));
  }

  function toggleRowDetailOnClick() {
    rowDetailOnClick.value = !rowDetailOnClick.value;
  }

  function pendingSummaryForPane(pane: PaneState) {
    const tab = ctx.getPaneTab(pane);
    return summarizePendingTabs(tab ? pendingTabsForDatabase(ctx.panes.value, tab) : []);
  }

  async function applyFilters(pane: PaneState, filters: any) {
    const t = ctx.getPaneTab(pane);
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
      ctx.sortPayload(t),
    );
  }

  async function clearFilters(pane: PaneState) {
    const t = ctx.getPaneTab(pane);
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
      ctx.sortPayload(t),
    );
  }

  function getSchema(connectionId: string, database: string | null): Record<string, string[]> {
    if (!database) return {};
    const tables = store.openConnections[connectionId]?.tables[database] ?? [];
    const schema: Record<string, string[]> = {};
    for (const t of tables) {
      schema[t.name] = [];
    }
    for (const pane of ctx.panes.value) {
      for (const tab of pane.tabs) {
        if (tab.type === "table" && tab.connectionId === connectionId && tab.database === database) {
          if (tab.tableStructure?.length) {
            schema[tab.tableName] = tab.tableStructure.map((c: any) => c.field);
          }
        }
      }
    }
    return schema;
  }

  return {
    rowDetailOnClick,
    activeLogConnectionId,
    activeLogDatabase,
    activeLogConnectionName,
    activePane,
    activePanePagination,
    pendingTableAction,
    tableGridColumns,
    toggleRowDetailOnClick,
    pendingSummaryForPane,
    applyFilters,
    clearFilters,
    getSchema,
  };
}
