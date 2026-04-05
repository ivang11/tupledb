<script setup lang="ts">
import { ref, onMounted, computed, nextTick, watch } from "vue";
import { useConnectionStore } from "@/stores/connections";
import {
  SearchIcon,
  RefreshCwIcon,
  DatabaseIcon,
  TableIcon,
  ChevronRightIcon,
  ChevronLeftIcon,
  ChevronDownIcon,
  LayoutListIcon,
  TablePropertiesIcon,
  DownloadIcon,
  UploadIcon,
  PlusIcon,
  CheckIcon,
  XIcon,
  ArrowRightIcon,
  ServerIcon,
  PlugZapIcon,
  PencilIcon,
  ShieldCheckIcon,
  TerminalIcon,
  Trash2Icon,
  HardDriveIcon,
  CopyIcon,
  PanelRightOpenIcon,
  FilterIcon,
  ArrowUpIcon,
  ArrowDownIcon,
  ArrowUpDownIcon,
  KeyRoundIcon,
} from "lucide-vue-next";
import QueryEditor from "@/components/QueryEditor.vue";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Badge } from "@/components/ui/badge";
import { Separator } from "@/components/ui/separator";
import FilterBar from "@/components/FilterBar.vue";
import { ScrollArea } from "@/components/ui/scroll-area";
import { save, open } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from "@/components/ui/dialog";
import type { Connection, Environment } from "@/types/connection";
import { v4 as uuidv4 } from "uuid";

const store = useConnectionStore();
const search = ref("");

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

const showDeleteConnDialog = ref(false);
const connToDelete = ref<string | null>(null);

// ── Side panel copy ───────────────────────────────────────────────────────────
const copiedField = ref<string | null>(null);

async function copyFieldValue(paneId: string, colName: string, value: any) {
  const text = value === null || value === undefined ? "" : String(value);
  await navigator.clipboard.writeText(text);
  copiedField.value = `${paneId}:${colName}`;
  setTimeout(() => {
    copiedField.value = null;
  }, 1500);
}

// ── Side panel width resize ───────────────────────────────────────────────────
const sidePanelWidths = ref<Record<string, number>>({});
const sidePanelSearch = ref<Record<string, string>>({});

function startSidePanelResize(e: MouseEvent, paneId: string) {
  e.preventDefault();
  const startX = e.clientX;
  const startWidth = sidePanelWidths.value[paneId] ?? 320;

  const onMove = (ev: MouseEvent) => {
    const newWidth = Math.max(
      200,
      Math.min(700, startWidth + (startX - ev.clientX)),
    );
    sidePanelWidths.value[paneId] = newWidth;
  };
  const onUp = () => {
    window.removeEventListener("mousemove", onMove);
    window.removeEventListener("mouseup", onUp);
  };
  window.addEventListener("mousemove", onMove);
  window.addEventListener("mouseup", onUp);
}

// ── Side panel textarea auto-resize ──────────────────────────────────────────
function autoResizeTextarea(el: HTMLTextAreaElement | null) {
  if (!el) return;
  nextTick(() => {
    el.style.height = "auto";
    el.style.height = el.scrollHeight + "px";
  });
}

function resizeAllPanelTextareas() {
  nextTick(() => {
    document
      .querySelectorAll<HTMLTextAreaElement>("[data-row-detail-panel] textarea")
      .forEach((el) => {
        el.style.height = "auto";
        el.style.height = el.scrollHeight + "px";
      });
  });
}

// ── Column resize ─────────────────────────────────────────────────────────────
const columnWidths = ref<Record<string, Record<string, number>>>({});

// ── Structure panel resize (columns / indexes split) ─────────────────────────
const structureIndexHeights = ref<Record<string, number>>({});

function startStructureResize(e: MouseEvent, paneId: string) {
  e.preventDefault();
  const handle = e.currentTarget as HTMLElement;
  const container = handle.parentElement as HTMLElement;
  const startY = e.clientY;
  const startHeight =
    structureIndexHeights.value[paneId] ?? container.offsetHeight * 0.4;

  const onMove = (ev: MouseEvent) => {
    const delta = startY - ev.clientY;
    const newHeight = Math.max(
      60,
      Math.min(container.offsetHeight - 80, startHeight + delta),
    );
    structureIndexHeights.value[paneId] = newHeight;
  };
  const onUp = () => {
    window.removeEventListener("mousemove", onMove);
    window.removeEventListener("mouseup", onUp);
  };
  window.addEventListener("mousemove", onMove);
  window.addEventListener("mouseup", onUp);
}

function getColWidth(pane: PaneState, colName: string): number | undefined {
  const tab = getPaneTab(pane);
  if (!tab) return undefined;
  return columnWidths.value[tab.id]?.[colName];
}

function startColResize(e: MouseEvent, pane: PaneState, colName: string) {
  e.preventDefault();
  e.stopPropagation();
  const th = (e.currentTarget as HTMLElement).closest("th") as HTMLElement;
  const startX = e.clientX;
  const startWidth = th.getBoundingClientRect().width;
  const tab = getPaneTab(pane);
  if (!tab) return;

  const onMove = (ev: MouseEvent) => {
    const newWidth = Math.max(60, startWidth + ev.clientX - startX);
    if (!columnWidths.value[tab.id]) columnWidths.value[tab.id] = {};
    columnWidths.value[tab.id][colName] = newWidth;
  };
  const onUp = () => {
    window.removeEventListener("mousemove", onMove);
    window.removeEventListener("mouseup", onUp);
  };
  window.addEventListener("mousemove", onMove);
  window.addEventListener("mouseup", onUp);
}

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

function openEditConnDialog(conn: Connection) {
  newConn.value = JSON.parse(JSON.stringify(conn));
  sshEnabled.value = !!conn.ssh;
  if (conn.ssh) {
    sshForm.value = {
      host: conn.ssh.host,
      port: conn.ssh.port,
      user: conn.ssh.user,
      password: conn.ssh.auth.type === "password" ? conn.ssh.auth.password : "",
      private_key_path:
        conn.ssh.auth.type === "key" ? conn.ssh.auth.private_key_path : "",
      passphrase:
        conn.ssh.auth.type === "key" ? conn.ssh.auth.passphrase || "" : "",
    };
    sshAuthType.value = conn.ssh.auth.type === "password" ? "password" : "key";
  } else {
    sshForm.value = {
      host: "",
      port: 22,
      user: "",
      password: "",
      private_key_path: "",
      passphrase: "",
    };
    sshAuthType.value = "password";
  }
  testConnResult.value = null;
  showNewConnDialog.value = true;
}

function handleEditConnection(conn: Connection) {
  openEditConnDialog(conn);
}

function handleDuplicateConnection(conn: Connection) {
  const duplicate = JSON.parse(JSON.stringify(conn));
  duplicate.id = uuidv4();
  duplicate.name = `${conn.name} (Copy)`;

  newConn.value = duplicate;
  sshEnabled.value = !!conn.ssh;
  if (conn.ssh) {
    sshForm.value = {
      host: conn.ssh.host,
      port: conn.ssh.port,
      user: conn.ssh.user,
      password: conn.ssh.auth.type === "password" ? conn.ssh.auth.password : "",
      private_key_path:
        conn.ssh.auth.type === "key" ? conn.ssh.auth.private_key_path : "",
      passphrase:
        conn.ssh.auth.type === "key" ? conn.ssh.auth.passphrase || "" : "",
    };
    sshAuthType.value = conn.ssh.auth.type === "password" ? "password" : "key";
  } else {
    sshForm.value = {
      host: "",
      port: 22,
      user: "",
      password: "",
      private_key_path: "",
      passphrase: "",
    };
    sshAuthType.value = "password";
  }

  testConnResult.value = null;
  showNewConnDialog.value = true;
  sidebarContextMenu.value.show = false;
}

// ── Tab types ────────────────────────────────────────────────────────────────

interface TableTab {
  type: "table";
  id: string;
  connectionId: string;
  tableName: string;
  database: string;
  queryResult: any | null;
  tableStructure: any[];
  tableIndexes: any[];
  foreignKeys: any[];
  page: number;
  pageSize: number;
  viewMode: "content" | "structure";
  filters: any | null;
  sortColumn: string | null;
  sortDesc: boolean;
  pendingChanges: Record<string, Record<string, any>>;
  pendingDeletions: Record<string, boolean>;
  pendingTruncate: boolean;
  /** PK string of the row selected for the side panel */
  selectedRowPk: string | null;
  /** Column name when editing inline in the grid (double-click) */
  inlineEditColumn: string | null;
}

interface QueryTab {
  type: "query";
  id: string;
  connectionId: string;
  database: string | null;
}

type AnyTab = TableTab | QueryTab;

// ── Pane system ──────────────────────────────────────────────────────────────

interface PaneState {
  id: string;
  tabs: AnyTab[];
  activeTabId: string | null;
  viewMode: "content" | "structure";
  page: number;
  pageSize: number;
  showFilters: boolean;
}

function createPane(): PaneState {
  return {
    id: crypto.randomUUID(),
    tabs: [],
    activeTabId: null,
    viewMode: "content",
    page: 0,
    pageSize: 50,
    showFilters: false,
  };
}

const panes = ref<PaneState[]>([createPane()]);
const activePaneId = ref<string>(panes.value[0].id);
const paneWidths = ref<number[]>([1]);

// Resize state
const panesContainer = ref<HTMLElement | null>(null);
const draggingPaneIdx = ref<number | null>(null);

function getPane(paneId?: string): PaneState {
  return (
    panes.value.find((p) => p.id === (paneId ?? activePaneId.value)) ??
    panes.value[0]
  );
}

function addPane() {
  const pane = createPane();
  panes.value.push(pane);
  paneWidths.value.push(1);
  activePaneId.value = pane.id;
}

function removePane(paneId: string) {
  if (panes.value.length <= 1) return;
  const idx = panes.value.findIndex((p) => p.id === paneId);
  if (idx === -1) return;
  panes.value.splice(idx, 1);
  paneWidths.value.splice(idx, 1);
  if (activePaneId.value === paneId) {
    activePaneId.value = panes.value[Math.min(idx, panes.value.length - 1)].id;
  }
}

function startResize(e: MouseEvent, idx: number) {
  e.preventDefault();
  draggingPaneIdx.value = idx;
  const startX = e.clientX;
  const startWidths = [...paneWidths.value];

  const onMove = (e: MouseEvent) => {
    if (!panesContainer.value) return;
    const containerWidth = panesContainer.value.offsetWidth;
    const delta = e.clientX - startX;
    const total = startWidths.reduce((a, b) => a + b, 0);
    const deltaFlex = (delta / containerWidth) * total;
    const newWidths = [...startWidths];
    newWidths[idx] = Math.max(0.15, newWidths[idx] + deltaFlex);
    newWidths[idx + 1] = Math.max(0.15, newWidths[idx + 1] - deltaFlex);
    paneWidths.value = newWidths;
  };

  const onUp = () => {
    draggingPaneIdx.value = null;
    window.removeEventListener("mousemove", onMove);
    window.removeEventListener("mouseup", onUp);
  };

  window.addEventListener("mousemove", onMove);
  window.addEventListener("mouseup", onUp);
}

// ── Pane helpers ─────────────────────────────────────────────────────────────

function getPaneTab(pane: PaneState): TableTab | null {
  const tab = pane.tabs.find((t) => t.id === pane.activeTabId);
  return tab?.type === "table" ? (tab as TableTab) : null;
}

function isPaneActiveTabQuery(pane: PaneState): boolean {
  return pane.tabs.find((t) => t.id === pane.activeTabId)?.type === "query";
}

function getPaneGroupedTabs(pane: PaneState) {
  const groups: {
    connectionId: string;
    database: string | null;
    tabs: AnyTab[];
  }[] = [];
  for (const tab of pane.tabs) {
    const lastGroup = groups[groups.length - 1];
    if (
      lastGroup &&
      lastGroup.connectionId === tab.connectionId &&
      lastGroup.database === tab.database
    ) {
      lastGroup.tabs.push(tab);
    } else {
      groups.push({
        connectionId: tab.connectionId,
        database: tab.database,
        tabs: [tab],
      });
    }
  }
  return groups;
}

function getPaneQueryTabs(pane: PaneState): QueryTab[] {
  return pane.tabs.filter((t): t is QueryTab => t.type === "query");
}

function getPrimaryKey(pane: PaneState): string | null {
  const tab = getPaneTab(pane);
  if (!tab) return null;
  return (
    (tab.tableStructure as any[]).find((c) => c.key === "PRI")?.field || null
  );
}

function hasPendingChangesInPane(pane: PaneState): boolean {
  const tab = getPaneTab(pane);
  if (!tab) return false;
  return (
    tab.pendingTruncate ||
    Object.keys(tab.pendingChanges).length > 0 ||
    Object.keys(tab.pendingDeletions).length > 0
  );
}

function getFkMap(
  pane: PaneState,
): Record<string, { table: string; column: string }> {
  const tab = getPaneTab(pane);
  if (!tab) return {};
  const map: Record<string, { table: string; column: string }> = {};
  for (const fk of tab.foreignKeys as any[]) {
    map[fk.column] = {
      table: fk.referenced_table,
      column: fk.referenced_column,
    };
  }
  const connTables =
    store.openConnections[tab.connectionId]?.tables[tab.database] ?? [];
  const tableNames = (connTables as any[]).map((t: any) =>
    t.name.toLowerCase(),
  );
  const heuristicCols = [
    ...((tab.queryResult as any)?.columns ?? []).map((c: any) => c.name),
    ...((tab.tableStructure as any[]) ?? []).map((c: any) => c.field),
  ];
  for (const colName of heuristicCols) {
    if (colName.endsWith("_id") && !map[colName]) {
      const prefix = colName.slice(0, -3);
      for (const candidate of [prefix + "s", prefix + "es", prefix]) {
        const i = tableNames.indexOf(candidate.toLowerCase());
        if (i !== -1) {
          map[colName] = { table: (connTables as any[])[i].name, column: "id" };
          break;
        }
      }
    }
  }
  return map;
}

function getPaneConnection(pane: PaneState): Connection | null {
  const tab = getPaneTab(pane);
  if (!tab) return null;
  return store.openConnections[tab.connectionId]?.connection ?? null;
}

// Sidebar: check across all panes
function isTableOpenInAnyPane(
  tableName: string,
  database: string,
  connectionId: string,
): boolean {
  return panes.value.some((pane) =>
    pane.tabs.some(
      (t) =>
        t.type === "table" &&
        (t as TableTab).tableName === tableName &&
        (t as TableTab).database === database &&
        t.connectionId === connectionId,
    ),
  );
}

