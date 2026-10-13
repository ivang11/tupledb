import { onMounted, onBeforeUnmount, watch } from 'vue'
import { useAppSettings, fontCss } from './useAppSettings'
export function useAppearance() {
  const { settings } = useAppSettings()
  const media = window.matchMedia('(prefers-color-scheme: dark)')
  function apply() {
    const value = settings.value, root = document.documentElement
    const dark = value.theme === 'dark' || (value.theme === 'system' && media.matches)
    root.dataset.theme = dark ? 'dark' : 'light'; root.classList.toggle('dark', dark)
    root.style.setProperty('--font-sans', value.uiFont === 'system' ? '-apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif' : 'Inter, -apple-system, sans-serif')
    root.style.setProperty('--font-table', fontCss(value.tableFont)); root.style.setProperty('--table-font-size', `${value.tableFontSize}px`)
    window.dispatchEvent(new Event('tupledb:appearance'))
  }
  watch(settings, apply, { deep: true, immediate: true })
  onMounted(() => media.addEventListener('change', apply))
  onBeforeUnmount(() => media.removeEventListener('change', apply))
}
