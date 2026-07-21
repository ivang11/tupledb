<template>
  <Dialog
    :open="open"
    @update:open="(val: boolean) => emit('update:open', val)"
  >
    <DialogContent class="sm:max-w-lg overflow-y-auto max-h-[90vh]">
      <DialogHeader>
        <DialogTitle>{{
          isEdit ? "Edit Connection" : "New Connection"
        }}</DialogTitle>
        <DialogDescription>
          {{
            isEdit
              ? "Update your connection settings"
              : "Configure your MySQL connection settings"
          }}
        </DialogDescription>
      </DialogHeader>

      <div class="space-y-5 py-2">
        <div class="grid grid-cols-2 gap-4">
          <div class="space-y-2">
            <Label>Connection Name</Label>
            <Input v-model="connection.name" placeholder="Local Development" />
          </div>
          <div class="space-y-2">
            <Label>Environment</Label>
            <select
              v-model="connection.environment"
              class="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
            >
              <option value="LOCAL">Local</option>
              <option value="DEV">Development</option>
              <option value="STAGING">Staging</option>
              <option value="PRODUCTION">Production</option>
            </select>
          </div>
        </div>

        <div
          class="flex items-center justify-between rounded-md border border-border bg-muted/20 px-3 py-2.5"
        >
          <div class="space-y-0.5">
            <p class="text-xs font-semibold text-foreground">Read-only mode</p>
            <p class="text-xs text-muted-foreground">
              Block write operations for this connection
            </p>
          </div>
          <div
            @click="toggleReadOnly"
            :class="[
              'relative w-9 h-5 rounded-full transition-colors cursor-pointer shrink-0',
              connection.allow_writes === false ? 'bg-primary' : 'bg-muted',
            ]"
          >
            <div
              :class="[
                'absolute top-0.5 left-0.5 size-4 rounded-full bg-white shadow transition-transform',
                connection.allow_writes === false
                  ? 'translate-x-4'
                  : 'translate-x-0',
              ]"
            />
          </div>
        </div>

        <Separator />

        <div class="space-y-4">
          <div
            class="flex items-center gap-2 text-xs font-bold text-muted-foreground uppercase tracking-wider"
          >
            <HardDriveIcon class="size-3.5" /> MySQL Settings
          </div>
          <div class="grid grid-cols-12 gap-3">
            <div class="col-span-8 space-y-2">
              <Label>Host</Label>
              <Input v-model="connection.mysql.host" placeholder="127.0.0.1" />
            </div>
            <div class="col-span-4 space-y-2">
              <Label>Port</Label>
              <Input v-model.number="connection.mysql.port" type="number" />
            </div>
          </div>
          <div class="grid grid-cols-2 gap-3">
            <div class="space-y-2">
              <Label>User</Label>
              <Input v-model="connection.mysql.user" placeholder="root" />
            </div>
            <div class="space-y-2">
              <Label>Password</Label>
              <Input
                v-model="connection.mysql.password"
                type="password"
                :placeholder="
                  isEdit ? 'Leave blank to keep existing' : '••••••••'
                "
              />
            </div>
          </div>
          <div class="grid grid-cols-2 gap-3">
            <div class="space-y-2">
              <Label
                >Database
                <span class="text-muted-foreground font-normal"
                  >(optional)</span
                ></Label
              >
              <Input
                v-model="connection.mysql.database"
                placeholder="Leave blank to pick after connecting"
              />
            </div>
            <div class="space-y-2">
              <Label
                >Timeout
                <span class="text-muted-foreground font-normal"
                  >(seconds)</span
                ></Label
              >
              <Input
                v-model.number="connection.timeout_secs"
                type="number"
                min="1"
                placeholder="30"
              />
            </div>
          </div>
        </div>

        <Separator />

        <!-- SSH Tunnel -->
        <div class="space-y-4">
          <label class="flex items-center gap-3 cursor-pointer select-none">
            <div
              @click="sshEnabled = !sshEnabled"
              :class="[
                'relative w-9 h-5 rounded-full transition-colors shrink-0',
                sshEnabled ? 'bg-primary' : 'bg-muted',
              ]"
            >
              <div
                :class="[
                  'absolute top-0.5 left-0.5 size-4 rounded-full bg-white shadow transition-transform',
                  sshEnabled ? 'translate-x-4' : 'translate-x-0',
                ]"
              />
            </div>
            <div
              class="flex items-center gap-2 text-xs font-bold text-muted-foreground uppercase tracking-wider"
            >
              <ShieldCheckIcon class="size-3.5" /> SSH Tunnel
            </div>
          </label>

          <div v-if="sshEnabled" class="space-y-3 pl-1">
            <div class="grid grid-cols-12 gap-3">
              <div class="col-span-8 space-y-2">
                <Label>SSH Host</Label>
                <Input
                  v-model="sshForm.host"
                  placeholder="bastion.example.com"
                />
              </div>
              <div class="col-span-4 space-y-2">
                <Label>Port</Label>
                <Input v-model.number="sshForm.port" type="number" />
              </div>
            </div>
            <div class="space-y-2">
              <Label>SSH User</Label>
              <Input v-model="sshForm.user" placeholder="ubuntu" />
            </div>

            <div class="flex gap-2 pt-1">
              <button
                @click="sshAuthType = 'password'"
                :class="[
                  'flex-1 h-8 rounded-md text-xs font-bold border transition-all',
                  sshAuthType === 'password'
                    ? 'bg-primary text-primary-foreground border-primary'
                    : 'text-muted-foreground border-input hover:border-primary/50',
                ]"
              >
                Password
              </button>
              <button
                @click="sshAuthType = 'key'"
                :class="[
                  'flex-1 h-8 rounded-md text-xs font-bold border transition-all',
                  sshAuthType === 'key'
                    ? 'bg-primary text-primary-foreground border-primary'
                    : 'text-muted-foreground border-input hover:border-primary/50',
                ]"
              >
                SSH Key
              </button>
            </div>

            <div v-if="sshAuthType === 'password'" class="space-y-2">
              <Label>SSH Password</Label>
              <Input
                v-model="sshForm.password"
                type="password"
                placeholder="••••••••"
              />
            </div>

            <div v-if="sshAuthType === 'key'" class="space-y-3">
              <div class="space-y-2">
                <Label>Private Key</Label>
                <div class="flex gap-1.5">
                  <Input
                    v-model="sshForm.private_key_path"
                    placeholder="~/.ssh/id_rsa"
                    class="flex-1"
                  />
                  <button
                    type="button"
                    @click="pickSshKey"
                    title="Browse…"
                    class="shrink-0 flex items-center justify-center h-9 w-9 rounded-md border border-input bg-background text-muted-foreground transition-colors hover:bg-accent hover:text-foreground hover:border-ring/50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/40"
                  >
                    <FolderOpenIcon class="size-4" />
                  </button>
                </div>
              </div>
              <div class="space-y-2">
                <Label
                  >Passphrase
                  <span class="text-muted-foreground font-normal"
                    >(optional)</span
                  ></Label
                >
                <Input
                  v-model="sshForm.passphrase"
                  type="password"
                  placeholder="••••••••"
                />
              </div>
            </div>
          </div>
        </div>
      </div>

      <div
        v-if="testResult"
        :class="[
          'text-xs px-3 py-2 rounded-md font-medium',
          testResult.ok
            ? 'bg-green-500/10 text-green-500'
            : 'bg-destructive/10 text-destructive',
        ]"
      >
        {{ testResult.msg }}
      </div>

      <div class="flex items-center justify-between pt-4 border-t">
        <Button variant="ghost" @click="emit('update:open', false)"
          >Cancel</Button
        >
        <div class="flex gap-2">
          <Button
            variant="outline"
            :disabled="isTesting || isSaving"
            @click="test"
          >
            {{ isTesting ? "Testing..." : "Test" }}
          </Button>
          <Button
            variant="outline"
            :disabled="isSaving || !connection.name"
            @click="emit('save', buildConn(), false)"
          >
            {{ isEdit ? "Update" : "Save only" }}
          </Button>
          <Button
            v-if="showConnectButton"
            :disabled="isSaving || !connection.name"
            @click="emit('save', buildConn(), true)"
          >
            {{ isSaving ? "Saving..." : "Save & Connect" }}
          </Button>
        </div>
      </div>
    </DialogContent>
  </Dialog>
