import { computed, ref } from 'vue'
import { tabHasPendingChanges } from '@/lib/pendingChanges'
import type { AnyTab, TableTab } from '@/types/workspace'

export function useUnsavedChangesGuard() {
  const requests = ref<Array<{ label: string; tabs: TableTab[]; action: () => void }>>([])
  const open = computed(() => requests.value.length > 0)
  function request(tabs: AnyTab[], action: () => void) {
    const pending = tabs.filter((tab): tab is TableTab => tab.type === 'table' && tabHasPendingChanges(tab))
    if (!pending.length) { action(); return }
    requests.value.push({ label: pending.map(tab => tab.tableName).join(', '), tabs: pending, action })
  }
  function discardAndContinue() {
    for (const request of requests.value) for (const tab of request.tabs) {
      tab.pendingChanges = {}; tab.pendingStructureChanges = {}; tab.pendingDeletions = {};
      tab.pendingInserts = []; tab.pendingDrop = false; tab.pendingTruncate = false;
      tab.inlineEditColumn = null;
    }
    const actions = requests.value.map(request => request.action)
    requests.value = []
    actions.forEach(action => action())
  }
  function cancel() { requests.value = [] }
  return { requests, open, request, discardAndContinue, cancel }
}
