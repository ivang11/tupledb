export function resizeDetailField(element: HTMLTextAreaElement): void {
  const compact = element.closest('[data-row-detail-panel]')?.getAttribute('data-density') === 'compact'
  const cap = compact ? 48 : 180
  element.style.height = 'auto'
  const height = element.scrollHeight
  element.style.height = `${Math.min(cap, height)}px`
  element.style.overflowY = height > cap ? 'auto' : 'hidden'
}
