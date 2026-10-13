<template>
  <div class="border-l border-border pl-4 py-1">
    <div class="flex flex-wrap items-center gap-2 text-xs">
      <span class="font-semibold">{{ label }}</span>
      <span v-for="(value, key) in details" :key="key" class="text-muted-foreground font-mono">{{ key }}: {{ value }}</span>
    </div>
    <ExplainPlanNode v-for="child in children" :key="child.key" :node="child.value" :name="child.key" />
  </div>
</template>
<script setup lang="ts">
import { computed } from 'vue'
const props = defineProps<{ node: Record<string, any>; name: string }>()
const label = computed(() => props.node['Node Type'] ?? props.node.table_name ?? props.name.replaceAll('_', ' '))
const details = computed(() => Object.fromEntries(Object.entries(props.node).filter(([key, value]) => value != null && typeof value !== 'object' && !['Node Type', 'table_name'].includes(key))))
const children = computed(() => Object.entries(props.node).flatMap(([key, value]) => {
  if (!value || typeof value !== 'object') return []
  return Array.isArray(value) ? value.filter(item => item && typeof item === 'object').map((item, index) => ({ key: `${key} ${index + 1}`, value: item })) : [{ key, value }]
}))
</script>
