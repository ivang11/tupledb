#!/usr/bin/env node
import { readFileSync, writeFileSync } from 'node:fs'
import { join, resolve } from 'node:path'
import { spawnSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'

const rootDir = resolve(fileURLToPath(new URL('..', import.meta.url)))
const version = process.argv[2]

function fail(message) {
  console.error(`Error: ${message}`)
  process.exit(1)
}

function readJson(path) {
  return JSON.parse(readFileSync(path, 'utf8'))
}

function writeJson(path, data) {
  writeFileSync(path, `${JSON.stringify(data, null, 2)}\n`)
}

function run(command, args, cwd = rootDir) {
  const result = spawnSync(command, args, {
    cwd,
    encoding: 'utf8',
    stdio: 'inherit',
  })

  if (result.status !== 0) {
    fail(`${command} ${args.join(' ')} failed`)
  }
}

function updateCargoToml(path, nextVersion) {
  const content = readFileSync(path, 'utf8')
  const versionPattern = /^version = "[^"]+"/m

  if (!versionPattern.test(content)) {
    fail(`Could not find package version in ${path}`)
  }

  const updated = content.replace(versionPattern, `version = "${nextVersion}"`)
  writeFileSync(path, updated)
}

if (!version || version === '--help' || version === '-h') {
  console.log(`Usage: npm run version:set -- <version>

Examples:
  npm run version:set -- 0.6.5
  npm run version:set -- 1.0.0
`)
  process.exit(version ? 0 : 1)
}

if (!/^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/.test(version)) {
  fail(`Invalid semver version: ${version}`)
}

const packageJsonPath = join(rootDir, 'package.json')
const packageLockPath = join(rootDir, 'package-lock.json')
const cargoTomlPath = join(rootDir, 'src-tauri/Cargo.toml')
const tauriConfigPath = join(rootDir, 'src-tauri/tauri.conf.json')

const packageJson = readJson(packageJsonPath)
packageJson.version = version
writeJson(packageJsonPath, packageJson)

const packageLock = readJson(packageLockPath)
packageLock.version = version
if (packageLock.packages?.['']) {
  packageLock.packages[''].version = version
}
writeJson(packageLockPath, packageLock)

const tauriConfig = readJson(tauriConfigPath)
tauriConfig.version = version
writeJson(tauriConfigPath, tauriConfig)

updateCargoToml(cargoTomlPath, version)
run('cargo', ['check'], join(rootDir, 'src-tauri'))

console.log(`Version updated to ${version}`)
