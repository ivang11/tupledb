import { computed, ref, watch, watchEffect, type Ref } from "vue";
import { useConnectionStore } from "@/stores/connections";

interface WorkspaceConnectionStateContext {
  selectedSidebarConnectionId: Ref<string | null>;
  sidebarToggleVisible: Ref<boolean>;
  connectSaved: (connection: any) => Promise<boolean | void>;
  resetWorkspaceState: () => void;
}

export function useWorkspaceConnectionState(ctx: WorkspaceConnectionStateContext) {
  const store = useConnectionStore();

  const openConnectionIds = computed(() => Object.keys(store.openConnections));
  const hasOpenConnections = computed(() => openConnectionIds.value.length > 0);
  const connectionEnvironments = computed(() =>
    Object.fromEntries(
      Object.entries(store.openConnections).map(([id, state]) => [
        id,
        state.connection.environment,
      ]),
    ),
  );
  const activeConnectionState = computed(() =>
    ctx.selectedSidebarConnectionId.value
      ? store.openConnections[ctx.selectedSidebarConnectionId.value]
      : null,
  );
  const showConnectionManager = ref(Object.keys(store.openConnections).length === 0);

  function selectedDatabaseForConnection(connectionId: string) {
    return store.openConnections[connectionId]?.selectedDatabase ?? null;
  }

  function goHome() {
    showConnectionManager.value = true;
  }

  function closeConnectionManager() {
    showConnectionManager.value = false;
    if (!ctx.selectedSidebarConnectionId.value || !store.openConnections[ctx.selectedSidebarConnectionId.value]) {
      ctx.selectedSidebarConnectionId.value = Object.keys(store.openConnections)[0] ?? null;
    }
  }

  async function connectFromManager(conn: any) {
    const connected = await ctx.connectSaved(conn);
    if (connected !== false) {
      showConnectionManager.value = false;
    }
  }

  watchEffect(() => {
    ctx.sidebarToggleVisible.value = !showConnectionManager.value && hasOpenConnections.value;
  });

  watch(
    () => openConnectionIds.value.length,
    (count) => {
      if (count === 0) {
        showConnectionManager.value = true;
        ctx.selectedSidebarConnectionId.value = null;
        ctx.resetWorkspaceState();
      }
    },
  );

  watchEffect(() => {
    if (showConnectionManager.value) return;
    if (!ctx.selectedSidebarConnectionId.value || !store.openConnections[ctx.selectedSidebarConnectionId.value]) {
      const firstId = Object.keys(store.openConnections)[0] ?? null;
      if (firstId) ctx.selectedSidebarConnectionId.value = firstId;
    }
  });

  return {
    openConnectionIds,
    hasOpenConnections,
    connectionEnvironments,
    activeConnectionState,
    showConnectionManager,
    selectedDatabaseForConnection,
    goHome,
    closeConnectionManager,
    connectFromManager,
  };
}
