import { useAppSettings } from './useAppSettings'
import { computed, onBeforeUnmount, ref, watch } from 'vue'

export interface GridPreferences { hidden: string[]; order: string[]; pinned: string[]; widths: Record<string, number> }
export function useGridPreferences(key: () => string | undefined) {
  const defaults = (): GridPreferences => ({ hidden: [], order: [], pinned: [], widths: {} })
  function load() {
    try {
      const parsed = JSON.parse(localStorage.getItem(`tupledb:grid:${key() ?? 'query'}`) ?? 'null')
      return parsed && Array.isArray(parsed.hidden) && Array.isArray(parsed.order) && Array.isArray(parsed.pinned)
        ? { ...defaults(), ...parsed } as GridPreferences : defaults()
    } catch { return defaults() }
  }
  const preferences = ref(load())
  const { settings } = useAppSettings()
  watch(key, () => { preferences.value = load() })
  let timer: ReturnType<typeof setTimeout> | undefined
  function persist() { try { localStorage.setItem(`tupledb:grid:${key() ?? 'query'}`, JSON.stringify(preferences.value)) } catch {} }
  watch(preferences, () => { clearTimeout(timer); timer = setTimeout(persist, 150) }, { deep: true })
  onBeforeUnmount(() => { clearTimeout(timer); persist() })
  return { preferences, rowHeight: computed(() => Math.max(settings.value.tableFontSize + 8, settings.value.density === 'compact' ? 22 : 26)) }
}
