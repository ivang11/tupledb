import { onMounted, onUnmounted } from 'vue'

export function useKeyboardShortcut(key: string, callback: () => void, ctrlOrMeta = true) {
  const handleKeyDown = (event: KeyboardEvent) => {
    const isModifierPressed = ctrlOrMeta ? (event.ctrlKey || event.metaKey) : true
    
    if (isModifierPressed && event.key.toLowerCase() === key.toLowerCase()) {
      event.preventDefault()
      callback()
    }
  }

  onMounted(() => {
    window.addEventListener('keydown', handleKeyDown)
  })

  onUnmounted(() => {
    window.removeEventListener('keydown', handleKeyDown)
  })
}
