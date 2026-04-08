import { ref, readonly } from 'vue'
import {
  DEFAULT_KEYBINDINGS,
  KEYBINDING_DEFS,
  type KeybindingAction,
} from '@/config/keybindingActions'

const STORAGE_KEY = 'db-viewer:keybindings'

// Singleton state shared across all component instances
const _overrides = ref<Partial<Record<KeybindingAction, string>>>({})
const _bindings = ref<Record<KeybindingAction, string>>({ ...DEFAULT_KEYBINDINGS })

function _loadFromStorage() {
  try {
    const saved = localStorage.getItem(STORAGE_KEY)
    if (saved) {
      const parsed = JSON.parse(saved) as Partial<Record<KeybindingAction, string>>
      _overrides.value = parsed
      _bindings.value = { ...DEFAULT_KEYBINDINGS, ...parsed }
    }
  } catch {
    // ignore, keep defaults
  }
}

function _saveToStorage() {
  if (Object.keys(_overrides.value).length === 0) {
    localStorage.removeItem(STORAGE_KEY)
  } else {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(_overrides.value))
  }
}

// Initialize on module load
_loadFromStorage()

// ── Parsing helpers ────────────────────────────────────────────────────────────

export interface ParsedKeybinding {
  key: string
  ctrl: boolean
  shift: boolean
  alt: boolean
}

export function parseKeybindingString(str: string): ParsedKeybinding {
  const parts = str.split('+')
  const mods = parts.slice(0, -1).map((p) => p.toLowerCase())
  const key = parts[parts.length - 1].toLowerCase()
  return {
    key,
    ctrl: mods.includes('ctrl'),
    shift: mods.includes('shift'),
    alt: mods.includes('alt'),
  }
}

export function formatKeybinding(binding: string): string {
  // Normalizes and returns a display string, e.g. "Ctrl+Shift+F"
  const parsed = parseKeybindingString(binding)
  const parts: string[] = []
  if (parsed.ctrl) parts.push('Ctrl')
  if (parsed.shift) parts.push('Shift')
  if (parsed.alt) parts.push('Alt')
  const key =
    parsed.key.length === 1 ? parsed.key.toUpperCase() : _capitalizeKey(parsed.key)
  parts.push(key)
  return parts.join('+')
}

function _capitalizeKey(key: string): string {
  const keyNames: Record<string, string> = {
    enter: 'Enter',
    escape: 'Escape',
    backspace: 'Backspace',
    delete: 'Delete',
    tab: 'Tab',
    space: 'Space',
    arrowup: 'ArrowUp',
    arrowdown: 'ArrowDown',
    arrowleft: 'ArrowLeft',
    arrowright: 'ArrowRight',
    f1: 'F1', f2: 'F2', f3: 'F3', f4: 'F4', f5: 'F5',
    f6: 'F6', f7: 'F7', f8: 'F8', f9: 'F9', f10: 'F10',
    f11: 'F11', f12: 'F12',
  }
  return keyNames[key] ?? key.charAt(0).toUpperCase() + key.slice(1)
}

export function keybindingMatchesEvent(binding: string, event: KeyboardEvent): boolean {
  const parsed = parseKeybindingString(binding)
  const eventKey = event.key.toLowerCase()
  const ctrlPressed = event.ctrlKey || event.metaKey
  return (
    eventKey === parsed.key &&
    parsed.ctrl === ctrlPressed &&
    parsed.shift === event.shiftKey &&
    parsed.alt === event.altKey
  )
}

/** Convert a binding string to CodeMirror keymap format, e.g. "Ctrl+Shift+F" → "Mod-Shift-f" */
export function keybindingToCodeMirror(binding: string): string {
  const parsed = parseKeybindingString(binding)
  let result = ''
  if (parsed.ctrl) result += 'Mod-'
  if (parsed.shift) result += 'Shift-'
  if (parsed.alt) result += 'Alt-'
  result += _capitalizeKey(parsed.key)
  return result
}

/** Build a binding string from a KeyboardEvent (for recording mode) */
export function keybindingFromEvent(event: KeyboardEvent): string | null {
  const ignoredKeys = ['control', 'meta', 'shift', 'alt', 'os']
  if (ignoredKeys.includes(event.key.toLowerCase())) return null

  const parts: string[] = []
  if (event.ctrlKey || event.metaKey) parts.push('Ctrl')
  if (event.shiftKey) parts.push('Shift')
  if (event.altKey) parts.push('Alt')

  const key = event.key.length === 1 ? event.key.toUpperCase() : event.key
  parts.push(key)
  return parts.join('+')
}

// ── Composable ────────────────────────────────────────────────────────────────

export function useKeybindings() {
  function getBinding(action: KeybindingAction): string {
    return _bindings.value[action]
  }

  function matchesEvent(action: KeybindingAction, event: KeyboardEvent): boolean {
    return keybindingMatchesEvent(_bindings.value[action], event)
  }

  function getCodeMirrorKey(action: KeybindingAction): string {
    return keybindingToCodeMirror(_bindings.value[action])
  }

  function setBinding(action: KeybindingAction, key: string) {
    _overrides.value = { ..._overrides.value, [action]: key }
    _bindings.value = { ...DEFAULT_KEYBINDINGS, ..._overrides.value }
    _saveToStorage()
  }

  function resetBinding(action: KeybindingAction) {
    const next = { ..._overrides.value }
    delete next[action]
    _overrides.value = next
    _bindings.value = { ...DEFAULT_KEYBINDINGS, ..._overrides.value }
    _saveToStorage()
  }

  function resetAll() {
    _overrides.value = {}
    _bindings.value = { ...DEFAULT_KEYBINDINGS }
    localStorage.removeItem(STORAGE_KEY)
  }

  function exportJson(): string {
    return JSON.stringify(_bindings.value, null, 2)
  }

  function importJson(json: string) {
    const parsed = JSON.parse(json) as Partial<Record<KeybindingAction, string>>
    const validActions = Object.keys(DEFAULT_KEYBINDINGS) as KeybindingAction[]
    const filtered: Partial<Record<KeybindingAction, string>> = {}
    for (const action of validActions) {
      if (parsed[action]) filtered[action] = parsed[action]
    }
    _overrides.value = filtered
    _bindings.value = { ...DEFAULT_KEYBINDINGS, ...filtered }
    _saveToStorage()
  }

  return {
    bindings: readonly(_bindings),
    overrides: readonly(_overrides),
    defs: KEYBINDING_DEFS,
    getBinding,
    matchesEvent,
    getCodeMirrorKey,
    setBinding,
    resetBinding,
    resetAll,
    exportJson,
    importJson,
  }
}
