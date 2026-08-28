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
            aria-label="Database name"
            :model-value="name"
            autocomplete="off"
            autocapitalize="off"
            autofocus
            spellcheck="false"
            :disabled="isCreating"
            @update:model-value="emit('update:name', String($event))"
          />
        </div>

        <div class="grid grid-cols-2 gap-3">
          <div class="space-y-2 min-w-0">
            <Label for="new-database-character-set">Character Set</Label>
            <Select
              :model-value="characterSet"
              :disabled="isCreating || isLoadingOptions || !options"
              @update:model-value="emit('update:characterSet', String($event))"
            >
              <SelectTrigger id="new-database-character-set" class="w-full">
                <SelectValue :placeholder="isLoadingOptions ? 'Loading...' : 'Server default'" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem :value="DATABASE_OPTION_DEFAULT">
                  Default
                </SelectItem>
                <SelectItem v-for="item in characterSets" :key="item" :value="item">
                  {{ item }}
                </SelectItem>
              </SelectContent>
            </Select>
          </div>

          <div class="space-y-2 min-w-0">
            <Label for="new-database-collation">Collation</Label>
            <Select
              :model-value="collation"
              :disabled="isCreating || isLoadingOptions || !options"
              @update:model-value="emit('update:collation', String($event))"
            >
              <SelectTrigger id="new-database-collation" class="w-full">
                <SelectValue :placeholder="isLoadingOptions ? 'Loading...' : 'Server default'" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem :value="DATABASE_OPTION_DEFAULT">
                  Default
                </SelectItem>
                <SelectItem v-for="item in collations" :key="item.name" :value="item.name">
                  {{ item.name }}
                </SelectItem>
              </SelectContent>
            </Select>
          </div>
        </div>

        <div v-if="optionsError" class="space-y-1 text-xs text-destructive">
          <p>Could not load character sets and collations. The server defaults will be used.</p>
          <p class="break-words opacity-80">{{ optionsError }}</p>
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
import { computed } from "vue";
import { DatabaseIcon, PlusIcon } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

interface DatabaseCreationOptions {
  defaultCharacterSet: string;
  defaultCollation: string;
  collations: Array<{ name: string; characterSet: string; isDefault: boolean }>;
}

const DATABASE_OPTION_DEFAULT = "__server_default__";

const props = defineProps<{
  open: boolean;
  connectionName: string;
  name: string;
  characterSet: string;
  collation: string;
  options: DatabaseCreationOptions | null;
  isLoadingOptions: boolean;
  optionsError: string;
  isCreating: boolean;
}>();

const characterSets = computed(() => [
  ...new Set(props.options?.collations.map((option) => option.characterSet) ?? []),
]);
const collations = computed(() =>
  props.characterSet === DATABASE_OPTION_DEFAULT
    ? []
    : (props.options?.collations.filter(
        (option) => option.characterSet === props.characterSet,
      ) ?? []),
);
const emit = defineEmits<{
  "update:open": [val: boolean];
  "update:name": [val: string];
  "update:characterSet": [val: string];
  "update:collation": [val: string];
  create: [];
}>();

</script>
