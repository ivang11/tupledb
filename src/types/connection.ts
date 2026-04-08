export type Environment = 'LOCAL' | 'DEV' | 'STAGING' | 'PRODUCTION'

export interface MySqlSettings {
  host: string
  port: number
  user: string
  password?: string
  database?: string
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
  mysql: MySqlSettings
  ssh?: SshSettings
  timeout_secs?: number
  allow_writes?: boolean
}
