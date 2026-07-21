<template>
  <div :class="viewportClass" v-bind="attrs">
    <slot />
  </div>
</template>

<script setup lang="ts">
import { computed, useAttrs, type HTMLAttributes } from "vue";
import { cn } from "@/lib/utils";

defineOptions({ inheritAttrs: false });

const props = defineProps<{
  class?: HTMLAttributes["class"];
}>();

const attrs = useAttrs();

const viewportClass = computed(() =>
  cn(
    "min-h-0 min-w-0 overflow-auto custom-scrollbar",
    props.class,
  ),
);
</script>

