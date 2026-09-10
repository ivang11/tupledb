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
    normalizeType(normalizedType) === normalizeType(originalType)
  ) {
    delete pending[originalName]
    return
  }
  pending[originalName] = { newName: normalizedName, newType: normalizedType }
}

// SQL keywords are case-insensitive, quoted PostgreSQL type names are not.
export function normalizeType(type: string): string {
  return type.replace(/"(?:[^"]|"")*"|[^"]+/g, part =>
    part.startsWith('"') ? part : part.toLowerCase(),
  )
}
