import { computed, nextTick, ref, watch, type Ref } from "vue";
import { useConnectionStore } from "@/stores/connections";
import type { AnyTab, PaneState, QueryTab, TableTab } from "@/types/workspace";

interface WorkspaceCoordinatorContext {
  panes: Ref<PaneState[]>;
  activePaneId: Ref<string>;
  getPane: (paneId?: string) => PaneState;
  getPaneTab: (pane: PaneState) => TableTab | null;
  addPane: () => string;
  switchToTab: (tabId: string, paneId?: string) => void;
  selectDatabase: (connectionId: string, database: string) => Promise<void>;
  disconnectConn: (connectionId: string) => void;
  selectedSidebarConnectionId: Ref<string | null>;
  sidebarDatabaseContextMenu: Ref<{ show: boolean } & Record<string, any>>;
  sidebarRef: Ref<{ scrollToTable: (tableName: string, database: string, connectionId: string) => void } | null>;
}

function workspaceKey(workspace: { connectionId: string | null; database: string | null }) {
  return workspace.connectionId && workspace.database
    ? `${workspace.connectionId}:${workspace.database}`
    : "";
}

export function useWorkspaceCoordinator(ctx: WorkspaceCoordinatorContext) {
  const store = useConnectionStore();
  const draggingTab = ref<{ paneId: string; tabId: string } | null>(null);
  const paneWorkspaces = ref<Record<string, { connectionId: string; database: string | null }>>({});
  const workspaceActiveTabs = ref<Record<string, string>>({});
  const selectingWorkspaceDatabase = ref(false);

  const isSinglePaneWorkspace = computed(() => ctx.panes.value.length === 1);

  function activeWorkspaceDatabase() {
    const connectionId = ctx.selectedSidebarConnectionId.value;
    if (!connectionId) return { connectionId: null, database: null };
    return {
      connectionId,
      database: store.openConnections[connectionId]?.selectedDatabase ?? null,
    };
  }

  function tabMatchesWorkspace(tab: AnyTab, workspace: { connectionId: string | null; database: string | null }) {
    const { connectionId, database } = workspace;
    if (!connectionId) return true;
    return tab.connectionId === connectionId && (tab.database ?? null) === database;
  }

  function paneWorkspace(pane: PaneState) {
    const saved = paneWorkspaces.value[pane.id];
    if (saved) return saved;

    if (isSinglePaneWorkspace.value) return activeWorkspaceDatabase();

    const activeTab = pane.tabs.find((tab) => tab.id === pane.activeTabId);
    if (activeTab) {
      return { connectionId: activeTab.connectionId, database: activeTab.database ?? null };
    }

    return { connectionId: null, database: null };
  }

  function paneWorkspaceLabel(pane: PaneState) {
    const workspace = paneWorkspace(pane);
    if (!workspace.connectionId || !workspace.database) return "";

    const connectionName = store.openConnections[workspace.connectionId]?.connection.name ?? "";
    const openConnectionCount = Object.keys(store.openConnections).length;
    return openConnectionCount > 1 && connectionName
      ? `${connectionName} / ${workspace.database}`
      : workspace.database;
  }

  const isComparingDatabases = computed(() => {
    const keys = new Set(
      ctx.panes.value
        .map((pane) => workspaceKey(paneWorkspace(pane)))
        .filter(Boolean),
    );
    return keys.size > 1;
  });

  function rememberWorkspaceActiveTab(tab: AnyTab) {
    const key = workspaceKey({ connectionId: tab.connectionId, database: tab.database ?? null });
    if (key) workspaceActiveTabs.value[key] = tab.id;
  }

  function setPaneWorkspaceFromTab(paneId: string, tab: AnyTab) {
    paneWorkspaces.value[paneId] = {
      connectionId: tab.connectionId,
      database: tab.database ?? null,
    };
    rememberWorkspaceActiveTab(tab);
  }

  function tabBelongsToPaneWorkspace(tab: AnyTab, pane: PaneState) {
    return tabMatchesWorkspace(tab, paneWorkspace(pane));
  }

  function visiblePaneTabs(pane: PaneState) {
    return pane.tabs.filter((tab) => tabBelongsToPaneWorkspace(tab, pane));
  }

  function getVisiblePaneTab(pane: PaneState) {
    const tab = pane.tabs.find((tab) => tab.id === pane.activeTabId);
    return tab && tabBelongsToPaneWorkspace(tab, pane) ? ctx.getPaneTab(pane) : null;
  }

  function isVisiblePaneActiveTabQuery(pane: PaneState) {
    const tab = pane.tabs.find((tab) => tab.id === pane.activeTabId);
    return !!tab && tabBelongsToPaneWorkspace(tab, pane) && tab.type === "query";
  }

  function getVisiblePaneActiveQueryTab(pane: PaneState): QueryTab | null {
    const tab = pane.tabs.find((tab) => tab.id === pane.activeTabId);
    return tab && tabBelongsToPaneWorkspace(tab, pane) && tab.type === "query"
      ? tab
      : null;
  }

  function syncSinglePaneToActiveWorkspace() {
    if (selectingWorkspaceDatabase.value) return;
    if (!isSinglePaneWorkspace.value) return;
    const pane = ctx.panes.value[0];
    const activeTab = pane.tabs.find((tab) => tab.id === pane.activeTabId);
    if (activeTab && tabBelongsToPaneWorkspace(activeTab, pane)) return;

    const nextTab = visiblePaneTabs(pane)[0] ?? null;
    if (nextTab) ctx.switchToTab(nextTab.id, pane.id);
    else pane.activeTabId = null;
  }

  function activateFirstAvailableTab(pane: PaneState) {
    const nextTab = pane.tabs[0] ?? null;
    if (nextTab) {
      ctx.switchToTab(nextTab.id, pane.id);
      setPaneWorkspaceFromTab(pane.id, nextTab);
    } else {
      pane.activeTabId = null;
    }
  }

  function handleCloseDatabase(connectionId: string, database: string) {
    const connState = store.openConnections[connectionId];
    if (!connState) return;

    const openedDatabases =
      connState.openedDatabases?.length
        ? connState.openedDatabases
        : connState.selectedDatabase
          ? [connState.selectedDatabase]
          : [];
    const remainingDatabases = openedDatabases.filter((db) => db !== database);

    if (remainingDatabases.length === 0) {
      ctx.disconnectConn(connectionId);
      ctx.sidebarDatabaseContextMenu.value.show = false;
      return;
    }

    store.closeDatabase(connectionId, database);
    const nextDatabase =
      store.openConnections[connectionId]?.selectedDatabase ??
      remainingDatabases[0] ??
      null;

    for (const pane of ctx.panes.value) {
      const activeTabWasClosed = pane.tabs.some(
        (tab) =>
          tab.id === pane.activeTabId &&
          tab.connectionId === connectionId &&
          (tab.database ?? null) === database,
      );

      pane.tabs = pane.tabs.filter(
        (tab) =>
          !(
            tab.connectionId === connectionId &&
            (tab.database ?? null) === database
          ),
      );

      const workspace = paneWorkspaces.value[pane.id];
      if (
        workspace?.connectionId === connectionId &&
        workspace.database === database
      ) {
        if (nextDatabase) {
          paneWorkspaces.value[pane.id] = { connectionId, database: nextDatabase };
        } else {
          delete paneWorkspaces.value[pane.id];
        }
      }

      if (activeTabWasClosed || !pane.tabs.some((tab) => tab.id === pane.activeTabId)) {
        const sameConnectionTab = nextDatabase
          ? pane.tabs.find(
              (tab) =>
                tab.connectionId === connectionId &&
                (tab.database ?? null) === nextDatabase,
            )
          : null;

        if (sameConnectionTab) {
          ctx.switchToTab(sameConnectionTab.id, pane.id);
          setPaneWorkspaceFromTab(pane.id, sameConnectionTab);
        } else {
          activateFirstAvailableTab(pane);
        }
      }
    }

    ctx.sidebarDatabaseContextMenu.value.show = false;
  }

  async function handleSelectDatabase(connectionId: string, database: string) {
    selectingWorkspaceDatabase.value = true;
    try {
      await ctx.selectDatabase(connectionId, database);
      ctx.selectedSidebarConnectionId.value = connectionId;
      const pane = ctx.getPane(ctx.activePaneId.value);
      paneWorkspaces.value[pane.id] = { connectionId, database };

      const key = workspaceKey({ connectionId, database });
      const rememberedTabId = workspaceActiveTabs.value[key];
      const nextTab =
        (rememberedTabId
          ? visiblePaneTabs(pane).find((tab) => tab.id === rememberedTabId)
          : null) ??
        visiblePaneTabs(pane)[0] ??
        null;
      if (nextTab) ctx.switchToTab(nextTab.id, pane.id);
      else pane.activeTabId = null;
    } finally {
      nextTick(() => {
        selectingWorkspaceDatabase.value = false;
      });
    }
  }

  function activatePane(paneId: string) {
    ctx.activePaneId.value = paneId;
    const pane = ctx.getPane(paneId);
    const workspace = paneWorkspace(pane);
    if (!workspace.connectionId || !store.openConnections[workspace.connectionId]) return;

    ctx.selectedSidebarConnectionId.value = workspace.connectionId;
    if (workspace.database) {
      store.openConnections[workspace.connectionId].selectedDatabase = workspace.database;
    }
  }

  function activatePaneTabAfterRemoval(
    pane: PaneState,
    removedIndex: number,
    workspace?: { connectionId: string | null; database: string | null },
  ) {
    const candidateTabs = workspace
      ? pane.tabs.filter((tab) => tabMatchesWorkspace(tab, workspace))
      : pane.tabs;

    if (candidateTabs.length === 0) {
      pane.activeTabId = null;
      return;
    }

    const nextTab = candidateTabs[Math.min(removedIndex, candidateTabs.length - 1)];
    ctx.switchToTab(nextTab.id, pane.id);
    setPaneWorkspaceFromTab(pane.id, nextTab);
  }

  function moveTabToPane(tabId: string, sourcePaneId: string, targetPaneId: string, targetTabId: string | null = null) {
    const sourcePane = ctx.panes.value.find((pane) => pane.id === sourcePaneId);
    const targetPane = ctx.panes.value.find((pane) => pane.id === targetPaneId);
    if (!sourcePane || !targetPane) return;

    const sourceIndex = sourcePane.tabs.findIndex((tab) => tab.id === tabId);
    if (sourceIndex === -1) return;
    const sourceWorkspace = paneWorkspace(sourcePane);

    let insertIndex = targetTabId
      ? targetPane.tabs.findIndex((tab) => tab.id === targetTabId)
      : targetPane.tabs.length;

    if (sourcePane.id === targetPane.id) {
      if (insertIndex === -1) insertIndex = targetPane.tabs.length;
      if (insertIndex === sourceIndex || insertIndex === sourceIndex + 1) return;
      const [tab] = sourcePane.tabs.splice(sourceIndex, 1);
      if (sourceIndex < insertIndex) insertIndex -= 1;
      targetPane.tabs.splice(insertIndex, 0, tab);
      ctx.switchToTab(tab.id, targetPane.id);
      setPaneWorkspaceFromTab(targetPane.id, tab);
      return;
    }

    const [tab] = sourcePane.tabs.splice(sourceIndex, 1);
    if (sourcePane.activeTabId === tab.id) {
      activatePaneTabAfterRemoval(sourcePane, sourceIndex, sourceWorkspace);
    }

    if (insertIndex === -1) insertIndex = targetPane.tabs.length;
    targetPane.tabs.splice(insertIndex, 0, tab);
    ctx.switchToTab(tab.id, targetPane.id);
    setPaneWorkspaceFromTab(targetPane.id, tab);
    ctx.activePaneId.value = targetPane.id;
  }

  async function openDatabaseInSplit(connectionId: string, database: string) {
    selectingWorkspaceDatabase.value = true;
    const sourcePaneId = ctx.activePaneId.value;
    const sourcePane = ctx.getPane(sourcePaneId);
    const sourceWorkspace = paneWorkspace(sourcePane);
    if (sourceWorkspace.connectionId) {
      paneWorkspaces.value[sourcePane.id] = {
        connectionId: sourceWorkspace.connectionId,
        database: sourceWorkspace.database,
      };
    }

    try {
      await ctx.selectDatabase(connectionId, database);

      const targetPaneId = ctx.addPane();
      const targetPane = ctx.getPane(targetPaneId);
      paneWorkspaces.value[targetPaneId] = { connectionId, database };

      const sourceAlreadyShowsDatabase =
        sourceWorkspace.connectionId === connectionId && sourceWorkspace.database === database;

      if (!sourceAlreadyShowsDatabase) {
        const matchingTabs = sourcePane.tabs.filter((tab) =>
          tabMatchesWorkspace(tab, { connectionId, database }),
        );

        for (const tab of matchingTabs) {
          moveTabToPane(tab.id, sourcePane.id, targetPane.id);
        }
      }

      const firstTab = visiblePaneTabs(targetPane)[0] ?? null;
      if (firstTab) ctx.switchToTab(firstTab.id, targetPane.id);
      else targetPane.activeTabId = null;

      ctx.activePaneId.value = targetPaneId;
      ctx.selectedSidebarConnectionId.value = connectionId;
      store.openConnections[connectionId].selectedDatabase = database;
    } finally {
      nextTick(() => {
        selectingWorkspaceDatabase.value = false;
      });
    }
  }

  function splitActivePane() {
    if (isComparingDatabases.value) return;

    const pane = ctx.getPane(ctx.activePaneId.value);
    const paneTabs = visiblePaneTabs(pane);
    const activeVisibleTab = paneTabs.find((tab) => tab.id === pane.activeTabId) ?? paneTabs[0] ?? null;

    if (!activeVisibleTab || paneTabs.length <= 1) {
      ctx.addPane();
      return;
    }

    const targetPaneId = ctx.addPane();
    moveTabToPane(activeVisibleTab.id, pane.id, targetPaneId);
  }

  function handleTabDragStart(paneId: string, tabId: string, event: DragEvent) {
    draggingTab.value = { paneId, tabId };
    ctx.activePaneId.value = paneId;
    event.dataTransfer?.setData("text/plain", `${paneId}:${tabId}`);
    if (event.dataTransfer) event.dataTransfer.effectAllowed = "move";
  }

  function handleTabDrop(targetPaneId: string, targetTabId: string | null, event: DragEvent) {
    event.preventDefault();
    const data = event.dataTransfer?.getData("text/plain");
    const [fallbackPaneId, fallbackTabId] = data?.includes(":") ? data.split(":") : [];
    const sourcePaneId = draggingTab.value?.paneId ?? fallbackPaneId;
    const tabId = draggingTab.value?.tabId ?? fallbackTabId;
    draggingTab.value = null;
    if (!sourcePaneId || !tabId) return;
    moveTabToPane(tabId, sourcePaneId, targetPaneId, targetTabId);
  }

  function syncSidebarToActiveTab(connId: string, db: string, tableName: string) {
    if (selectingWorkspaceDatabase.value) return;
    const pane = ctx.getPane(ctx.activePaneId.value);
    const activeTab = pane.tabs.find((tab) => tab.id === pane.activeTabId);
    if (!activeTab || !tabBelongsToPaneWorkspace(activeTab, pane)) return;

    if (store.openConnections[connId]) {
      ctx.selectedSidebarConnectionId.value = connId;
      store.openConnections[connId].selectedDatabase = db;
    }
    nextTick(() => ctx.sidebarRef.value?.scrollToTable(tableName, db, connId));
  }

  function resetWorkspaceState() {
    paneWorkspaces.value = {};
    workspaceActiveTabs.value = {};
  }

  watch(
    () => [
      isSinglePaneWorkspace.value,
      ctx.selectedSidebarConnectionId.value,
      ctx.selectedSidebarConnectionId.value
        ? store.openConnections[ctx.selectedSidebarConnectionId.value]?.selectedDatabase ?? null
        : null,
    ],
    () => syncSinglePaneToActiveWorkspace(),
  );

  watch(
    () => ctx.panes.value.map((pane) => `${pane.id}:${pane.activeTabId ?? ""}`).join("|"),
    () => {
      for (const pane of ctx.panes.value) {
        const activeTab = pane.tabs.find((tab) => tab.id === pane.activeTabId);
        if (activeTab) setPaneWorkspaceFromTab(pane.id, activeTab);
      }
    },
  );

  watch(
    () => {
      const pane = ctx.getPane(ctx.activePaneId.value);
      const tab = ctx.getPaneTab(pane);
      return tab ? `${tab.connectionId}:${tab.database}:${tab.tableName}` : null;
    },
    (key) => {
      if (!key) return;
      const [connId, db, tableName] = key.split(":");
      syncSidebarToActiveTab(connId, db, tableName);
    },
  );

  return {
    isSinglePaneWorkspace,
    isComparingDatabases,
    paneWorkspaceLabel,
    visiblePaneTabs,
    getVisiblePaneTab,
    isVisiblePaneActiveTabQuery,
    getVisiblePaneActiveQueryTab,
    handleCloseDatabase,
    handleSelectDatabase,
    activatePane,
    openDatabaseInSplit,
    splitActivePane,
    handleTabDragStart,
    handleTabDrop,
    resetWorkspaceState,
  };
}