</template>

<script setup lang="ts">
import { ref, watch } from "vue";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import { useConnectionStore } from "@/stores/connections";
import type { Connection } from "@/types/connection";
import {
  ShieldCheckIcon,
  HardDriveIcon,
  FolderOpenIcon,
} from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Separator } from "@/components/ui/separator";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
} from "@/components/ui/dialog";

const props = withDefaults(
  defineProps<{
    open: boolean;
    connection: Connection;
    isSaving?: boolean;
    showConnectButton?: boolean;
  }>(),
  {
    isSaving: false,
    showConnectButton: false,
  },
);

const emit = defineEmits<{
  "update:open": [val: boolean];
  save: [conn: Connection, andConnect: boolean];
}>();

const store = useConnectionStore();

const sshEnabled = ref(false);
const sshAuthType = ref<"password" | "key">("password");
const sshForm = ref({
  host: "",
  port: 22,
  user: "",
  password: "",
  private_key_path: "",
  passphrase: "",
});
const isTesting = ref(false);
const testResult = ref<{ ok: boolean; msg: string } | null>(null);

const isEdit = ref(false);

watch(
  () => [props.open, props.connection] as const,
  ([open]) => {
    if (!open) return;
    testResult.value = null;
    isEdit.value = store.connections.some((c) => c.id === props.connection.id);

    sshEnabled.value = !!props.connection.ssh;
    if (props.connection.ssh) {
      const ssh = props.connection.ssh;
      sshForm.value = {
        host: ssh.host,
        port: ssh.port,
        user: ssh.user,
        password: ssh.auth.type === "password" ? ssh.auth.password : "",
        private_key_path:
          ssh.auth.type === "key" ? ssh.auth.private_key_path : "",
        passphrase: ssh.auth.type === "key" ? (ssh.auth.passphrase ?? "") : "",
      };
      sshAuthType.value = ssh.auth.type === "password" ? "password" : "key";
    } else {
      sshForm.value = {
        host: "",
        port: 22,
        user: "",
        password: "",
        private_key_path: "",
        passphrase: "",
      };
      sshAuthType.value = "password";
    }
  },
  { immediate: true },
);

