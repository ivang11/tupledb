<template>
  <div class="fixed inset-0 z-60 pointer-events-none">
    <div
      class="pointer-events-auto absolute left-1 right-1 top-0 h-1 cursor-n-resize"
      @mousedown="startResize('North', $event)"
    />
    <div
      class="pointer-events-auto absolute bottom-0 left-1 right-1 h-1 cursor-s-resize"
      @mousedown="startResize('South', $event)"
    />
    <div
      class="pointer-events-auto absolute bottom-1 left-0 top-1 w-1 cursor-w-resize"
      @mousedown="startResize('West', $event)"
    />
    <div
      class="pointer-events-auto absolute bottom-1 right-0 top-1 w-1 cursor-e-resize"
      @mousedown="startResize('East', $event)"
    />
    <div
      class="pointer-events-auto absolute left-0 top-0 size-2 cursor-nw-resize"
      @mousedown="startResize('NorthWest', $event)"
    />
    <div
      class="pointer-events-auto absolute right-0 top-0 size-2 cursor-ne-resize"
      @mousedown="startResize('NorthEast', $event)"
    />
    <div
      class="pointer-events-auto absolute bottom-0 left-0 size-2 cursor-sw-resize"
      @mousedown="startResize('SouthWest', $event)"
    />
    <div
      class="pointer-events-auto absolute bottom-0 right-0 size-2 cursor-se-resize"
      @mousedown="startResize('SouthEast', $event)"
    />
  </div>
</template>

<script setup lang="ts">
import { getCurrentWindow } from '@tauri-apps/api/window'

type ResizeDirection =
  | 'East'
  | 'North'
  | 'NorthEast'
  | 'NorthWest'
  | 'South'
  | 'SouthEast'
  | 'SouthWest'
  | 'West'

const win = getCurrentWindow()

function startResize(direction: ResizeDirection, event: MouseEvent) {
  event.preventDefault()
  event.stopPropagation()
  void win.startResizeDragging(direction)
}
</script>
