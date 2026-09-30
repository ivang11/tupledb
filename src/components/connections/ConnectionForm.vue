<template>
  <div class="connection-form flex h-full min-h-0 flex-col">
    <div class="connection-settings min-h-0 flex-1 overflow-y-auto px-5 py-3">
      <div ref="settingsContent" class="flex flex-col gap-3">
        <section aria-label="Connection settings" class="space-y-2.5">
          <div class="grid grid-cols-[minmax(0,1fr)_10rem] gap-3">
            <div class="space-y-1">
              <Label for="connection-name">Connection name</Label>
              <Input id="connection-name" v-model="connection.name" placeholder="Local Development" />
            </div>
            <div class="space-y-1">
              <Label for="connection-engine">Database engine</Label>
              <select id="connection-engine" :value="connection.database.engine" :disabled="isEdit"
                @change="changeEngine(($event.target as HTMLSelectElement).value)">
                <option v-for="driver in store.availableDrivers" :key="driver.engine" :value="driver.engine">{{ driver.label }}</option>
              </select>
            </div>
          </div>
          <template v-if="connection.database.engine !== 'sqlite'">
            <div class="grid grid-cols-[minmax(0,1fr)_7rem] gap-3">
              <div class="space-y-1">
                <Label for="connection-host">Host</Label>
                <Input id="connection-host" v-model="connection.database.settings.host" v-bind="fieldAttrs('host')" placeholder="127.0.0.1" />
              </div>
              <div class="space-y-1">
                <Label for="connection-port">Port</Label>
                <Input id="connection-port" v-model.number="connection.database.settings.port" v-bind="fieldAttrs('port')" type="number" min="1" max="65535" />
              </div>
            </div>
            <div class="grid grid-cols-2 gap-3">
              <div class="space-y-1">
                <Label for="connection-user">User</Label>
                <Input id="connection-user" v-model="connection.database.settings.user" v-bind="fieldAttrs('user')" placeholder="root" />
              </div>
              <div class="space-y-1">
                <Label for="connection-password">Password</Label>
                <Input id="connection-password" v-model="connection.database.settings.password" v-bind="fieldAttrs('password')" type="password"
                  :placeholder="isEdit ? 'Leave blank to keep existing' : '••••••••'" />
              </div>
            </div>
            <div class="grid grid-cols-2 gap-3">
              <div class="space-y-1">
                <Label for="connection-database">Database <span class="font-normal text-muted-foreground">(optional)</span></Label>
                <Input id="connection-database" v-model="connection.database.settings.database" v-bind="fieldAttrs('database')" placeholder="Leave blank to pick after connecting" />
              </div>
              <div class="space-y-1">
                <Label for="connection-environment">Environment</Label>
                <select id="connection-environment" v-model="connection.environment">
                  <option value="LOCAL">Local</option><option value="DEV">Development</option>
                  <option value="STAGING">Staging</option><option value="PRODUCTION">Production</option>
                </select>
              </div>
            </div>
          </template>
          <p v-if="!isAvailable" class="text-xs text-muted-foreground">{{ engineLabel }} connections are not available in this version.</p>
          <div class="grid grid-cols-[minmax(0,1fr)_7rem] items-end gap-3 border-t pt-2.5">
            <div class="flex h-9 items-center justify-between gap-4">
              <Label for="connection-read-only">Read-only mode</Label>
              <button id="connection-read-only" type="button" role="switch" :aria-checked="connection.allow_writes === false" aria-label="Read-only mode"
                class="relative h-5 w-9 shrink-0 rounded-full transition-colors" :class="connection.allow_writes === false ? 'bg-primary' : 'bg-muted'" @click="toggleReadOnly">
                <span class="absolute left-0.5 top-0.5 size-4 rounded-full bg-white transition-transform" :class="connection.allow_writes === false ? 'translate-x-4' : ''" />
              </button>
            </div>
            <div class="space-y-1"><Label for="connection-timeout">Timeout (s)</Label>
              <Input id="connection-timeout" v-model.number="connection.timeout_secs" type="number" min="1" placeholder="30" /></div>
          </div>
          <template v-if="connection.database.engine === 'postgresql'">
            <div class="space-y-1 border-t pt-2.5"><Label for="connection-tls">TLS mode</Label>
              <select id="connection-tls" v-model="connection.database.settings.ssl_mode" v-bind="fieldAttrs('tls')">
                <option value="disable">Disable</option><option value="prefer">Prefer</option><option value="require">Require encryption</option>
                <option value="verify_ca">Verify CA</option><option value="verify_full">Verify CA and hostname</option>
              </select></div>
            <div v-if="['verify_ca', 'verify_full'].includes(connection.database.settings.ssl_mode)" class="space-y-1">
              <Label for="connection-ca">CA certificate <span class="font-normal text-muted-foreground">(PEM)</span></Label>
              <textarea id="connection-ca" v-model="connection.database.settings.ssl_root_cert" v-bind="fieldAttrs('tls')" rows="3"
                placeholder="-----BEGIN CERTIFICATE-----&#10;...&#10;-----END CERTIFICATE-----"
                class="w-full resize-y rounded-md border border-input bg-background px-3 py-2 font-mono text-xs outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50" />
              <p class="text-xs text-muted-foreground">Paste the CA certificate that signed the server's certificate. If you don't have one, use “Require encryption”.</p>
            </div>
          </template>
          <div class="flex items-center justify-between gap-4">
            <div class="space-y-1">
              <Label for="connection-ssh-enabled" class="flex items-center gap-2"><ShieldCheckIcon class="size-3.5" /> SSH tunnel</Label>
              <p class="text-xs text-muted-foreground">Connect through a bastion or remote server.</p>
            </div>
            <button id="connection-ssh-enabled" type="button" role="switch" :aria-checked="sshEnabled" aria-label="SSH tunnel"
              class="relative h-5 w-9 shrink-0 rounded-full transition-colors" :class="sshEnabled ? 'bg-primary' : 'bg-muted'"
              @click="sshEnabled = !sshEnabled">
              <span class="absolute left-0.5 top-0.5 size-4 rounded-full bg-white transition-transform" :class="sshEnabled ? 'translate-x-4' : ''" />
            </button>
          </div>
        </section>

        <section v-show="sshEnabled" aria-label="SSH settings" class="ssh-settings space-y-3">
          <p class="text-sm font-medium">SSH connection</p>
          <fieldset :disabled="!sshEnabled" class="min-w-0 space-y-2.5" aria-label="SSH tunnel settings">
            <div class="grid grid-cols-[minmax(0,1fr)_7rem] gap-3">
              <div class="space-y-1"><Label for="connection-ssh-host">SSH host</Label>
                <Input id="connection-ssh-host" v-model="sshForm.host" v-bind="fieldAttrs('sshHost')" placeholder="bastion.example.com" /></div>
              <div class="space-y-1"><Label for="connection-ssh-port">Port</Label>
                <Input id="connection-ssh-port" v-model.number="sshForm.port" v-bind="fieldAttrs('sshPort')" type="number" min="1" max="65535" /></div>
            </div>
            <div class="grid grid-cols-2 gap-3">
              <div class="space-y-1"><Label for="connection-ssh-user">SSH user</Label>
                <Input id="connection-ssh-user" v-model="sshForm.user" v-bind="fieldAttrs('sshUser')" placeholder="ubuntu" /></div>
              <div class="space-y-1"><Label for="connection-ssh-auth">Authentication</Label>
                <select id="connection-ssh-auth" v-model="sshAuthType"><option value="password">Password</option><option value="key">SSH key</option></select></div>
            </div>
            <div v-if="sshAuthType === 'password'" class="space-y-1"><Label for="connection-ssh-password">SSH password</Label>
              <Input id="connection-ssh-password" v-model="sshForm.password" v-bind="fieldAttrs('sshPassword')" type="password" placeholder="••••••••" /></div>
            <template v-else>
              <div class="space-y-1"><Label for="connection-ssh-key">Private key</Label>
                <div class="flex gap-2"><Input id="connection-ssh-key" v-model="sshForm.private_key_path" v-bind="fieldAttrs('sshKey')" placeholder="~/.ssh/id_rsa" class="flex-1" />
                  <Button type="button" variant="outline" size="icon" aria-label="Browse private key" @click="pickSshKey"><FolderOpenIcon class="size-4" /></Button></div></div>
              <div class="space-y-1"><Label for="connection-ssh-passphrase">Passphrase <span class="font-normal text-muted-foreground">(optional)</span></Label>
                <Input id="connection-ssh-passphrase" v-model="sshForm.passphrase" v-bind="fieldAttrs('sshPassphrase')" type="password" placeholder="••••••••" /></div>
            </template>
          </fieldset>
        </section>
      </div>
    </div>

    <div ref="footer" class="shrink-0 border-t bg-muted/20 px-5 py-3">
      <div v-if="testMessage" role="status" aria-live="polite" class="mb-3 flex items-start gap-2 rounded-md px-3 py-2 text-xs font-medium"
        :class="isTesting ? 'bg-muted/50 text-foreground' : testResult?.ok ? 'bg-green-500/10 text-green-500' : 'bg-destructive/10 text-destructive'">
        <Loader2Icon v-if="isTesting" class="mt-0.5 size-3.5 shrink-0 animate-spin motion-reduce:animate-none" aria-hidden="true" />
        <CheckCircle2Icon v-else-if="testResult?.ok" class="mt-0.5 size-3.5 shrink-0" aria-hidden="true" />
        <XCircleIcon v-else class="mt-0.5 size-3.5 shrink-0" aria-hidden="true" />
        <span class="max-h-20 flex-1 overflow-y-auto break-words">{{ testMessage }}</span>
        <span v-if="isTesting" class="shrink-0 tabular-nums text-muted-foreground">{{ elapsedSeconds }}s</span>
      </div>
      <slot name="error" />
      <div class="flex flex-wrap items-center justify-between gap-3">
        <Button type="button" variant="outline" :disabled="isTesting || isSaving || !isAvailable" @click="test">{{ isTesting ? 'Testing...' : 'Test' }}</Button>
        <div class="ml-auto flex gap-2">
          <Button type="button" variant="ghost" :disabled="isSaving" @click="emit('update:open', false)">Cancel</Button>
          <Button type="button" variant="outline" :disabled="isTesting || isSaving || !connection.name" @click="emit('save', buildConn(), false)">{{ isEdit ? 'Update' : 'Save only' }}</Button>
          <Button v-if="showConnectButton" type="button" :disabled="isTesting || isSaving || !connection.name || !isAvailable" @click="emit('save', buildConn(), true)">{{ isSaving ? 'Saving...' : 'Save & Connect' }}</Button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, toRaw, ref, watch } from "vue";
