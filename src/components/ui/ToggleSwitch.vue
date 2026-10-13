<template>
  <button
    :id="id"
    type="button"
    role="switch"
    :aria-checked="modelValue"
    :aria-label="label"
    :disabled="disabled"
    class="toggle-switch"
    :class="modelValue ? 'bg-(--acc)' : 'bg-(--bg-3)'"
    @click="emit('update:modelValue', !modelValue)"
  >
    <span />
  </button>
</template>

<script setup lang="ts">
defineProps<{
  modelValue: boolean
  id?: string
  label?: string
  disabled?: boolean
}>()
const emit = defineEmits<{ 'update:modelValue': [value: boolean] }>()
</script>

<style scoped>
.toggle-switch {
  position: relative;
  height: 18px;
  width: 32px;
  flex-shrink: 0;
  border-radius: 999px;
  transition: background-color 120ms ease;
}
.toggle-switch:disabled {
  opacity: 0.5;
}
.toggle-switch:focus-visible {
  outline: 2px solid var(--ring);
  outline-offset: 2px;
}
.toggle-switch > span {
  position: absolute;
  left: 2px;
  top: 2px;
  width: 14px;
  height: 14px;
  border-radius: 999px;
  background: #fff;
  transition: transform 120ms ease;
}
.toggle-switch[aria-checked="true"] > span {
  transform: translateX(14px);
}
</style>
