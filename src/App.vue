<template>
  <template v-if="benchmarkBuild">
    <BenchmarkGrid
      v-if="benchmarkConfig"
      :row-count="benchmarkConfig.rows"
      :column-count="benchmarkConfig.columns"
      :settle-ms="benchmarkConfig.settleMs"
    />
    <pre v-else class="p-4 text-sm">{{ benchmarkError ?? 'Loading benchmark…' }}</pre>
  </template>
  <div v-else class="h-screen w-screen overflow-hidden bg-background">
    <AppLayout />
  </div>
</template>

<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { defineAsyncComponent, onMounted, ref } from 'vue'

const benchmarkBuild = import.meta.env.VITE_TUPLEDB_BENCHMARK === '1'
const benchmarkConfig = ref<{ rows: number; columns: number; settleMs: number } | null>(null)
const benchmarkError = ref<string | null>(null)
const AppLayout = defineAsyncComponent(() => import('@/layouts/AppLayout.vue'))
const BenchmarkGrid = defineAsyncComponent(() => import('@/components/BenchmarkGrid.vue'))

onMounted(async () => {
  if (!benchmarkBuild) return
  try {
    benchmarkConfig.value = await invoke('benchmark_config')
  } catch (error) {
    benchmarkError.value = error instanceof Error ? error.message : String(error)
  }
})
</script>
