<script setup lang="ts">
import { ref } from "vue";
import { Trash2Icon, CheckIcon } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from "@/components/ui/dialog";

const props = defineProps<{
  open: boolean;
  count: number;
  isExecuting: boolean;
  type?: 'drop' | 'truncate';
}>();

const emit = defineEmits<{
  "update:open": [val: boolean];
  confirm: [disableFkChecks: boolean];
}>();

const disableFkChecks = ref(false);
</script>

<template>
  <Dialog
    :open="open"
    @update:open="(val: boolean) => !val && !isExecuting && emit('update:open', val)"
  >
    <DialogContent class="sm:max-w-sm">
      <DialogHeader>
        <DialogTitle class="flex items-center gap-2">
          <Trash2Icon class="size-5 text-destructive" />
          {{ type === 'truncate' ? 'Truncate' : 'Delete' }} Tables
        </DialogTitle>
        <DialogDescription class="pt-2">
          Are you sure you want to
          <strong class="text-foreground">{{ type === 'truncate' ? 'TRUNCATE' : 'DROP' }}</strong>
          <strong class="text-foreground">{{ count }}</strong>
          table{{ count !== 1 ? "s" : "" }}?<br />
          <span class="text-muted-foreground mt-2 block">
            {{ type === 'truncate' ? 'All data will be lost. This cannot be undone.' : 'All data will be permanently deleted. This cannot be undone.' }}
          </span>
        </DialogDescription>
      </DialogHeader>

      <div class="py-4">
        <button
          @click="disableFkChecks = !disableFkChecks"
          class="flex items-start gap-3 cursor-pointer w-full group bg-muted/20 p-3 rounded-lg border border-border hover:bg-muted/40 transition-colors text-left"
        >
          <div
            class="mt-0.5 shrink-0 flex items-center justify-center size-4 rounded border-2 border-border group-hover:border-foreground/40 transition-colors"
            :class="{ 'bg-destructive border-destructive': disableFkChecks }"
          >
            <CheckIcon
              v-if="disableFkChecks"
              class="size-3 text-destructive-foreground"
            />
          </div>
          <div class="flex flex-col min-w-0">
            <span class="text-sm font-bold text-foreground"
              >Disable Foreign Key Checks</span
            >
            <span class="text-xs text-muted-foreground leading-relaxed mt-0.5"
              >Allows dropping tables referenced by other tables. Use with
              caution.</span
            >
          </div>
        </button>
      </div>

      <DialogFooter class="gap-2 sm:gap-0 mt-2">
        <Button
          variant="ghost"
          @click="emit('update:open', false)"
          :disabled="isExecuting"
          >Cancel</Button
        >
        <Button
          variant="destructive"
          @click="emit('confirm', disableFkChecks)"
          :disabled="isExecuting"
        >
          <span v-if="!isExecuting">{{ type === 'truncate' ? 'Truncate' : 'Delete' }} {{ count }} Table{{ count !== 1 ? 's' : '' }}</span>
          <span v-else>{{ type === 'truncate' ? 'Truncating' : 'Deleting' }}...</span>
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
