<template>
  <ConnectionRail
    v-if="Object.keys(store.openConnections).length > 0"
    :open-connections="store.openConnections"
    :selected-connection-id="selectedSidebarConnectionId"
    @home="goHome"
    @select-database="handleSelectDatabase"
    @context-menu-connection="openSidebarContextMenu"
    @context-menu-database="openSidebarDatabaseContextMenu"
  />

  <Sidebar
    v-show="sidebarVisible && Object.keys(store.openConnections).length > 0"
    ref="sidebarRef"
    :width="sidebarWidth"
    :search="search"
    :selected-connection-id="selectedSidebarConnectionId"
    :open-connections="store.openConnections"
    :is-table-active="isTableActiveInAnyPane"
    :is-table-open="isTableOpenInAnyPane"
    :pending-table-action="pendingTableAction"
    :filtered-tables="filteredTables"
    :is-table-selected="isTableSelected"
    @update:search="search = $event"
    @update:selected-connection-id="selectedSidebarConnectionId = $event"
    @new-connection="openNewConnDialog"
    @new-database="
      (id) => {
        if (!expandedConnections.has(id)) expandedConnections.add(id);
        selectedSidebarConnectionId = id;
        showNewDb = id;
        newDbName = '';
      }
    "
    @connect-saved="connectSaved"
    @select-database="handleSelectDatabase"
    @toggle-database="toggleDatabase"
    @load-table="loadTableData"
    @toggle-table-selection="toggleTableSelection"
    @select-table-range="selectTableRange"
    @clear-table-selection="clearTableSelection"
    @open-query="openQueryTab"
    @import-sql="openImportSelector"
    @export-database="openExportSelector"
    @context-menu-connection="openSidebarContextMenu"
    @context-menu-table="openSidebarTableContextMenu"
    @context-menu-database="openSidebarDatabaseContextMenu"
    @export-connections="exportConnections"
    @import-connections="importConnections"
    @resize-start="startSidebarResize"
  />

  <ConnectionStartScreen
    v-if="showConnectionManager"
    :connections="store.connections"
    :open-connection-ids="openConnectionIds"
    :selected-connection-id="selectedSidebarConnectionId"
    :connecting-id="connectingId"
    :overlay="Object.keys(store.openConnections).length > 0"
    @close="closeConnectionManager"
    @connect-saved="connectFromManager"
    @new-connection="openNewConnDialog"
    @context-menu-connection="openSidebarContextMenu"
    @export-connections="exportConnections"
    @import-connections="importConnections"
  />
</template>

<script setup lang="ts">
import { defineAsyncComponent } from "vue";
import { useConnectionStore } from "@/stores/connections";
import { useWorkspaceConnectionPanelContext } from "@/composables/useWorkspaceConnectionPanelContext";
import ConnectionStartScreen from "@/components/ConnectionStartScreen.vue";

const ConnectionRail = defineAsyncComponent(() => import("@/components/ConnectionRail.vue"));
const Sidebar = defineAsyncComponent(() => import("@/components/Sidebar.vue"));

const store = useConnectionStore();

const {
  sidebarRef,
  sidebarVisible,
  sidebarWidth,
  search,
  selectedSidebarConnectionId,
  expandedConnections,
  showNewDb,
  newDbName,
  connectingId,
  filteredTables,
  openConnectionIds,
  showConnectionManager,
  isTableActiveInAnyPane,
  isTableOpenInAnyPane,
  pendingTableAction,
  isTableSelected,
  goHome,
  handleSelectDatabase,
  openSidebarContextMenu,
  openSidebarDatabaseContextMenu,
  openNewConnDialog,
  connectSaved,
  toggleDatabase,
  loadTableData,
  toggleTableSelection,
  selectTableRange,
  clearTableSelection,
  openQueryTab,
  openImportSelector,
  openExportSelector,
  openSidebarTableContextMenu,
  exportConnections,
  importConnections,
  startSidebarResize,
  closeConnectionManager,
  connectFromManager,
} = useWorkspaceConnectionPanelContext();
</script>
