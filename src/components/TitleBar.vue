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
const { sidebarVisible } = useSidebarState()
const { getBinding } = useKeybindings()
const { checkForUpdates, isChecking, isInstalling } = useUpdater()
const toggleSidebarKey = computed(() => formatKeybinding(getBinding('toggleSidebar')))
const updaterTitle = computed(() => {
  if (isInstalling.value) return 'Installing update'
  if (isChecking.value) return 'Checking for updates'
  return 'Check for updates'
})
</script>

<template>
  <div
    data-tauri-drag-region
    class="flex h-8 shrink-0 items-center justify-between border-b border-border bg-muted/40 px-2 select-none z-50 relative"
    @dblclick="win.toggleMaximize()"
  >
    <!-- App identity -->
    <div class="flex items-center gap-1.5">
      <div data-tauri-drag-region class="flex items-center gap-1.5 pointer-events-none">
        <div class="size-2.5 rounded-full bg-primary/30 flex items-center justify-center">
          <div class="size-1 rounded-full bg-primary" />
        </div>
        <span class="text-[10px] font-black uppercase tracking-[0.25em] text-muted-foreground/60">
          DB Viewer
        </span>
      </div>
      <button
        class="flex size-6 items-center justify-center rounded hover:bg-muted/60 transition-colors text-muted-foreground/40 hover:text-muted-foreground"
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
        class="flex size-6 items-center justify-center rounded hover:bg-muted/60 transition-colors text-muted-foreground/50 hover:text-muted-foreground"
        :class="{ 'pointer-events-none opacity-70': isChecking || isInstalling }"
        :title="updaterTitle"
        :disabled="isChecking || isInstalling"
        @click="checkForUpdates"
      >
        <LoaderCircleIcon v-if="isChecking || isInstalling" class="size-3 animate-spin" />
        <DownloadIcon v-else class="size-3" />
      </button>
      <button
        class="flex size-6 items-center justify-center rounded hover:bg-muted/60 transition-colors text-muted-foreground/50 hover:text-muted-foreground"
        title="Keyboard shortcuts"
        @click="emit('open-keybindings')"
      >
        <KeyboardIcon class="size-3" />
      </button>
      <div class="w-px h-3.5 bg-border/50 mx-1" />
      <button
        class="flex size-6 items-center justify-center rounded hover:bg-muted/60 transition-colors text-muted-foreground/50 hover:text-muted-foreground"
        @click="win.minimize()"
        title="Minimize"
      >
        <MinusIcon class="size-3" />
      </button>
      <button
        class="flex size-6 items-center justify-center rounded hover:bg-muted/60 transition-colors text-muted-foreground/50 hover:text-muted-foreground"
        @click="win.toggleMaximize()"
        title="Maximize"
      >
        <SquareIcon class="size-3" />
      </button>
      <button
        class="flex size-6 items-center justify-center rounded hover:bg-destructive hover:text-white transition-colors text-muted-foreground/50"
        @click="win.close()"
        title="Close"
      >
        <XIcon class="size-3" />
      </button>
    </div>
  </div>
</template>
