<template>
  <Dialog
    :open="open"
    @update:open="(val: boolean) => !val && !isCreating && emit('update:open', val)"
  >
    <DialogContent class="sm:max-w-105">
      <form
        class="space-y-4"
        @submit.prevent="emit('create')"
      >
        <DialogHeader>
          <DialogTitle class="flex items-center gap-2">
            <DatabaseIcon class="size-5 text-primary" />
            New Database
          </DialogTitle>
          <DialogDescription>
            Create a database in
            <strong class="text-foreground">{{ connectionName }}</strong>.
          </DialogDescription>
        </DialogHeader>

        <div class="space-y-2">
          <Label for="new-database-name">Database Name</Label>
          <Input
            id="new-database-name"
            :model-value="name"
            placeholder="database_name"
            autocomplete="off"
            autocapitalize="off"
            autofocus
            spellcheck="false"
            :disabled="isCreating"
            @update:model-value="emit('update:name', String($event))"
          />
        </div>

        <DialogFooter class="gap-2 sm:gap-0">
          <Button
            type="button"
            variant="ghost"
            :disabled="isCreating"
            @click="emit('update:open', false)"
          >
            Cancel
          </Button>
          <Button
            type="submit"
            :disabled="isCreating || !name.trim()"
          >
            <PlusIcon v-if="!isCreating" class="size-4 mr-2" />
            {{ isCreating ? "Creating..." : "Create Database" }}
          </Button>
        </DialogFooter>
      </form>
    </DialogContent>
  </Dialog>
</template>

<script setup lang="ts">
import { DatabaseIcon, PlusIcon } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

defineProps<{
  open: boolean;
  connectionName: string;
  name: string;
  isCreating: boolean;
}>();

const emit = defineEmits<{
  "update:open": [val: boolean];
  "update:name": [val: string];
  create: [];
}>();

</script>

