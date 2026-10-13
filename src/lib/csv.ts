import { rowValue, type DataRow, type NamedColumn } from './rowAccess.js'
export interface CsvOptions { delimiter: string; quotes: 'needed' | 'always'; lineBreak: string; decimal: '.' | ','; nullMarker: string }
export function parseCsv(source: string, delimiter = ','): string[][] {
  if (delimiter.length !== 1) throw new Error('Choose a single-character delimiter')
  const text = source.replace(/^\uFEFF/, '')
  const rows: string[][] = []; let row: string[] = [], field = '', quoted = false, closed = false
  for (let i = 0; i < text.length; i++) {
    const char = text[i]
    if (quoted) { if (char === '"') { if (text[i + 1] === '"') { field += '"'; i++ } else { quoted = false; closed = true } } else field += char; continue }
    if (closed && ![delimiter, '\r', '\n'].includes(char)) throw new Error(`Unexpected character after quoted field at offset ${i}`)
    if (char === '"') { if (field) throw new Error(`Quote inside unquoted field at offset ${i}`); quoted = true; continue }
    if (char === delimiter) { row.push(field); field = ''; closed = false; continue }
    if (char === '\r' || char === '\n') { if (char === '\r' && text[i + 1] === '\n') i++; row.push(field); rows.push(row); row = []; field = ''; closed = false; continue }
    field += char
  }
  if (quoted) throw new Error('Unclosed quoted field')
  if (field || row.length || closed) { row.push(field); rows.push(row) }
  return rows
}
export function serializeCsv(columns: NamedColumn[], rows: DataRow[], options: CsvOptions): string {
  function field(value: unknown) {
    let text = value === null || value === undefined ? options.nullMarker : typeof value === 'object' ? JSON.stringify(value) : typeof value === 'number' && options.decimal === ',' ? String(value).replace('.', ',') : String(value)
    if (options.quotes === 'always' || text.includes(options.delimiter) || /["\r\n]/.test(text)) text = `"${text.replaceAll('"', '""')}"`
    return text
  }
  const names = columns.map(column => column.name ?? column.field ?? '')
  return [names.map(field).join(options.delimiter), ...rows.map(row => names.map(name => field(rowValue(row, name, columns))).join(options.delimiter))].join(options.lineBreak) + options.lineBreak
}
