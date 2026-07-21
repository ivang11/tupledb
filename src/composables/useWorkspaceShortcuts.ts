import { useActionShortcut } from "@/composables/useKeyboardShortcut";

export function useWorkspaceShortcuts(ctx: {
  sidebarVisible: { value: boolean };
  activePaneId: { value: string };
  sidebarRef: { value: { focusSearch: () => void } | null };
  getPane: (paneId?: string) => { id: string; activeTabId: string | null };
  toggleFocusPane: (paneId: string) => void;
  closeTab: (tabId: string, paneId?: string) => void;
  refreshActiveTab: (paneId?: string) => void;
}) {
  useActionShortcut("toggleSidebar", () => {
    ctx.sidebarVisible.value = !ctx.sidebarVisible.value;
  });

  useActionShortcut("focusPane", () => {
    ctx.toggleFocusPane(ctx.activePaneId.value);
  });

  useActionShortcut("closeTab", () => {
    const activePane = ctx.getPane(ctx.activePaneId.value);
    if (activePane?.activeTabId) {
      ctx.closeTab(activePane.activeTabId, activePane.id);
    }
  });

  useActionShortcut("sidebarSearch", () => {
    ctx.sidebarRef.value?.focusSearch();
  });

  useActionShortcut("refreshTable", () => {
    ctx.refreshActiveTab(ctx.activePaneId.value);
  });
}
