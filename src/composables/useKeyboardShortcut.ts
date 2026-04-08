import { onMounted, onUnmounted } from 'vue'
import type { KeybindingAction } from '@/config/keybindingActions'
import { useKeybindings } from './useKeybindings'

/** Shortcut tied to a named action — respects user keybinding configuration. */
export function useActionShortcut(action: KeybindingAction, callback: () => void) {
  const { matchesEvent } = useKeybindings()

  const handleKeyDown = (event: KeyboardEvent) => {
    if (matchesEvent(action, event)) {
      event.preventDefault()
      callback()
    }
  }

  onMounted(() => window.addEventListener('keydown', handleKeyDown))
  onUnmounted(() => window.removeEventListener('keydown', handleKeyDown))
}

/** Legacy raw-key shortcut. Prefer useActionShortcut for new code. */
export function useKeyboardShortcut(key: string, callback: () => void, ctrlOrMeta = true) {
  const handleKeyDown = (event: KeyboardEvent) => {
    const isModifierPressed = ctrlOrMeta ? event.ctrlKey || event.metaKey : true
    if (isModifierPressed && event.key.toLowerCase() === key.toLowerCase()) {
      event.preventDefault()
      callback()
    }
  }

  onMounted(() => window.addEventListener('keydown', handleKeyDown))
  onUnmounted(() => window.removeEventListener('keydown', handleKeyDown))
}
