# Database adapters

MySQL and PostgreSQL are available through the same adapter boundary. SQLite's
configuration shape is defined, but its connections are rejected before opening
sockets, files or SSH tunnels until its adapter is implemented.

## Module ownership

```text
src-tauri/src/
  commands/                 Tauri arguments, write policy, capabilities, IPC/events
  services/
    connections.rs          Shared connect/test lifecycle and SSH transport
    transfers.rs            Files, compression, progress, batching and export formats
  database/
    types.rs                Shared wire types
    driver.rs               Catalog, query, editing and import contracts
    capabilities.rs         Connected adapter support and connection metadata
    registry.rs             Available adapters and construction
    sql.rs                  SQL rendering and incremental import parser contracts
    results.rs              Result retention policy
    drivers/mysql/
      connection.rs         SQLx pool setup and server detection
      catalog.rs            Metadata and database administration
      queries.rs            Reads, execution and streaming
      editing.rs            Row and structure changes
      import.rs             Import sessions and execution
      script.rs             MySQL script parsing and INSERT compaction
      sql.rs                Identifier/literal rendering and dump directives
      values.rs             MySQL row and spatial-value decoding
    drivers/postgresql/
      connection.rs         PostgreSQL pools, TLS and session timeouts
      catalog.rs            Schema-aware catalogs, columns, keys and indexes
      database_options.rs   Native encoding and locale-provider database creation
      queries.rs            Bound filters, pagination, execution and streaming
      editing.rs            Transactional row/column edits and table operations
      import.rs             Native SQL/COPY streaming, transactions and cancellation
      export.rs             Snapshot-based SQL/COPY export and dependency ordering
      script.rs             PostgreSQL lexical splitting and import grammar guards
      session.rs            Backend-specific cancellation shared by SQL operations
      type_name.rs          Restricted PostgreSQL type grammar for column changes
      sql.rs                PostgreSQL identifier and literal rendering
      values.rs             Lossless text-protocol result decoding
```

The common database contracts do not depend on Tauri or SQLx. Commands do not
construct drivers. Services call contracts, and the registry is the production
entry point that selects a concrete adapter. MySQL-specific thread IDs remain
inside the MySQL module; application cancellation uses query/import IDs.

CSV/JSON output, file buffering and progress are shared. SQL exports obtain
identifier quoting, literals and session directives from the adapter. SQL import
uses an adapter-supplied incremental parser, including its opt-in compaction
policy, or delegates the stream to an engine with protocol-level data sections
(PostgreSQL COPY). A new adapter must not inherit MySQL grammar by accident.

Application activity logs describe catalog/table operations. They no longer
fabricate MySQL SQL for a generic driver. Explicit user queries still record the
executed SQL. Per-statement tracing of internal driver queries can be added at
the adapter boundary.

## Connection compatibility

Saved connections now use a tagged value:

```json
{
  "database": {
    "engine": "mysql",
    "settings": { "host": "localhost", "port": 3306, "user": "root" }
  }
}
```

The Rust deserializer also accepts the former `mysql: { ... }` field. On the next
save/export it writes the tagged format. It preserves IDs, SSH configuration,
passwords, database selection, timeouts and read-only settings. Ambiguous or
unknown engine configurations are rejected. No existing user file is rewritten
by the source-code refactor itself. Older app versions cannot read the new
format, so retain an export from the old version if downgrade is needed.

Connecting and testing use the same pool setup and both validate an explicitly
configured database. Empty password fields preserve existing credentials for the
same engine; database passwords never carry over to a different engine.

`connect` returns `{ engine, serverVersion, capabilities }`. The frontend stores
that metadata separately from the persisted configuration. The backend registry
controls availability; frontend engine metadata only controls labels and editor
dialects. Capabilities describe adapter support, not server authorization.

## Common object and editing contracts

Table operations on drivers receive `TableRef { catalog, schema, name }`, never
a dotted SQL string. Table listings and foreign-key metadata return that same
identity. The frontend store accepts explicit references and new tabs retain
them across reads, edits and refreshes. Identity keys use JSON tuples, so dots
and separators inside names do not collapse distinct objects. An ambiguous bare
name in the frontend cache is rejected rather than resolved arbitrarily.

Tauri temporarily accepts either a reference or a legacy bare table name through
`TableTarget`; adapters only receive explicit references. The separate `database`
argument at this boundary must match `reference.catalog`. MySQL rejects a
separate schema. SQL dump target qualification belongs to the dialect and omits
the source catalog so dumps can be restored into another database.