function buildConn(): Connection {
  const conn = { ...props.connection };
  conn.mysql = { ...props.connection.mysql };
  const password = conn.mysql.password?.trim() ?? "";
  const database = conn.mysql.database?.trim() ?? "";
  conn.mysql.password = password || undefined;
  conn.mysql.database = database || undefined;

  if (sshEnabled.value) {
    conn.ssh = {
      host: sshForm.value.host,
      port: sshForm.value.port,
      user: sshForm.value.user,
      auth:
        sshAuthType.value === "password"
          ? { type: "password" as const, password: sshForm.value.password }
          : {
              type: "key" as const,
              private_key_path: sshForm.value.private_key_path,
              passphrase: sshForm.value.passphrase || undefined,
            },
    };
  } else {
    conn.ssh = undefined;
  }
  return conn;
}

function toggleReadOnly() {
  props.connection.allow_writes = props.connection.allow_writes === false;
}

async function test() {
  isTesting.value = true;
  testResult.value = null;
  try {
    const msg = await store.testConnection(buildConn());
    testResult.value = { ok: true, msg: msg ?? "Connection successful" };
  } catch (e: any) {
    testResult.value = { ok: false, msg: String(e) };
  } finally {
    isTesting.value = false;
  }
}

async function pickSshKey() {
  try {
    const selected = await openFileDialog({
      multiple: false,
    });
    if (selected && typeof selected === "string") {
      sshForm.value.private_key_path = selected;
    }
  } catch (e) {
    console.error("Error picking SSH key:", e);
  }
}
</script>
