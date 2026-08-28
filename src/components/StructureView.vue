<template>
  <div class="flex flex-col flex-1 min-h-0">
    <ScrollArea class="flex-1 min-h-0">
      <div class="px-5 py-4">
        <div class="mb-3 flex items-center justify-between gap-4">
          <div class="text-[10px] tracking-[0.14em] uppercase text-(--fg-5) font-semibold">
            Columns · {{ tableStructure.length }}
          </div>
          <div class="text-[10px] text-(--fg-5)">
            {{ canEdit ? "Double-click a name or type to edit" : editDisabledReason }}
          </div>
        </div>

        <table v-if="tableStructure.length" class="w-full border-collapse text-[12px]">
          <thead>
            <tr class="text-(--fg-5)">
              <th
                v-for="(h, index) in ['', 'Name', 'Type', 'Nullable', 'Default', 'Extra', 'Foreign key']"
                :key="index"
                class="text-left px-2.5 py-1.5 text-[10px] tracking-widest uppercase font-semibold border-b border-(--line-2)"
              >{{ h }}</th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="col in tableStructure"
              :key="col.field"
              class="group border-b border-(--line-faint) transition-colors"
              :class="editingCell?.column === col.field ? 'bg-(--bg-1)' : 'hover:bg-(--bg-1)'"
            >
              <td class="px-2.5 py-2 w-7">
                <span
                  v-if="col.key === 'PRI'"
                  class="inline-flex items-center px-1.5 py-0.5 rounded text-[9px] font-bold uppercase text-(--pk) bg-[oklch(0.78_0.14_80/0.18)]"
                >PK</span>
                <span
                  v-else-if="col.key === 'UNI'"
                  class="inline-flex items-center px-1.5 py-0.5 rounded text-[9px] font-bold uppercase text-(--env-staging) bg-[oklch(0.75_0.14_250/0.18)]"
                >UNI</span>
                <span
                  v-else-if="col.key === 'MUL'"
                  class="inline-flex items-center px-1.5 py-0.5 rounded text-[9px] font-bold uppercase text-(--fg-4) bg-(--bg-2)"
                >IDX</span>
              </td>
              <td
                class="px-2.5 py-2 font-mono transition-colors"
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
                  class="block w-full min-w-0 rounded-[3px] border-0 bg-(--bg-0) p-0 font-mono text-[12px] leading-[inherit] text-inherit outline-none ring-1 ring-(--line-2) focus:ring-(--fg-5)"
                  aria-label="Column name"
                  @blur="commitEdit(col, 'name')"
                  @keydown.enter.prevent="commitEdit(col, 'name')"
                  @keydown.esc.prevent="cancelEdit"
                />
                <span v-else>{{ pendingValue(col, 'name') }}</span>
              </td>
              <td
                class="px-2.5 py-2 font-mono transition-colors"
                :class="[
                  isFieldPending(col, 'type') ? 'bg-amber-500/10 text-amber-500' : 'text-(--fg-3)',
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
                  class="block w-full min-w-0 rounded-[3px] border-0 bg-(--bg-0) p-0 font-mono text-[12px] leading-[inherit] text-inherit outline-none ring-1 ring-(--line-2) focus:ring-(--fg-5)"
                  aria-label="Column type"
                  @blur="commitEdit(col, 'type')"
                  @keydown.enter.prevent="commitEdit(col, 'type')"
                  @keydown.esc.prevent="cancelEdit"
                />
                <span v-else>{{ pendingValue(col, 'type') }}</span>
              </td>
              <td class="px-2.5 py-2 font-mono">
                <span
                  v-if="col.nullable"
                  class="text-[10px] font-bold uppercase text-(--fg-2)"
                >YES</span>
                <span v-else class="text-[10px] font-bold uppercase text-(--fg-5)">NO</span>
              </td>
              <td class="px-2.5 py-2 font-mono text-(--fg-4)">
                <span v-if="col.default_value === null" class="italic">NULL</span>
                <span v-else>{{ col.default_value }}</span>
              </td>
              <td class="px-2.5 py-2 font-mono text-(--fg-4)">{{ col.extra || "—" }}</td>
              <td class="px-2.5 py-2 font-mono">
                <span
                  v-if="fkMap[col.field]"
                  class="inline-flex items-center gap-1 text-(--acc)"
                >
                  <ArrowRightIcon class="size-3 shrink-0" />
                  {{ fkMap[col.field].table }}.{{ fkMap[col.field].column }}
                </span>
                <span v-else class="text-(--fg-5)">—</span>
              </td>
            </tr>
          </tbody>
        </table>
        <div
          v-else-if="metadataLoading"
          class="px-2 py-4 text-xs text-(--fg-5) italic"
        >Loading metadata…</div>

        <div v-if="ddl" class="mt-8">
          <div
            class="text-[10px] tracking-[0.14em] uppercase text-(--fg-5) font-semibold mb-3 flex items-center gap-2"
          >
            <CodeIcon class="size-3" />
            DDL
          </div>
          <pre
            class="font-mono text-[11.5px] leading-relaxed text-(--fg-2) bg-(--bg-1) border border-(--line-2) rounded-md px-3.5 py-3 overflow-auto whitespace-pre"
          >{{ ddl }}</pre>
        </div>
        <div
          v-else-if="metadataLoading"
          class="mt-6 text-xs text-(--fg-5) italic"
        >Loading DDL…</div>
        <div v-if="hasPendingChanges" class="h-20" aria-hidden="true"></div>
      </div>
    </ScrollArea>
  </div>
</template>

<script setup lang="ts">
import { nextTick, ref } from "vue";
import { ArrowRightIcon, CodeIcon } from "lucide-vue-next";
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
  fkMap: Record<string, { table: string; column: string }>;
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
    : value.toLowerCase() !== column.field_type.toLowerCase();
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
