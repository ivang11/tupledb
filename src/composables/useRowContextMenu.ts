import { ref } from "vue";
import type { PaneState, TableTab } from "@/types/workspace";

interface RowContextMenuContext {
  getPaneTab: (pane: PaneState) => TableTab | null;
  duplicateRow: (pane: PaneState, row: any) => void;
  duplicateSelectedRows: (pane: PaneState) => void;
  toggleDeletion: (pane: PaneState, row: any) => void;
  toggleDeletionSelected: (pane: PaneState) => void;
}

export function useRowContextMenu(ctx: RowContextMenuContext) {
  const rowContextMenu = ref({
    show: false,
    x: 0,
    y: 0,
    pane: null as PaneState | null,
    row: null as any,
  });

  const showDeleteRowDialog = ref(false);
  const deleteRowTarget = ref<{ pane: PaneState; row: any } | null>(null);

  function openRowContextMenu(pane: PaneState, row: any, x: number, y: number) {
    rowContextMenu.value = { show: true, x, y, pane, row };
    const close = () => {
      rowContextMenu.value.show = false;
      window.removeEventListener("click", close);
    };
    setTimeout(() => window.addEventListener("click", close), 0);
  }

  function handleRowContextDelete() {
    rowContextMenu.value.show = false;
    deleteRowTarget.value = { pane: rowContextMenu.value.pane!, row: rowContextMenu.value.row };
    showDeleteRowDialog.value = true;
  }

  function handleRowContextDuplicate() {
    rowContextMenu.value.show = false;
    const pane = rowContextMenu.value.pane!;
    const tab = ctx.getPaneTab(pane);
    if (tab && tab.selectedRowPks.length > 1) {
      ctx.duplicateSelectedRows(pane);
    } else {
      ctx.duplicateRow(pane, rowContextMenu.value.row);
    }
  }

  async function confirmDeleteRow() {
    showDeleteRowDialog.value = false;
    if (!deleteRowTarget.value) return;
    const { pane, row } = deleteRowTarget.value;
    const tab = ctx.getPaneTab(pane);
    if (tab && tab.selectedRowPks.length > 1) {
      ctx.toggleDeletionSelected(pane);
    } else {
      ctx.toggleDeletion(pane, row);
    }
    deleteRowTarget.value = null;
  }

  return {
    rowContextMenu,
    showDeleteRowDialog,
    deleteRowTarget,
    openRowContextMenu,
    handleRowContextDelete,
    handleRowContextDuplicate,
    confirmDeleteRow,
  };
}
