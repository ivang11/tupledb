import { spawn, spawnSync } from 'node:child_process'
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'

const root = resolve(import.meta.dirname, '..')
const repetitionsArg = process.argv.find(argument => argument.startsWith('--repetitions='))
const repetitions = Number(repetitionsArg?.split('=')[1] ?? 5)
const scenarioArg = process.argv.find(argument => argument.startsWith('--scenario='))?.split('=')[1]
const settleMsArg = process.argv.find(argument => argument.startsWith('--settle-ms='))
const settleMs = Number(settleMsArg?.split('=')[1] ?? 0)
const noBuild = process.argv.includes('--no-build')
const allScenarios = [
  { rows: 300, columns: 20 },
  { rows: 300, columns: 200 },
  { rows: 5000, columns: 20 },
  { rows: 5000, columns: 200 },
]
function parseScenario(value) {
  const match = value?.match(/^(\d+)x(\d+)$/)
  if (!match) return null
  const rows = Number(match[1])
  const columns = Number(match[2])
  if (!Number.isInteger(rows) || !Number.isInteger(columns)) return null
  if (rows < 1 || rows > 5_000 || columns < 4 || columns > 256) return null
  return { rows, columns }
}

const customScenario = scenarioArg ? parseScenario(scenarioArg) : null
const scenarios = scenarioArg
  ? customScenario
    ? [customScenario]
    : allScenarios.filter(scenario => `${scenario.rows}x${scenario.columns}` === scenarioArg)
  : allScenarios

if (!Number.isInteger(repetitions) || repetitions < 1 || repetitions > 30) {
  throw new Error('--repetitions must be an integer between 1 and 30')
}
if (scenarios.length === 0) throw new Error(`Unknown --scenario=${scenarioArg}; expected ROWSxCOLUMNS within 1..5000 x 4..256`)
if (!Number.isInteger(settleMs) || settleMs < 0 || settleMs > 5_000) {
  throw new Error('--settle-ms must be an integer between 0 and 5000')
}

function run(command, commandArgs, options = {}) {
  const result = spawnSync(command, commandArgs, {
    cwd: root,
    stdio: 'inherit',
    env: { ...process.env, ...options.env },
  })
  if (result.status !== 0) throw new Error(`${command} failed with status ${result.status}`)
}

function build() {
  run('node', [
    'node_modules/@tauri-apps/cli/tauri.js',
    'build',
    '--no-bundle',
    '--config',
    '{"bundle":{"createUpdaterArtifacts":false}}',
  ], { env: { VITE_TUPLEDB_BENCHMARK: '1' } })
}

function processSnapshot() {
  const result = spawnSync('ps', ['-axo', 'pid=,ppid=,rss=,command='], { encoding: 'utf8' })
  if (result.status !== 0) return []
  return result.stdout.trim().split('\n').flatMap(line => {
    const match = line.trim().match(/^(\d+)\s+(\d+)\s+(\d+)\s+(.+)$/)
    return match ? [{ pid: Number(match[1]), ppid: Number(match[2]), rssKiB: Number(match[3]), command: match[4] }] : []
  })
}

function associatedRss(rootPid, baselinePids) {
  const processes = processSnapshot()
  const included = new Set([rootPid])
  let changed = true
  while (changed) {
    changed = false
    for (const process of processes) {
      if (included.has(process.ppid) && !included.has(process.pid)) {
        included.add(process.pid)
        changed = true
      }
    }
  }

  for (const process of processes) {
    const isNew = !baselinePids.has(process.pid)
    const belongsToWebView = /WebKit\.(WebContent|GPU|Networking)|com\.tupledb\.app|TupleDB\.app/.test(process.command)
    if (isNew && belongsToWebView) included.add(process.pid)
  }
  return processes.filter(process => included.has(process.pid)).reduce((sum, process) => sum + process.rssKiB, 0)
}

function parseMetrics(line, metrics) {
  if (!line.includes('TUPLEDB_TAURI_METRIC')) return false
  const lineMetrics = Object.fromEntries(
    [...line.matchAll(/([a-zA-Z0-9_]+)=([0-9.]+)/g)].map(match => [match[1], Number(match[2])]),
  )
  if (Object.hasOwn(lineMetrics, 'first_paint_ms')) {
    lineMetrics.frontend_first_paint_ms = lineMetrics.first_paint_ms
    lineMetrics.first_paint_ms = lineMetrics.process_ms
  }
  delete lineMetrics.process_ms
  Object.assign(metrics, lineMetrics)
  return Object.hasOwn(metrics, 'frame_max_ms')
}

