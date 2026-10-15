<template>
  <Teleport to="body">
    <div data-notice-stack class="pointer-events-none fixed bottom-6 right-0 z-150 flex max-h-[calc(100vh-2rem)] w-[392px] max-w-full flex-col gap-2 overflow-y-auto p-4 custom-scrollbar">
      <slot />
    </div>
  </Teleport>
</template>