function isTableActiveInAnyPane(
  tableName: string,
  database: string,
  connectionId: string,
): boolean {
  return panes.value.some((pane) => {
    const tab = getPaneTab(pane);
    return (
      tab?.tableName === tableName &&
      tab?.database === database &&
      tab?.connectionId === connectionId
    );
  });
}

// ── Core tab operations ──────────────────────────────────────────────────────

const isSaving = ref(false);
const disableFkChecks = ref(false);

// ── Insert row inline ─────────────────────────────────────────────────────────
const insertingRowPaneId = ref<string | null>(null);
const insertRowValues = ref<Record<string, string>>({});
const insertRowLoading = ref(false);
const insertRowError = ref<string | null>(null);

function isColAutoIncrement(pane: PaneState, colName: string): boolean {
  const tab = getPaneTab(pane);
  if (!tab) return false;
  return (tab.tableStructure as any[]).find((c: any) => c.field === colName)?.extra === "auto_increment";
}

function isBooleanCol(pane: PaneState, colName: string): boolean {
  const tab = getPaneTab(pane);
  if (!tab) return false;
  const col = (tab.tableStructure as any[]).find((c: any) => c.field === colName);
  const type = (col?.type ?? "").toLowerCase();
  return type === "tinyint(1)" || type === "boolean" || type === "bool";
}

function openInsertRowDialog(pane: PaneState) {
  if (insertingRowPaneId.value === pane.id) {
    insertingRowPaneId.value = null;
    return;
  }
  const tab = getPaneTab(pane);
  if (!tab) return;
  insertRowValues.value = Object.fromEntries(
    (tab.tableStructure as any[])
      .filter((col: any) => col.extra !== "auto_increment")
      .map((col: any) => [col.field, col.default ?? ""])
  );
  insertRowError.value = null;
  insertingRowPaneId.value = pane.id;
  nextTick(() => {
    const firstInput = document.querySelector<HTMLInputElement>(".insert-row-input");
    firstInput?.focus();
  });
}

function cancelInsertRow() {
  insertingRowPaneId.value = null;
  insertRowError.value = null;
}

async function submitInsertRow(pane: PaneState) {
  const tab = getPaneTab(pane);
  const conn = getPaneConnection(pane);
  if (!tab || !conn) return;
  insertRowLoading.value = true;
  insertRowError.value = null;
  try {
    const values = Object.entries(insertRowValues.value).map(([column, value]) => {
      if (value === "" || value === null) return { column, value: null };
      // Convertir true/false a 1/0 siempre (cubre tinyint(1) y cualquier variante)
      const lower = String(value).toLowerCase().trim();
      if (lower === "true") return { column, value: 1 };
      if (lower === "false") return { column, value: 0 };
      return { column, value };
    });
    syncStoreForFetch(tab.connectionId, tab.database);
    await invoke("insert_row", {
      connectionId: conn.id,
      database: tab.database,
      table: tab.tableName,
      values,
      disableFkChecks: disableFkChecks.value,
    });
    insertingRowPaneId.value = null;
    await refreshActiveTab(pane.id);
  } catch (e: any) {
    const msg = String(e);
    // Extraer solo el mensaje de MySQL si está disponible
    const match = msg.match(/: (\d{4} \(.+?\): .+)$/);
    insertRowError.value = match ? match[1] : msg;
  } finally {
    insertRowLoading.value = false;
  }
}

function getActiveTab(paneId?: string): TableTab | null {
  return getPaneTab(getPane(paneId));
}

function syncStoreForFetch(connectionId: string, database: string) {
  const connState = store.openConnections[connectionId];
  store.activeConnection = connState?.connection ?? null;
  store.activeDatabase = database;
  store.tables = connState?.tables[database] ?? [];
}

function saveToActiveTab(pane: PaneState) {
  const tab = getPaneTab(pane);
  if (!tab) return;
  tab.queryResult = store.queryResult;
  tab.tableStructure = store.tableStructure;
  tab.tableIndexes = store.tableIndexes;
  tab.foreignKeys = store.foreignKeys;
}

function openQueryTab(
  connectionId: string,
  database: string | null = null,
  paneId?: string,
) {
  const pane = getPane(paneId);
  const id = crypto.randomUUID();
  const tab: QueryTab = { type: "query", id, connectionId, database };

  let insertIndex = pane.tabs.length;
  for (let i = pane.tabs.length - 1; i >= 0; i--) {
    const t = pane.tabs[i];
    if (t.connectionId === connectionId && t.database === database) {
      insertIndex = i + 1;
      break;
    } else if (
      t.connectionId === connectionId &&
      insertIndex === pane.tabs.length
    ) {
      insertIndex = i + 1;
    }
  }
  pane.tabs.splice(insertIndex, 0, tab);
  pane.activeTabId = id;
}

function switchToTab(tabId: string, paneId?: string) {
  const pane = getPane(paneId);
  const tab = pane.tabs.find((t) => t.id === tabId);
  if (!tab) return;
  pane.activeTabId = tabId;
  if (tab.type === "table") {
    const t = tab as TableTab;
    pane.viewMode = t.viewMode;
    pane.page = t.page;
    pane.pageSize = t.pageSize;
    if (pane.id === activePaneId.value) {
      syncStoreForFetch(t.connectionId, t.database);
      store.activeTable = t.tableName;
      store.queryResult = t.queryResult;
      store.tableStructure = t.tableStructure;
      store.tableIndexes = t.tableIndexes;
      store.foreignKeys = t.foreignKeys;
    }
  } else {
    if (pane.id === activePaneId.value) {
      store.activeTable = null;
      store.queryResult = null;
      store.tableStructure = [];
      store.tableIndexes = [];
      store.foreignKeys = [];
    }
  }
}

function closeTab(tabId: string, paneId?: string, event?: MouseEvent) {
  event?.stopPropagation();
  const pane = getPane(paneId);
  const idx = pane.tabs.findIndex((t) => t.id === tabId);
  if (idx === -1) return;
  pane.tabs.splice(idx, 1);
  if (pane.activeTabId === tabId) {
    if (pane.tabs.length === 0) {
      pane.activeTabId = null;
      if (pane.id === activePaneId.value) {
        store.activeTable = null;
        store.queryResult = null;
        store.tableStructure = [];
        store.foreignKeys = [];
      }
    } else {
      switchToTab(pane.tabs[Math.min(idx, pane.tabs.length - 1)].id, pane.id);
    }
  }
}

async function loadTableData(
  tableName: string,
  connectionId: string,
  database: string,
  initialFilter?: any,
  paneId?: string,
) {
  const pane = getPane(paneId);

  if (!initialFilter) {
    const existing = pane.tabs.find(
      (t) =>
        t.type === "table" &&
        (t as TableTab).tableName === tableName &&
        (t as TableTab).database === database &&
        t.connectionId === connectionId,
    );
    if (existing) {
      switchToTab(existing.id, pane.id);
      return;
    }
  }

  syncStoreForFetch(connectionId, database);

  const id = crypto.randomUUID();
  const tab: TableTab = {
    type: "table",
    id,
    connectionId,
    tableName,
    database,
    queryResult: null,
    tableStructure: [],
    tableIndexes: [],
    foreignKeys: [],
    page: 0,
    pageSize: pane.pageSize,
    viewMode: "content",
    filters: initialFilter ?? null,
    sortColumn: null,
    sortDesc: false,
    pendingChanges: {},
    pendingDeletions: {},
    pendingTruncate: false,
    selectedRowPk: null,
    inlineEditColumn: null,
  };

  let insertIndex = pane.tabs.length;
  for (let i = pane.tabs.length - 1; i >= 0; i--) {
    const t = pane.tabs[i];
    if (t.connectionId === connectionId && t.database === database) {
      insertIndex = i + 1;
      break;
    } else if (
      t.connectionId === connectionId &&
      insertIndex === pane.tabs.length
    ) {
      insertIndex = i + 1;
    }
  }
  pane.tabs.splice(insertIndex, 0, tab);
  pane.activeTabId = id;
  pane.page = 0;
  pane.viewMode = "content";
  store.queryResult = null;
  store.tableStructure = [];
  store.tableIndexes = [];
  store.foreignKeys = [];

  try {
    await Promise.all([
      store.fetchTableData(tableName, 0, tab.pageSize, initialFilter ?? null),
      store.fetchTableStructure(tableName),
      store.fetchTableIndexes(tableName),
      store.fetchForeignKeys(tableName),
    ]);
    saveToActiveTab(pane);
  } catch (e: any) {
    if (String(e).includes("No active session")) {
      store.disconnectConnection(connectionId);
    }
  }
}

async function refreshActiveTab(paneId?: string) {
  const pane = getPane(paneId);
  const tab = getPaneTab(pane);
  if (!tab) return;
  syncStoreForFetch(tab.connectionId, tab.database);
  try {
    await Promise.all([
      store.fetchTableData(
        tab.tableName,
        pane.page,
        pane.pageSize,
        tab.filters ?? null,
        sortPayload(tab),
      ),
      store.fetchTableStructure(tab.tableName),
      store.fetchTableIndexes(tab.tableName),
      store.fetchForeignKeys(tab.tableName),
    ]);
    saveToActiveTab(pane);
  } catch (e: any) {
    if (String(e).includes("No active session")) {
      store.disconnectConnection(tab.connectionId);
    }
  }
}

async function changePage(pane: PaneState, delta: number) {
  const tab = getPaneTab(pane);
  if (!tab) return;
  tab.selectedRowPk = null;
  tab.inlineEditColumn = null;
  pane.page += delta;
  tab.page = pane.page;
  syncStoreForFetch(tab.connectionId, tab.database);
  await store.fetchTableData(
    tab.tableName,
    pane.page,
    pane.pageSize,
    tab.filters,
    sortPayload(tab),
  );
  saveToActiveTab(pane);
}

async function changeLimit(pane: PaneState, newLimit: number) {
  if (!newLimit || newLimit < 1) return;
  const tab = getPaneTab(pane);
  if (!tab) return;
  const offset = pane.page * pane.pageSize;
  pane.pageSize = newLimit;
  tab.pageSize = newLimit;
  pane.page = Math.floor(offset / newLimit);
  tab.page = pane.page;
  tab.selectedRowPk = null;
  tab.inlineEditColumn = null;
  syncStoreForFetch(tab.connectionId, tab.database);
  await store.fetchTableData(tab.tableName, pane.page, pane.pageSize, tab.filters, sortPayload(tab));
  saveToActiveTab(pane);
}

async function gotoOffset(pane: PaneState, newOffset: number) {
  if (newOffset < 0) return;
  const tab = getPaneTab(pane);
  if (!tab) return;
  tab.selectedRowPk = null;
  tab.inlineEditColumn = null;
  pane.page = Math.floor(newOffset / pane.pageSize);
  tab.page = pane.page;
  syncStoreForFetch(tab.connectionId, tab.database);
  await store.fetchTableData(tab.tableName, pane.page, pane.pageSize, tab.filters, sortPayload(tab));
  saveToActiveTab(pane);
}

function sortPayload(
  tab: TableTab | null,
): { column: string; desc: boolean } | null {
  if (!tab || !tab.sortColumn) return null;
  return { column: tab.sortColumn, desc: tab.sortDesc };
}

async function onSortColumnHeaderClick(pane: PaneState, column: string) {
  const tab = getPaneTab(pane);
  if (!tab) return;
  if (tab.sortColumn === column) {
    tab.sortDesc = !tab.sortDesc;
  } else {
    tab.sortColumn = column;
    tab.sortDesc = false;
  }
  tab.selectedRowPk = null;
  tab.inlineEditColumn = null;
  pane.page = 0;
  tab.page = 0;
  syncStoreForFetch(tab.connectionId, tab.database);
  await store.fetchTableData(
    tab.tableName,
    pane.page,
    pane.pageSize,
    tab.filters,
    sortPayload(tab),
  );
  tab.queryResult = store.queryResult;
  saveToActiveTab(pane);
}

function updatePendingChange(
  pane: PaneState,
  row: any,
  column: string,
  newValue: any,
) {
  const tab = getPaneTab(pane);
  const pk = getPrimaryKey(pane);
  if (!tab || !pk) return;
  const pkVal = String(row[pk]);
  const originalValue = row[column];
  if (newValue === originalValue) {
    if (tab.pendingChanges[pkVal]) {
      delete tab.pendingChanges[pkVal][column];
      if (Object.keys(tab.pendingChanges[pkVal]).length === 0) {
        delete tab.pendingChanges[pkVal];
      }
    }
    return;
  }
  if (!tab.pendingChanges[pkVal]) tab.pendingChanges[pkVal] = {};
  tab.pendingChanges[pkVal][column] = newValue;
}

function toggleDeletion(pane: PaneState, row: any) {
  const tab = getPaneTab(pane);
  const pk = getPrimaryKey(pane);
  if (!tab || !pk) return;
  const pkVal = String(row[pk]);
  if (tab.pendingDeletions[pkVal]) {
    delete tab.pendingDeletions[pkVal];
  } else {
    tab.pendingDeletions[pkVal] = true;
  }
}

function discardChanges(pane: PaneState) {
  const tab = getPaneTab(pane);
  if (!tab) return;
  tab.pendingChanges = {};
  tab.pendingDeletions = {};
  tab.pendingTruncate = false;
  tab.selectedRowPk = null;
  tab.inlineEditColumn = null;
}

function clearRowSelection(pane: PaneState) {
  const tab = getPaneTab(pane);
  if (!tab) return;
  tab.selectedRowPk = null;
  tab.inlineEditColumn = null;
}

function getSelectedRow(pane: PaneState): Record<string, any> | null {
  const tab = getPaneTab(pane);
  const pk = getPrimaryKey(pane);
  if (!tab?.queryResult?.rows || !tab.selectedRowPk || !pk) return null;
  return (
    tab.queryResult.rows.find(
      (r: any) => String(r[pk]) === tab.selectedRowPk,
    ) ?? null
  );
}

function selectTableRow(pane: PaneState, row: any, clickedColName?: string) {
  const tab = getPaneTab(pane);
  const pk = getPrimaryKey(pane);
  if (!tab || !pk) return;
  const pkVal = String((row as any)[pk]);
  if (
    clickedColName &&
    tab.selectedRowPk === pkVal &&
    tab.inlineEditColumn &&
    tab.inlineEditColumn !== clickedColName
  ) {
    tab.inlineEditColumn = null;
  }
  if (tab.selectedRowPk === pkVal) return;
  tab.selectedRowPk = pkVal;
  tab.inlineEditColumn = null;
}

function onTableRowClick(pane: PaneState, row: any, e: MouseEvent) {
  const el = e.target as HTMLElement;
  if (el.closest("button")) return;
  const td = el.closest("td");
  if (!td?.parentElement) return;
  const idx = Array.from(td.parentElement.children).indexOf(td);
  if (idx === 0) {
    if (!el.closest("button")) selectTableRow(pane, row);
    return;
  }
  const cols = getPaneTab(pane)?.queryResult?.columns;
  const col = cols?.[idx - 1] as { name: string } | undefined;
  selectTableRow(pane, row, col?.name);
}