async function benchmarkOnce(scenario, iteration) {
  const baselinePids = new Set(processSnapshot().map(process => process.pid))
  const configDir = mkdtempSync(join(tmpdir(), 'tupledb-tauri-benchmark-'))
  const binary = resolve(root, 'src-tauri/target/release/tupledb')
  const env = {
    ...process.env,
    TUPLEDB_BENCHMARK_MODE: '1',
    TUPLEDB_BENCHMARK_ROWS: String(scenario.rows),
    TUPLEDB_BENCHMARK_COLUMNS: String(scenario.columns),
    TUPLEDB_BENCHMARK_SETTLE_MS: String(settleMs),
    TUPLEDB_CONFIG_DIR: configDir,
  }

  return await new Promise((resolvePromise, reject) => {
    const child = spawn(binary, [], { cwd: root, env, stdio: ['ignore', 'pipe', 'pipe'] })
    const metrics = {}
    let peakRssKiB = 0
    let buffer = ''
    let finished = false
    const sample = () => {
      peakRssKiB = Math.max(peakRssKiB, associatedRss(child.pid, baselinePids))
    }
    const interval = setInterval(sample, 500)
    const timeout = setTimeout(() => finish(new Error(`Tauri timed out for ${scenario.rows}x${scenario.columns}`)), 30_000)

    function finish(error) {
      if (finished) return
      finished = true
      clearInterval(interval)
      clearTimeout(timeout)
      sample()
      child.kill('SIGTERM')
      rmSync(configDir, { recursive: true, force: true })
      if (error) return reject(error)
      resolvePromise({ shell: 'tauri-optimized', ...scenario, settle_ms: settleMs, iteration, ...metrics, rss_mib: peakRssKiB / 1024 })
    }

    function consume(chunk) {
      buffer += chunk.toString()
      const lines = buffer.split('\n')
      buffer = lines.pop() ?? ''
      for (const line of lines) {
        if (parseMetrics(line, metrics)) finish()
      }
    }
    child.stdout.on('data', consume)
    child.stderr.on('data', consume)
    child.on('error', finish)
    child.on('exit', code => {
      if (!finished) finish(new Error(`Tauri exited early with status ${code}: ${buffer}`))
    })
  })
}

function median(values) {
  const sorted = values.filter(Number.isFinite).sort((left, right) => left - right)
  const middle = Math.floor(sorted.length / 2)
  return sorted.length % 2 ? sorted[middle] : (sorted[middle - 1] + sorted[middle]) / 2
}

if (!noBuild) build()

const results = []
for (const scenario of scenarios) {
  for (let iteration = 1; iteration <= repetitions; iteration += 1) {
    process.stdout.write(`Benchmark Tauri optimized ${scenario.rows}x${scenario.columns} (${iteration}/${repetitions})... `)
    const result = await benchmarkOnce(scenario, iteration)
    results.push(result)
    console.log(`${result.first_paint_ms.toFixed(1)} ms, ${result.rss_mib.toFixed(1)} MiB`)
    await new Promise(resolveDelay => setTimeout(resolveDelay, 500))
  }
}

const metrics = ['first_paint_ms', 'frame_p50_ms', 'frame_p95_ms', 'frame_p99_ms', 'frame_max_ms', 'rss_mib']
const summary = scenarios.map(scenario => {
  const matching = results.filter(result => result.rows === scenario.rows && result.columns === scenario.columns)
  return Object.fromEntries([
    ['shell', 'tauri-optimized'], ['rows', scenario.rows], ['columns', scenario.columns],
    ...metrics.map(metric => [metric, median(matching.map(result => result[metric]))]),
  ])
})
const lines = [
  '| Dataset | Shell | First paint | RSS peak | Frame p50 | Frame p95 | Frame p99 | Frame max |',
  '|---:|---|---:|---:|---:|---:|---:|---:|',
  ...summary.map(row => `| ${row.rows}×${row.columns} | ${row.shell} | ${row.first_paint_ms.toFixed(1)} ms | ${row.rss_mib.toFixed(1)} MiB | ${row.frame_p50_ms.toFixed(2)} ms | ${row.frame_p95_ms.toFixed(2)} ms | ${row.frame_p99_ms.toFixed(2)} ms | ${row.frame_max_ms.toFixed(2)} ms |`),
]
const generatedAt = new Date().toISOString()
const report = `# Optimized Tauri grid benchmark\n\nGenerated: ${generatedAt}\n\nRepetitions per point: ${repetitions}. Values are medians.\n\n${lines.join('\n')}\n`
const outputDir = resolve(root, 'src-tauri/target/benchmarks')
mkdirSync(outputDir, { recursive: true })
writeFileSync(resolve(outputDir, 'tauri-grid-optimized.json'), JSON.stringify({ generatedAt, repetitions, results, summary }, null, 2))
writeFileSync(resolve(outputDir, 'tauri-grid-optimized.md'), report)
console.log(`\n${report}`)