Columns expose `primary_key_position`, `value_kind`, `is_identity` and
`is_generated`. Native `key`, `extra` and type labels remain for display and
compatibility, not as requirements for other engines. Foreign-key columns retain
constraint name, position and the referenced catalog/schema. Single-column
navigation uses that reference. Composite relation shortcuts are intentionally
disabled until the UI can supply every related value; heuristic `_id` links are
no longer presented as actual constraints.

Updates/deletions carry `key: [{ column, value }, ...]`. Both grids, row selection
and pending edits preserve the full ordered identity, including value types.
Both adapters verify the complete primary key against metadata before any write and
updates all changed columns in one statement, including primary-key edits.
Tables without a primary key remain non-editable. Composite keys use OFFSET
pagination with all primary-key columns as deterministic tie-breakers; scalar
keyset cursors are rejected for them on both sides of the IPC boundary.

MySQL integers outside JavaScript's safe range and all DECIMAL values travel as
strings. DECIMAL decoding preserves MySQL's full precision without an
intermediate fixed-precision numeric type. Metadata-aware editing does not guess
that a text value is numeric, boolean, NULL or SQL. Inserts bind literal data;
blank fields with server defaults are omitted, while generated and identity
columns are excluded from new/duplicated row forms. For PostgreSQL, `is_identity`
reflects only `GENERATED ALWAYS AS IDENTITY`, which PostgreSQL itself rejects
explicit values for; `GENERATED BY DEFAULT AS IDENTITY` and classic `SERIAL`
columns accept explicit values and are treated as ordinary columns with a
default, matching the server's own semantics. Use the SQL editor for
explicit expressions. This intentionally replaces implicit `NOW()` parsing.
Sessions with temporarily disabled foreign-key checks during row writes are
closed instead of being returned to the pool, including on errors/cancellation.

## PostgreSQL: first functional milestone

The PostgreSQL Database field is optional. Blank/omitted/null values enable server
browsing: the adapter tries `postgres`, the user-named database, then `template1`
as its internal initial database. It retries only missing-database/access-denied
errors, never authentication, TLS or network failures. An explicitly configured
database is respected without falling back; the existing configured-database
filter remains available. The internal initial database is not automatically
selected or opened in the sidebar.

Database listings exclude templates, databases refusing connections and databases
without the role's CONNECT privilege. This cannot preflight database-specific
authentication rules or connection limits, so opening a listed database may still
report a server error. If no initial candidate is accessible, the user must name
an existing database they can access.

One saved connection owns isolated pools per database, opened on demand. Pool
options retain credentials, TLS policy, forwarded SSH endpoint, timeouts and
read-only settings; idle sessions expire after 60 seconds. Disconnect closes all
pools before removing the SSH tunnel and prevents pools from reopening. No MySQL
`USE` operation or globally mutable current-database state is emulated.

The connection form offers PostgreSQL defaults and TLS mode selection;
the sidebar groups accessible tables and views by schema. Selection, tabs,
refresh, row edits, context menus and exports retain full table references, so
`public.users` and `sales.users` are distinct throughout the workflow.

