<template>
  <Dialog :open="props.open" @update:open="emit('update:open', $event)">
    <DialogContent
      class="max-w-lg"
      @keydown="onRecordKeydown"
    >
      <DialogHeader>
        <DialogTitle class="flex items-center gap-2 text-sm">
          <KeyboardIcon class="size-4" />
          Keyboard Shortcuts
        </DialogTitle>
        <DialogDescription class="text-xs">
          Click a binding to reassign it. Changes apply immediately.
        </DialogDescription>
      </DialogHeader>

      <!-- Keybinding list -->
      <div class="space-y-4 py-1 max-h-105 overflow-y-auto pr-1">
        <template v-for="[category, defs] in categories" :key="category">
          <div>
            <div class="text-[10px] font-semibold uppercase tracking-widest text-muted-foreground mb-2 px-1">
              {{ category }}
            </div>
            <div class="space-y-1">
              <div
                v-for="def in defs"
                :key="def.action"
                class="flex items-center justify-between px-2 py-1.5 rounded-md hover:bg-muted/50 group"
              >
                <div class="min-w-0">
                  <div class="text-xs font-medium">{{ def.label }}</div>
                  <div class="text-[11px] text-muted-foreground">{{ def.description }}</div>
                </div>

                <div class="flex items-center gap-1.5 shrink-0 ml-4">
                  <!-- Recording in progress for this action -->
                  <template v-if="recording === def.action">
                    <kbd
                      class="inline-flex items-center gap-1 px-2 py-0.5 rounded border border-primary bg-primary/10 text-[11px] font-mono text-primary min-w-22.5 justify-center animate-pulse"
                    >
                      {{ recordingKey ? formatKeybinding(recordingKey) : 'Press keys…' }}
                    </kbd>
                    <Button
                      variant="ghost"
                      size="icon"
                      class="size-6 text-muted-foreground"
                      :disabled="!recordingKey"
                      @click="confirmRecording"
                      title="Confirm"
                    >
                      <CheckIcon class="size-3" />
                    </Button>
                    <Button
                      variant="ghost"
                      size="icon"
                      class="size-6 text-muted-foreground"
                      @click="cancelRecording"
                      title="Cancel"
                    >
                      <XIcon class="size-3" />
                    </Button>
                  </template>

<!-- Normal display -->
                  <template v-else>
                    <button
                      class="inline-flex items-center gap-1 px-2 py-0.5 rounded border border-input bg-muted/40 text-[11px] font-mono text-foreground min-w-22.5 justify-center hover:border-primary hover:bg-primary/10 hover:text-primary transition-colors cursor-pointer"
                      :class="{ 'border-primary/50 text-primary': isCustomized(def.action) }"
                      @click="startRecording(def.action)"
                      :title="`Click to reassign (current: ${kb.getBinding(def.action)})`"
                    >
                      {{ formatKeybinding(kb.getBinding(def.action)) }}
                    </button>
                    <Button
                      v-if="isCustomized(def.action)"
                      variant="ghost"
                      size="icon"
                      class="size-6 text-muted-foreground opacity-0 group-hover:opacity-100 transition-opacity"
                      @click="kb.resetBinding(def.action)"
                      title="Reset to default"
                    >
                      <RotateCcwIcon class="size-3" />
                    </Button>
                    <div v-else class="size-6" />
                  </template>
                </div>
              </div>
            </div>
          </div>
        </template>
      </div>

      <!-- Import error -->
      <p v-if="importError" class="text-xs text-destructive px-1">{{ importError }}</p>

      <!-- Footer actions -->
      <div class="flex items-center justify-between pt-2 border-t">
        <div class="flex gap-2">
          <Button variant="outline" size="sm" class="h-7 text-xs gap-1.5" @click="handleImport">
            <UploadIcon class="size-3" />
            Import JSON
          </Button>
          <Button variant="outline" size="sm" class="h-7 text-xs gap-1.5" @click="handleExport">
            <DownloadIcon class="size-3" />
            Export JSON
          </Button>
        </div>
        <Button
          variant="ghost"
          size="sm"
          class="h-7 text-xs text-muted-foreground"
          @click="kb.resetAll()"
          title="Reset all shortcuts to defaults"
        >
          <RotateCcwIcon class="size-3 mr-1" />
          Reset all
        </Button>
      </div>
    </DialogContent>
  </Dialog>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'
import { KeyboardIcon, RotateCcwIcon, DownloadIcon, UploadIcon, XIcon, CheckIcon } from 'lucide-vue-next'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
} from '@/components/ui/dialog'
import {
  useKeybindings,
  keybindingFromEvent,
  formatKeybinding,
} from '@/composables/useKeybindings'
import { KEYBINDING_DEFS, type KeybindingAction, type KeybindingCategory } from '@/config/keybindingActions'
import { save as saveDialog, open as openDialog } from '@tauri-apps/plugin-dialog'
import { writeTextFile, readTextFile } from '@tauri-apps/plugin-fs'

const props = defineProps<{ open: boolean }>()
const emit = defineEmits<{ 'update:open': [val: boolean] }>()

const kb = useKeybindings()

// ── Recording mode ────────────────────────────────────────────────────────────
const recording = ref<KeybindingAction | null>(null)
const recordingKey = ref<string | null>(null)

function startRecording(action: KeybindingAction) {
  recording.value = action
  recordingKey.value = null
}

function onRecordKeydown(event: KeyboardEvent) {
  if (!recording.value) return
  event.preventDefault()
  event.stopPropagation()

  if (event.key === 'Escape') {
    recording.value = null
    recordingKey.value = null
    return
  }

  const captured = keybindingFromEvent(event)
  if (captured) {
    recordingKey.value = captured
  }
}

function confirmRecording() {
  if (recording.value && recordingKey.value) {
    kb.setBinding(recording.value, recordingKey.value)
  }
  recording.value = null
  recordingKey.value = null
}

function cancelRecording() {
  recording.value = null
  recordingKey.value = null
}

// ── Group defs by category ────────────────────────────────────────────────────
const categories = computed(() => {
  const map = new Map<KeybindingCategory, typeof KEYBINDING_DEFS>()
  for (const def of KEYBINDING_DEFS) {
    if (!map.has(def.category)) map.set(def.category, [])
    map.get(def.category)!.push(def)
  }
  return map
})

// ── Export / Import ───────────────────────────────────────────────────────────
const importError = ref<string | null>(null)

async function handleExport() {
  const path = await saveDialog({
    defaultPath: 'keybindings.json',
    filters: [{ name: 'JSON', extensions: ['json'] }],
  })
  if (!path) return
  await writeTextFile(path, kb.exportJson())
}

async function handleImport() {
  importError.value = null
  const path = await openDialog({
    filters: [{ name: 'JSON', extensions: ['json'] }],
  })
  if (!path || Array.isArray(path)) return
  try {
    const text = await readTextFile(path)
    kb.importJson(text)
  } catch (e) {
    importError.value = e instanceof Error ? e.message : 'Invalid file'
  }
}

function isCustomized(action: KeybindingAction): boolean {
  return action in kb.overrides.value
}
</script>
