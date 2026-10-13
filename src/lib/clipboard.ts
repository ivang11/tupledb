import { useToast } from '@/composables/useToast'

export async function copyText(text: string): Promise<void> {
  try {
    await navigator.clipboard.writeText(text)
  } catch (error) {
    useToast().error('Could not copy to clipboard', String(error))
  }
}

export function rowTsv(record: Record<string, unknown>): string {
  return Object.values(record).map(value => {
    if (value === null || value === undefined) return ''
    const text = typeof value === 'object' ? JSON.stringify(value) : String(value)
    return /[\t\r\n"]/.test(text) ? `"${text.replaceAll('"', '""')}"` : text
  }).join('\t')
}
