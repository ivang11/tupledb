#!/usr/bin/env node
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  statSync,
  writeFileSync,
} from 'node:fs'
import { tmpdir } from 'node:os'
import { basename, dirname, join, resolve } from 'node:path'
import { spawnSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'

const rootDir = resolve(fileURLToPath(new URL('..', import.meta.url)))
const packageJsonPath = join(rootDir, 'package.json')
const defaultArtifactsDir = join(rootDir, 'src-tauri/target/release/bundle')

function parseArgs(argv) {
  const args = {
    artifactsDir: defaultArtifactsDir,
    draft: false,
    dryRun: false,
    notes: undefined,
    repo: undefined,
    requiredPlatforms: [],
    tag: undefined,
    version: undefined,
  }

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i]

    if (arg === '--artifacts-dir') {
      args.artifactsDir = resolve(argv[++i])
    } else if (arg === '--draft') {
      args.draft = true
    } else if (arg === '--dry-run') {
      args.dryRun = true
    } else if (arg === '--notes') {
      args.notes = argv[++i]
    } else if (arg === '--repo') {
      args.repo = argv[++i]
    } else if (arg === '--require-platform') {
      args.requiredPlatforms.push(argv[++i])
    } else if (arg === '--tag') {
      args.tag = argv[++i]
    } else if (arg === '--version') {
      args.version = argv[++i]
    } else if (arg === '--help' || arg === '-h') {
      printHelp()
      process.exit(0)
    } else {
      fail(`Unknown argument: ${arg}`)
    }
  }

  return args
}

function printHelp() {
  console.log(`Usage: npm run release:publish -- [options]

Creates latest.json from existing Tauri updater artifacts and uploads the release to GitHub.
Run this after the signed platform builds have completed.

Options:
  --artifacts-dir <path>      Directory containing release artifacts. Defaults to Tauri bundle output.
  --version <version>         Version to publish. Defaults to package.json version.
  --tag <tag>                 Git tag/release name. Defaults to v<version>.
  --repo <owner/repo>         GitHub repo. Defaults to git remote origin or ivang11/tupledb.
  --notes <text>              Release notes. Defaults to "TupleDB <version>".
  --require-platform <name>   Require a platform in latest.json. Can be repeated.
  --draft                     Create the release as draft if it does not exist.
  --dry-run                   Generate latest.json and print gh commands without uploading.
`)
}

function fail(message) {
  console.error(`Error: ${message}`)
  process.exit(1)
}

function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: rootDir,
    encoding: 'utf8',
    stdio: options.capture ? 'pipe' : 'inherit',
  })

  if (result.status !== 0) {
    if (options.allowFailure) return result
    fail(`${command} ${args.join(' ')} failed`)
  }

  return result
}

function getPackageVersion() {
  const pkg = JSON.parse(readFileSync(packageJsonPath, 'utf8'))
  return pkg.version
}

function getRepoFromGit() {
  const result = run('git', ['remote', 'get-url', 'origin'], {
    capture: true,
    allowFailure: true,
  })

  if (result.status !== 0) return undefined

  const remote = result.stdout.trim()
  const match = remote.match(/github\.com[:/](?<repo>[^/]+\/[^/.]+)(?:\.git)?$/)
  return match?.groups?.repo
}

function ensureGhAvailable() {
  const result = run('gh', ['--version'], {
    capture: true,
    allowFailure: true,
  })

  if (result.status !== 0) {
    fail('GitHub CLI is not available. Install gh and run gh auth login.')
  }
}

function releaseExists(tag, repo) {
  const result = run('gh', ['release', 'view', tag, '--repo', repo], {
    capture: true,
    allowFailure: true,
  })

  return result.status === 0
}

function collectFiles(dir) {
  if (!existsSync(dir)) {
    fail(`Artifacts directory does not exist: ${dir}`)
  }

  const entries = readdirSync(dir)
  const files = []

  for (const entry of entries) {
    const path = join(dir, entry)
    const stat = statSync(path)

    if (stat.isDirectory()) {
      files.push(...collectFiles(path))
    } else if (stat.isFile()) {
      files.push(path)
    }
  }

  return files
}

function inferArch(path) {
  const value = path.toLowerCase()

  if (value.includes('aarch64') || value.includes('arm64')) return 'aarch64'
  if (value.includes('x86_64') || value.includes('amd64') || value.includes('x64')) return 'x86_64'

  return undefined
}

function inferSiblingMacArch(archivePath, files) {
  const archiveDir = dirname(archivePath)
  const siblingDmg = files.find((file) => dirname(file) === archiveDir && file.endsWith('.dmg'))

  return siblingDmg ? inferArch(siblingDmg) : undefined
}

function macArchLabel(arch) {
  if (arch === 'aarch64') return 'Apple_Silicon'
  if (arch === 'x86_64') return 'Intel'

  return arch
}

function stageAsset(sourcePath, uploadDir, preferredName, stagedNames) {
  let name = preferredName ?? basename(sourcePath)

  if (stagedNames.has(name)) {
    const extIndex = name.indexOf('.')
    const prefix = extIndex === -1 ? name : name.slice(0, extIndex)
    const suffix = extIndex === -1 ? '' : name.slice(extIndex)
    let counter = 2

    while (stagedNames.has(`${prefix}-${counter}${suffix}`)) {
      counter += 1
    }

    name = `${prefix}-${counter}${suffix}`
  }

  stagedNames.add(name)
  const stagedPath = join(uploadDir, name)
  copyFileSync(sourcePath, stagedPath)
  return { name, path: stagedPath }
}

