<script setup lang="ts">
import { ref, computed } from 'vue'
import { ChevronUp, X } from 'lucide-vue-next'

const props = defineProps<{
  open: boolean
  title: string
  progress: { current: number; total: number; status: string }
  dismissable?: boolean
  stackIndex?: number
}>()

const emit = defineEmits<{
  dismiss: []
}>()

const minimized = ref(false)

// Stack panels vertically when multiple are active (each unit ≈ 110px panel height + 8px gap)
const bottomOffset = computed(() => `${(props.stackIndex ?? 0) * 118 + 16}px`)
</script>

<template>
  <Transition
    enter-active-class="transition-all duration-200 ease-out"
    enter-from-class="opacity-0 translate-y-4"
    enter-to-class="opacity-100 translate-y-0"
    leave-active-class="transition-all duration-150 ease-in"
    leave-from-class="opacity-100 translate-y-0"
    leave-to-class="opacity-0 translate-y-4"
  >
    <div
      v-if="open"
      class="fixed right-4 z-50 w-72 rounded-lg border bg-background shadow-lg overflow-hidden"
      :style="{ bottom: bottomOffset }"
    >
      <!-- Header -->
      <div class="flex items-center justify-between px-4 py-3 border-b bg-muted/40">
        <span class="text-sm font-medium truncate">{{ title }}</span>
        <div class="flex items-center gap-1 ml-2 shrink-0">
          <button
            class="rounded p-0.5 text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
            @click="minimized = !minimized"
            :title="minimized ? 'Expand' : 'Minimize'"
          >
            <ChevronUp
              class="size-3.5 transition-transform duration-200"
              :class="{ 'rotate-180': minimized }"
            />
          </button>
          <button
            v-if="dismissable"
            class="rounded p-0.5 text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
            title="Dismiss"
            @click="emit('dismiss')"
          >
            <X class="size-3.5" />
          </button>
        </div>
      </div>

      <!-- Body -->
      <Transition
        enter-active-class="transition-all duration-200 ease-out"
        enter-from-class="opacity-0 max-h-0"
        enter-to-class="opacity-100 max-h-40"
        leave-active-class="transition-all duration-150 ease-in"
        leave-from-class="opacity-100 max-h-40"
        leave-to-class="opacity-0 max-h-0"
      >
        <div v-if="!minimized" class="px-4 py-3">
          <div class="flex items-center justify-between mb-2">
            <span class="text-xs text-muted-foreground truncate pr-2">{{ progress.status }}</span>
            <span v-if="progress.total" class="text-xs font-semibold tabular-nums shrink-0">
              {{ Math.round((progress.current / progress.total) * 100) }}%
            </span>
          </div>
          <div class="h-1.5 w-full bg-muted rounded-full overflow-hidden">
            <div
              class="h-full bg-primary transition-all duration-300 ease-out"
              :style="{ width: `${progress.total ? (progress.current / progress.total) * 100 : 0}%` }"
            />
          </div>
          <div class="mt-2 text-[10px] text-muted-foreground text-right tabular-nums">
            {{ progress.current.toLocaleString() }} / {{ progress.total.toLocaleString() }} statements
          </div>
        </div>
      </Transition>
    </div>
  </Transition>
</template>