function isInlineEditingCell(
  pane: PaneState,
  row: any,
  colName: string,
): boolean {
  const tab = getPaneTab(pane);
  const pk = getPrimaryKey(pane);
  if (!tab || !pk || !tab.inlineEditColumn) return false;
  return (
    tab.inlineEditColumn === colName &&
    tab.selectedRowPk === String((row as any)[pk])
  );
}

function startInlineCellEdit(pane: PaneState, row: any, colName: string) {
  const tab = getPaneTab(pane);
  const pk = getPrimaryKey(pane);
  if (!tab || !pk) return;
  const pkVal = String((row as any)[pk]);
  if (tab.pendingDeletions[pkVal]) return;
  tab.selectedRowPk = pkVal;
  tab.inlineEditColumn = colName;
  nextTick(() => {
    try {
      const sel = `input[data-grid-edit="${CSS.escape(String(pkVal))}"][data-col="${CSS.escape(colName)}"]`;
      document.querySelector<HTMLInputElement>(sel)?.focus();
      document.querySelector<HTMLInputElement>(sel)?.select();
    } catch {
      /* ignore */
    }
  });
}

function onGridCellBlur(pane: PaneState) {
  const tab = getPaneTab(pane);
  if (tab) tab.inlineEditColumn = null;
}

function cellEditValue(pane: PaneState, row: any, colName: string): string {
  const tab = getPaneTab(pane);
  const pk = getPrimaryKey(pane);
  if (!tab || !pk) return "";
  const pkVal = String((row as any)[pk]);
  const pending = tab.pendingChanges[pkVal]?.[colName];
  if (pending !== undefined) return pending === null ? "" : String(pending);
  const v = (row as any)[colName];
  if (v === null || v === undefined) return "";
  if (typeof v === "object") return JSON.stringify(v);
  return String(v);
}

function setViewMode(pane: PaneState, mode: "content" | "structure") {
  pane.viewMode = mode;
  if (mode !== "content") clearRowSelection(pane);
}

async function applyChanges(pane: PaneState) {
  const tab = getPaneTab(pane);
  const conn = getPaneConnection(pane);
  if (!tab || !conn) return;

  isSaving.value = true;
  try {
    if (tab.pendingTruncate) {
      await invoke("truncate_table", {
        connectionId: conn.id,
        database: tab.database,
        table: tab.tableName,
        disableFkChecks: disableFkChecks.value,
      });
    } else {
      const pk = getPrimaryKey(pane);
      if (!pk) throw new Error("Table has no Primary Key");

      const updates = Object.entries(tab.pendingChanges).map(
        ([pkValue, changes]) => ({
          pk_column: pk,
          pk_value: isNaN(Number(pkValue)) ? pkValue : Number(pkValue),
          changes: Object.entries(changes).map(([column, value]) => ({
            column,
            value:
              value === null
                ? null
                : isNaN(Number(value))
                  ? value
                  : Number(value),
          })),
        }),
      );

      const deletions = Object.keys(tab.pendingDeletions).map((pkValue) => ({
        pk_column: pk,
        pk_value: isNaN(Number(pkValue)) ? pkValue : Number(pkValue),
      }));

      await invoke("apply_table_changes", {
        connectionId: conn.id,
        database: tab.database,
        table: tab.tableName,
        updates,
        deletions,
        disableFkChecks: disableFkChecks.value,
      });
    }

    tab.pendingChanges = {};
    tab.pendingDeletions = {};
    tab.pendingTruncate = false;
    tab.selectedRowPk = null;
    tab.inlineEditColumn = null;
    await refreshActiveTab(pane.id);
  } catch (e: any) {
    alert(`Failed to apply changes: ${e}`);
  } finally {
    isSaving.value = false;
  }
}

async function handleDeleteTable(pane: PaneState) {
  const tab = getPaneTab(pane);
  const conn = getPaneConnection(pane);
  if (!tab || !conn) return;
  const confirmed = confirm(
    `Are you sure you want to DROP TABLE \`${tab.tableName}\`? This cannot be undone.`,
  );
  if (!confirmed) return;
  try {
    await invoke("drop_table", {
      connection_id: conn.id,
      database: tab.database,
      table: tab.tableName,
      disable_fk_checks: disableFkChecks.value,
    });
    closeTab(tab.id, pane.id);
    await store.fetchTablesForConnection(tab.connectionId, tab.database);
  } catch (e: any) {
    alert(`Failed to drop table: ${e}`);
  }
}

function handleTruncateTable(pane: PaneState) {
  const tab = getPaneTab(pane);
  if (!tab) return;
  tab.pendingTruncate = !tab.pendingTruncate;
  if (tab.pendingTruncate) {
    tab.pendingChanges = {};
    tab.pendingDeletions = {};
  }
}

async function navigateToRelated(
  pane: PaneState,
  targetTable: string,
  filterColumn: string,
  filterValue: any,
) {
  const tab = getPaneTab(pane);
  if (!tab) return;
  const filter = {
    match_all: true,
    rows: [
      {
        active: true,
        column: filterColumn,
        operator: "equals",
        value: String(filterValue),
      },
    ],
  };
  await loadTableData(
    targetTable,
    tab.connectionId,
    tab.database,
    filter,
    pane.id,
  );
}

// ── Sidebar state ────────────────────────────────────────────────────────────

const expandedConnections = ref<Set<string>>(new Set());
const expandedDatabases = ref<Set<string>>(new Set());

function dbKey(connectionId: string, db: string) {
  return `${connectionId}:${db}`;
}

function getAvailableDatabases(connectionId: string): string[] {
  return store.openConnections[connectionId]?.databases ?? [];
}

onMounted(async () => {
  await store.fetchConnections();
  for (const id of Object.keys(store.openConnections)) {
    expandedConnections.value.add(id);
  }
});

const connectingId = ref<string | null>(null);

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

async function toggleConnection(connectionId: string) {
  if (expandedConnections.value.has(connectionId)) {
    expandedConnections.value.delete(connectionId);
  } else {
    expandedConnections.value.add(connectionId);
  }
}

async function toggleDatabase(connectionId: string, db: string) {
  const key = dbKey(connectionId, db);
  if (expandedDatabases.value.has(key)) {
    expandedDatabases.value.delete(key);
  } else {
    expandedDatabases.value.add(key);
    const connState = store.openConnections[connectionId];
    if (connState && !connState.tables[db]) {
      await store.fetchTablesForConnection(connectionId, db);
    }
  }
}