import { useResizeObserver } from "@vueuse/core";
import { databaseEngines } from '@/lib/databaseEngines';
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import { useConnectionStore } from "@/stores/connections";
import { useConnectionTest } from "@/composables/useConnectionTest";
import type { Connection } from "@/types/connection";
import {
  ShieldCheckIcon,
  FolderOpenIcon,
  Loader2Icon,
  CheckCircle2Icon,
  XCircleIcon,
} from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
const props = withDefaults(
  defineProps<{
    open: boolean;
    connection: Connection;
    isSaving?: boolean;
    showConnectButton?: boolean;
    editing?: boolean;
  }>(),
  {
    isSaving: false,
    showConnectButton: false,
  },
);

const emit = defineEmits<{
  "update:open": [val: boolean];
  save: [conn: Connection, andConnect: boolean];
  "height-change": [height: number];
}>();

const store = useConnectionStore();
const engineLabel = computed(() => databaseEngines[props.connection.database.engine].label);
const isAvailable = computed(() => store.availableDrivers.some(driver => driver.engine === props.connection.database.engine));

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
const { isTesting, testResult, testMessage, elapsedSeconds, testConnection, resetTest, fieldAttrs } = useConnectionTest();

const isEdit = ref(false);
function changeEngine(engine: string) {
  if (isEdit.value || !store.availableDrivers.some(d => d.engine === engine)) return
  props.connection.database = engine === 'postgresql'
    ? { engine, settings: { host: '127.0.0.1', port: 5432, user: 'postgres', ssl_mode: 'prefer' } }
    : { engine: 'mysql', settings: { host: '127.0.0.1', port: 3306, user: 'root' } }
  resetTest()
}

