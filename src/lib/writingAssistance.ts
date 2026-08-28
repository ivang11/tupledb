const TECHNICAL_TEXT_INPUTS = [
  'input:not([type])',
  'input[type="text"]',
  'input[type="search"]',
  'textarea',
].join(',')

function configureInput(element: Element) {
  if (element.closest('[data-writing-assistance="on"]')) return
  element.setAttribute('spellcheck', 'false')
  element.setAttribute('autocorrect', 'off')
  element.setAttribute('autocapitalize', 'none')
  element.setAttribute('autocomplete', 'off')
}

function configureTree(root: ParentNode) {
  if (root instanceof Element && root.matches(TECHNICAL_TEXT_INPUTS)) {
    configureInput(root)
  }
  root.querySelectorAll(TECHNICAL_TEXT_INPUTS).forEach(configureInput)
}

/**
 * Database values and identifiers are technical text. WebKit must not apply
 * macOS spelling corrections or capitalization when these fields lose focus.
 * Dynamic dialogs and inline editors are covered by the observer as well.
 */
export function installTechnicalInputDefaults(root: Document = document) {
  configureTree(root)
  const observer = new MutationObserver((records) => {
    for (const record of records) {
      for (const node of record.addedNodes) {
        if (node instanceof Element) configureTree(node)
      }
    }
  })
  observer.observe(root, { childList: true, subtree: true })
  return () => observer.disconnect()
}
