export type KeybindingAction =
  | 'closeTab'
  | 'runQuery'
  | 'formatQuery'
  | 'applyFilters'
  | 'sidebarSearch'
  | 'refreshTable'
  | 'toggleSidebar'
  | 'focusPane'

export type KeybindingCategory = 'Tabs' | 'Editor' | 'Data' | 'Navigation'

export interface KeybindingDef {
  action: KeybindingAction
  label: string
  description: string
  defaultKey: string
  category: KeybindingCategory
}

export const KEYBINDING_DEFS: KeybindingDef[] = [
  {
    action: 'closeTab',
    label: 'Close Tab',
    description: 'Close the active tab',
    defaultKey: 'Ctrl+W',
    category: 'Tabs',
  },
  {
    action: 'runQuery',
    label: 'Run Query',
    description: 'Execute the SQL query in the editor',
    defaultKey: 'Ctrl+Enter',
    category: 'Editor',
  },
  {
    action: 'formatQuery',
    label: 'Format Query',
    description: 'Beautify/format the SQL query',
    defaultKey: 'Ctrl+Shift+F',
    category: 'Editor',
  },
  {
    action: 'applyFilters',
    label: 'Apply Filters',
    description: 'Apply the current table filters',
    defaultKey: 'Ctrl+Enter',
    category: 'Data',
  },
  {
    action: 'sidebarSearch',
    label: 'Focus Sidebar Search',
    description: 'Focus the table filter input in the sidebar',
    defaultKey: 'Ctrl+K',
    category: 'Navigation',
  },
  {
    action: 'refreshTable',
    label: 'Refresh Table',
    description: 'Reload data for the active table tab',
    defaultKey: 'Ctrl+R',
    category: 'Data',
  },
  {
    action: 'toggleSidebar',
    label: 'Toggle Sidebar',
    description: 'Show or hide the sidebar',
    defaultKey: 'Ctrl+B',
    category: 'Navigation',
  },
  {
    action: 'focusPane',
    label: 'Focus / Unpin Pane',
    description: 'Pin the active pane to full width or unpin it',
    defaultKey: 'Ctrl+Shift+M',
    category: 'Navigation',
  },
]

export const DEFAULT_KEYBINDINGS = Object.fromEntries(
  KEYBINDING_DEFS.map((d) => [d.action, d.defaultKey]),
) as Record<KeybindingAction, string>
