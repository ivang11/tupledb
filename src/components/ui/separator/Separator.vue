<template>
  <Separator
    data-slot="separator"
    v-bind="delegatedProps"
    :class="
      cn(
        'bg-border shrink-0 data-[orientation=horizontal]:h-px data-[orientation=horizontal]:w-full data-[orientation=vertical]:h-full data-[orientation=vertical]:w-px',
        props.class,
      )
    "
  />
</template>

<script setup lang="ts">
import { reactiveOmit } from "@vueuse/core";
import { Separator, type SeparatorProps } from "reka-ui";
import type { PropType } from "vue";
import { cn } from "@/lib/utils";

const props = defineProps({
  orientation: { type: String as PropType<SeparatorProps["orientation"]>, required: false, default: "horizontal" },
  decorative: { type: Boolean, required: false, default: true },
  asChild: { type: Boolean, required: false },
  as: { type: null, required: false },
  class: {
    type: [Boolean, null, String, Object, Array],
    required: false,
    skipCheck: true,
  },
});

const delegatedProps = reactiveOmit(props, "class");
</script>
