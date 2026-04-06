<script setup lang="ts">
import { DatabaseIcon, ServerIcon, ShieldCheckIcon } from 'lucide-vue-next'
import { Badge } from '@/components/ui/badge'
import type { Connection, Environment } from '@/types/connection'

defineProps<{
  connections: Connection[]
}>()

const emit = defineEmits<{
  'connect': [conn: Connection]
  'context-menu': [e: MouseEvent, conn: Connection]
}>()

const getEnvColor = (env: Environment): string => {
  switch (env) {
    case 'PRODUCTION': return 'bg-red-500/10 text-red-500 border-red-500/20'
    case 'STAGING': return 'bg-orange-500/10 text-orange-500 border-orange-500/20'
    case 'DEV': return 'bg-blue-500/10 text-blue-500 border-blue-500/20'
    default: return 'bg-green-500/10 text-green-500 border-green-500/20'
  }
}
</script>

<template>
  <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
    <div
      v-for="conn in connections"
      :key="conn.id"
      class="group relative flex items-center gap-4 p-5 rounded-2xl border bg-card hover:border-primary/50 transition-all cursor-pointer shadow-sm hover:shadow-md select-none"
      @dblclick="emit('connect', conn)"
      @contextmenu="emit('context-menu', $event, conn)"
    >
      <div class="size-14 rounded-xl bg-primary/10 flex items-center justify-center text-primary shrink-0 transition-transform group-hover:scale-105">
        <DatabaseIcon class="size-7" />
      </div>
      <div class="flex-1 min-w-0">
        <div class="flex items-center gap-2 mb-1">
          <span class="font-bold text-lg truncate">{{ conn.name }}</span>
          <Badge variant="outline" :class="[getEnvColor(conn.environment), 'text-[10px] uppercase py-0 px-2 h-4.5 font-bold tracking-wider']">
            {{ conn.environment }}
          </Badge>
        </div>
        <div class="text-sm text-muted-foreground truncate flex items-center gap-2">
          <ServerIcon class="size-3.5" />
          {{ conn.mysql.user }}@{{ conn.mysql.host }}
          <template v-if="conn.ssh">
            <div class="h-3 w-px bg-border mx-1"></div>
            <ShieldCheckIcon class="size-3.5 text-primary" />
            <span class="text-xs font-medium">SSH</span>
          </template>
        </div>
      </div>
    </div>

    <!-- Empty State -->
    <div v-if="connections.length === 0" class="col-span-full py-20 flex flex-col items-center justify-center text-center text-muted-foreground">
      <DatabaseIcon class="size-12 opacity-20 mb-4" />
      <p class="text-lg font-medium">No connections found</p>
      <p class="text-sm">Try a different search or create a new connection</p>
    </div>
  </div>
</template>
