<script setup lang="ts">
import { CheckCircleIcon, XCircleIcon, InfoIcon, XIcon } from 'lucide-vue-next'
import { useToast } from '@/composables/useToast'

const { toasts, dismiss } = useToast()
</script>

<template>
  <Teleport to="body">
    <div class="fixed bottom-10 right-4 z-[200] flex flex-col gap-2 pointer-events-none">
      <TransitionGroup
        enter-active-class="transition-all duration-300 ease-out"
        enter-from-class="opacity-0 translate-x-4 scale-95"
        enter-to-class="opacity-100 translate-x-0 scale-100"
        leave-active-class="transition-all duration-200 ease-in"
        leave-from-class="opacity-100 translate-x-0 scale-100"
        leave-to-class="opacity-0 translate-x-4 scale-95"
      >
        <div
          v-for="toast in toasts"
          :key="toast.id"
          class="pointer-events-auto flex items-start gap-3 rounded-lg border shadow-xl px-4 py-3 min-w-[280px] max-w-[400px]"
          :class="{
            'bg-card border-green-500/20': toast.type === 'success',
            'bg-card border-destructive/20': toast.type === 'error',
            'bg-card border-border': toast.type === 'info',
          }"
        >
          <CheckCircleIcon
            v-if="toast.type === 'success'"
            class="size-4 shrink-0 text-green-500 mt-0.5"
          />
          <XCircleIcon
            v-else-if="toast.type === 'error'"
            class="size-4 shrink-0 text-destructive mt-0.5"
          />
          <InfoIcon
            v-else
            class="size-4 shrink-0 text-primary mt-0.5"
          />

          <div class="flex-1 min-w-0">
            <p class="text-sm font-semibold text-foreground leading-tight">{{ toast.title }}</p>
            <p v-if="toast.message" class="text-[11px] text-muted-foreground mt-0.5 leading-snug">
              {{ toast.message }}
            </p>
          </div>

          <button
            class="shrink-0 size-5 flex items-center justify-center rounded text-muted-foreground/50 hover:text-muted-foreground hover:bg-muted/40 transition-colors mt-0.5"
            @click="dismiss(toast.id)"
          >
            <XIcon class="size-3" />
          </button>
        </div>
      </TransitionGroup>
    </div>
  </Teleport>
</template>
