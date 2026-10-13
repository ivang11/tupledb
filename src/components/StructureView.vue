<template>
  <div class="flex flex-col flex-1 min-h-0">
    <div class="flex h-7 shrink-0 items-center justify-between gap-4 border-b border-(--line-2) bg-(--bg-1) px-3 text-[11px]">
      <span class="font-medium text-(--fg-2)">
        Columns <span class="tabular-nums text-(--fg-4)">{{ tableStructure.length }}</span>
      </span>
      <span class="truncate text-(--fg-4)">
        {{ canEdit ? "Double-click a name or type to edit" : editDisabledReason }}
      </span>
    </div>

    <ScrollArea class="flex-1 min-h-0">
      <table v-if="tableStructure.length" class="structure-table w-full border-collapse text-[11.5px]">
        <thead>
          <tr>
            <th class="w-9 text-right">#</th>
            <th>Name</th>
            <th>Type</th>
            <th class="w-16">Nullable</th>
            <th>Default</th>
            <th>Extra</th>
            <th>Foreign key</th>
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="(col, index) in tableStructure"
            :key="col.field"
            class="group transition-colors"
            :class="editingCell?.column === col.field ? 'bg-(--bg-1)' : 'hover:bg-(--bg-1)'"
          >
            <td class="gutter text-right tabular-nums">{{ index + 1 }}</td>
            <td
              class="font-mono transition-colors"
              :class="[
                isFieldPending(col, 'name') ? 'bg-amber-500/10 text-amber-500' : 'text-(--fg-1)',
                canEdit ? 'cursor-text' : 'cursor-not-allowed',
              ]"
              :title="canEdit ? 'Double-click to edit the column name' : editDisabledReason"
              @dblclick="startEdit(col, 'name')"
            >
              <input
                v-if="isEditing(col, 'name')"
                :ref="setEditInput"
                v-model="draftValue"
                autocomplete="off"
                autocapitalize="none"
                autocorrect="off"
                spellcheck="false"
                class="block w-full min-w-0 rounded-[3px] border-0 bg-(--bg-0) p-0 font-mono text-[11.5px] leading-[inherit] text-inherit outline-none ring-1 ring-(--line-2) focus:ring-(--fg-5)"
                aria-label="Column name"
                @blur="commitEdit(col, 'name')"
                @keydown.enter.prevent="commitEdit(col, 'name')"
                @keydown.esc.prevent="cancelEdit"
              />
              <span v-else class="inline-flex items-center gap-2">
                {{ pendingValue(col, 'name') }}
                <span v-if="col.key === 'PRI'" class="key-badge text-(--pk)" title="Primary key">PK</span>
                <span v-else-if="col.key === 'UNI'" class="key-badge text-(--env-staging)" title="Unique">UQ</span>
                <span v-else-if="col.key === 'MUL'" class="key-badge text-(--fg-4)" title="Indexed">IX</span>
              </span>
            </td>
            <td
              class="font-mono transition-colors"
              :class="[
                isFieldPending(col, 'type') ? 'bg-amber-500/10 text-amber-500' : 'text-(--fg-2)',
                canEdit ? 'cursor-text' : 'cursor-not-allowed',
              ]"
              :title="canEdit ? 'Double-click to edit the column type' : editDisabledReason"
              @dblclick="startEdit(col, 'type')"
            >
              <input
                v-if="isEditing(col, 'type')"
                :ref="setEditInput"
                v-model="draftValue"
                autocomplete="off"
                autocapitalize="none"
                autocorrect="off"
                spellcheck="false"
                class="block w-full min-w-0 rounded-[3px] border-0 bg-(--bg-0) p-0 font-mono text-[11.5px] leading-[inherit] text-inherit outline-none ring-1 ring-(--line-2) focus:ring-(--fg-5)"
                aria-label="Column type"
                @blur="commitEdit(col, 'type')"
                @keydown.enter.prevent="commitEdit(col, 'type')"
                @keydown.esc.prevent="cancelEdit"
              />
              <span v-else>{{ pendingValue(col, 'type') }}</span>
            </td>
            <td class="text-center">
              <CheckIcon v-if="col.nullable" class="inline size-3 text-(--fg-2)" aria-label="Nullable" />
              <span v-else class="text-(--fg-5)" aria-label="Not nullable">—</span>
            </td>
            <td class="font-mono text-(--fg-3)">
              <span v-if="col.default_value === null" class="italic text-(--fg-5)">NULL</span>
              <span v-else>{{ col.default_value }}</span>
            </td>
            <td class="font-mono text-(--fg-3)">{{ col.extra || "" }}</td>
            <td class="font-mono">
              <span
                v-if="fkMap[col.field]"
                class="inline-flex items-center gap-1 text-(--acc)"
              >
                <ArrowRightIcon class="size-3 shrink-0" />
                {{ tableLabel(fkMap[col.field].table) }}.{{ fkMap[col.field].column }}
              </span>
            </td>
          </tr>
        </tbody>
      </table>
      <div
        v-else-if="metadataLoading"
        class="px-3 py-4 text-[11.5px] text-(--fg-4)"
      >Loading metadata…</div>

      <div v-if="ddl" class="border-t border-(--line-2)">
        <div class="flex h-7 items-center justify-between border-b border-(--line-2) bg-(--bg-1) px-3 text-[11px]">
          <span class="flex items-center gap-1.5 font-medium text-(--fg-2)">
            <CodeIcon class="size-3 text-(--fg-4)" />
            DDL
          </span>
          <button
            type="button"
            class="inline-flex h-5 items-center gap-1 rounded-[4px] px-1.5 text-[10.5px] text-(--fg-3) transition-colors hover:bg-(--bg-2) hover:text-(--fg-1)"
            title="Copy DDL"
            @click="copyDdl"
          >
            <CheckIcon v-if="ddlCopied" class="size-3 text-(--acc)" />
            <CopyIcon v-else class="size-3" />
            {{ ddlCopied ? 'Copied' : 'Copy' }}
          </button>
        </div>
        <pre
          class="font-mono text-[11.5px] leading-[1.6] text-(--fg-2) px-3 py-2.5 overflow-auto whitespace-pre custom-scrollbar"
        >{{ ddl }}</pre>
      </div>
      <div
        v-else-if="metadataLoading"
        class="px-3 py-3 text-[11.5px] text-(--fg-4)"
      >Loading DDL…</div>
      <div v-if="hasPendingChanges" class="h-20" aria-hidden="true"></div>
    </ScrollArea>
  </div>
