import { afterEach, describe, expect, it } from 'vitest'
import { installTechnicalInputDefaults } from '@/lib/writingAssistance'

afterEach(() => {
  document.body.innerHTML = ''
})

describe('technical input writing assistance', () => {
  it('disables macOS correction for inputs added dynamically', async () => {
    const uninstall = installTechnicalInputDefaults()
    const input = document.createElement('input')
    input.type = 'text'
    document.body.appendChild(input)
    await Promise.resolve()

    expect(input.getAttribute('spellcheck')).toBe('false')
    expect(input.getAttribute('autocorrect')).toBe('off')
    expect(input.getAttribute('autocapitalize')).toBe('none')
    expect(input.getAttribute('autocomplete')).toBe('off')
    uninstall()
  })

  it('allows an explicit opt-in for natural-language fields', async () => {
    const uninstall = installTechnicalInputDefaults()
    const wrapper = document.createElement('div')
    wrapper.dataset.writingAssistance = 'on'
    const input = document.createElement('textarea')
    wrapper.appendChild(input)
    document.body.appendChild(wrapper)
    await Promise.resolve()

    expect(input.hasAttribute('autocorrect')).toBe(false)
    uninstall()
  })
})
