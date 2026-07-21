#!/usr/bin/env node
import { existsSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from 'node:fs'
import { basename, join, resolve } from 'node:path'
import { spawnSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'

const rootDir = resolve(fileURLToPath(new URL('..', import.meta.url)))
const packageJsonPath = join(rootDir, 'package.json')
const bundleDir = join(rootDir, 'src-tauri/target/release/bundle/appimage')

function parseArgs(argv) {
  const args = {
    draft: false,
    dryRun: false,
    notes: undefined,
    repo: undefined,
    tag: undefined,
    version: undefined,
  }

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i]

    if (arg === '--draft') {
      args.draft = true
    } else if (arg === '--dry-run') {
      args.dryRun = true
    } else if (arg === '--notes') {
      args.notes = argv[++i]
    } else if (arg === '--repo') {
      args.repo = argv[++i]
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
  console.log(`Usage: npm run release:github -- [options]

Creates latest.json from the existing AppImage + .sig and uploads the release to GitHub.
Run this after npm run build:app.

Options:
  --version <version>  Version to publish. Defaults to package.json version.
  --tag <tag>          Git tag/release name. Defaults to v<version>.
  --repo <owner/repo>  GitHub repo. Defaults to git remote origin or ivang11/tupledb.
  --notes <text>       Release notes. Defaults to "TupleDB <version>".
  --draft             Create the release as draft if it does not exist.
  --dry-run           Generate latest.json and print gh commands without uploading.
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

function findAppImage(version) {
  if (!existsSync(bundleDir)) {
    fail(`Bundle directory does not exist: ${bundleDir}. Run npm run build:app first.`)
  }

  const candidates = readdirSync(bundleDir)
    .filter((file) => file.endsWith('.AppImage'))
    .filter((file) => file.includes(version))
    .sort()

  if (candidates.length === 0) {
    fail(`No AppImage for version ${version} found in ${bundleDir}. Run npm run build:app first.`)
  }

  if (candidates.length > 1) {
    fail(`More than one AppImage matched version ${version}: ${candidates.join(', ')}`)
  }

  return join(bundleDir, candidates[0])
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

const args = parseArgs(process.argv.slice(2))
const version = args.version ?? getPackageVersion()
const tag = args.tag ?? `v${version}`
const repo = args.repo ?? getRepoFromGit() ?? 'ivang11/tupledb'
const notes = args.notes ?? `TupleDB ${version}`
const appImagePath = findAppImage(version)
const signaturePath = `${appImagePath}.sig`

if (!existsSync(signaturePath)) {
  fail(`Signature file does not exist: ${signaturePath}. Build with TAURI_SIGNING_PRIVATE_KEY_PATH first.`)
}

const appImageName = basename(appImagePath)
const latestJsonPath = join(bundleDir, 'latest.json')
const latestJson = {
  version,
  notes,
  pub_date: new Date().toISOString(),
  platforms: {
    'linux-x86_64': {
      signature: readFileSync(signaturePath, 'utf8').trim(),
      url: `https://github.com/${repo}/releases/download/${tag}/${appImageName}`,
    },
  },
}

mkdirSync(bundleDir, { recursive: true })
writeFileSync(latestJsonPath, `${JSON.stringify(latestJson, null, 2)}\n`)

console.log(`Generated ${latestJsonPath}`)
console.log(`Release: ${repo} ${tag}`)
console.log(`Asset: ${appImagePath}`)
console.log(`Signature: ${signaturePath}`)

if (args.dryRun) {
  console.log('\nDry run: no files uploaded.')
  console.log(`gh release create ${tag} ${appImagePath} ${signaturePath} ${latestJsonPath} --repo ${repo} --title ${tag} --notes "${notes}"`)
  console.log(`gh release upload ${tag} ${appImagePath} ${signaturePath} ${latestJsonPath} --repo ${repo} --clobber`)
  process.exit(0)
}

ensureGhAvailable()

if (releaseExists(tag, repo)) {
  run('gh', [
    'release',
    'upload',
    tag,
    appImagePath,
    signaturePath,
    latestJsonPath,
    '--repo',
    repo,
    '--clobber',
  ])
} else {
  const command = [
    'release',
    'create',
    tag,
    appImagePath,
    signaturePath,
    latestJsonPath,
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
