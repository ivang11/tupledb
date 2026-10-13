<template>
  <Dialog :open="open" @update:open="!$event && emit('cancel')"><DialogContent class="sm:max-w-xl">
    <DialogHeader><DialogTitle>Query parameters</DialogTitle><DialogDescription>Provide values for named parameters. Numeric values retain their exact precision.</DialogDescription></DialogHeader>
    <div class="space-y-2 max-h-80 overflow-auto"><div v-for="parameter in parameters" :key="parameter.name" class="flex items-center gap-2 text-xs">
      <label class="w-28 font-mono truncate">:{{ parameter.name }}</label><select v-model="parameter.type" class="bg-background border rounded px-2 py-1"><option value="text">Text</option><option value="number">Number</option><option value="boolean">Boolean</option><option value="null">NULL</option></select>
      <input v-if="parameter.type !== 'null'" v-model="parameter.value" :aria-label="parameter.name" class="flex-1 min-w-0 bg-background border rounded px-2 py-1.5" />
    </div></div>
    <p v-if="error" class="text-xs text-destructive">{{ error }}</p>
    <DialogFooter><Button variant="outline" @click="emit('cancel')">Cancel</Button><Button @click="emit('execute')">Execute</Button></DialogFooter>
  </DialogContent></Dialog>
</template>
<script setup lang="ts">
import type { QueryParameter } from '@/lib/sqlExecution'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
defineProps<{ open: boolean; parameters: QueryParameter[]; error: string }>()
const emit = defineEmits<{ cancel: []; execute: [] }>()
</script>
