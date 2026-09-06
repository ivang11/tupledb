# Database adapters

This change establishes the adapter boundary while keeping MySQL as the only
available engine. PostgreSQL and SQLite configuration shapes are defined, but
their connections are rejected before opening sockets, files or SSH tunnels.

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
```

The common database contracts do not depend on Tauri or SQLx. Commands do not
construct drivers. Services call contracts, and the registry is the production
entry point that selects a concrete adapter. MySQL-specific thread IDs remain
inside the MySQL module; application cancellation uses query/import IDs.

CSV/JSON output, file buffering and progress are shared. SQL exports obtain
identifier quoting, literals and session directives from the adapter. SQL import
uses an adapter-supplied incremental parser, including its opt-in compaction
policy. A new adapter must not inherit MySQL grammar by accident.

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
MySQL verifies the complete primary key against metadata before any write and
updates all changed columns in one statement, including primary-key edits.
Tables without a primary key remain non-editable. Composite keys use OFFSET
pagination with all primary-key columns as deterministic tie-breakers; scalar
keyset cursors are rejected for them on both sides of the IPC boundary.

MySQL integers outside JavaScript's safe range and all DECIMAL values travel as
strings. DECIMAL decoding preserves MySQL's full precision without an
intermediate fixed-precision numeric type. Metadata-aware editing does not guess
that a text value is numeric, boolean, NULL or SQL. Inserts bind literal data;
blank fields with server defaults are omitted, while generated and identity
columns are excluded from new/duplicated row forms. Use the SQL editor for
explicit expressions. This intentionally replaces implicit `NOW()` parsing.
Sessions with temporarily disabled foreign-key checks during row writes are
closed instead of being returned to the pool, including on errors/cancellation.

## Next: implement and enable engines

The common preparation is complete; no further general module split is required
before starting PostgreSQL. Enabling it still requires its adapter and UI:

1. Implement PostgreSQL and SQLite adapters with their own decoding, cancellation,
   session/read-only semantics, DDL and import/export support. Unsupported optional
   features must return errors and have false capabilities; never report success
   for an operation that did nothing.
2. Add engine-specific connection forms, schema navigation and capability checks
   for the remaining menus. The sidebar's selection/context menus and bulk-export
   selection still use bare names for MySQL; migrate those interactions to the
   explicit references when adding schema navigation. PostgreSQL database routing
   must use separate physical connections, not MySQL's `USE` semantics. Add an
   adapter to `AVAILABLE_DRIVERS` only after these paths are tested with duplicate
   table names across schemas. Autocompletion must use schema-qualified caches.
3. Run shared behavior tests against each real engine, plus engine-specific tests
   for scripts, types, transactions, cancellation and DDL preservation.

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
```

Integration tests create and remove randomly named test databases. Use an
isolated test server. SSH integration requires its separate environment settings.

The refactor was verified with 86 frontend unit tests, 41 component/store tests,
42 Rust unit tests, 7 configuration tests and 18 integration tests against an
isolated MySQL 8.4 container. Type checking and the web build also passed.

Five integration failures were reproduced on the original revision before fixing
the tests: multi-statement fixtures needed the simple SQL protocol, batching had
to be forced independently of the server's packet limit, and cancellation needed
to wait for actual server execution. The SELECT cancellation fixture now uses
SLEEP inside a table query because standalone SLEEP has different interruption
semantics ([MySQL manual](https://dev.mysql.com/doc/refman/8.4/en/miscellaneous-functions.html)).
The existing behavioral assertions are retained, and cancellation now exercises
the adapter's operation-ID contract.
