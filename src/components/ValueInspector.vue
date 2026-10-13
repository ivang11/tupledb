<template>
  <Dialog :open="open" @update:open="emit('update:open', $event)">
    <DialogContent class="sm:max-w-3xl max-h-[85vh] flex flex-col">
      <DialogHeader><DialogTitle>{{ title }}</DialogTitle><DialogDescription>{{ description }}</DialogDescription></DialogHeader>
      <div class="flex items-center gap-2 text-xs">
        <button class="px-2 py-1 rounded hover:bg-muted" :class="formatted ? 'bg-muted' : ''" :disabled="!isJson" @click="formatted = !formatted">{{ formatted ? 'Raw value' : 'Format JSON' }}</button>
        <button class="px-2 py-1 rounded hover:bg-muted ml-auto" @click="copyText(raw)">Copy exact value</button>
      </div>
      <pre class="min-h-24 overflow-auto whitespace-pre-wrap break-words rounded border bg-muted/30 p-4 font-mono text-xs select-text">{{ displayed }}</pre>
    </DialogContent>
  </Dialog>
</template>
<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription } from '@/components/ui/dialog'
import { copyText } from '@/lib/clipboard'
const props = defineProps<{ open: boolean; title: string; value: unknown }>()
const emit = defineEmits<{ 'update:open': [value: boolean] }>()
const formatted = ref(true)
const raw = computed(() => props.value === null ? 'NULL' : typeof props.value === 'object' ? JSON.stringify(props.value) : String(props.value ?? ''))
const json = computed(() => { try { return { valid: true, value: JSON.parse(raw.value) } } catch { return { valid: false, value: null } } })
const isJson = computed(() => json.value.valid && typeof json.value.value === 'object' && json.value.value !== null)
const displayed = computed(() => formatted.value && isJson.value ? JSON.stringify(json.value.value, null, 2) : raw.value)
const description = computed(() => props.value === null ? 'SQL NULL' : `${raw.value.length.toLocaleString()} characters${isJson.value ? ' · JSON' : ''}`)
watch(() => props.open, () => { formatted.value = true })
</script>
