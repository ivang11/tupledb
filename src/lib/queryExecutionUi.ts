export interface QueryCancelButtonState {
  visible: boolean
  disabled: boolean
  label: string
  canRequestCancel: boolean
}

export function getQueryCancelButtonState(
  isRunning: boolean,
  isCancelling: boolean,
  activeQueryId: string | null,
): QueryCancelButtonState {
  const visible = isRunning
  const canRequestCancel = isRunning && !isCancelling && activeQueryId !== null

  return {
    visible,
    disabled: isCancelling,
    label: isCancelling ? 'Cancelling...' : 'Cancel',
    canRequestCancel,
  }
}

export function shouldSurfaceQueryError(isCancelling: boolean): boolean {
  return !isCancelling
}
