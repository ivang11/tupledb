// Requirements: Unix, Node, Cargo, Docker, OpenSSL 3 and /usr/bin/ssh + ssh-keygen.
// Creates only a disposable server, certificates and SSH known_hosts file.
import { spawn } from 'node:child_process';
import { mkdtempSync, writeFileSync, rmSync, mkdirSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { randomUUID } from 'node:crypto';
import { setTimeout as delay } from 'node:timers/promises';

const root = dirname(dirname(fileURLToPath(import.meta.url)));
const temp = mkdtempSync(join(tmpdir(), 'tupledb-transport-'));
const container = `tupledb-transport-${randomUUID()}`;
let child;
let interrupted = false;
let started = false;
const interrupt = () => { interrupted = true; child?.kill('SIGTERM'); };
process.on('SIGINT', interrupt);
process.on('SIGTERM', interrupt);

async function run(command, args, { capture = false, env = process.env, cleanup = false } = {}) {
  if (interrupted && !cleanup) throw new Error('Interrupted');
  return new Promise((resolve, reject) => {
    const proc = spawn(command, args, { cwd: root, env, stdio: capture ? ['ignore', 'pipe', 'inherit'] : 'inherit' });
    child = proc;
    let output = '';
    proc.stdout?.on('data', data => { output += data; });
    proc.on('error', reject);
    proc.on('close', code => {
      if (child === proc) child = undefined;
      code === 0 ? resolve(output.trim()) : reject(new Error(`${command} ${args[0]} exited with ${code}`));
    });
  });
}

const docker = (...args) => run('docker', args);
const exec = (...args) => docker('exec', container, ...args);
const copy = (source, destination) => docker('cp', join(temp, source), `${container}:${destination}`);
const quote = value => `'${value.replaceAll("'", "'\\''")}'`;

try {
  console.log('Generating disposable CA, valid/expired server certificates and SSH key…');
  for (const name of ['ca', 'wrong-ca']) {
    await run('openssl', ['req', '-x509', '-newkey', 'rsa:2048', '-nodes', '-keyout', join(temp, `${name}.key`), '-out', join(temp, `${name}.crt`), '-days', '2', '-subj', `/CN=TupleDB disposable ${name}`, '-addext', 'basicConstraints=critical,CA:TRUE', '-addext', 'keyUsage=critical,keyCertSign,cRLSign']);
  }
  await run('openssl', ['req', '-newkey', 'rsa:2048', '-nodes', '-keyout', join(temp, 'server.key'), '-out', join(temp, 'server.csr'), '-subj', '/CN=localhost', '-addext', 'subjectAltName=DNS:localhost']);
  await run('openssl', ['x509', '-req', '-in', join(temp, 'server.csr'), '-CA', join(temp, 'ca.crt'), '-CAkey', join(temp, 'ca.key'), '-CAcreateserial', '-out', join(temp, 'server.crt'), '-days', '2', '-copy_extensions', 'copy']);
  writeFileSync(join(temp, 'index.txt'), '');
  writeFileSync(join(temp, 'serial'), '1000\n');
  writeFileSync(join(temp, 'ca.cnf'), `[ca]
default_ca = fixture
[fixture]
database = ${join(temp, 'index.txt')}
serial = ${join(temp, 'serial')}
new_certs_dir = ${temp}
certificate = ${join(temp, 'ca.crt')}
private_key = ${join(temp, 'ca.key')}
default_md = sha256
policy = subject
copy_extensions = copy
[subject]
commonName = supplied
`);
  await run('openssl', ['ca', '-batch', '-notext', '-config', join(temp, 'ca.cnf'), '-in', join(temp, 'server.csr'), '-out', join(temp, 'expired.crt'), '-startdate', '20200101000000Z', '-enddate', '20210101000000Z']);
  await run('ssh-keygen', ['-q', '-t', 'ed25519', '-N', '', '-f', join(temp, 'ssh_key')]);
  writeFileSync(join(temp, 'sshd_config'), `Port 22
ListenAddress 0.0.0.0
HostKey /etc/ssh/ssh_host_ed25519_key
PermitRootLogin prohibit-password
PubkeyAuthentication yes
PasswordAuthentication no
KbdInteractiveAuthentication no
AuthorizedKeysFile /etc/ssh/tupledb_authorized_keys
AllowTcpForwarding yes
UsePAM no
PidFile /run/tupledb-sshd.pid
`);
  const wrapper = join(temp, 'bin');
  mkdirSync(wrapper);
  writeFileSync(join(wrapper, 'ssh'), `#!/bin/sh\nexec /usr/bin/ssh -F /dev/null -o ${quote(`UserKnownHostsFile=${join(temp, 'known_hosts')}`)} "$@"\n`, { mode: 0o700 });

  console.log(`Starting isolated PostgreSQL 17 + SSH server: ${container}`);
  // Mark before creation so cleanup also handles interruption during docker run.
  started = true;
  await docker('run', '--rm', '-d', '--name', container, '-e', 'POSTGRES_PASSWORD=tupledb_transport_test', '-p', '127.0.0.1::5432', '-p', '127.0.0.1::22', 'postgres:17');
  let ready = false;
  for (let attempt = 0; attempt < 60; attempt++) {
    try { await exec('pg_isready', '-U', 'postgres'); ready = true; break; }
    catch (error) { if (interrupted) throw error; await delay(1000); }
  }
  if (!ready) throw new Error('PostgreSQL did not start within 60 seconds');
  await exec('sh', '-c', 'apt-get update -qq && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends openssh-server');
  await copy('server.key', '/var/lib/postgresql/data/server.key');
  await copy('server.crt', '/var/lib/postgresql/data/server.crt');
  await copy('expired.crt', '/var/lib/postgresql/data/expired.crt');
  await copy('ssh_key.pub', '/etc/ssh/tupledb_authorized_keys');
  await copy('sshd_config', '/etc/ssh/tupledb_sshd_config');
  await exec('sh', '-c', 'chown postgres:postgres /var/lib/postgresql/data/server.key /var/lib/postgresql/data/server.crt /var/lib/postgresql/data/expired.crt && chmod 600 /var/lib/postgresql/data/server.key && chown root:root /etc/ssh/tupledb_authorized_keys && chmod 600 /etc/ssh/tupledb_authorized_keys && mkdir -p /run/sshd && passwd -d root && /usr/sbin/sshd -f /etc/ssh/tupledb_sshd_config');
  await exec('psql', '-U', 'postgres', '-c', "ALTER SYSTEM SET ssl='on'");
  await exec('psql', '-U', 'postgres', '-c', 'SELECT pg_reload_conf()');
  const port = async service => (await run('docker', ['port', container, service], { capture: true })).split(':').at(-1);
  const env = {
    ...process.env,
    PATH: `${wrapper}:${process.env.PATH}`,
    TUPLEDB_TRANSPORT_PG_PORT: await port('5432/tcp'),
    TUPLEDB_TRANSPORT_SSH_PORT: await port('22/tcp'),
    TUPLEDB_TRANSPORT_CA: join(temp, 'ca.crt'),
    TUPLEDB_TRANSPORT_WRONG_CA: join(temp, 'wrong-ca.crt'),
    TUPLEDB_TRANSPORT_SSH_KEY: join(temp, 'ssh_key'),
  };
  const test = filter => run('cargo', ['test', '--manifest-path', 'src-tauri/Cargo.toml', '--test', 'postgresql_transport', filter, '--', '--ignored', '--test-threads=1', '--nocapture'], { env });
  console.log('Checking encryption, CA/hostname validation, SSH and SQL transfer roundtrips…');
  await test('transport_');
  await exec('psql', '-U', 'postgres', '-c', "ALTER SYSTEM SET ssl_cert_file='expired.crt'");
  await exec('psql', '-U', 'postgres', '-c', 'SELECT pg_reload_conf()');
  // Reload is asynchronous; wait for PostgreSQL to report the new effective setting.
  let expiredReady = false;
  for (let attempt = 0; attempt < 50; attempt++) {
    const setting = await run('docker', ['exec', container, 'psql', '-U', 'postgres', '-Atc', 'SHOW ssl_cert_file'], { capture: true });
    if (setting === 'expired.crt') { expiredReady = true; break; }
    await delay(100);
  }
  if (!expiredReady) throw new Error('Expired certificate was not loaded');
  console.log('Checking that a trusted CA cannot make an expired certificate valid…');
  await test('expired_certificate_');
} catch (error) {
  console.error(error.message);
  process.exitCode = 1;
} finally {
  if (started) {
    try { await run('docker', ['rm', '-f', '-v', container], { cleanup: true }); }
    catch (error) { console.error(`Check disposable container ${container}: ${error.message}`); process.exitCode = 1; }
  }
  rmSync(temp, { recursive: true, force: true });
  console.log('Removed disposable transport fixture and temporary keys.');
  process.removeListener('SIGINT', interrupt);
  process.removeListener('SIGTERM', interrupt);
}
