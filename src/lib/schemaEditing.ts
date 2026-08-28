export interface PendingStructureChange {
  newName: string
  newType: string
}

export function stageStructureChange(
  pending: Record<string, PendingStructureChange>,
  originalName: string,
  originalType: string,
  newName: string,
  newType: string,
) {
  const normalizedName = newName.trim()
  const normalizedType = newType.trim()
  if (
    normalizedName === originalName &&
    normalizedType.toLowerCase() === originalType.toLowerCase()
  ) {
    delete pending[originalName]
    return
  }
  pending[originalName] = { newName: normalizedName, newType: normalizedType }
}
