<script setup lang="ts">
import { ref } from "vue";
import { Trash2Icon, XIcon, CheckIcon } from "lucide-vue-next";
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
  type: "truncate" | "drop";
  tableName: string;
  isExecuting: boolean;
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
    @update:open="(val) => !val && !isExecuting && emit('update:open', val)"
  >
    <DialogContent class="sm:max-w-[420px]">
      <DialogHeader>
        <DialogTitle class="flex items-center gap-2">
          <Trash2Icon
            v-if="type === 'truncate'"
            class="size-5 text-destructive"
          />
          <XIcon v-else class="size-5 text-destructive" />
          {{ type === "truncate" ? "Truncate" : "Drop" }} Table
        </DialogTitle>
        <DialogDescription class="pt-2">
          Are you sure you want to
          <strong class="text-foreground">{{
            type === "truncate" ? "TRUNCATE" : "DROP"
          }}</strong>
          table
          <code
            class="bg-muted px-1.5 py-0.5 rounded font-bold text-foreground"
            >{{ tableName }}</code
          >?<br />
          <span
            v-if="type === 'truncate'"
            class="text-muted-foreground mt-2 block"
            >All data will be lost. This cannot be undone.</span
          >
          <span v-else class="text-muted-foreground mt-2 block"
            >This table and all its data will be permanently deleted.</span
          >
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
              >Allows truncating/dropping tables referenced by other tables. Use
              with caution.</span
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
          class="min-w-[100px] font-bold"
        >
          {{ isExecuting ? "Executing..." : "Confirm" }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
