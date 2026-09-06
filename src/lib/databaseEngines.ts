import type { Connection, DatabaseEngine } from '../types/connection'

// Presentation only. The backend registry decides which engines are available
// and each connected adapter reports its actual capabilities.
export const databaseEngines = {
  mysql: { label: 'MySQL', abbreviation: 'My', formatter: 'mysql' },
  postgresql: { label: 'PostgreSQL', abbreviation: 'Pg', formatter: 'postgresql' },
  sqlite: { label: 'SQLite', abbreviation: 'Sl', formatter: 'sqlite' },
} as const satisfies Record<DatabaseEngine, { label: string; abbreviation: string; formatter: string }>

export function connectionAddress(connection: Connection): string {
  const database = connection.database
  if (database.engine === 'sqlite') return database.settings.path
  const { user, host, port } = database.settings
  return `${user}@${host}:${port}`
}
