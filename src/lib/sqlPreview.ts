import { decodeRowKey, primaryKeyColumns } from './tableIdentity.js'
import { normalizeChangeValue } from './tableEditing.js'
import type { TableTab } from '../types/workspace.js'
import type { DatabaseEngine } from '../types/connection.js'

/** Review text only; mutations continue through the validated, parameterized driver. */
export function pendingSqlPreview(tabs: TableTab[], engine: DatabaseEngine): string {
  const quote = (name: string) => engine === 'mysql' ? `\`${name.replaceAll('`', '``')}\`` : `"${name.replaceAll('"', '""')}"`
  const output: string[] = []
  for (const tab of tabs) {
    const relation = engine === 'mysql' ? `${quote(tab.database)}.${quote(tab.tableName)}` : `${quote(tab.reference?.schema ?? 'public')}.${quote(tab.tableName)}`
    if (tab.pendingDrop) { output.push(`DROP TABLE ${relation};`); continue }
    if (tab.pendingTruncate) output.push(`TRUNCATE TABLE ${relation};`)
    const keys = primaryKeyColumns(tab.tableStructure)
    const statement = (build: (bind: (value: unknown) => string) => string) => {
      const values: unknown[] = []
      const sql = build(value => { values.push(value); return engine === 'postgresql' ? `$${values.length}` : '?' })
      output.push(`${sql};${values.length ? `\n-- Parameters: ${JSON.stringify(values)}` : ''}`)
    }
    for (const [key, changes] of Object.entries(tab.pendingChanges)) {
      if (tab.pendingDeletions[key]) continue
      if (!keys) throw new Error('A primary key is required to preview row changes')
      const identity = decodeRowKey(key, keys)
      statement(bind => `UPDATE ${relation} SET ${Object.entries(changes).map(([column, value]) => `${quote(column)} = ${bind(normalizeChangeValue(value, tab.tableStructure.find(item => item.field === column)?.value_kind))}`).join(', ')} WHERE ${identity.map(part => `${quote(part.column)} = ${bind(part.value)}`).join(' AND ')}`)
    }
    for (const key of Object.keys(tab.pendingDeletions)) {
      if (!keys) throw new Error('A primary key is required to preview deletions')
      const identity = decodeRowKey(key, keys)
      statement(bind => `DELETE FROM ${relation} WHERE ${identity.map(part => `${quote(part.column)} = ${bind(part.value)}`).join(' AND ')}`)
    }
    for (const insert of tab.pendingInserts) statement(bind => insert.values.length
      ? `INSERT INTO ${relation} (${insert.values.map(value => quote(value.column)).join(', ')}) VALUES (${insert.values.map(value => bind(value.value)).join(', ')})`
      : engine === 'mysql' ? `INSERT INTO ${relation} () VALUES ()` : `INSERT INTO ${relation} DEFAULT VALUES`)
    for (const [oldName, change] of Object.entries(tab.pendingStructureChanges ?? {})) {
      if (engine === 'mysql') output.push(`ALTER TABLE ${relation} CHANGE COLUMN ${quote(oldName)} ${quote(change.newName)} ${change.newType};`)
      else {
        if (change.newType !== tab.tableStructure.find(column => column.field === oldName)?.field_type) output.push(`ALTER TABLE ${relation} ALTER COLUMN ${quote(oldName)} TYPE ${change.newType};`)
        if (change.newName !== oldName) output.push(`ALTER TABLE ${relation} RENAME COLUMN ${quote(oldName)} TO ${quote(change.newName)};`)
      }
    }
  }
  return output.join('\n\n')
}
