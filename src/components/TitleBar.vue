<script setup lang="ts">
import { getCurrentWindow } from '@tauri-apps/api/window'
import { MinusIcon, SquareIcon, XIcon } from 'lucide-vue-next'

const win = getCurrentWindow()
</script>

<template>
  <div
    data-tauri-drag-region
    class="flex h-8 shrink-0 items-center justify-between border-b bg-muted/80 backdrop-blur-md px-3 select-none z-50 relative"
    @dblclick="win.toggleMaximize()"
  >
    <div data-tauri-drag-region class="flex items-center gap-2 pointer-events-none">
      <div class="size-3 rounded-full bg-primary/20 flex items-center justify-center">
        <div class="size-1 rounded-full bg-primary"></div>
      </div>
      <span class="text-[10px] font-black uppercase tracking-[0.2em] text-muted-foreground">DB Viewer</span>
    </div>
    
    <div class="flex items-center gap-1">
      <button
        class="flex size-6 items-center justify-center rounded-md hover:bg-muted transition-colors text-muted-foreground"
        @click="win.minimize()"
      >
        <MinusIcon class="size-3" />
      </button>
      <button
        class="flex size-6 items-center justify-center rounded-md hover:bg-muted transition-colors text-muted-foreground"
        @click="win.toggleMaximize()"
      >
        <SquareIcon class="size-3" />
      </button>
      <button
        class="flex size-6 items-center justify-center rounded-md hover:bg-destructive hover:text-white transition-colors text-muted-foreground"
        @click="win.close()"
      >
        <XIcon class="size-3" />
      </button>
    </div>
  </div>
</template>
