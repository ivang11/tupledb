<template>
  <div class="overflow-auto flex-1 p-4 select-text">
    <div class="flex items-center justify-between mb-4"><div><h3 class="text-sm font-semibold">Execution plan</h3><p class="text-xs text-muted-foreground mt-1">Planner estimates · the query was not executed</p></div><button class="text-xs px-2 py-1 hover:bg-muted rounded" @click="copyText(JSON.stringify(plan, null, 2))">Copy JSON</button></div>
    <ExplainPlanNode v-if="plan && typeof plan === 'object'" :node="Array.isArray(plan) ? { Plan: plan } : plan" name="Query" />
    <pre v-else class="text-xs whitespace-pre-wrap">{{ plan }}</pre>
  </div>
</template>
<script setup lang="ts">
import { computed } from 'vue'
import ExplainPlanNode from './ExplainPlanNode.vue'
import { copyText } from '@/lib/clipboard'
import { rowValue } from '@/lib/rowAccess'
const props = defineProps<{ result: { rows: any[]; columns: { name: string }[] } }>()
const plan = computed(() => {
  const value = props.result.rows[0] ? rowValue(props.result.rows[0], props.result.columns[0]?.name, props.result.columns) : null
  if (typeof value !== 'string') return value
  try { return JSON.parse(value) } catch { return value }
})
</script>