function findRequiredSig(artifactPath) {
  const signaturePath = `${artifactPath}.sig`

  if (!existsSync(signaturePath)) {
    fail(`Signature file does not exist: ${signaturePath}`)
  }

  return signaturePath
}

function addPlatform(platforms, platform, asset, signaturePath, repo, tag) {
  if (platforms[platform]) {
    fail(`More than one updater artifact matched ${platform}`)
  }

  platforms[platform] = {
    signature: readFileSync(signaturePath, 'utf8').trim(),
    url: `https://github.com/${repo}/releases/download/${tag}/${asset.name}`,
  }
}

function buildReleaseArtifacts(files, uploadDir, version, repo, tag) {
  const stagedNames = new Set()
  const uploadAssets = []
  const platforms = {}

  const appImages = files
    .filter((file) => file.endsWith('.AppImage'))
    .filter((file) => basename(file).includes(version))

  for (const appImagePath of appImages) {
    const signaturePath = findRequiredSig(appImagePath)
    const appImageAsset = stageAsset(
      appImagePath,
      uploadDir,
      `TupleDB_${version}_Linux_x86_64.AppImage`,
      stagedNames,
    )
    const signatureAsset = stageAsset(
      signaturePath,
      uploadDir,
      `${appImageAsset.name}.sig`,
      stagedNames,
    )

    uploadAssets.push(appImageAsset.path, signatureAsset.path)
    addPlatform(platforms, 'linux-x86_64', appImageAsset, signaturePath, repo, tag)
  }

  const macUpdaterArchives = files.filter((file) => file.endsWith('.app.tar.gz'))

  for (const archivePath of macUpdaterArchives) {
    const arch = inferArch(archivePath) ?? inferSiblingMacArch(archivePath, files)

    if (!arch) {
      fail(`Could not infer macOS architecture for ${archivePath}`)
    }

    const platform = `darwin-${arch}`
    const archLabel = macArchLabel(arch)
    const signaturePath = findRequiredSig(archivePath)
    const archiveAsset = stageAsset(
      archivePath,
      uploadDir,
      `TupleDB_${version}_macOS_${archLabel}_updater.app.tar.gz`,
      stagedNames,
    )
    const signatureAsset = stageAsset(
      signaturePath,
      uploadDir,
      `${archiveAsset.name}.sig`,
      stagedNames,
    )

    uploadAssets.push(archiveAsset.path, signatureAsset.path)
    addPlatform(platforms, platform, archiveAsset, signaturePath, repo, tag)
  }

  const manualInstallers = files
    .filter((file) => file.endsWith('.dmg'))
    .filter((file) => basename(file).includes(version))

  for (const installerPath of manualInstallers) {
    const arch = inferArch(installerPath)
    const preferredName = arch ? `TupleDB_${version}_macOS_${macArchLabel(arch)}.dmg` : undefined
    const installerAsset = stageAsset(installerPath, uploadDir, preferredName, stagedNames)
    uploadAssets.push(installerAsset.path)
  }

  return { platforms, uploadAssets }
}

const args = parseArgs(process.argv.slice(2))
const version = args.version ?? getPackageVersion()
const tag = args.tag ?? `v${version}`
const repo = args.repo ?? getRepoFromGit() ?? 'ivang11/tupledb'
const notes = args.notes ?? `TupleDB ${version}`
const uploadDir = mkdtempSync(join(tmpdir(), 'tupledb-release-'))
const files = collectFiles(args.artifactsDir)
const { platforms, uploadAssets } = buildReleaseArtifacts(files, uploadDir, version, repo, tag)

for (const requiredPlatform of args.requiredPlatforms) {
  if (!platforms[requiredPlatform]) {
    fail(`Required platform missing from latest.json: ${requiredPlatform}`)
  }
}

if (Object.keys(platforms).length === 0) {
  fail(`No updater artifacts found in ${args.artifactsDir}`)
}

const latestJsonPath = join(uploadDir, 'latest.json')
const latestJson = {
  version,
  notes,
  pub_date: new Date().toISOString(),
  platforms,
}

mkdirSync(dirname(latestJsonPath), { recursive: true })
writeFileSync(latestJsonPath, `${JSON.stringify(latestJson, null, 2)}\n`)
uploadAssets.push(latestJsonPath)

console.log(`Generated ${latestJsonPath}`)
console.log(`Release: ${repo} ${tag}`)
console.log(`Platforms: ${Object.keys(platforms).sort().join(', ')}`)
for (const asset of uploadAssets) {
  console.log(`Asset: ${asset}`)
}

if (args.dryRun) {
  console.log('\nDry run: no files uploaded.')
  console.log(`gh release create ${tag} ${uploadAssets.join(' ')} --repo ${repo} --title ${tag} --notes "${notes}"`)
  console.log(`gh release upload ${tag} ${uploadAssets.join(' ')} --repo ${repo} --clobber`)
  process.exit(0)
}

ensureGhAvailable()

if (releaseExists(tag, repo)) {
  run('gh', [
    'release',
    'upload',
    tag,
    ...uploadAssets,
    '--repo',
    repo,
    '--clobber',
  ])
} else {
  const command = [
    'release',
    'create',
    tag,
    ...uploadAssets,
    '--repo',
    repo,
    '--title',
    tag,
    '--notes',
    notes,
  ]

  if (args.draft) command.push('--draft')
  run('gh', command)
}

console.log(`Published ${tag} to https://github.com/${repo}/releases/tag/${tag}`)
