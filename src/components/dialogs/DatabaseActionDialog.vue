<script setup lang="ts">
import { Trash2Icon } from "lucide-vue-next";
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
  databaseName: string;
  isExecuting: boolean;
}>();

const emit = defineEmits<{
  "update:open": [val: boolean];
  confirm: [];
}>();
</script>

<template>
  <Dialog
    :open="open"
    @update:open="(val) => !val && !isExecuting && emit('update:open', val)"
  >
    <DialogContent class="sm:max-w-[420px]">
      <DialogHeader>
        <DialogTitle class="flex items-center gap-2">
          <Trash2Icon class="size-5 text-destructive" />
          Drop Database
        </DialogTitle>
        <DialogDescription class="pt-2">
          Are you sure you want to
          <strong class="text-foreground">DROP</strong>
          database
          <code
            class="bg-muted px-1.5 py-0.5 rounded font-bold text-foreground"
            >{{ databaseName }}</code
          >?<br />
          <span class="text-muted-foreground mt-2 block"
            >This database and all its tables, data, and structure will be
            permanently deleted.</span
          >
        </DialogDescription>
      </DialogHeader>

      <DialogFooter class="gap-2 sm:gap-0 mt-2">
        <Button
          variant="ghost"
          @click="emit('update:open', false)"
          :disabled="isExecuting"
          >Cancel</Button
        >
        <Button
          variant="destructive"
          @click="emit('confirm')"
          :disabled="isExecuting"
        >
          <Trash2Icon v-if="!isExecuting" class="size-4 mr-2" />
          {{ isExecuting ? "Dropping..." : "Drop Database" }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
