import { ref, watch } from 'vue'
import { emit, listen } from '@tauri-apps/api/event'
export interface AppSettings {
  theme: 'system' | 'light' | 'dark'; uiFont: string; editorFont: string; tableFont: string; editorFontSize: number; tableFontSize: number;
  density: 'compact' | 'comfortable'; columnWidth: 'narrow' | 'normal' | 'wide' | 'fit'; alternatingRows: boolean;
  restoreSession: boolean; keepAlive: boolean; reconnect: boolean;
  indent: 'spaces' | 'tabs'; tabSize: number; completionKey: 'both' | 'enter' | 'tab'; closeBrackets: boolean; uppercaseKeywords: boolean;
  queryTimeout: number; resultRowLimit: number; queryParameters: boolean;
  csvDelimiter: string; csvQuotes: 'needed' | 'always'; csvLineBreak: '\n' | '\r\n'; csvDecimal: '.' | ','; csvNull: string;
}
export const DEFAULT_SETTINGS: AppSettings = {
  theme: 'system', uiFont: 'system', editorFont: 'system-mono', tableFont: 'system-mono', editorFontSize: 13, tableFontSize: 12,
  density: 'comfortable', columnWidth: 'normal', alternatingRows: true, restoreSession: true, keepAlive: false, reconnect: false,
  indent: 'spaces', tabSize: 2, completionKey: 'both', closeBrackets: true, uppercaseKeywords: false,
  queryTimeout: 300, resultRowLimit: 5000, queryParameters: true, csvDelimiter: ',', csvQuotes: 'needed', csvLineBreak: '\n', csvDecimal: '.', csvNull: '\\N',
}
export const FONT_OPTIONS = [
  { value: 'system-mono', label: 'System monospace', css: 'ui-monospace, "SF Mono", Menlo, Consolas, monospace' },
  { value: 'Menlo', label: 'Menlo', css: 'Menlo, ui-monospace, monospace' },
  { value: 'Monaco', label: 'Monaco', css: 'Monaco, ui-monospace, monospace' },
  { value: 'JetBrains Mono', label: 'JetBrains Mono', css: '"JetBrains Mono", ui-monospace, monospace' },
]
export function fontCss(value: string) { return FONT_OPTIONS.find(font => font.value === value)?.css ?? FONT_OPTIONS[0].css }
function load(): AppSettings {
  try {
    const saved = JSON.parse(localStorage.getItem('tupledb:settings') ?? '{}')
    const result = { ...DEFAULT_SETTINGS }
    for (const key of Object.keys(DEFAULT_SETTINGS) as (keyof AppSettings)[]) {
      const value = saved[key]
      if (typeof value === typeof DEFAULT_SETTINGS[key]) (result as any)[key] = value
    }
    for (const [key, values] of Object.entries({ theme: ['system','light','dark'], density: ['compact','comfortable'], columnWidth: ['narrow','normal','wide','fit'], indent: ['spaces','tabs'], completionKey: ['both','enter','tab'], csvQuotes: ['needed','always'], csvDelimiter: [',',';','\t','|'], csvLineBreak: ['\n','\r\n'], csvDecimal: ['.',','] })) {
      if (!values.includes((result as any)[key])) (result as any)[key] = (DEFAULT_SETTINGS as any)[key]
    }
    result.editorFontSize = Math.min(24, Math.max(10, result.editorFontSize || 13)); result.tableFontSize = Math.min(20, Math.max(10, result.tableFontSize || 12))
    result.tabSize = Math.min(8, Math.max(1, result.tabSize || 2)); result.queryTimeout = Math.min(3600, Math.max(1, result.queryTimeout || 300))
    result.resultRowLimit = Math.min(50000, Math.max(100, result.resultRowLimit || 5000))
    return result
  } catch { return { ...DEFAULT_SETTINGS } }
}
const SETTINGS_KEY = 'tupledb:settings'
const SETTINGS_EVENT = 'tupledb:settings-changed'
const settings = ref<AppSettings>(load())
// Settings are edited in their own window, so every window persists what it
// changes and broadcasts it; the others reload from storage. The last persisted
// snapshot keeps the echo of our own broadcast from re-triggering a write.
let lastPersisted = JSON.stringify(settings.value)
watch(settings, value => {
  const json = JSON.stringify(value)
  if (json === lastPersisted) return
  lastPersisted = json
  try { localStorage.setItem(SETTINGS_KEY, json) } catch {}
  void emit(SETTINGS_EVENT, json).catch(() => {})
}, { deep: true })
void listen<string>(SETTINGS_EVENT, event => {
  if (typeof event.payload !== 'string' || event.payload === lastPersisted) return
  lastPersisted = event.payload
  try { localStorage.setItem(SETTINGS_KEY, event.payload) } catch {}
  settings.value = load()
}).catch(() => {})
export function useAppSettings() { return { settings, reset: () => { settings.value = { ...DEFAULT_SETTINGS } } } }
