<template>
  <div
    data-tauri-drag-region
    class="flex h-8 shrink-0 items-center justify-between border-b border-border bg-muted/40 px-2 select-none z-50 relative"
    @dblclick="win.toggleMaximize()"
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
      <button
        class="flex size-6 items-center justify-center rounded text-(--fg-2) transition-colors hover:bg-(--bg-2) hover:text-(--fg-1)"
        :class="{ 'pointer-events-none opacity-70': isChecking || isInstalling }"
        :title="updaterTitle"
        :disabled="isChecking || isInstalling"
        @click="() => checkForUpdates()"
      >
        <LoaderCircleIcon v-if="isChecking || isInstalling" class="size-3 animate-spin" />
        <DownloadIcon v-else class="size-3" />
      </button>
      <button
        class="flex size-6 items-center justify-center rounded text-(--fg-2) transition-colors hover:bg-(--bg-2) hover:text-(--fg-1)"
        title="Keyboard shortcuts"
        @click="emit('open-keybindings')"
      >
        <KeyboardIcon class="size-3.5" />
      </button>
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
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { getCurrentWindow } from '@tauri-apps/api/window'
import {
  DownloadIcon,
  KeyboardIcon,
  LoaderCircleIcon,
  MinusIcon,
  PanelLeftCloseIcon,
  PanelLeftOpenIcon,
  SquareIcon,
  XIcon,
} from 'lucide-vue-next'
import { useSidebarState } from '@/composables/useSidebarState'
import { useKeybindings, formatKeybinding } from '@/composables/useKeybindings'
import { useUpdater } from '@/composables/useUpdater'

const emit = defineEmits<{ 'open-keybindings': [] }>()

const win = getCurrentWindow()
const { sidebarVisible, sidebarToggleVisible } = useSidebarState()
const { getBinding } = useKeybindings()
const { checkForUpdates, isChecking, isInstalling } = useUpdater()
const toggleSidebarKey = computed(() => formatKeybinding(getBinding('toggleSidebar')))
const updaterTitle = computed(() => {
  if (isInstalling.value) return 'Installing update'
  if (isChecking.value) return 'Checking for updates'
  return 'Check for updates'
})
</script>
