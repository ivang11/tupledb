<template>
    <div v-if="show" ref="menu" role="menu" aria-label="Context menu" tabindex="-1"
      class="fixed z-100 min-w-48 max-w-[calc(100vw-16px)] max-h-[calc(100vh-16px)] overflow-auto bg-background border border-border rounded-lg shadow-xl p-1 context-menu-surface"
      :style="{ left: position.x + 'px', top: position.y + 'px' }"
      @keydown="onKeydown" @click.capture="onMenuClick" @contextmenu.prevent>
      <slot />
    </div>
</template>

<script setup lang="ts">
import { nextTick, onUnmounted, ref, watch } from 'vue'
const props = defineProps<{ show: boolean; x: number; y: number }>()
const emit = defineEmits<{ dismiss: [] }>()
const menu = ref<HTMLElement | null>(null)
const position = ref({ x: props.x, y: props.y })
let previousFocus: HTMLElement | null = null
function items() { return [...(menu.value?.querySelectorAll<HTMLButtonElement>('button:not(:disabled)') ?? [])] }
function dismiss() { emit('dismiss') }
function onMenuClick(event: MouseEvent) {
  const button = (event.target as HTMLElement).closest('button')
  if (button && !button.disabled) void nextTick(dismiss)
}
function onOutside(event: PointerEvent) {
  if (!menu.value?.contains(event.target as Node)) dismiss()
}
function onKeydown(event: KeyboardEvent) {
  const buttons = items()
  const current = buttons.indexOf(document.activeElement as HTMLButtonElement)
  let next: number
  if (event.key === 'Escape' || event.key === 'Tab') {
    event.preventDefault(); dismiss(); return
  } else if (event.key === 'ArrowDown') next = (current + 1) % buttons.length
  else if (event.key === 'ArrowUp') next = (current - 1 + buttons.length) % buttons.length
  else if (event.key === 'Home') next = 0
  else if (event.key === 'End') next = buttons.length - 1
  else return
  event.preventDefault()
  buttons[next]?.focus()
}
watch(() => [props.show, props.x, props.y], async () => {
  document.removeEventListener('pointerdown', onOutside, true)
  window.removeEventListener('resize', dismiss)
  if (!props.show) { if (previousFocus?.isConnected) previousFocus.focus(); return }
  previousFocus = document.activeElement as HTMLElement | null
  position.value = { x: props.x, y: props.y }
  await nextTick()
  if (!props.show || !menu.value) return
  const rect = menu.value.getBoundingClientRect()
  position.value = {
    x: Math.max(8, Math.min(props.x, window.innerWidth - rect.width - 8)),
    y: Math.max(8, Math.min(props.y, window.innerHeight - rect.height - 8)),
  }
  for (const button of menu.value.querySelectorAll('button')) button.setAttribute('role', 'menuitem')
  items()[0]?.focus()
  document.addEventListener('pointerdown', onOutside, true)
  window.addEventListener('resize', dismiss)
}, { immediate: true })
onUnmounted(() => {
  document.removeEventListener('pointerdown', onOutside, true)
  window.removeEventListener('resize', dismiss)
})
</script>

<style>
.context-menu-surface button:focus-visible { outline: none; background: var(--muted); color: var(--foreground); }
.context-menu-surface button:disabled { opacity: .4; cursor: not-allowed; }
</style>
