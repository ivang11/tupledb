import { provideWorkspaceConnectionPanelContext } from "@/composables/useWorkspaceConnectionPanelContext";
import { provideWorkspaceDialogsContext } from "@/composables/useWorkspaceDialogsContext";
import { provideWorkspacePaneContext } from "@/composables/useWorkspacePaneContext";

export function useWorkspaceViewContexts(parts: Record<string, any>) {
  const context = {
    ...parts.workspace,
    ...parts.panelResizing,
    ...parts.tableTabs,
    ...parts.rowEditing,
    ...parts.rowContext,
    ...parts.sidebarManager,
    ...parts.tableUi,
    ...parts.coordinator,
    ...parts.connectionState,
    sidebarRef: parts.sidebarRef,
    sidebarVisible: parts.sidebarState.sidebarVisible,
    sidebarToggleVisible: parts.sidebarState.sidebarToggleVisible,
  };

  provideWorkspacePaneContext(context);
  provideWorkspaceDialogsContext(context);
  provideWorkspaceConnectionPanelContext(context);
}