Database creation offers Encoding and Collation using native PostgreSQL metadata,
not MySQL character sets. Defaults come from `template1`; explicit options use
`template0`. The adapter validates encoding/collation combinations and renders
the appropriate libc LC_COLLATE/LC_CTYPE, ICU locale/rules (PostgreSQL 15+) or
builtin locale (17+) configuration. ICU/builtin options are currently offered for
UTF8 only. Nondeterministic collations are excluded because PostgreSQL cannot use
them as database-wide defaults. The server remains authoritative for locale,
encoding and role permissions. See [CREATE DATABASE](https://www.postgresql.org/docs/17/sql-createdatabase.html)
and [collation metadata](https://www.postgresql.org/docs/17/catalog-pg-collation.html).

Dropping a database closes only its cached pool and never forces other clients
off the server. Deleting the connection's initial database or a template is
rejected; reconnect with another initial database to delete the former.

Implemented operations include metadata (columns, ordered composite primary
keys, foreign keys and indexes), table reads, filters, sorting, pagination,
query execution with bounded retained results, transactional row edits, literal
inserts with server defaults, deletion, drop/truncate and streaming CSV/JSON
exports. PostgreSQL casts bound values using native column types from metadata.
Driver-private input metadata separates those casts from display types: input
types have no narrowing length/precision modifiers, and destination assignments
enforce column/domain constraints. Overlength character values are rejected, not
silently truncated, including character arrays and scalar domains. Filter values
are not rounded or truncated before comparison. Sorting qualifies source columns
so ORDER BY uses native values rather than the text projections used for decoding.
Primary-key identity excludes INCLUDE payload columns. Only the literal `pg_`
schema prefix is hidden; user schemas such as `pgapp` remain available.
Large integers and NUMERIC remain exact strings; JSON/JSONB, arrays and other
native values retain their PostgreSQL text representation rather than losing
precision through JavaScript conversion.

Database CSV exports include the schema in escaped filenames. Database JSON
exports use serialized `[schema, name]` tuples as object keys to avoid ambiguity
when identifiers contain dots. Single-table exports remain arrays/CSV rows;
empty PostgreSQL tables retain CSV headers without creating a phantom JSON row.

The SQL editor accepts one statement per execution. Connections set no server
statement timeout — long-running statements (large exports/imports, expensive
DDL, analytical queries) rely on this driver's own `pg_cancel_backend`-based
cancellation (connection.rs, session.rs) instead of a wall-clock cutoff. An
`idle_in_transaction_session_timeout` of 120 seconds still guards against a
leaked open transaction. Read-only connections default all pooled
transactions to read-only, including table/view reads. Each editor execution gets a session that
is closed afterwards, so session settings and open transactions do not leak into
the pool; multi-execution interactive transactions are not supported. Read-only
editor queries additionally run in a server-enforced read-only transaction,
including SELECT expressions that call functions with side effects. This follows
PostgreSQL's [transaction access rules](https://www.postgresql.org/docs/current/sql-set-transaction.html).
Use a restricted database role as the security boundary; the UI policy does not
replace server permissions.

Capabilities enable manual query cancellation, standalone DDL inspection and
column name/type alteration. Disabling foreign-key checks remains unsupported;
table deletion does not implicitly cascade.
Views are readable; their write permissions and editability are not inferred.

Editor queries register independent cancellation channels, including while waiting
for a pool slot. Cancellation targets the original backend PID and start time via
a separate control connection, so a one-slot pool cannot deadlock. Disconnect
also cancels active editor queries; registrations are removed on every exit.
Autocommitted statements that finish before cancellation are not undone.

Column changes use native ALTER TYPE and RENAME in one transaction with a ten-second
lock timeout. Existing compatible defaults, constraints, identity, comments and
collation are retained. Type input accepts names, qualified/quoted user types,
numeric modifiers, time/interval qualifiers and arrays, never arbitrary ALTER
clauses. Incompatible assignment casts fail without applying the rename; explicit
USING conversions belong in the SQL editor. Quoted type-name case is significant.
See PostgreSQL's [ALTER TABLE documentation](https://www.postgresql.org/docs/17/sql-altertable.html).
SQL import is available from the database menu. The transfer service opens the
file and forwards progress; PostgreSQL owns a native streaming import loop, while
MySQL retains its existing parser and batching. PostgreSQL imports run in one
transaction on a disposable session connected to the selected database.
The importer supports UTF8 plain SQL, INSERT, dollar-quoted functions/DO blocks,
nested comments, and text/CSV `COPY ... FROM STDIN` sections terminated by `\.`.
COPY data streams in bounded chunks; individual SQL statements and physical lines
are limited to 16 MiB. Dumps should use `standard_conforming_strings=on` and UTF8;
the runner verifies these settings after statements, including function calls.
Plain BEGIN/COMMIT wrappers are normalized to the import's outer transaction;
custom transaction control is rejected. The first error aborts the import, and
success is reported only after COMMIT succeeds. Schema/data changes roll back on
error/cancellation; PostgreSQL sequence operations such as `setval` and external
function side effects are not generally transactional. Import only trusted files.
Operations forbidden inside transactions (e.g. CREATE DATABASE, VACUUM) are not
supported: create/select the database first. Use a single-database plain dump
without `--create`; archives/compressed dumps must be converted externally.
Modern pg_dump `\restrict`/`\unrestrict` guards are validated; other psql commands
(including database switching, file inclusion and shell commands) are rejected.
Server-file/PROGRAM/COPY TO and binary COPY are not supported. See PostgreSQL's
[COPY](https://www.postgresql.org/docs/current/sql-copy.html) and
[pg_dump](https://www.postgresql.org/docs/current/app-pgdump.html) documentation.

Import cancellation wakes the active runner and cancels its backend using a
separate control connection, including when the import pool has only one slot.
The backend PID and start timestamp identify the exact session. The import
connection remains owned until cancellation completes and is never returned to
the pool. Disconnect signals all active imports. Cancellation stops at the commit
boundary, so a successful commit is never reported as canceled.

### Native PostgreSQL SQL export

The database export dialog now offers SQL for schema + data, schema only and data
only. Single-table SQL export remains data only. `exportSql` and `inspectDdl` are
separate capabilities. Both are implemented: the Structure panel uses the same
catalog definitions as export for table/view DDL, owned sequences, constraints,
indexes, triggers and comments, without rows or unrelated database objects.
Referenced schemas/types/functions/tables must already exist when executing this
standalone DDL. Object-specific unsupported DDL is explained in the panel without
marking the connection offline.

The native driver uses a disposable REPEATABLE READ, READ ONLY session, locks
selected tables against concurrent DDL, and streams COPY text directly from the
server. No display/JSON value conversion is involved. Date, interval, float,
binary, money and timezone settings are normalized in both source and script.
Ordinary rows share a consistent snapshot; sequences are not MVCC-transactional.
Identity/generated columns, serial/standalone sequences and their current state,
enums, domains, composite/range types, custom collations, stored functions/procedures,
constraints, indexes, triggers, schema/table/column/type/index/collation comments, views and materialized
views are handled. Materialized views are recreated without data and refreshed
when the source was populated; stale materialized contents are not copied.
Domain defaults are deparsed from their expression trees with schema-qualified,
current object names, including renamed types, functions and sequences. View
column defaults are restored through dependency-ordered ALTER VIEW statements,
also included in inspected DDL. Keeping these defaults separate from CREATE VIEW
allows their functions to depend on the view's own row type without a false cycle.
Selecting a partitioned parent includes its descendants and uses ONLY when
copying physical tables, avoiding duplicate rows.
On PostgreSQL 17+, identity columns on partitions inherit the root sequence;
they do not require or generate separate child sequences. PostgreSQL 16 only
generates identity values when inserting through the root. Both behaviors are
preserved across export/restore, including nested partitions, without blocking
unrelated table DDL inspection.
Partitioned indexes are created on ONLY the parent and their children attached
bottom-up, preserving nested hierarchy and names. Only root indexes are dropped
explicitly. Collations retain provider, locale, deterministic flag and ICU rules;
the target still needs the corresponding locale/provider installed. Provider
versions are recomputed on the target, as recommended by the
[CREATE COLLATION documentation](https://www.postgresql.org/docs/17/sql-createcollation.html).

Catalog dependencies order creation and reverse-order drops without CASCADE.
Foreign keys/indexes/triggers are applied after data. Required unselected
tables/views cause a clear selection error rather than silently exporting their
data. Referenced types/functions/sequences are included; selecting every relation
also includes standalone supported types/functions/sequences. Dependencies hidden
inside procedural/dynamic SQL cannot be inferred for partial exports. Data-only
scripts require an existing compatible schema and retain the destination's
constraint/trigger behavior.

This is a portable schema/data export, not a replacement for a full administrative
backup: roles, ownership, grants, tablespaces, security labels, replication setup,
extended statistics and physical per-column storage tuning are not copied.
Raw OIDs/snapshot values remain source identifiers, not remapped target objects;
regrole values require the corresponding destination role. Unsupported selected
features (RLS, foreign/typed/inherited tables, partition-specific column overrides,
custom replica identities, invalid physical indexes, rewrite rules,
extension-owned objects, aggregates, SQL-standard function bodies, custom base
types/operators/text-search dependencies and canonical range functions)
fail preflight and direct users to pg_dump. Databases containing large objects are
rejected because their OID contents cannot be silently omitted. CREATE statements
and COPY physical rows exceeding the importer's 16 MiB limit are rejected.

The shared file service handles gzip and writes through a private temporary file
in the destination directory. Only successful exports are published by rename;
failure/cancellation leaves any existing destination unchanged. Cancellation and
disconnect interrupt active server work using the session-specific control path.
Gzip output must be decompressed before using the plain-SQL importer.

TLS modes map to SQLx's PostgreSQL options. `verify_full` over SSH is rejected
until remote certificate-name verification can be configured independently of
the forwarded localhost endpoint. `verify_ca`/`verify_full` require a PEM CA
certificate (`PostgreSqlSettings::ssl_root_cert`, entered in the connection
dialog); connecting fails fast with a clear error instead of a TLS handshake
failure when it is missing, since most self-hosted servers use a private CA
that the OS trust store does not already know about.

SQLx 0.8.6 needs a narrow upstream backport to handle rustls's contextual
hostname-mismatch errors in `verify_ca`. The pinned local core crate, provenance,
security guarantees and removal criteria are documented in
[`TUPLEDB-PATCH.md`](../src-tauri/vendor/sqlx-core/TUPLEDB-PATCH.md).
This does not disable certificate-chain, expiry or signature verification, and
does not change `verify_full` hostname verification.

## Next engine work

1. Extend coverage of the remaining advanced database-local export objects listed
   above, and verify additional server versions and packaged desktop platforms.
2. Add SQLite as a separate adapter with file-based connection configuration and
   its own transaction, metadata and type semantics.
3. Keep each optional capability false until both backend and UI paths are ready.

Avoid making a runtime plugin loader part of this migration. Statically compiled
adapter modules provide the required extension point without imposing a public
plugin ABI or requiring every engine to use SQLx.

## Verification

```sh
npm run test:unit
npm run test:component
./node_modules/.bin/vue-tsc --noEmit
npm run build
cargo test --manifest-path src-tauri/Cargo.toml --lib --test connection_config
TUPLEDB_TEST_MYSQL_URL=mysql://root@127.0.0.1:3306/mysql cargo test --manifest-path src-tauri/Cargo.toml --test mysql_integration -- --ignored
TUPLEDB_TEST_POSTGRESQL_URL=postgres://postgres@127.0.0.1:5432/postgres cargo test --manifest-path src-tauri/Cargo.toml --test postgresql_integration -- --ignored --skip import_real_pg_dump
node scripts/test-postgresql-transport.mjs
```

Integration tests create and remove randomly named test databases. Use an
isolated test server. SSH integration requires its separate environment settings.
For the real pg_dump restore test, also set `TUPLEDB_TEST_PG_DUMP_CONTAINER` to
the Docker container serving that same PostgreSQL URL and omit `--skip`.
The test invokes pg_dump inside that container against its fixture-owned database
as `postgres`, then restores both COPY and INSERT dumps into fresh test databases.

The PostgreSQL milestone is covered by 87 frontend unit tests, 55 component/store
tests, 54 Rust unit tests, 8 configuration tests, 18 MySQL integration tests and
35 PostgreSQL integration tests. The real-server suites run against isolated
MySQL 8.4, PostgreSQL 16 and PostgreSQL 17 containers.

The transport runner requires Unix, Node, Cargo, Docker, OpenSSL 3 and OpenSSH.
It provisions a disposable PostgreSQL 17/SSH server on random loopback ports,
generates temporary certificates/keys, and isolates SSH configuration and
known_hosts from the user's files. Its five tests cover 18 connection scenarios:
all TLS modes, hostname mismatch, missing/wrong CA, invalid PEM, expired
certificates, SSH, and SQL import/export/restore across catalog pools. The runner
removes its container, volume and temporary keys even when a test fails.

A packaged macOS app smoke test covered connecting without a database, browsing,
single-click table loading, row editing, queries, database creation with encoding
and collation, and SQL export/import/restore. Other PostgreSQL versions, Windows,
Linux and signed/notarized distribution have not been verified here.

Regression coverage includes full/structure-only and partial exports of domain
and view defaults, repeated restores, renamed and quoted dependencies, sequence
state, null-valued default expressions, standalone view DDL, and version-specific
identity behavior for direct leaf-partition inserts.

Five integration failures were reproduced on the original revision before fixing
the tests: multi-statement fixtures needed the simple SQL protocol, batching had
to be forced independently of the server's packet limit, and cancellation needed
to wait for actual server execution. The SELECT cancellation fixture now uses
SLEEP inside a table query because standalone SLEEP has different interruption
semantics ([MySQL manual](https://dev.mysql.com/doc/refman/8.4/en/miscellaneous-functions.html)).
The existing behavioral assertions are retained, and cancellation now exercises
the adapter's operation-ID contract.