</template>

<script setup lang="ts">
import type { TableRef } from '@/types/database'
import { normalizeType } from '@/lib/schemaEditing'
import { tableLabel } from '@/lib/tableReference'
import { nextTick, ref } from "vue";
import { ArrowRightIcon, CheckIcon, CodeIcon, CopyIcon } from "lucide-vue-next";
import { copyText } from "@/lib/clipboard";
import { ScrollArea } from "@/components/ui/scroll-area";

interface ColumnStructure {
  field: string;
  field_type: string;
  nullable: boolean;
  key: string;
  default_value: string | null;
  extra: string;
}

const props = withDefaults(defineProps<{
  tableStructure: ColumnStructure[];
  tableIndexes: any[];
  fkMap: Record<string, { table: string | TableRef; column: string }>;
  ddl: string | null;
  metadataLoading?: boolean;
  metadataLoaded?: boolean;
  paneId: string;
  indexPanelHeight: number | undefined;
  canEdit?: boolean;
  editDisabledReason?: string;
  pendingColumnChanges?: Record<string, { newName: string; newType: string }>;
  hasPendingChanges?: boolean;
  updateColumn?: (oldName: string, newName: string, newType: string) => void;
}>(), {
  canEdit: true,
  editDisabledReason: "This connection is read-only",
  pendingColumnChanges: () => ({}),
  hasPendingChanges: false,
});

const emit = defineEmits<{
  "start-index-resize": [e: MouseEvent, paneId: string];
}>();

// emit is exported so existing parent bindings still work even though
// indexes have moved to their own tab.
void emit;

type EditableField = "name" | "type";
const editingCell = ref<{ column: string; field: EditableField } | null>(null);
const draftValue = ref("");
const editInput = ref<HTMLInputElement | null>(null);

function setEditInput(element: unknown) {
  editInput.value = element as HTMLInputElement | null;
}

function pendingValue(column: ColumnStructure, field: EditableField) {
  const pending = props.pendingColumnChanges[column.field];
  if (field === "name") return pending?.newName ?? column.field;
  return pending?.newType ?? column.field_type;
}

function isFieldPending(column: ColumnStructure, field: EditableField) {
  const value = pendingValue(column, field);
  return field === "name"
    ? value !== column.field
    : normalizeType(value) !== normalizeType(column.field_type);
}

function isEditing(column: ColumnStructure, field: EditableField) {
  return editingCell.value?.column === column.field && editingCell.value.field === field;
}

async function startEdit(column: ColumnStructure, field: EditableField) {
  if (!props.canEdit) return;
  editingCell.value = { column: column.field, field };
  draftValue.value = pendingValue(column, field);
  await nextTick();
  editInput.value?.focus();
  editInput.value?.select();
}

function cancelEdit() {
  editingCell.value = null;
}

const ddlCopied = ref(false);
async function copyDdl() {
  if (!props.ddl) return;
  await copyText(props.ddl);
  ddlCopied.value = true;
  setTimeout(() => { ddlCopied.value = false; }, 1500);
}

function commitEdit(column: ColumnStructure, field: EditableField) {
  if (!isEditing(column, field)) return;
  const value = draftValue.value.trim();
  if (!value) {
    editingCell.value = null;
    return;
  }
  const newName = field === "name" ? value : pendingValue(column, "name");
  const newType = field === "type" ? value : pendingValue(column, "type");
  props.updateColumn?.(column.field, newName, newType);
  editingCell.value = null;
}
</script>

<style scoped>
.structure-table th,
.structure-table td {
  height: 26px;
  padding: 0 10px;
  white-space: nowrap;
  border-bottom: 1px solid var(--line-faint);
  border-right: 1px solid var(--line-faint);
  text-align: left;
  vertical-align: middle;
}
.structure-table th:last-child,
.structure-table td:last-child {
  border-right: 0;
}
.structure-table th {
  position: sticky;
  top: 0;
  z-index: 1;
  height: 28px;
  font-size: 10.5px;
  font-weight: 500;
  color: var(--fg-3);
  background: var(--bg-1);
  border-bottom-color: var(--line-2);
}
.structure-table .gutter {
  font-size: 10.5px;
  color: var(--fg-4);
  background: var(--bg-1);
  border-right-color: var(--line-2);
}
.key-badge {
  font-family: var(--font-mono);
  font-size: 8px;
  font-weight: 700;
  letter-spacing: 0.04em;
}
</style>
