const isTauriRuntime =
  typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window

const isMacOS =
  typeof navigator !== 'undefined' && /Mac/i.test(navigator.platform)

export const usesNativeMacWindowControls = isTauriRuntime && isMacOS