watch(
  () => [props.open, props.connection] as const,
  ([open]) => {
    if (!open) return;
    resetTest();
    isEdit.value = props.editing ?? store.connections.some((c) => c.id === props.connection.id);

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

const settingsContent = ref<HTMLElement>();
const footer = ref<HTMLElement>();
useResizeObserver([settingsContent, footer], () => {
  if (!settingsContent.value || !footer.value) return;
  const contentHeight = settingsContent.value.getBoundingClientRect().height;
  if (contentHeight > 0) emit('height-change', Math.ceil(contentHeight + footer.value.getBoundingClientRect().height + 24));
});

// A result only describes the exact settings that were tested. Ignore late
// progress/results after editing the form, changing profiles or closing it.
watch(
  () => [props.open, props.connection.id, props.connection.database,
    props.connection.timeout_secs, props.connection.allow_writes,
    sshEnabled.value, sshAuthType.value, sshForm.value],
  resetTest,
  { deep: true },
);

function buildConn(): Connection {
  const conn = { ...props.connection };
  conn.database = structuredClone(toRaw(props.connection.database));
  if (conn.database.engine !== "sqlite") {
    // Whitespace can be part of a password; preserve it exactly.
    conn.database.settings.password ||= undefined;
    const database = conn.database.settings.database?.trim();
    conn.database.settings.database = database || undefined;
  }

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
  await testConnection(buildConn());
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

<style scoped>
.ssh-settings {
  border-top: 1px solid var(--border);
  padding-top: 0.75rem;
}
.connection-form select {
  display: block;
  height: 2.25rem;
  width: 100%;
  border-radius: var(--radius);
  border: 1px solid var(--input);
  background: var(--background);
  padding: 0 0.75rem;
  font-size: 0.875rem;
}
.connection-form select:focus-visible {
  outline: 2px solid var(--ring);
  outline-offset: 2px;
}
.connection-form button:focus-visible {
  outline: 2px solid var(--ring);
  outline-offset: 2px;
}
.connection-test-field[data-test-state="checking"] {
  border-color: var(--color-amber-500);
  background-color: color-mix(in oklab, var(--color-amber-500) 5%, transparent);
}
.connection-test-field[data-test-state="success"] {
  border-color: color-mix(in oklab, var(--color-green-400) 40%, var(--input));
  background-color: color-mix(in oklab, var(--color-green-400) 2.5%, transparent);
}
.connection-test-field[data-test-state="error"] {
  border-color: var(--destructive);
  background-color: color-mix(in oklab, var(--destructive) 5%, transparent);
}
</style>