function disconnectConn(id: string) {
  for (const pane of panes.value) {
    pane.tabs = pane.tabs.filter((t) => t.connectionId !== id);
    if (pane.activeTabId && !pane.tabs.find((t) => t.id === pane.activeTabId)) {
      if (pane.tabs.length > 0) {
        switchToTab(pane.tabs[0].id, pane.id);
      } else {
        pane.activeTabId = null;
        if (pane.id === activePaneId.value) {
          store.activeTable = null;
          store.queryResult = null;
          store.tableStructure = [];
          store.foreignKeys = [];
        }
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

const getEnvColor = (env: Environment) => {
  switch (env) {
    case "PRODUCTION":
      return "bg-red-500/10 text-red-500 border-red-500/20";
    case "STAGING":
      return "bg-orange-500/10 text-orange-500 border-orange-500/20";
    case "DEV":
      return "bg-blue-500/10 text-blue-500 border-blue-500/20";
    default:
      return "bg-green-500/10 text-green-500 border-green-500/20";
  }
};

const closedConnections = computed(() =>
  store.connections.filter((c) => !store.openConnections[c.id]),
);

// ── Database management ──────────────────────────────────────────────────────

const newDbName = ref("");
const showNewDb = ref<string | null>(null);
const isCreatingDb = ref(false);

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

// ── Import / Export ──────────────────────────────────────────────────────────

const isImporting = ref(false);
const importResult = ref<{ executed: number; errors: string[] } | null>(null);
const importProgress = ref({ current: 0, total: 0, status: "" });

async function importSql(connectionId: string, database: string) {
  const path = await open({
    filters: [{ name: "SQL", extensions: ["sql"] }],
    multiple: false,
  });
  if (!path) return;

  syncStoreForFetch(connectionId, database);

  isImporting.value = true;
  importResult.value = null;
  importProgress.value = { current: 0, total: 0, status: "Reading file..." };

  let unlisten: UnlistenFn | null = null;
  try {
    unlisten = await listen<{ current: number; total: number; status: string }>(
      "import-progress",
      (event) => {
        importProgress.value = event.payload;
      },
    );

    const result = await invoke<{ executed: number; errors: string[] }>(
      "import_sql",
      {
        connectionId,
        database,
        path,
      },
    );
    importResult.value = result;
    await store.fetchTablesForConnection(connectionId, database);
    const tab = getActiveTab();
    if (tab && tab.connectionId === connectionId && tab.database === database) {
      await loadTableData(tab.tableName, connectionId, database);
    }
  } catch (e: any) {
    importResult.value = { executed: 0, errors: [String(e)] };
  } finally {
    isImporting.value = false;
    if (unlisten) unlisten();
  }
}

const dbExportOptions = [
  { mode: "full", label: "Full", desc: "Schema + Data" },
  { mode: "structure", label: "Structure", desc: "Schema only" },
  { mode: "data", label: "Data", desc: "Data only" },
];
const showDbExportMenu = ref(false);
const isExportingDb = ref(false);
const exportProgress = ref({ current: 0, total: 0, status: "" });
const exportResult = ref<{ success: boolean; message: string } | null>(null);

const showTableSelector = ref(false);
const selectedExportTables = ref<string[]>([]);
const currentExportMode = ref("full");
const exportContext = ref<{ connectionId: string; database: string } | null>(
  null,
);

function openExportSelector(connectionId: string, database: string) {
  exportContext.value = { connectionId, database };
  currentExportMode.value = "full";
  selectedExportTables.value = (
    store.openConnections[connectionId]?.tables[database] ?? []
  ).map((t: any) => t.name);
  showTableSelector.value = true;
  showDbExportMenu.value = false;
}

function toggleAllTables() {
  if (!exportContext.value) return;
  const all = (
    store.openConnections[exportContext.value.connectionId]?.tables[
      exportContext.value.database
    ] ?? []
  ).map((t: any) => t.name);
  if (selectedExportTables.value.length === all.length) {
    selectedExportTables.value = [];
  } else {
    selectedExportTables.value = all;
  }
}

const exportContextTables = computed(() => {
  if (!exportContext.value) return [];
  return (
    store.openConnections[exportContext.value.connectionId]?.tables[
      exportContext.value.database
    ] ?? []
  );
});

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
    unlisten = await listen<{ current: number; total: number; status: string }>(
      "export-progress",
      (event) => {
        exportProgress.value = event.payload;
      },
    );

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

// ── Table actions (from sidebar context menu) ────────────────────────────────

const showTableActionDialog = ref(false);
const tableActionData = ref<{
  type: "truncate" | "drop";
  connectionId: string;
  database: string;
  tableName: string;
} | null>(null);
const isExecutingTableAction = ref(false);
const tableActionDisableFk = ref(false);

function confirmSidebarDropTable() {
  const { connectionId, database, tableName } = sidebarTableContextMenu.value;
  if (!connectionId || !database || !tableName) return;
  tableActionData.value = { type: "drop", connectionId, database, tableName };
  tableActionDisableFk.value = false;
  showTableActionDialog.value = true;
  sidebarTableContextMenu.value.show = false;
}

function confirmSidebarTruncateTable() {
  const { connectionId, database, tableName } = sidebarTableContextMenu.value;
  if (!connectionId || !database || !tableName) return;
  tableActionData.value = {
    type: "truncate",
    connectionId,
    database,
    tableName,
  };
  tableActionDisableFk.value = false;
  showTableActionDialog.value = true;
  sidebarTableContextMenu.value.show = false;
}

async function executeTableAction() {
  if (!tableActionData.value) return;
  const { type, connectionId, database, tableName } = tableActionData.value;

  isExecutingTableAction.value = true;
  try {
    if (type === "drop") {
      await invoke("drop_table", {
        connection_id: connectionId,
        database,
        table: tableName,
        disable_fk_checks: tableActionDisableFk.value,
      });
      for (const pane of panes.value) {
        const relatedTabs = pane.tabs.filter(
          (t) =>
            t.type === "table" &&
            (t as TableTab).tableName === tableName &&
            (t as TableTab).database === database &&
            t.connectionId === connectionId,
        );
        relatedTabs.forEach((t) => closeTab(t.id, pane.id));
      }
      await store.fetchTablesForConnection(connectionId, database);
    } else {
      await invoke("truncate_table", {
        connectionId,
        database,
        table: tableName,
        disableFkChecks: tableActionDisableFk.value,
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
          if (tab.id === pane.activeTabId) {
            await refreshActiveTab(pane.id);
          }
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

// ── New Connection Dialog ────────────────────────────────────────────────────

const showNewConnDialog = ref(false);
const isSavingConn = ref(false);
const isTestingConn = ref(false);
const testConnResult = ref<{ ok: boolean; msg: string } | null>(null);

const sshEnabled = ref(false);
const sshAuthType = ref<"password" | "key">("password");
const sshForm = ref({
  host: "",
  port: 22,
  user: "",
  password: "",
  private_key_path: "",
  passphrase: "",
});

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
  sshEnabled.value = false;
  sshAuthType.value = "password";
  sshForm.value = {
    host: "",
    port: 22,
    user: "",
    password: "",
    private_key_path: "",
    passphrase: "",
  };
  testConnResult.value = null;
  showNewConnDialog.value = true;
}

function buildConnWithSsh(): Connection {
  const conn = { ...newConn.value };
  if (sshEnabled.value) {
    conn.ssh = {
      host: sshForm.value.host,
      port: sshForm.value.port,
      user: sshForm.value.user,
      auth:
        sshAuthType.value === "password"
          ? { type: "password" as const, password: sshForm.value.password }
          : {
              type: "key" as const,
              private_key_path: sshForm.value.private_key_path,
              passphrase: sshForm.value.passphrase || undefined,
            },
    };
  }
  return conn;
}

async function testNewConn() {
  isTestingConn.value = true;
  testConnResult.value = null;
  try {
    const msg = await store.testConnection(buildConnWithSsh());
    testConnResult.value = { ok: true, msg: msg ?? "Connection successful" };
  } catch (e: any) {
    testConnResult.value = { ok: false, msg: String(e) };
  } finally {
    isTestingConn.value = false;
  }
}

async function saveNewConn(andConnect = false) {
  if (!newConn.value.name) return;
  isSavingConn.value = true;
  const conn = buildConnWithSsh();
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
    if (pane.activeTabId && !pane.tabs.find((t) => t.id === pane.activeTabId)) {
      pane.activeTabId = pane.tabs[0]?.id ?? null;
    }
  }
  await store.removeConnection(id);
  showDeleteConnDialog.value = false;
  connToDelete.value = null;
}

watch(
  () => panes.value.map((p) => getPaneTab(p)?.selectedRowPk).join(","),
  () => resizeAllPanelTextareas(),
);
</script>

<template>
  <div class="h-full flex overflow-hidden bg-background">
    <!-- Sidebar: Multi-connection explorer -->
    <aside class="w-72 flex flex-col border-r bg-muted/10">
      <!-- Header -->
      <div class="p-4 border-b bg-background/50 backdrop-blur-sm">
        <div class="flex items-center justify-between mb-3">
          <div class="flex items-center gap-2">
            <div
              class="size-8 rounded bg-primary/10 flex items-center justify-center text-primary shrink-0"
            >
              <ServerIcon class="size-4" />
            </div>
            <div>
              <h2 class="text-sm font-bold">Explorer</h2>
              <span class="text-[10px] text-muted-foreground"
                >{{ Object.keys(store.openConnections).length }} connection{{
                  Object.keys(store.openConnections).length !== 1 ? "s" : ""
                }}</span
              >
            </div>
          </div>
          <button
            @click="openNewConnDialog"
            class="size-7 flex items-center justify-center rounded-md text-muted-foreground hover:text-primary hover:bg-primary/10 transition-colors"
            title="New Connection"
          >
            <PlusIcon class="size-4" />
          </button>
        </div>
        <div
          v-if="Object.keys(store.openConnections).length > 0"
          class="relative"
        >
          <SearchIcon
            class="absolute left-2.5 top-1/2 -translate-y-1/2 size-3.5 text-muted-foreground"
          />
          <Input
            v-model="search"
            placeholder="Filter tables..."
            class="h-8 pl-8 text-xs bg-muted/50 border-none rounded-lg"
          />
        </div>
      </div>

      <!-- Tree -->
      <ScrollArea class="flex-1 p-2">
        <!-- Empty state -->
        <div
          v-if="
            Object.keys(store.openConnections).length === 0 &&
            closedConnections.length === 0
          "
          class="flex flex-col items-center justify-center py-12 px-4 text-center"
        >
          <PlugZapIcon class="size-8 text-muted-foreground/30 mb-3" />
          <p class="text-xs text-muted-foreground/60 font-medium">
            No connections saved
          </p>
        </div>

        <!-- Connection nodes (connected) -->
        <template v-if="Object.keys(store.openConnections).length > 0">
          <div class="px-2 py-1.5 mb-0.5">
            <span
              class="text-[10px] font-bold text-muted-foreground/50 uppercase tracking-widest"
              >Connected</span
            >
          </div>
        </template>
        <div
          v-for="(connState, connId) in store.openConnections"
          :key="connId"
          class="mb-1"
        >
          <!-- Connection header -->
          <div
            class="flex items-center gap-1 group rounded-md hover:bg-muted/40 transition-colors select-none"
          >
            <button
              class="flex-1 flex items-center gap-2 px-2 py-1.5 min-w-0"
              @click="toggleConnection(connId as string)"
              @contextmenu="
                openSidebarContextMenu($event, connState.connection)
              "
            >
              <ChevronDownIcon
                v-if="expandedConnections.has(connId as string)"
                class="size-3.5 text-muted-foreground shrink-0 transition-transform"
              />
              <ChevronRightIcon
                v-else
                class="size-3.5 text-muted-foreground shrink-0 transition-transform"
              />
              <DatabaseIcon class="size-3.5 shrink-0 text-primary/70" />
              <span class="text-xs font-bold truncate flex-1 text-left">{{
                connState.connection.name
              }}</span>
              <Badge
                variant="outline"
                :class="[
                  getEnvColor(connState.connection.environment),
                  'text-[9px] uppercase py-0 px-1 h-3.5 shrink-0',
                ]"
              >
                {{ connState.connection.environment }}
              </Badge>
            </button>
          </div>

          <!-- Databases -->
          <div
            v-if="expandedConnections.has(connId as string)"
            class="ml-4 mt-0.5 space-y-0.5"
          >
            <!-- New DB form -->
            <div v-if="showNewDb === connId" class="px-1 pb-1">
              <div class="flex items-center gap-1">
                <Input
                  v-model="newDbName"
                  placeholder="database_name"
                  class="h-7 text-xs bg-muted/50 border-none flex-1"
                  autofocus
                  @keyup.enter="createDatabase(connId as string)"
                  @keyup.escape="
                    showNewDb = null;
                    newDbName = '';
                  "
                />
                <button
                  class="flex items-center justify-center size-7 rounded-md bg-primary text-primary-foreground hover:bg-primary/90 transition-colors shrink-0"
                  :disabled="isCreatingDb || !newDbName.trim()"
                  @click="createDatabase(connId as string)"
                >
                  <CheckIcon class="size-3.5" />
                </button>
                <button
                  class="flex items-center justify-center size-7 rounded-md hover:bg-muted/60 transition-colors text-muted-foreground shrink-0"
                  @click="
                    showNewDb = null;
                    newDbName = '';
                  "
                >
                  <XIcon class="size-3.5" />
                </button>
              </div>
            </div>

            <!-- Add DB button -->
            <button
              v-if="showNewDb !== connId"
              class="w-full flex items-center gap-2 px-2 py-1 rounded text-[10px] text-muted-foreground/50 hover:text-muted-foreground transition-colors"
              @click="
                showNewDb = connId as string;
                newDbName = '';
              "
            >
              <PlusIcon class="size-3" />
              New database
            </button>

            <!-- Database rows -->
            <div v-for="db in connState.databases" :key="db">
              <div
                class="flex items-center gap-1 group/db rounded-md hover:bg-muted/30 transition-colors"
              >
                <button
                  class="flex-1 flex items-center gap-2 px-2 py-1.5 min-w-0"
                  @click="toggleDatabase(connId as string, db)"
                >
                  <ChevronDownIcon
                    v-if="expandedDatabases.has(dbKey(connId as string, db))"
                    class="size-3 text-muted-foreground shrink-0"
                  />
                  <ChevronRightIcon
                    v-else
                    class="size-3 text-muted-foreground/40 shrink-0"
                  />
                  <DatabaseIcon
                    class="size-3 shrink-0 text-muted-foreground/60"
                  />
                  <span class="text-xs truncate flex-1 text-left font-medium">{{
                    db
                  }}</span>
                </button>
                <!-- Import / Export / Query icons -->
                <div
                  class="flex items-center gap-0.5 opacity-0 group-hover/db:opacity-100 transition-opacity shrink-0 mr-1"
                >
                  <button
                    class="size-5 flex items-center justify-center rounded text-muted-foreground/50 hover:text-primary hover:bg-primary/10 transition-colors"
                    title="New Query"
                    @click.stop="openQueryTab(connId as string, db)"
                  >
                    <TerminalIcon class="size-3" />
                  </button>
                  <button
                    class="size-5 flex items-center justify-center rounded text-muted-foreground/50 hover:text-foreground hover:bg-muted/60 transition-colors"
                    title="Import SQL"
                    @click.stop="importSql(connId as string, db)"
                  >
                    <UploadIcon class="size-3" />
                  </button>
                  <button
                    class="size-5 flex items-center justify-center rounded text-muted-foreground/50 hover:text-foreground hover:bg-muted/60 transition-colors"
                    title="Export database"
                    @click.stop="openExportSelector(connId as string, db)"
                  >
                    <DownloadIcon class="size-3" />
                  </button>
                </div>
              </div>

              <!-- Tables -->
              <div
                v-if="expandedDatabases.has(dbKey(connId as string, db))"
                class="ml-4 space-y-0.5 mt-0.5"
              >
                <button
                  v-for="table in filteredTables(connId as string, db)"
                  :key="table.name"
                  @click="loadTableData(table.name, connId as string, db)"
                  @contextmenu="
                    openSidebarTableContextMenu(
                      $event,
                      connId as string,
                      db,
                      table.name,
                    )
                  "
                  :class="[
                    'w-full flex items-center gap-2 px-2 py-1.5 rounded-md text-xs transition-all text-left group/tbl',
                    isTableActiveInAnyPane(table.name, db, connId as string)
                      ? 'bg-primary text-primary-foreground shadow-sm'
                      : 'hover:bg-primary/5 text-foreground',
                  ]"
                >
                  <TableIcon
                    :class="[
                      'size-3 shrink-0',
                      isTableActiveInAnyPane(table.name, db, connId as string)
                        ? 'text-primary-foreground/70'
                        : 'text-muted-foreground group-hover/tbl:text-primary',
                    ]"
                  />
                  <span class="flex-1 truncate">{{ table.name }}</span>
                  <span
                    v-if="
                      isTableOpenInAnyPane(table.name, db, connId as string)
                    "
                    class="size-1.5 rounded-full shrink-0"
                    :class="
                      isTableActiveInAnyPane(table.name, db, connId as string)
                        ? 'bg-primary-foreground/60'
                        : 'bg-primary/40'
                    "
                  />
                </button>

                <div
                  v-if="
                    filteredTables(connId as string, db).length === 0 && search
                  "
                  class="px-2 py-1 text-[10px] text-muted-foreground/40 italic"
                >
                  No matches
                </div>
              </div>
            </div>
          </div>
        </div>

        <!-- Saved but closed connections -->
        <template v-if="closedConnections.length > 0">
          <div
            class="border-t border-muted/30 my-2"
            v-if="Object.keys(store.openConnections).length > 0"
          ></div>
          <div class="px-2 py-1.5 mb-0.5">
            <span
              class="text-[10px] font-bold text-muted-foreground/50 uppercase tracking-widest"
              >Saved</span
            >
          </div>
          <div
            v-for="conn in closedConnections"
            :key="conn.id"
            class="flex items-center gap-1 group rounded-md hover:bg-muted/30 transition-colors mb-0.5 pr-1 select-none"
            @dblclick="connectSaved(conn)"
            @contextmenu="openSidebarContextMenu($event, conn)"
          >
            <div class="flex-1 flex items-center gap-2 px-2 py-1.5 min-w-0">
              <div
                class="size-1.5 rounded-full bg-muted-foreground/20 shrink-0"
              ></div>
              <DatabaseIcon
                class="size-3.5 shrink-0 text-muted-foreground/40"
              />
              <span class="text-xs truncate flex-1 text-muted-foreground/70">{{
                conn.name
              }}</span>
              <Badge
                variant="outline"
                :class="[
                  getEnvColor(conn.environment),
                  'text-[9px] uppercase py-0 px-1 h-3.5 shrink-0',
                ]"
              >
                {{ conn.environment }}
              </Badge>
            </div>
          </div>
        </template>
      </ScrollArea>
    </aside>

    <!-- Panes container -->
    <div
      ref="panesContainer"
      class="flex-1 flex min-w-0 overflow-hidden"
      :class="{ 'select-none': draggingPaneIdx !== null }"
    >
      <template v-for="(pane, paneIdx) in panes" :key="pane.id">
        <!-- Resize handle between panes -->
        <div
          v-if="paneIdx > 0"
          class="w-px bg-border hover:w-1 hover:bg-primary/40 cursor-col-resize shrink-0 transition-all z-10"
          :class="{ 'w-1 bg-primary/50': draggingPaneIdx === paneIdx - 1 }"
          @mousedown.prevent="startResize($event, paneIdx - 1)"
        />

        <!-- Pane -->
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
          <div
            v-if="
              pane.tabs.length > 0 ||
              Object.keys(store.openConnections).length > 0
            "
            class="flex items-end border-b bg-muted/5 overflow-x-auto shrink-0 h-10"
          >
            <button
              v-for="tab in pane.tabs"
              :key="tab.id"
              @click="switchToTab(tab.id, pane.id)"
              :class="[
                'flex items-center gap-2 px-3 h-full border-r transition-colors min-w-0 max-w-[220px] group/tab relative shrink-0',
                tab.id === pane.activeTabId
                  ? 'bg-background text-foreground shadow-[inset_0_2px_0_0] shadow-primary'
                  : 'bg-transparent text-muted-foreground hover:bg-muted/30 hover:text-foreground',
              ]"
              :title="
                tab.type === 'query'
                  ? `Query · ${store.openConnections[tab.connectionId]?.connection.name}`
                  : `${(tab as any).tableName} · ${store.openConnections[tab.connectionId]?.connection.name} · ${(tab as any).database}`
              "
            >
              <TerminalIcon
                v-if="tab.type === 'query'"
                class="size-3 shrink-0 opacity-60 mt-0.5"
              />
              <TableIcon v-else class="size-3 shrink-0 opacity-60 mt-0.5" />
              <div class="flex flex-col items-start min-w-0 flex-1">
                <div class="flex items-center gap-1.5 min-w-0">
                  <span class="text-sm font-semibold truncate leading-tight">{{
                    tab.type === "query" ? "Query" : (tab as any).tableName
                  }}</span>
                </div>
                <span
                  class="text-[9px] font-medium truncate leading-tight opacity-50"
                >
                  {{ store.openConnections[tab.connectionId]?.connection.name
                  }}<template v-if="tab.database">
                    · {{ tab.database }}</template
                  >
                </span>
              </div>
              <span
                @click.stop="closeTab(tab.id, pane.id, $event)"
                :class="[
                  'shrink-0 size-3.5 flex items-center justify-center rounded transition-all hover:text-destructive',
                  tab.id === pane.activeTabId
                    ? 'opacity-40 hover:opacity-100'
                    : 'opacity-0 group-hover/tab:opacity-40 group-hover/tab:hover:opacity-100',
                ]"
              >
                <XIcon class="size-3" />
              </span>
            </button>

            <!-- New Query button -->
            <button
              v-if="Object.keys(store.openConnections).length > 0"
              @click="
                openQueryTab(
                  Object.keys(store.openConnections)[0],
                  null,
                  pane.id,
                )
              "
              class="flex items-center gap-1 px-3 h-full text-[11px] text-muted-foreground/50 hover:text-muted-foreground hover:bg-muted/20 transition-colors shrink-0 border-r"
              title="New Query"
            >
              <TerminalIcon class="size-3" />
              <PlusIcon class="size-2.5" />
            </button>

            <div class="flex-1" />

            <!-- Table controls (Content/Structure + Filter + Refresh) -->
            <template v-if="getPaneTab(pane)">
              <div class="flex items-center gap-1 px-2 border-r h-full">
                <button
                  type="button"
                  class="size-6 flex items-center justify-center rounded border transition-colors"
                  :class="
                    pane.showFilters
                      ? 'bg-primary/10 text-primary border-primary/20'
                      : 'text-muted-foreground border-transparent hover:border-border hover:bg-muted/30'
                  "
                  title="Toggle Filters"
                  @click="pane.showFilters = !pane.showFilters"
                >
                  <FilterIcon class="size-3" />
                </button>
                <button
                  type="button"
                  class="size-6 flex items-center justify-center rounded border border-transparent text-muted-foreground hover:border-border hover:bg-muted/30 transition-colors"
                  title="Refresh"
                  @click="refreshActiveTab(pane.id)"
                >
                  <RefreshCwIcon class="size-3" />
                </button>
              </div>
            </template>

            <!-- Split pane button (only on last pane) -->
            <button
              v-if="paneIdx === panes.length - 1"
              @click="addPane"
              class="flex items-center gap-1 px-3 h-full text-[11px] text-muted-foreground/40 hover:text-muted-foreground hover:bg-muted/20 transition-colors shrink-0"
              title="Split pane"
            >
              <PanelRightOpenIcon class="size-3.5" />
            </button>

            <!-- Close pane button (only when multiple panes) -->
            <button
              v-if="panes.length > 1"
              @click="removePane(pane.id)"
              class="flex items-center gap-1 px-3 h-full text-[11px] text-muted-foreground/40 hover:text-destructive hover:bg-muted/20 transition-colors shrink-0 border-l"
              title="Close pane"
            >
              <XIcon class="size-3.5" />
            </button>
          </div>

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
              class="flex-1 min-h-0"
              style="display: flex"
            />
          </div>

          <!-- Table content -->
          <template v-if="getPaneTab(pane) && !isPaneActiveTabQuery(pane)">
            <!-- Filters -->
            <FilterBar
              v-show="pane.viewMode === 'content' && pane.showFilters"
              :columns="getPaneTab(pane)?.queryResult?.columns ?? []"
              @apply="
                async (filters) => {
                  const t = getPaneTab(pane);
                  if (!t) return;
                  t.selectedRowPk = null;
                  t.inlineEditColumn = null;
                  syncStoreForFetch(t.connectionId, t.database);
                  await store.fetchTableData(
                    t.tableName,
                    pane.page,
                    pane.pageSize,
                    filters,
                    sortPayload(t),
                  );
                  t.filters = filters;
                  t.queryResult = store.queryResult;
                }
              "
              @clear="
                async () => {
                  const t = getPaneTab(pane);
                  if (!t) return;
                  t.selectedRowPk = null;
                  t.inlineEditColumn = null;
                  syncStoreForFetch(t.connectionId, t.database);
                  await store.fetchTableData(
                    t.tableName,
                    pane.page,
                    pane.pageSize,
                    null,
                    sortPayload(t),
                  );
                  t.filters = null;
                  t.queryResult = store.queryResult;
                }
              "
            />

            <!-- Structure View -->
            <div
              v-if="pane.viewMode === 'structure'"
              class="flex flex-col flex-1 min-h-0"
            >
              <!-- Columns -->
              <ScrollArea class="flex-1 min-h-0 bg-muted/5">
                <table class="w-max min-w-full border-collapse">
                  <thead>
                    <tr>
                      <th
                        class="sticky top-0 z-20 bg-background/95 backdrop-blur-md px-4 py-3 border-b border-r text-left text-[10px] font-black uppercase tracking-widest whitespace-nowrap"
                      >
                        Field
                      </th>
                      <th
                        class="sticky top-0 z-20 bg-background/95 backdrop-blur-md px-4 py-3 border-b border-r text-left text-[10px] font-black uppercase tracking-widest whitespace-nowrap"
                      >
                        Type
                      </th>
                      <th
                        class="sticky top-0 z-20 bg-background/95 backdrop-blur-md px-4 py-3 border-b border-r text-left text-[10px] font-black uppercase tracking-widest whitespace-nowrap"
                      >
                        Nullable
                      </th>
                      <th
                        class="sticky top-0 z-20 bg-background/95 backdrop-blur-md px-4 py-3 border-b border-r text-left text-[10px] font-black uppercase tracking-widest whitespace-nowrap"
                      >
                        Key
                      </th>
                      <th
                        class="sticky top-0 z-20 bg-background/95 backdrop-blur-md px-4 py-3 border-b border-r text-left text-[10px] font-black uppercase tracking-widest whitespace-nowrap"
                      >
                        Default
                      </th>
                      <th
                        class="sticky top-0 z-20 bg-background/95 backdrop-blur-md px-4 py-3 border-b border-r text-left text-[10px] font-black uppercase tracking-widest whitespace-nowrap"
                      >
                        Extra
                      </th>
                      <th
                        class="sticky top-0 z-20 bg-background/95 backdrop-blur-md px-4 py-3 border-b text-left text-[10px] font-black uppercase tracking-widest whitespace-nowrap"
                      >
                        Relations
                      </th>
                    </tr>
                  </thead>
                  <tbody>
                    <tr
                      v-for="(col, idx) in getPaneTab(pane)?.tableStructure"
                      :key="(col as any).field"
                      class="hover:bg-primary/5 transition-colors"
                      :class="
                        idx % 2 === 0 ? 'bg-background/30' : 'bg-transparent'
                      "
                    >
                      <td
                        class="px-4 py-3 border-b border-r font-mono text-sm font-semibold text-foreground whitespace-nowrap"
                      >
                        <div class="flex items-center gap-2">
                          <span
                            v-if="(col as any).key === 'PRI'"
                            class="text-[9px] font-black uppercase text-amber-400 bg-amber-400/10 px-1.5 py-0.5 rounded"
                            >PK</span
                          >
                          <span
                            v-else-if="(col as any).key === 'UNI'"
                            class="text-[9px] font-black uppercase text-blue-400 bg-blue-400/10 px-1.5 py-0.5 rounded"
                            >UNI</span
                          >
                          <span
                            v-else-if="(col as any).key === 'MUL'"
                            class="text-[9px] font-black uppercase text-purple-400 bg-purple-400/10 px-1.5 py-0.5 rounded"
                            >IDX</span
                          >
                          {{ (col as any).field }}
                        </div>
                      </td>
                      <td
                        class="px-4 py-3 border-b border-r font-mono text-xs text-primary whitespace-nowrap"
                      >
                        {{ (col as any).field_type }}
                      </td>
                      <td class="px-4 py-3 border-b border-r text-sm">
                        <span
                          v-if="(col as any).nullable"
                          class="text-[10px] font-bold text-green-400 uppercase"
                          >YES</span
                        >
                        <span
                          v-else
                          class="text-[10px] font-bold text-muted-foreground/50 uppercase"
                          >NO</span
                        >
                      </td>
                      <td
                        class="px-4 py-3 border-b border-r text-xs text-muted-foreground whitespace-nowrap"
                      >
                        {{ (col as any).key || "—" }}
                      </td>
                      <td
                        class="px-4 py-3 border-b border-r text-xs font-mono text-muted-foreground whitespace-nowrap"
                      >
                        <span
                          v-if="(col as any).default_value === null"
                          class="italic opacity-40"
                          >NULL</span
                        >
                        <span v-else>{{ (col as any).default_value }}</span>
                      </td>
                      <td
                        class="px-4 py-3 border-b border-r text-xs text-muted-foreground whitespace-nowrap"
                      >
                        {{ (col as any).extra || "—" }}
                      </td>
                      <td class="px-4 py-3 border-b text-xs whitespace-nowrap">
                        <span
                          v-if="getFkMap(pane)[(col as any).field]"
                          class="flex items-center gap-1 text-primary/70 font-mono"
                        >
                          <ArrowRightIcon class="size-3 shrink-0" />
                          {{ getFkMap(pane)[(col as any).field].table }}.{{
                            getFkMap(pane)[(col as any).field].column
                          }}
                        </span>
                        <span v-else class="text-muted-foreground/30">—</span>
                      </td>
                    </tr>
                  </tbody>
                </table>
              </ScrollArea>

              <!-- Resize handle -->
              <div
                class="shrink-0 h-1 border-t cursor-row-resize hover:bg-primary/40 transition-colors bg-transparent"
                @mousedown="startStructureResize($event, pane.id)"
              />

              <!-- Indexes -->
              <div
                class="shrink-0 flex flex-col"
                :style="{
                  height:
                    (structureIndexHeights[pane.id] ?? 0) > 0
                      ? structureIndexHeights[pane.id] + 'px'
                      : '40%',
                }"
              >
                <div
                  class="px-4 py-2 bg-muted/20 border-b flex items-center gap-2 shrink-0"
                >
                  <KeyRoundIcon class="size-3.5 text-muted-foreground/60" />
                  <span
                    class="text-[10px] font-black uppercase tracking-widest text-muted-foreground/60"
                    >Indexes</span
                  >
                </div>
                <div class="flex-1 min-h-0 overflow-auto">
                  <table
                    v-if="getPaneTab(pane)?.tableIndexes?.length"
                    class="w-max min-w-full border-collapse"
                  >
                    <thead>
                      <tr>
                        <th
                          class="sticky top-0 z-20 bg-background/95 backdrop-blur-md px-4 py-3 border-b border-r text-left text-[10px] font-black uppercase tracking-widest whitespace-nowrap"
                        >
                          Name
                        </th>
                        <th
                          class="sticky top-0 z-20 bg-background/95 backdrop-blur-md px-4 py-3 border-b border-r text-left text-[10px] font-black uppercase tracking-widest whitespace-nowrap"
                        >
                          Algorithm
                        </th>
                        <th
                          class="sticky top-0 z-20 bg-background/95 backdrop-blur-md px-4 py-3 border-b border-r text-left text-[10px] font-black uppercase tracking-widest whitespace-nowrap"
                        >
                          Unique
                        </th>
                        <th
                          class="sticky top-0 z-20 bg-background/95 backdrop-blur-md px-4 py-3 border-b border-r text-left text-[10px] font-black uppercase tracking-widest whitespace-nowrap"
                        >
                          Columns
                        </th>
                        <th
                          class="sticky top-0 z-20 bg-background/95 backdrop-blur-md px-4 py-3 border-b text-left text-[10px] font-black uppercase tracking-widest whitespace-nowrap"
                        >
                          Comment
                        </th>
                      </tr>
                    </thead>
                    <tbody>
                      <template
                        v-for="(group, keyName) in Object.groupBy(
                          getPaneTab(pane)?.tableIndexes ?? [],
                          (i: any) => i.key_name,
                        )"
                        :key="keyName"
                      >
                        <tr class="hover:bg-primary/5 transition-colors">
                          <td
                            class="px-4 py-3 border-b border-r font-mono text-sm font-semibold text-foreground whitespace-nowrap"
                          >
                            <div class="flex items-center gap-2">
                              <span
                                v-if="keyName === 'PRIMARY'"
                                class="text-[9px] font-black uppercase text-amber-400 bg-amber-400/10 px-1.5 py-0.5 rounded"
                                >PK</span
                              >
                              <KeyRoundIcon
                                v-else
                                class="size-3 text-muted-foreground/50"
                              />
                              {{ keyName }}
                            </div>
                          </td>
                          <td
                            class="px-4 py-3 border-b border-r text-xs font-mono text-primary whitespace-nowrap"
                          >
                            {{ (group as any[])[0]?.index_type }}
                          </td>
                          <td
                            class="px-4 py-3 border-b border-r text-sm whitespace-nowrap"
                          >
                            <span
                              v-if="!(group as any[])[0]?.non_unique"
                              class="text-[10px] font-bold text-green-400 uppercase"
                              >YES</span
                            >
                            <span
                              v-else
                              class="text-[10px] font-bold text-muted-foreground/50 uppercase"
                              >NO</span
                            >
                          </td>
                          <td
                            class="px-4 py-3 border-b border-r text-xs font-mono text-foreground/80 whitespace-nowrap"
                          >
                            {{
                              (group as any[])
                                .sort(
                                  (a: any, b: any) =>
                                    a.seq_in_index - b.seq_in_index,
                                )
                                .map((i: any) => i.column_name)
                                .join(", ")
                            }}
                          </td>
                          <td
                            class="px-4 py-3 border-b text-xs text-muted-foreground whitespace-nowrap"
                          >
                            {{ (group as any[])[0]?.comment || "—" }}
                          </td>
                        </tr>
                      </template>
                    </tbody>
                  </table>
                  <div
                    v-else
                    class="px-4 py-6 text-xs text-muted-foreground/40 italic"
                  >
                    No indexes found
                  </div>
                </div>
              </div>
            </div>

            <!-- Data Table + row detail panel -->
            <div
              v-if="pane.viewMode === 'content'"
              class="flex flex-1 min-h-0 min-w-0 flex-row"
            >
              <ScrollArea class="flex-1 min-w-0 relative bg-muted/5">
                <div
                  v-if="
                    getPaneTab(pane)?.queryResult &&
                    getPaneTab(pane)?.queryResult?.rows?.length === 0
                  "
                  class="absolute inset-0 flex flex-col items-center justify-center p-12 text-center"
                >
                  <DatabaseIcon class="size-10 text-muted-foreground/20 mb-4" />
                  <p class="text-lg font-bold text-foreground">No records</p>
                  <p
                    class="text-sm text-muted-foreground/60 max-w-[250px] mt-2"
                  >
                    This table does not contain any data, or your filters didn't
                    match any rows.
                  </p>
                </div>
                <template v-else>
                  <table class="w-max min-w-full border-collapse">
                    <thead>
                      <tr>
                        <th
                          v-for="col in getPaneTab(pane)?.queryResult?.columns"
                          :key="(col as any).name"
                          class="sticky top-0 z-20 bg-background/95 backdrop-blur-md px-4 py-3 border-b border-r last:border-r-0 text-left whitespace-nowrap cursor-pointer hover:bg-muted/40 transition-colors select-none group/sortth relative"
                          :style="
                            getColWidth(pane, (col as any).name)
                              ? {
                                  width:
                                    getColWidth(pane, (col as any).name) + 'px',
                                  minWidth:
                                    getColWidth(pane, (col as any).name) + 'px',
                                }
                              : { minWidth: '180px' }
                          "
                          title="Sort by this column"
                          @click="
                            onSortColumnHeaderClick(pane, (col as any).name)
                          "
                        >
                          <div class="flex items-center justify-between gap-2">
                            <div class="flex items-center gap-1.5 min-w-0">
                              <span
                                class="block text-xs font-semibold font-mono tracking-normal text-foreground truncate"
                                >{{ (col as any).name }}</span
                              >
                              <span
                                v-if="getPrimaryKey(pane) === (col as any).name"
                                class="text-[8px] font-black text-amber-500 border border-amber-500/30 px-1 rounded shrink-0"
                                >PK</span
                              >
                            </div>
                            <span
                              class="shrink-0 flex flex-col items-center justify-center opacity-60 group-hover/sortth:opacity-100"
                            >
                              <ArrowDownIcon
                                v-if="
                                  getPaneTab(pane)?.sortColumn ===
                                    (col as any).name &&
                                  getPaneTab(pane)?.sortDesc
                                "
                                class="size-3.5 text-primary"
                              />
                              <ArrowUpIcon
                                v-else-if="
                                  getPaneTab(pane)?.sortColumn ===
                                  (col as any).name
                                "
                                class="size-3.5 text-primary"
                              />
                              <ArrowUpDownIcon
                                v-else
                                class="size-3.5 text-muted-foreground"
                              />
                            </span>
                          </div>
                          <span
                            class="block text-[9px] font-medium font-mono tracking-normal text-muted-foreground opacity-70 mt-0.5"
                            >{{ (col as any).type_name }}</span
                          >
                          <div
                            class="absolute top-0 right-0 h-full w-1.5 cursor-col-resize z-30 opacity-0 group-hover/sortth:opacity-100 hover:bg-primary/40 transition-colors"
                            @mousedown="
                              startColResize($event, pane, (col as any).name)
                            "
                            @click.stop
                            title=""
                          />
                        </th>
                      </tr>
                    </thead>
                    <tbody>
                      <tr
                        v-for="(row, idx) in getPaneTab(pane)?.queryResult
                          ?.rows"
                        :key="idx"
                        class="hover:bg-primary/5 transition-colors group/row"
                        :class="[
                          (idx as number) % 2 === 0
                            ? 'bg-background/30'
                            : 'bg-transparent',
                          getPaneTab(pane)?.pendingTruncate
                            ? 'bg-destructive/20 opacity-70 grayscale'
                            : '',
                          getPaneTab(pane)?.pendingDeletions[
                            String((row as any)[getPrimaryKey(pane) || ''])
                          ]
                            ? 'bg-destructive/20 text-destructive line-through'
                            : '',
                          getPrimaryKey(pane) &&
                          getPaneTab(pane)?.selectedRowPk ===
                            String((row as any)[getPrimaryKey(pane)!])
                            ? '!bg-primary/10 ring-1 ring-inset ring-primary/25'
                            : '',
                          getPrimaryKey(pane) ? 'cursor-pointer' : '',
                        ]"
                        @click="onTableRowClick(pane, row, $event)"
                      >
                        <td
                          v-for="col in getPaneTab(pane)?.queryResult?.columns"
                          :key="(col as any).name"
                          class="px-4 py-3 text-sm font-medium border-b border-r last:border-r-0 relative group/cell overflow-hidden"
                          :style="
                            getColWidth(pane, (col as any).name)
                              ? {
                                  width:
                                    getColWidth(pane, (col as any).name) + 'px',
                                  maxWidth:
                                    getColWidth(pane, (col as any).name) + 'px',
                                }
                              : { maxWidth: '300px' }
                          "
                          :class="[
                            getPaneTab(pane)?.pendingChanges[
                              String((row as any)[getPrimaryKey(pane) || ''])
                            ]?.[(col as any).name] !== undefined
                              ? 'bg-amber-500/10 border-amber-500/30'
                              : '',
                            getPaneTab(pane)?.pendingDeletions[
                              String((row as any)[getPrimaryKey(pane) || ''])
                            ]
                              ? 'border-destructive/20'
                              : '',
                          ]"
                          @dblclick.stop="
                            getPrimaryKey(pane) &&
                            !getPaneTab(pane)?.pendingDeletions[
                              String((row as any)[getPrimaryKey(pane) || ''])
                            ] &&
                            startInlineCellEdit(pane, row, (col as any).name)
                          "
                        >
                          <template
                            v-if="
                              getPrimaryKey(pane) &&
                              isInlineEditingCell(pane, row, (col as any).name)
                            "
                          >
                            <input
                              :data-grid-edit="
                                String((row as any)[getPrimaryKey(pane)!])
                              "
                              :data-col="(col as any).name"
                              :value="
                                cellEditValue(pane, row, (col as any).name)
                              "
                              @input="
                                (e) =>
                                  updatePendingChange(
                                    pane,
                                    row,
                                    (col as any).name,
                                    (e.target as HTMLInputElement).value,
                                  )
                              "
                              @blur="onGridCellBlur(pane)"
                              class="bg-background/90 border border-primary/35 rounded px-2 py-1 text-sm font-medium focus:outline-none focus:ring-1 focus:ring-ring w-full min-w-0"
                              :class="
                                getPaneTab(pane)?.pendingDeletions[
                                  String(
                                    (row as any)[getPrimaryKey(pane) || ''],
                                  )
                                ]
                                  ? 'text-destructive'
                                  : 'text-foreground'
                              "
                              @click.stop
                            />
                          </template>
                          <template
                            v-else-if="
                              (row as any)[(col as any).name] === null &&
                              getPaneTab(pane)?.pendingChanges[
                                String((row as any)[getPrimaryKey(pane) || ''])
                              ]?.[(col as any).name] === undefined
                            "
                          >
                            <div
                              class="flex items-center gap-1.5 min-w-0 h-full"
                            >
                              <span
                                class="text-[10px] italic font-normal tracking-wide shrink-0"
                                :class="
                                  getPaneTab(pane)?.pendingDeletions[
                                    String(
                                      (row as any)[getPrimaryKey(pane) || ''],
                                    )
                                  ]
                                    ? 'text-destructive/50'
                                    : 'text-muted-foreground/30'
                                "
                                >NULL</span
                              >
                              <button
                                v-if="
                                  getFkMap(pane)[(col as any).name] &&
                                  (row as any)[(col as any).name] != null
                                "
                                type="button"
                                @click.stop="
                                  navigateToRelated(
                                    pane,
                                    getFkMap(pane)[(col as any).name].table,
                                    getFkMap(pane)[(col as any).name].column,
                                    (row as any)[(col as any).name],
                                  )
                                "
                                class="shrink-0 text-white/60 hover:text-white transition-colors"
                                :title="`Go to ${getFkMap(pane)[(col as any).name].table}`"
                              >
                                <ArrowRightIcon class="size-3" />
                              </button>
                            </div>
                          </template>
                          <template v-else>
                            <div
                              class="flex items-center gap-1.5 min-w-0 h-full"
                            >
                              <span
                                class="truncate text-sm font-medium select-none min-w-0"
                                :class="
                                  getPaneTab(pane)?.pendingDeletions[
                                    String(
                                      (row as any)[getPrimaryKey(pane) || ''],
                                    )
                                  ]
                                    ? 'text-destructive font-bold'
                                    : 'text-foreground/80'
                                "
                                >{{
                                  cellEditValue(pane, row, (col as any).name)
                                }}</span
                              >
                              <button
                                v-if="
                                  getFkMap(pane)[(col as any).name] &&
                                  (row as any)[(col as any).name] != null
                                "
                                type="button"
                                @click.stop="
                                  navigateToRelated(
                                    pane,
                                    getFkMap(pane)[(col as any).name].table,
                                    getFkMap(pane)[(col as any).name].column,
                                    (row as any)[(col as any).name],
                                  )
                                "
                                class="shrink-0 text-white/60 hover:text-white transition-colors"
                                :title="`Go to ${getFkMap(pane)[(col as any).name].table} where ${getFkMap(pane)[(col as any).name].column} = ${(row as any)[(col as any).name]}`"
                              >
                                <ArrowRightIcon class="size-3" />
                              </button>
                            </div>
                          </template>
                          <div
                            v-if="
                              getPaneTab(pane)?.pendingChanges[
                                String((row as any)[getPrimaryKey(pane) || ''])
                              ]?.[(col as any).name] !== undefined
                            "
                            class="absolute top-0 right-0 w-1.5 h-1.5 bg-amber-500 rounded-bl-full"
                          />
                        </td>
                      </tr>
                      <tr
                        v-if="insertingRowPaneId === pane.id"
                        class="bg-emerald-500/10 ring-1 ring-inset ring-emerald-500/20"
                      >
                        <td
                          v-for="col in getPaneTab(pane)?.queryResult?.columns"
                          :key="(col as any).name"
                          class="px-1 py-1 border-b border-r last:border-r-0"
                          :style="getColWidth(pane, (col as any).name) ? { width: getColWidth(pane, (col as any).name) + 'px', maxWidth: getColWidth(pane, (col as any).name) + 'px' } : { maxWidth: '300px' }"
                        >
                          <span
                            v-if="isColAutoIncrement(pane, (col as any).name)"
                            class="px-3 text-xs text-muted-foreground italic"
                          >auto</span>
                          <input
                            v-else
                            v-model="insertRowValues[(col as any).name]"
                            :placeholder="isBooleanCol(pane, (col as any).name) ? '0 / 1' : ''"
                            class="insert-row-input w-full h-7 px-3 text-sm bg-transparent focus:outline-none focus:ring-1 focus:ring-emerald-500/50 rounded"
                            @keydown.enter="submitInsertRow(pane)"
                            @keydown.escape="cancelInsertRow"
                          />
                        </td>
                      </tr>
                    </tbody>
                  </table>
                  <div
                    v-if="!getPrimaryKey(pane)"
                    class="p-8 text-center bg-muted/20 border-t border-dashed mt-auto"
                  >
                    <p class="text-sm text-muted-foreground italic">
                      Edition is disabled because this table has no Primary Key.
                    </p>
                  </div>
                </template>
              </ScrollArea>

              <aside
                v-if="getPrimaryKey(pane) && getSelectedRow(pane)"
                data-row-detail-panel
                class="shrink-0 border-l border-border bg-card flex flex-col min-h-0 min-w-0 relative"
                :style="{ width: (sidePanelWidths[pane.id] ?? 320) + 'px' }"
              >
                <!-- Resize handle -->
                <div
                  class="absolute left-0 top-0 h-full w-1 cursor-col-resize hover:bg-primary/40 transition-colors z-10"
                  @mousedown="startSidePanelResize($event, pane.id)"
                />

                <div
                  class="h-11 shrink-0 border-b flex items-center justify-between gap-2 px-3 bg-muted/25"
                >
                  <div class="min-w-0">
                    <p
                      class="text-[10px] font-bold text-muted-foreground uppercase tracking-wider truncate"
                    >
                      Selected row
                    </p>
                    <p
                      class="text-[11px] font-mono font-semibold text-foreground truncate"
                    >
                      {{ getPrimaryKey(pane) }} =
                      {{ getPaneTab(pane)?.selectedRowPk }}
                    </p>
                  </div>
                  <button
                    type="button"
                    class="size-8 shrink-0 flex items-center justify-center rounded-md text-muted-foreground hover:text-foreground hover:bg-muted/60 transition-colors"
                    title="Close panel"
                    @click="clearRowSelection(pane)"
                  >
                    <XIcon class="size-4" />
                  </button>
                </div>

                <!-- Field search -->
                <div class="px-3 py-2 border-b shrink-0">
                  <div class="relative">
                    <SearchIcon
                      class="absolute left-2.5 top-1/2 -translate-y-1/2 size-3 text-muted-foreground/50"
                    />
                    <input
                      type="text"
                      placeholder="Filter fields..."
                      :value="sidePanelSearch[pane.id] ?? ''"
                      @input="
                        (e) =>
                          (sidePanelSearch[pane.id] = (
                            e.target as HTMLInputElement
                          ).value)
                      "
                      class="w-full bg-muted/30 border border-input rounded-md pl-7 pr-3 py-1.5 text-xs focus:outline-none focus:ring-1 focus:ring-ring"
                    />
                  </div>
                </div>

                <ScrollArea class="flex-1 min-h-0">
                  <div class="p-3 space-y-3.5 pb-6">
                    <div
                      v-for="col in getPaneTab(
                        pane,
                      )?.queryResult?.columns.filter(
                        (c: any) =>
                          !sidePanelSearch[pane.id] ||
                          c.name
                            .toLowerCase()
                            .includes(sidePanelSearch[pane.id].toLowerCase()),
                      )"
                      :key="'detail-' + (col as any).name"
                      class="space-y-1"
                    >
                      <div class="flex items-center justify-between gap-1">
                        <Label
                          class="text-xs font-bold text-foreground uppercase tracking-wide"
                          >{{ (col as any).name }}</Label
                        >
                        <button
                          type="button"
                          class="size-5 shrink-0 flex items-center justify-center rounded transition-colors"
                          :class="
                            copiedField === `${pane.id}:${(col as any).name}`
                              ? 'text-green-400'
                              : 'text-muted-foreground/30 hover:text-foreground'
                          "
                          title="Copy value"
                          @click="
                            copyFieldValue(
                              pane.id,
                              (col as any).name,
                              cellEditValue(
                                pane,
                                getSelectedRow(pane)!,
                                (col as any).name,
                              ),
                            )
                          "
                        >
                          <CheckIcon
                            v-if="
                              copiedField === `${pane.id}:${(col as any).name}`
                            "
                            class="size-3"
                          />
                          <CopyIcon v-else class="size-3" />
                        </button>
                      </div>
                      <div class="flex items-start gap-1.5">
                        <textarea
                          :ref="
                            (el) =>
                              autoResizeTextarea(el as HTMLTextAreaElement)
                          "
                          rows="1"
                          class="flex-1 min-w-0 rounded-md border border-input bg-background px-3 py-1.5 text-xs font-mono resize-none overflow-hidden leading-relaxed focus:outline-none focus:ring-1 focus:ring-ring disabled:opacity-50 disabled:cursor-not-allowed"
                          :disabled="
                            !!getPaneTab(pane)?.pendingDeletions[
                              String(
                                (getSelectedRow(pane) as any)[
                                  getPrimaryKey(pane)!
                                ],
                              )
                            ]
                          "
                          :value="
                            cellEditValue(
                              pane,
                              getSelectedRow(pane)!,
                              (col as any).name,
                            )
                          "
                          @input="
                            (e) => {
                              const t = e.target as HTMLTextAreaElement;
                              t.style.height = 'auto';
                              t.style.height = t.scrollHeight + 'px';
                              updatePendingChange(
                                pane,
                                getSelectedRow(pane)!,
                                (col as any).name,
                                t.value,
                              );
                            }
                          "
                        />
                        <button
                          v-if="getFkMap(pane)[(col as any).name]"
                          type="button"
                          class="mt-1 size-7 shrink-0 flex items-center justify-center rounded-md text-primary/50 hover:text-primary hover:bg-primary/10 transition-colors"
                          :title="`Go to ${getFkMap(pane)[(col as any).name].table}`"
                          @click="
                            navigateToRelated(
                              pane,
                              getFkMap(pane)[(col as any).name].table,
                              getFkMap(pane)[(col as any).name].column,
                              (getSelectedRow(pane) as any)[(col as any).name],
                            )
                          "
                        >
                          <ArrowRightIcon class="size-3.5" />
                        </button>
                      </div>
                      <p class="text-[10px] text-foreground/60 font-mono">
                        {{ (col as any).type_name }}
                      </p>
                    </div>
                  </div>
                </ScrollArea>
              </aside>
            </div>

            <!-- Pending Changes Bar (only for active pane) -->
            <div
              v-if="
                hasPendingChangesInPane(pane) &&
                (panes.length === 1 || pane.id === activePaneId)
              "
              class="fixed bottom-6 left-1/2 -translate-x-1/2 z-50 flex items-center gap-6 bg-card border border-primary/20 shadow-2xl rounded-full px-6 py-3 animate-in fade-in slide-in-from-bottom-4 duration-300"
            >
              <div class="flex items-center gap-3">
                <div
                  class="size-2 rounded-full bg-amber-500 animate-pulse"
                ></div>
                <span
                  class="text-xs font-bold uppercase tracking-widest text-foreground"
                >
                  <template v-if="getPaneTab(pane)?.pendingTruncate">
                    Entire Table marked for Truncate
                  </template>
                  <template v-else>
                    {{
                      Object.keys(getPaneTab(pane)?.pendingChanges || {}).length
                    }}
                    Updates &amp;
                    {{
                      Object.keys(getPaneTab(pane)?.pendingDeletions || {})
                        .length
                    }}
                    Deletions Pending
                  </template>
                </span>
              </div>

              <div class="h-4 w-px bg-border"></div>

              <div class="flex items-center gap-2">
                <div class="flex items-center gap-2 mr-2">
                  <label class="flex items-center gap-2 cursor-pointer group">
                    <input
                      type="checkbox"
                      v-model="disableFkChecks"
                      class="size-3.5 rounded border-input accent-primary"
                    />
                    <span
                      class="text-[10px] font-bold text-muted-foreground group-hover:text-foreground transition-colors uppercase tracking-tight"
                      >Disable FK Checks</span
                    >
                  </label>
                </div>
                <Button
                  variant="ghost"
                  size="sm"
                  class="h-8 text-xs font-bold uppercase tracking-tight"
                  @click="discardChanges(pane)"
                  >Discard</Button
                >
                <Button
                  size="sm"
                  class="h-8 px-4 text-xs font-bold uppercase tracking-tight shadow-lg"
                  :disabled="isSaving"
                  @click="applyChanges(pane)"
                >
                  {{ isSaving ? "Saving..." : "Apply Changes" }}
                </Button>
              </div>
            </div>

            <!-- Pagination Footer -->
            <footer
              v-if="getPaneTab(pane) && !isPaneActiveTabQuery(pane)"
              class="h-12 border-t flex items-center justify-between px-6 bg-background shrink-0"
            >
              <div class="flex items-center gap-2">
                <div class="flex items-center rounded border bg-muted/30 p-0.5 gap-0.5">
                  <button
                    type="button"
                    @click="setViewMode(pane, 'content')"
                    :class="[
                      'flex items-center gap-1 px-2 h-6 rounded text-[10px] font-bold uppercase tracking-wider transition-all',
                      pane.viewMode === 'content'
                        ? 'bg-background text-foreground shadow-sm'
                        : 'text-muted-foreground hover:text-foreground',
                    ]"
                  >
                    <LayoutListIcon class="size-3" /> Data
                  </button>
                  <button
                    type="button"
                    @click="setViewMode(pane, 'structure')"
                    :class="[
                      'flex items-center gap-1 px-2 h-6 rounded text-[10px] font-bold uppercase tracking-wider transition-all',
                      pane.viewMode === 'structure'
                        ? 'bg-background text-foreground shadow-sm'
                        : 'text-muted-foreground hover:text-foreground',
                    ]"
                  >
                    <TablePropertiesIcon class="size-3" /> Structure
                  </button>
                </div>
                <button
                  type="button"
                  :class="[
                    'size-6 flex items-center justify-center rounded border transition-colors',
                    insertingRowPaneId === pane.id
                      ? 'bg-emerald-500/15 border-emerald-500/40 text-emerald-500'
                      : 'border-transparent text-muted-foreground hover:border-border hover:bg-muted/30 hover:text-foreground'
                  ]"
                  title="Insert new row"
                  @click="openInsertRowDialog(pane)"
                >
                  <PlusIcon class="size-3.5" />
                </button>
              </div>
              <div v-if="insertingRowPaneId === pane.id" class="flex items-center gap-2">
                <span v-if="insertRowError" class="text-[10px] text-destructive max-w-xs truncate cursor-help" :title="insertRowError">{{ insertRowError }}</span>
                <span v-else class="text-[10px] text-muted-foreground">Enter · Esc to cancel</span>
                <Button size="sm" class="h-6 text-[10px] px-3 bg-emerald-600 hover:bg-emerald-700" :disabled="insertRowLoading" @click="submitInsertRow(pane)">
                  {{ insertRowLoading ? '...' : 'Insert' }}
                </Button>
                <Button size="sm" variant="ghost" class="h-6 text-[10px] px-2" @click="cancelInsertRow">Cancel</Button>
              </div>
              <div v-else-if="pane.viewMode === 'content'" class="flex items-center gap-3">
                <div
                  class="text-[11px] font-bold text-muted-foreground uppercase tracking-wider"
                >
                  {{ pane.page * pane.pageSize + 1 }} -
                  {{
                    Math.min(
                      (pane.page + 1) * pane.pageSize,
                      getPaneTab(pane)?.queryResult?.total_count || 0,
                    )
                  }}
                  of {{ getPaneTab(pane)?.queryResult?.total_count }} rows
                </div>
                <div class="flex items-center gap-3">
                <div class="flex items-center gap-1.5">
                  <span class="text-[10px] font-bold text-muted-foreground uppercase tracking-wider">Limit</span>
                  <input
                    type="number"
                    :value="pane.pageSize"
                    min="1"
                    class="h-6 w-16 rounded border border-input bg-transparent px-2 text-[11px] font-bold text-center focus:outline-none focus:ring-1 focus:ring-ring [appearance:textfield] [&::-webkit-outer-spin-button]:appearance-none [&::-webkit-inner-spin-button]:appearance-none"
                    @change="changeLimit(pane, +($event.target as HTMLInputElement).value)"
                  />
                </div>
                <div class="flex items-center gap-1.5">
                  <span class="text-[10px] font-bold text-muted-foreground uppercase tracking-wider">Offset</span>
                  <input
                    type="number"
                    :value="pane.page * pane.pageSize"
                    min="0"
                    class="h-6 w-16 rounded border border-input bg-transparent px-2 text-[11px] font-bold text-center focus:outline-none focus:ring-1 focus:ring-ring [appearance:textfield] [&::-webkit-outer-spin-button]:appearance-none [&::-webkit-inner-spin-button]:appearance-none"
                    @change="gotoOffset(pane, +($event.target as HTMLInputElement).value)"
                  />
                </div>
                <div class="flex items-center gap-1">
                  <Button
                    variant="ghost"
                    size="sm"
                    class="h-7 w-7 p-0"
                    :disabled="pane.page === 0"
                    @click="changePage(pane, -1)"
                  >
                    <ChevronLeftIcon class="size-4" />
                  </Button>
                  <div class="text-[10px] font-bold px-1">
                    {{ pane.page + 1 }}
                  </div>
                  <Button
                    variant="ghost"
                    size="sm"
                    class="h-7 w-7 p-0"
                    :disabled="(pane.page + 1) * pane.pageSize >= (getPaneTab(pane)?.queryResult?.total_count || 0)"
                    @click="changePage(pane, +1)"
                  >
                    <ChevronRightIcon class="size-4" />
                  </Button>
                </div>
              </div>
              </div>
            </footer>
          </template>

          <!-- Empty State -->
          <div
            v-else-if="!isPaneActiveTabQuery(pane)"
            class="flex-1 flex flex-col items-center justify-center p-12 text-center bg-muted/5"
          >
            <div
              class="size-24 rounded-full bg-muted/20 flex items-center justify-center mb-8 border border-dashed border-muted-foreground/20"
            >
              <TableIcon class="size-10 text-muted-foreground/30" />
            </div>
            <h2 class="text-2xl font-bold tracking-tight mb-3">
              Table Explorer
            </h2>
            <p class="text-muted-foreground max-w-sm mb-8 leading-relaxed">
              {{
                Object.keys(store.openConnections).length === 0
                  ? "Connect to a database from the sidebar to get started."
                  : "Expand a database in the sidebar and select a table to browse."
              }}
            </p>
          </div>
        </div>
        <!-- end pane -->
      </template>
    </div>
    <!-- end panes container -->

    <!-- Table Selection Dialog (Export) -->
    <Dialog
      :open="showTableSelector"
      @update:open="(val: boolean) => !val && (showTableSelector = false)"
    >
      <DialogContent
        class="sm:max-w-md max-h-[85vh] flex flex-col p-0 overflow-hidden shadow-2xl border-primary/10"
      >
        <DialogHeader class="p-6 pb-4 bg-background border-b relative z-20">
          <DialogTitle>Database Export</DialogTitle>
          <DialogDescription>
            Configure your export for
            <span class="font-bold text-foreground"
              >`{{ exportContext?.database }}`</span
            >
          </DialogDescription>
        </DialogHeader>

        <ScrollArea class="flex-1 bg-background">
          <div class="px-6 py-4 border-b bg-muted/20">
            <h3
              class="text-[10px] font-black uppercase tracking-widest text-muted-foreground mb-3"
            >
              Export Mode
            </h3>
            <div class="grid grid-cols-3 gap-2">
              <button
                v-for="opt in dbExportOptions"
                :key="opt.mode"
                @click="currentExportMode = opt.mode"
                :class="[
                  'flex flex-col items-center gap-1 p-2.5 rounded-xl border transition-all',
                  currentExportMode === opt.mode
                    ? 'bg-primary border-primary text-primary-foreground shadow-lg shadow-primary/20'
                    : 'bg-background hover:border-primary/50 text-muted-foreground',
                ]"
              >
                <span class="text-xs font-bold">{{ opt.label }}</span>
                <span class="text-[9px] opacity-70 text-center">{{
                  opt.desc
                }}</span>
              </button>
            </div>
          </div>

          <div class="px-6 py-4">
            <div
              class="flex items-center justify-between mb-3 sticky top-0 bg-background py-1 z-10 border-b"
            >
              <h3
                class="text-[10px] font-black uppercase tracking-widest text-muted-foreground"
              >
                Tables Selection
              </h3>
              <div class="flex items-center gap-3">
                <span class="text-[10px] font-bold text-muted-foreground"
                  >{{ selectedExportTables.length }} selected</span
                >
                <button
                  @click="toggleAllTables"
                  class="text-[10px] font-black text-primary uppercase hover:underline"
                >
                  {{
                    selectedExportTables.length === exportContextTables.length
                      ? "None"
                      : "All"
                  }}
                </button>
              </div>
            </div>

            <div class="grid grid-cols-1 gap-1">
              <label
                v-for="table in exportContextTables"
                :key="(table as any).name"
                class="flex items-center gap-3 p-2 rounded-lg hover:bg-muted/50 cursor-pointer transition-colors group border border-transparent"
              >
                <input
                  type="checkbox"
                  :value="(table as any).name"
                  v-model="selectedExportTables"
                  class="size-4 rounded border-muted accent-primary cursor-pointer"
                />
                <TableIcon
                  class="size-3.5 text-muted-foreground group-hover:text-primary transition-colors"
                />
                <span class="text-sm font-medium truncate">{{
                  (table as any).name
                }}</span>
              </label>
            </div>
          </div>
        </ScrollArea>

        <div
          class="p-6 py-4 border-t bg-muted/10 flex flex-row items-center justify-between gap-4"
        >
          <Button
            variant="ghost"
            class="text-xs font-bold uppercase tracking-wider h-9"
            @click="showTableSelector = false"
            >Cancel</Button
          >
          <Button
            class="font-bold px-8 shadow-lg shadow-primary/30 h-10"
            :disabled="selectedExportTables.length === 0"
            @click="startExport"
          >
            Start {{ currentExportMode.toUpperCase() }} Export
          </Button>
        </div>
      </DialogContent>
    </Dialog>


    <!-- Import Progress Dialog -->
    <Dialog :open="isImporting">
      <DialogContent class="sm:max-w-md" :hide-close="true">
        <DialogHeader>
          <DialogTitle>Importing SQL</DialogTitle>
          <DialogDescription
            >Please wait while the SQL file is being
            imported.</DialogDescription
          >
        </DialogHeader>
        <div class="py-6">
          <div
            class="flex items-center justify-between mb-2 text-xs font-bold uppercase tracking-widest text-muted-foreground"
          >
            <span>{{ importProgress.status }}</span>
            <span v-if="importProgress.total"
              >{{
                Math.round(
                  (importProgress.current / importProgress.total) * 100,
                )
              }}%</span
            >
          </div>
          <div class="h-2 w-full bg-muted rounded-full overflow-hidden">
            <div
              class="h-full bg-primary transition-all duration-300 ease-out"
              :style="{
                width: `${importProgress.total ? (importProgress.current / importProgress.total) * 100 : 0}%`,
              }"
            ></div>
          </div>
          <div class="mt-2 text-[10px] text-muted-foreground text-center">
            {{ importProgress.current }} / {{ importProgress.total }} statements
          </div>
        </div>
      </DialogContent>
    </Dialog>

    <!-- Import Result Dialog -->
    <Dialog
      :open="!!importResult"
      @update:open="(val: boolean) => !val && (importResult = null)"
    >
      <DialogContent class="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle
            :class="
              importResult?.errors.length
                ? 'text-destructive'
                : 'text-green-500'
            "
          >
            {{
              importResult?.errors.length
                ? "Import finished with errors"
                : "Import successful"
            }}
          </DialogTitle>
          <DialogDescription>
            {{ importResult?.executed }} statements executed successfully.
          </DialogDescription>
        </DialogHeader>
        <ScrollArea
          v-if="importResult?.errors.length"
          class="mt-4 max-h-[300px] rounded-md border bg-muted/30 p-4"
        >
          <div
            class="text-[10px] font-black uppercase tracking-widest text-destructive mb-2"
          >
            Error Log:
          </div>
          <div
            v-for="(err, i) in importResult.errors"
            :key="i"
            class="text-xs font-mono mb-2 last:mb-0 break-all border-b border-muted last:border-0 pb-2"
          >
            {{ err }}
          </div>
        </ScrollArea>
        <div class="flex justify-end mt-4">
          <Button @click="importResult = null">Close</Button>
        </div>
      </DialogContent>
    </Dialog>

    <!-- Export Progress Dialog -->
    <Dialog :open="isExportingDb">
      <DialogContent class="sm:max-w-md" :hide-close="true">
        <DialogHeader>
          <DialogTitle>Exporting Data</DialogTitle>
          <DialogDescription
            >Please wait while the data is being exported.</DialogDescription
          >
        </DialogHeader>
        <div class="py-6">
          <div
            class="flex items-center justify-between mb-2 text-xs font-bold uppercase tracking-widest text-muted-foreground"
          >
            <span>{{ exportProgress.status }}</span>
            <span v-if="exportProgress.total"
              >{{
                Math.round(
                  (exportProgress.current / exportProgress.total) * 100,
                )
              }}%</span
            >
          </div>
          <div class="h-2 w-full bg-muted rounded-full overflow-hidden">
            <div
              class="h-full bg-primary transition-all duration-300 ease-out"
              :style="{
                width: `${exportProgress.total ? (exportProgress.current / exportProgress.total) * 100 : 0}%`,
              }"
            ></div>
          </div>
        </div>
      </DialogContent>
    </Dialog>

    <!-- Export Result Dialog -->
    <Dialog
      :open="!!exportResult"
      @update:open="(val: boolean) => !val && (exportResult = null)"
    >
      <DialogContent class="sm:max-w-md">
        <DialogHeader>
          <DialogTitle
            :class="
              exportResult?.success ? 'text-green-500' : 'text-destructive'
            "
          >
            {{ exportResult?.success ? "Export successful" : "Export failed" }}
          </DialogTitle>
          <DialogDescription>{{ exportResult?.message }}</DialogDescription>
        </DialogHeader>
        <div class="flex justify-end mt-4">
          <Button @click="exportResult = null">Close</Button>
        </div>
      </DialogContent>
    </Dialog>

    <!-- New Connection Dialog -->
    <Dialog
      :open="showNewConnDialog"
      @update:open="(val: boolean) => !val && (showNewConnDialog = false)"
    >
      <DialogContent class="sm:max-w-lg overflow-y-auto max-h-[90vh]">
        <DialogHeader>
          <DialogTitle>{{
            store.connections.some((c) => c.id === newConn.id)
              ? "Edit Connection"
              : "New Connection"
          }}</DialogTitle>
          <DialogDescription>
            {{
              store.connections.some((c) => c.id === newConn.id)
                ? "Update your connection settings"
                : "Configure your MySQL connection settings"
            }}
          </DialogDescription>
        </DialogHeader>

        <div class="space-y-5 py-2">
          <div class="grid grid-cols-2 gap-4">
            <div class="space-y-2">
              <Label>Connection Name</Label>
              <Input v-model="newConn.name" placeholder="Local Development" />
            </div>
            <div class="space-y-2">
              <Label>Environment</Label>
              <select
                v-model="newConn.environment"
                class="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
              >
                <option value="LOCAL">Local</option>
                <option value="DEV">Development</option>
                <option value="STAGING">Staging</option>
                <option value="PRODUCTION">Production</option>
              </select>
            </div>
          </div>

          <Separator />

          <div class="space-y-4">
            <div
              class="flex items-center gap-2 text-xs font-bold text-muted-foreground uppercase tracking-wider"
            >
              <HardDriveIcon class="size-3.5" /> MySQL Settings
            </div>
            <div class="grid grid-cols-12 gap-3">
              <div class="col-span-8 space-y-2">
                <Label>Host</Label>
                <Input v-model="newConn.mysql.host" placeholder="127.0.0.1" />
              </div>
              <div class="col-span-4 space-y-2">
                <Label>Port</Label>
                <Input v-model.number="newConn.mysql.port" type="number" />
              </div>
            </div>
            <div class="grid grid-cols-2 gap-3">
              <div class="space-y-2">
                <Label>User</Label>
                <Input v-model="newConn.mysql.user" placeholder="root" />
              </div>
              <div class="space-y-2">
                <Label>Password</Label>
                <Input
                  v-model="newConn.mysql.password"
                  type="password"
                  placeholder="••••••••"
                />
              </div>
            </div>
            <div class="space-y-2">
              <Label
                >Database
                <span class="text-muted-foreground font-normal"
                  >(optional)</span
                ></Label
              >
              <Input
                v-model="newConn.mysql.database"
                placeholder="Leave blank to pick after connecting"
              />
            </div>
          </div>

          <Separator />

          <!-- SSH Tunnel -->
          <div class="space-y-4">
            <label class="flex items-center gap-3 cursor-pointer select-none">
              <div
                @click="sshEnabled = !sshEnabled"
                :class="[
                  'relative w-9 h-5 rounded-full transition-colors shrink-0',
                  sshEnabled ? 'bg-primary' : 'bg-muted',
                ]"
              >
                <div
                  :class="[
                    'absolute top-0.5 left-0.5 size-4 rounded-full bg-white shadow transition-transform',
                    sshEnabled ? 'translate-x-4' : 'translate-x-0',
                  ]"
                />
              </div>
              <div
                class="flex items-center gap-2 text-xs font-bold text-muted-foreground uppercase tracking-wider"
              >
                <ShieldCheckIcon class="size-3.5" /> SSH Tunnel
              </div>
            </label>

            <div v-if="sshEnabled" class="space-y-3 pl-1">
              <div class="grid grid-cols-12 gap-3">
                <div class="col-span-8 space-y-2">
                  <Label>SSH Host</Label>
                  <Input
                    v-model="sshForm.host"
                    placeholder="bastion.example.com"
                  />
                </div>
                <div class="col-span-4 space-y-2">
                  <Label>Port</Label>
                  <Input v-model.number="sshForm.port" type="number" />
                </div>
              </div>
              <div class="space-y-2">
                <Label>SSH User</Label>
                <Input v-model="sshForm.user" placeholder="ubuntu" />
              </div>

              <div class="flex gap-2 pt-1">
                <button
                  @click="sshAuthType = 'password'"
                  :class="[
                    'flex-1 h-8 rounded-md text-xs font-bold border transition-all',
                    sshAuthType === 'password'
                      ? 'bg-primary text-primary-foreground border-primary'
                      : 'bg-transparent text-muted-foreground border-input hover:border-primary/50',
                  ]"
                >
                  Password
                </button>
                <button
                  @click="sshAuthType = 'key'"
                  :class="[
                    'flex-1 h-8 rounded-md text-xs font-bold border transition-all',
                    sshAuthType === 'key'
                      ? 'bg-primary text-primary-foreground border-primary'
                      : 'bg-transparent text-muted-foreground border-input hover:border-primary/50',
                  ]"
                >
                  SSH Key
                </button>
              </div>

              <div v-if="sshAuthType === 'password'" class="space-y-2">
                <Label>SSH Password</Label>
                <Input
                  v-model="sshForm.password"
                  type="password"
                  placeholder="••••••••"
                />
              </div>

              <div v-if="sshAuthType === 'key'" class="space-y-3">
                <div class="space-y-2">
                  <Label>Private Key Path</Label>
                  <Input
                    v-model="sshForm.private_key_path"
                    placeholder="~/.ssh/id_rsa"
                  />
                </div>
                <div class="space-y-2">
                  <Label
                    >Passphrase
                    <span class="text-muted-foreground font-normal"
                      >(optional)</span
                    ></Label
                  >
                  <Input
                    v-model="sshForm.passphrase"
                    type="password"
                    placeholder="••••••••"
                  />
                </div>
              </div>
            </div>
          </div>
        </div>

        <div
          v-if="testConnResult"
          :class="[
            'text-xs px-3 py-2 rounded-md font-medium',
            testConnResult.ok
              ? 'bg-green-500/10 text-green-500'
              : 'bg-destructive/10 text-destructive',
          ]"
        >
          {{ testConnResult.msg }}
        </div>

        <div class="flex items-center justify-between pt-4 border-t">
          <Button variant="ghost" @click="showNewConnDialog = false"
            >Cancel</Button
          >
          <div class="flex gap-2">
            <Button
              variant="outline"
              :disabled="isTestingConn || isSavingConn"
              @click="testNewConn"
            >
              {{ isTestingConn ? "Testing..." : "Test" }}
            </Button>
            <Button
              variant="outline"
              :disabled="isSavingConn"
              @click="saveNewConn(false)"
              >Save only</Button
            >
            <Button
              :disabled="isSavingConn || !newConn.name"
              @click="saveNewConn(true)"
              >Save &amp; Connect</Button
            >
          </div>
        </div>
      </DialogContent>
    </Dialog>

    <!-- Delete Connection Confirmation Dialog -->
    <Dialog
      :open="showDeleteConnDialog"
      @update:open="(val: boolean) => !val && (showDeleteConnDialog = false)"
    >
      <DialogContent class="sm:max-w-[400px]">
        <DialogHeader>
          <DialogTitle>Delete Connection</DialogTitle>
          <DialogDescription>
            Are you sure you want to delete this connection? This action cannot
            be undone.
          </DialogDescription>
        </DialogHeader>
        <DialogFooter class="gap-2 sm:gap-0">
          <Button variant="ghost" @click="showDeleteConnDialog = false"
            >Cancel</Button
          >
          <Button variant="destructive" @click="deleteConn">Delete</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>

    <!-- Table Action Dialog (Truncate / Drop from sidebar) -->
    <Dialog
      :open="showTableActionDialog"
      @update:open="
        (val: boolean) =>
          !val && !isExecutingTableAction && (showTableActionDialog = false)
      "
    >
      <DialogContent class="sm:max-w-[420px]">
        <DialogHeader>
          <DialogTitle class="flex items-center gap-2">
            <Trash2Icon
              v-if="tableActionData?.type === 'truncate'"
              class="size-5 text-destructive"
            />
            <XIcon v-else class="size-5 text-destructive" />
            {{ tableActionData?.type === "truncate" ? "Truncate" : "Drop" }}
            Table
          </DialogTitle>
          <DialogDescription class="pt-2">
            Are you sure you want to
            <strong class="text-foreground">{{
              tableActionData?.type === "truncate" ? "TRUNCATE" : "DROP"
            }}</strong>
            table
            <code
              class="bg-muted px-1.5 py-0.5 rounded font-bold text-foreground"
              >{{ tableActionData?.tableName }}</code
            >?<br />
            <span
              v-if="tableActionData?.type === 'truncate'"
              class="text-muted-foreground mt-2 block"
              >All data will be lost. This cannot be undone.</span
            >
            <span v-else class="text-muted-foreground mt-2 block"
              >This table and all its data will be permanently deleted.</span
            >
          </DialogDescription>
        </DialogHeader>

        <div class="py-4">
          <label
            class="flex items-start gap-3 cursor-pointer group bg-muted/20 p-3 rounded-lg border border-border hover:bg-muted/40 transition-colors"
          >
            <div class="mt-0.5 shrink-0">
              <input
                type="checkbox"
                v-model="tableActionDisableFk"
                class="size-4 rounded border-input accent-destructive cursor-pointer"
              />
            </div>
            <div class="flex flex-col min-w-0">
              <span class="text-sm font-bold text-foreground transition-colors"
                >Disable Foreign Key Checks</span
              >
              <span class="text-xs text-muted-foreground leading-relaxed mt-0.5"
                >Allows truncating/dropping tables referenced by other tables.
                Use with caution.</span
              >
            </div>
          </label>
        </div>

        <DialogFooter class="gap-2 sm:gap-0 mt-2">
          <Button
            variant="ghost"
            @click="showTableActionDialog = false"
            :disabled="isExecutingTableAction"
            >Cancel</Button
          >
          <Button
            variant="destructive"
            @click="executeTableAction"
            :disabled="isExecutingTableAction"
            class="min-w-[100px] font-bold"
          >
            {{ isExecutingTableAction ? "Executing..." : "Confirm" }}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>

    <!-- Connection Context Menu -->
    <div
      v-if="sidebarContextMenu.show"
      class="fixed z-[100] min-w-[160px] bg-background/95 backdrop-blur-md border rounded-lg shadow-xl p-1 animate-in fade-in zoom-in-95 duration-100"
      :style="{
        left: sidebarContextMenu.x + 'px',
        top: sidebarContextMenu.y + 'px',
      }"
    >
      <button
        class="w-full flex items-center gap-2 px-3 py-2 text-xs font-medium rounded-md hover:bg-muted transition-colors text-left"
        @click="handleEditConnection(sidebarContextMenu.connection!)"
      >
        <PencilIcon class="size-3.5 text-muted-foreground" /> Edit Connection
      </button>
      <button
        class="w-full flex items-center gap-2 px-3 py-2 text-xs font-medium rounded-md hover:bg-muted transition-colors text-left"
        @click="handleDuplicateConnection(sidebarContextMenu.connection!)"
      >
        <CopyIcon class="size-3.5 text-muted-foreground" /> Duplicate Connection
      </button>
      <button
        v-if="store.openConnections[sidebarContextMenu.connection!.id]"
        class="w-full flex items-center gap-2 px-3 py-2 text-xs font-medium rounded-md hover:bg-destructive/10 text-destructive transition-colors text-left"
        @click="disconnectConn(sidebarContextMenu.connection!.id)"
      >
        <XIcon class="size-3.5" /> Disconnect
      </button>
      <div class="h-px bg-border my-1"></div>
      <button
        class="w-full flex items-center gap-2 px-3 py-2 text-xs font-medium rounded-md text-destructive hover:bg-destructive/10 transition-colors text-left"
        @click="confirmDeleteConn(sidebarContextMenu.connection!.id)"
      >
        <Trash2Icon class="size-3.5" /> Delete Connection
      </button>
    </div>

    <!-- Table Context Menu -->
    <div
      v-if="sidebarTableContextMenu.show"
      class="fixed z-[100] min-w-[160px] bg-background/95 backdrop-blur-md border rounded-lg shadow-xl p-1 animate-in fade-in zoom-in-95 duration-100"
      :style="{
        left: sidebarTableContextMenu.x + 'px',
        top: sidebarTableContextMenu.y + 'px',
      }"
    >
      <div class="px-2 py-1 mb-1 border-b">
        <span
          class="text-[10px] font-semibold font-mono tracking-normal text-muted-foreground truncate block"
        >
          {{ sidebarTableContextMenu.tableName }}
        </span>
      </div>
      <button
        class="w-full flex items-center gap-2 px-3 py-2 text-xs font-medium rounded-md hover:bg-muted transition-colors text-left"
        @click="confirmSidebarTruncateTable"
      >
        <Trash2Icon class="size-3.5 text-muted-foreground" /> Truncate Table
      </button>
      <button
        class="w-full flex items-center gap-2 px-3 py-2 text-xs font-medium rounded-md hover:bg-muted transition-colors text-left"
        @click="confirmSidebarDropTable"
      >
        <XIcon class="size-3.5 text-muted-foreground" /> Drop Table
      </button>
    </div>
  </div>
</template>

<style scoped>
tr:nth-child(even) {
  background: transparent;
}
</style>
