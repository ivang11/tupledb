import type { DatabaseEngine } from '../types/connection.js'

function statementText(sql: string, engine: DatabaseEngine, mysqlBackslashEscapes: boolean): string {
  let clean = '', quote = '', escape = false, dollar = '', blockDepth = 0, line = false
  for (let i = 0; i < sql.length; i++) {
    const char = sql[i], next = sql[i + 1]
    if (line) { if (char === '\n') { line = false; clean += ' ' }; continue }
    if (blockDepth) {
      if (engine === 'postgresql' && char === '/' && next === '*') { blockDepth++; i++ }
      else if (char === '*' && next === '/') { blockDepth--; i++; clean += ' ' }
      continue
    }
    if (dollar) { if (sql.startsWith(dollar, i)) { i += dollar.length - 1; dollar = '' }; continue }
    if (quote) {
      if (escape && char === '\\') { i++; continue }
      if (char === quote) { if (next === quote) i++; else quote = '' }
      continue
    }
    if (char === '-' && next === '-' && (engine === 'postgresql' || /\s/.test(sql[i + 2] ?? ' '))) { line = true; i++; continue }
    if (char === '/' && next === '*') {
      if (engine === 'mysql' && /^\/\*(?:!|M!)/i.test(sql.slice(i))) throw new Error('Explain does not support executable SQL comments')
      blockDepth = 1; i++; continue
    }
    if (engine === 'mysql' && char === '#') { line = true; continue }
    if (['\'', '"', '`'].includes(char)) {
      quote = char
      escape = engine === 'mysql' ? mysqlBackslashEscapes : char === '\'' && /(?:^|\W)e$/i.test(sql.slice(0, i))
      clean += 'x'; continue
    }
    if (engine === 'postgresql' && char === '$') {
      const match = sql.slice(i).match(/^\$(?:[a-zA-Z_][a-zA-Z_0-9]*)?\$/)
      if (match) { dollar = match[0]; i += dollar.length - 1; clean += 'x'; continue }
    }
    clean += char
  }
  if (quote || dollar || blockDepth) throw new Error('Complete the SQL statement before requesting a plan; use doubled quotes inside literals')
  const statements = clean.split(';').filter(statement => statement.trim())
  if (statements.length !== 1) throw new Error('Explain one statement at a time')
  return statements[0]
}

/** EXPLAIN only: never ANALYZE, and never pass a second statement to the driver. */
export function explainSql(sql: string, engine: DatabaseEngine): string {
  if (engine !== 'mysql' && engine !== 'postgresql') throw new Error('Execution plans are available for MySQL and PostgreSQL')
  const statement = statementText(sql, engine, true)
  // MySQL's NO_BACKSLASH_ESCAPES is a connection setting. Accept SQL only when
  // it contains one statement under either interpretation of quoted strings.
  if (engine === 'mysql') statementText(sql, engine, false)
  if (!/^\s*(SELECT|WITH)\b/i.test(statement)) throw new Error('Explain supports SELECT and WITH queries')
  const source = sql.trim().replace(/;\s*$/, '')
  return engine === 'postgresql' ? `EXPLAIN (FORMAT JSON) ${source}` : `EXPLAIN FORMAT=JSON ${source}`
}
