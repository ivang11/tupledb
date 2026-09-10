export type Environment = 'LOCAL' | 'DEV' | 'STAGING' | 'PRODUCTION'

export interface MySqlSettings {
  host: string
  port: number
  user: string
  password?: string
  database?: string
}

export type DatabaseEngine = 'mysql' | 'postgresql' | 'sqlite'

export interface PostgreSqlSettings {
  host: string
  port: number
  user: string
  password?: string
  database?: string
  ssl_mode: 'disable' | 'prefer' | 'require' | 'verify_ca' | 'verify_full'
  ssl_root_cert?: string
}

export interface SqliteSettings {
  path: string
  read_only: boolean
}

export type DatabaseSettings =
  | { engine: 'mysql'; settings: MySqlSettings }
  | { engine: 'postgresql'; settings: PostgreSqlSettings }
  | { engine: 'sqlite'; settings: SqliteSettings }

export interface DatabaseCapabilities {
  schemas: boolean
  createDatabase: boolean
  databaseCollations: boolean
  editRows: boolean
  alterColumns: boolean
  truncateTable: boolean
  disableForeignKeyChecks: boolean
  cancelQuery: boolean
  importSql: boolean
  exportSql: boolean
  inspectDdl?: boolean
  estimatedRowCount: boolean
}

export interface ConnectionInfo {
  engine: DatabaseEngine
  serverVersion: string
  capabilities: DatabaseCapabilities
}

export interface DriverDescriptor {
  engine: DatabaseEngine
  label: string
  defaultPort: number | null
}

export type SshAuth =
  | { type: 'password'; password: string }
  | { type: 'key'; private_key_path: string; passphrase?: string }

export interface SshSettings {
  host: string
  port: number
  user: string
  auth: SshAuth
}

export interface Connection {
  id: string
  name: string
  environment: Environment
  database: DatabaseSettings
  ssh?: SshSettings
  timeout_secs?: number
  allow_writes?: boolean
}
