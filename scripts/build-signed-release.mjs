#!/usr/bin/env node
import { existsSync, readFileSync } from 'node:fs'
import { homedir } from 'node:os'
import { join } from 'node:path'
import { spawnSync } from 'node:child_process'

const defaultKeyPath = join(homedir(), '.config/tupledb/updater.key')
const keyPath = process.env.TAURI_SIGNING_PRIVATE_KEY_PATH || defaultKeyPath
const password = process.env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD ?? ''
const buildScript = process.env.TAURI_BUILD_SCRIPT || 'build:app:ci'

if (!existsSync(keyPath) && !process.env.TAURI_SIGNING_PRIVATE_KEY) {
  console.error(`Error: updater signing key not found at ${keyPath}`)
  console.error('')
  console.error('Create it once with:')
  console.error('  mkdir -p "$HOME/.config/tupledb"')
  console.error('  cp /tmp/tupledb-updater.key "$HOME/.config/tupledb/updater.key"')
  console.error('')
  console.error('Or set TAURI_SIGNING_PRIVATE_KEY_PATH to another key path.')
  process.exit(1)
}

const env = {
  ...process.env,
  TAURI_SIGNING_PRIVATE_KEY: process.env.TAURI_SIGNING_PRIVATE_KEY || readFileSync(keyPath, 'utf8').trim(),
  TAURI_SIGNING_PRIVATE_KEY_PASSWORD: password,
}
delete env.TAURI_SIGNING_PRIVATE_KEY_PATH

const result = spawnSync('npm', ['run', buildScript], {
  env,
  stdio: 'inherit',
})

process.exit(result.status ?? 1)
