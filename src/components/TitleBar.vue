<template>
  <div
    data-tauri-drag-region
    class="flex h-8 shrink-0 items-center justify-between border-b border-border bg-muted/40 select-none z-50 relative"
    :class="reserveNativeControlsSpace ? 'pl-[78px] pr-2' : 'px-2'"
    @dblclick="handleDoubleClick"
  >
    <!-- App identity -->
    <div class="flex items-center gap-1.5">
      <div data-tauri-drag-region class="flex items-center gap-1.5 pointer-events-none">
        <span class="wordmark text-[13px] leading-none tracking-normal">
          tuple<span class="wordmark-accent">db</span>
        </span>
      </div>
      <button
        v-if="sidebarToggleVisible"
        class="flex size-6 items-center justify-center rounded text-(--fg-2) transition-colors hover:bg-(--bg-2) hover:text-(--fg-1)"
        :title="sidebarVisible ? `Hide Sidebar (${toggleSidebarKey})` : `Show Sidebar (${toggleSidebarKey})`"
        @click="sidebarVisible = !sidebarVisible"
      >
        <PanelLeftCloseIcon v-if="sidebarVisible" class="size-3.5" />
        <PanelLeftOpenIcon v-else class="size-3.5" />
      </button>
    </div>

    <!-- Window controls -->
    <div class="flex items-center gap-0.5">
      <button class="flex size-6 items-center justify-center rounded text-(--fg-2) hover:bg-(--bg-2) hover:text-(--fg-1)" :title="`Quick search (${quickSearchKey})`" aria-label="Quick search" @click="openQuickSearch"><SearchIcon class="size-3.5" /></button>
      <button class="flex size-6 items-center justify-center rounded text-(--fg-2) hover:bg-(--bg-2) hover:text-(--fg-1)" title="Settings" aria-label="Settings" @click="openSettings"><SettingsIcon class="size-3.5" /></button>
      <template v-if="!usesNativeMacWindowControls">
        <div class="w-px h-3.5 bg-border/50 mx-1" />
        <button
          class="flex size-6 items-center justify-center rounded text-(--fg-2) transition-colors hover:bg-(--bg-2) hover:text-(--fg-1)"
          @click="win.minimize()"
          title="Minimize"
        >
          <MinusIcon class="size-3.5" />
        </button>
        <button
          class="flex size-6 items-center justify-center rounded text-(--fg-2) transition-colors hover:bg-(--bg-2) hover:text-(--fg-1)"
          @click="win.toggleMaximize()"
          title="Maximize"
        >
          <SquareIcon class="size-3.5" />
        </button>
        <button
          class="flex size-6 items-center justify-center rounded text-(--fg-2) transition-colors hover:bg-destructive hover:text-white"
          @click="win.close()"
          title="Close"
        >
          <XIcon class="size-3.5" />
        </button>
      </template>
    </div>
  </div>
</template>

<script setup lang="ts">
import { openSettingsWindow } from '@/lib/settingsWindow'
import { useToast } from '@/composables/useToast'
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { getCurrentWindow } from '@tauri-apps/api/window'
import {
  SearchIcon,
  SettingsIcon,
  MinusIcon,
  PanelLeftCloseIcon,
  PanelLeftOpenIcon,
  SquareIcon,
  XIcon,
} from 'lucide-vue-next'
import { useSidebarState } from '@/composables/useSidebarState'
import { useKeybindings, formatKeybinding } from '@/composables/useKeybindings'
import { usesNativeMacWindowControls } from '@/lib/platform'

function openSettings() {
  void openSettingsWindow().catch(error => useToast().error('Could not open settings', String(error)))
}
const win = getCurrentWindow()
const isFullscreen = ref(false)
const reserveNativeControlsSpace = computed(
  () => usesNativeMacWindowControls && !isFullscreen.value,
)
const { sidebarVisible, sidebarToggleVisible } = useSidebarState()
const { getBinding } = useKeybindings()
const quickSearchKey = computed(() => formatKeybinding(getBinding('quickSearch')))
const toggleSidebarKey = computed(() => formatKeybinding(getBinding('toggleSidebar')))

function openQuickSearch() { window.dispatchEvent(new Event('tupledb:quick-search')) }

function handleDoubleClick() {
  if (!usesNativeMacWindowControls) void win.toggleMaximize()
}

let stopListeningForResize: (() => void) | undefined
let isUnmounted = false

async function syncFullscreenState() {
  isFullscreen.value = await win.isFullscreen()
}

onMounted(async () => {
  if (!usesNativeMacWindowControls) return

  await syncFullscreenState()
  const unlisten = await win.onResized(() => void syncFullscreenState())

  if (isUnmounted) unlisten()
  else stopListeningForResize = unlisten
})

onUnmounted(() => {
  isUnmounted = true
  stopListeningForResize?.()
})
</script>
