import type { DatabaseEngine } from '../types/connection.js'
export interface SqlStatement { sql: string; from: number; to: number }
export interface QueryParameter { name: string; type: 'text' | 'number' | 'boolean' | 'null'; value: string }

/** Preserve statement positions, skipping semicolons in SQL literals and comments. */
export function splitSqlStatements(source: string, engine: DatabaseEngine): SqlStatement[] {
  const statements: SqlStatement[] = []
  let start = 0, quote = '', escape = false, dollar = '', block = 0, line = false
  function push(end: number) {
    const text = source.slice(start, end), leading = text.length - text.trimStart().length
    if (text.trim() && text.replace(/\/\*[\s\S]*?\*\/|--[^\n]*(?:\n|$)/g, '').trim()) statements.push({ sql: text.trim(), from: start + leading, to: end })
    start = end + 1
  }
  for (let index = 0; index < source.length; index++) {
    const char = source[index], next = source[index + 1]
    if (line) { if (char === '\n') line = false; continue }
    if (block) { if (engine === 'postgresql' && char === '/' && next === '*') { block++; index++ } else if (char === '*' && next === '/') { block--; index++ }; continue }
    if (dollar) { if (source.startsWith(dollar, index)) { index += dollar.length - 1; dollar = '' }; continue }
    if (quote) { if (char === '\\' && escape) { index++; continue }; if (char === quote) { if (next === quote) index++; else quote = '' }; continue }
    if (char === '-' && next === '-' && (engine !== 'mysql' || /\s/.test(source[index + 2] ?? ' '))) { line = true; index++; continue }
    if (engine === 'mysql' && char === '#') { line = true; continue }
    if (char === '/' && next === '*') { block = 1; index++; continue }
    if (['\'', '"', '`'].includes(char)) { quote = char; escape = engine === 'mysql' || char === '\'' && /(?:^|\W)e$/i.test(source.slice(0, index)); continue }
    if (engine === 'postgresql' && char === '$') {
      const match = source.slice(index).match(/^\$(?:[a-zA-Z_][a-zA-Z_0-9]*)?\$/)
      if (match) { dollar = match[0]; index += dollar.length - 1; continue }
    }
    if (char === ';') push(index)
  }
  if (quote || block || dollar) throw new Error('Complete quoted strings and comments before executing SQL')
  push(source.length)
  return statements
}

export function executionSql(source: string, selection: { from: number; to: number }, engine: DatabaseEngine): string {
  if (selection.from !== selection.to) return source.slice(selection.from, selection.to).trim()
  const statements = splitSqlStatements(source, engine)
  return (statements.find(statement => selection.from <= statement.to) ?? statements.at(-1))?.sql ?? ''
}

/** Replace only named value parameters outside literals, identifiers and comments. */
export function parameterNames(sql: string, engine: DatabaseEngine): string[] { return [...new Set(replaceParameters(sql, engine, name => `:${name}`).names)] }
function replaceParameters(sql: string, engine: DatabaseEngine, replacement: (name: string) => string) {
  let output = '', quote = '', dollar = '', block = 0, line = false, escape = false
  const names: string[] = []
  for (let i = 0; i < sql.length; i++) {
    const char = sql[i], next = sql[i + 1]
    if (line) { output += char; if (char === '\n') line = false; continue }
    if (block) { output += char; if (engine === 'postgresql' && char === '/' && next === '*') { block++; output += next; i++ } else if (char === '*' && next === '/') { block--; output += next; i++ }; continue }
    if (dollar) { if (sql.startsWith(dollar, i)) { output += dollar; i += dollar.length - 1; dollar = '' } else output += char; continue }
    if (quote) { output += char; if (escape && char === '\\') { output += next ?? ''; i++; continue }; if (char === quote) { if (next === quote) { output += next; i++ } else quote = '' }; continue }
    if (char === '-' && next === '-' && (engine !== 'mysql' || /\s/.test(sql[i + 2] ?? ' '))) { line = true; output += '--'; i++; continue }
    if (engine === 'mysql' && char === '#') { line = true; output += char; continue }
    if (char === '/' && next === '*') { block = 1; output += '/*'; i++; continue }
    if (['\'', '"', '`'].includes(char)) { quote = char; escape = engine === 'mysql' || char === '\'' && /(?:^|\W)e$/i.test(sql.slice(0, i)); output += char; continue }
    if (engine === 'postgresql' && char === '$') { const match = sql.slice(i).match(/^\$(?:[a-zA-Z_][a-zA-Z_0-9]*)?\$/); if (match) { dollar = match[0]; output += dollar; i += dollar.length - 1; continue } }
    if (char === ':' && sql[i - 1] !== ':' && next !== ':') {
      const match = sql.slice(i + 1).match(/^[a-zA-Z_][a-zA-Z_0-9]*/)
      if (match) { names.push(match[0]); output += replacement(match[0]); i += match[0].length; continue }
    }
    output += char
  }
  return { sql: output, names }
}
export function substituteParameters(sql: string, parameters: QueryParameter[], engine: DatabaseEngine): string {
  const values = new Map(parameters.map(parameter => [parameter.name, parameter]))
  return replaceParameters(sql, engine, name => {
    const parameter = values.get(name)
    if (!parameter) throw new Error(`Missing parameter: ${name}`)
    if (parameter.type === 'null') return 'NULL'
    if (parameter.type === 'number') {
      if (!/^[+-]?\d+(?:\.\d+)?(?:e[+-]?\d+)?$/i.test(parameter.value)) throw new Error(`Invalid number for :${name}`)
      return parameter.value
    }
    if (parameter.type === 'boolean') {
      if (!/^(true|false)$/i.test(parameter.value)) throw new Error(`Use true or false for :${name}`)
      return parameter.value.toUpperCase()
    }
    if (engine === 'mysql') {
      const hex = [...new TextEncoder().encode(parameter.value)].map(byte => byte.toString(16).padStart(2, '0')).join('')
      return hex ? `CONVERT(0x${hex} USING utf8mb4)` : "''"
    }
    return `E'${parameter.value.replaceAll('\\', '\\\\').replaceAll("'", "''")}'`
  }).sql
}
