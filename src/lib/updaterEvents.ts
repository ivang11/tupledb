export const UPDATER_REQUEST = 'tupledb:updater-request'
export const UPDATER_STATE = 'tupledb:updater-state'
export type UpdaterAction = 'sync' | 'check' | 'install' | 'restart'
export interface UpdaterState {
  currentVersion: string
  availableVersion: string | null
  isChecking: boolean
  isInstalling: boolean
  isUpToDate: boolean
  status: 'idle' | 'available' | 'downloading' | 'installing' | 'readyToRestart' | 'restarting'
  downloadedBytes: number
  totalBytes: number
  downloadPercent: number
  errorMessage: string | null
}
