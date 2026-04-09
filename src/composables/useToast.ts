import { ref } from 'vue'

export interface Toast {
  id: number
  type: 'success' | 'error' | 'info'
  title: string
  message?: string
  duration?: number
}

const toasts = ref<Toast[]>([])
let nextId = 0

export function useToast() {
  function show(toast: Omit<Toast, 'id'>) {
    const id = ++nextId
    const duration = toast.duration ?? (toast.type === 'error' ? 6000 : 3500)
    toasts.value.push({ ...toast, id })
    setTimeout(() => dismiss(id), duration)
    return id
  }

  function dismiss(id: number) {
    const idx = toasts.value.findIndex(t => t.id === id)
    if (idx !== -1) toasts.value.splice(idx, 1)
  }

  function success(title: string, message?: string) {
    return show({ type: 'success', title, message })
  }

  function error(title: string, message?: string) {
    return show({ type: 'error', title, message, duration: 7000 })
  }

  return { toasts, show, dismiss, success, error }
}
