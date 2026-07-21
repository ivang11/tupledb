# TupleDB

TupleDB is an open-source desktop client for MySQL, built with Tauri, Vue, and
Rust.

The project is focused on everyday database work: connecting to local or remote
MySQL servers, browsing schemas, inspecting tables, editing data, running SQL.

> Status: early public release. TupleDB is usable, but the API surface,
> packaging, and distribution flow may still change before a stable `1.0`.

## Highlights

- Native desktop app powered by Tauri.
- MySQL-first workflow with direct and SSH-tunneled connections.
- Multi-connection workspace with tabs, split panes, resizable panels, and a
  connection rail.
- Table browsing with pagination, sorting, filtering, virtualized rows, and
  optional exact row counts.
- Row editing with pending changes before applying inserts, updates, duplicates,
  and deletes.
- SQL editor with formatting, autocomplete, cancellation, execution timing,
  streamed results, local history, and saved queries.
- Schema tools for table structure, DDL, indexes, and foreign-key navigation.
- Import and export flows for SQL, CSV, JSON, and compressed exports.
- Auto-update support through signed Tauri updater artifacts.

## Downloads

Releases are published from GitHub Actions.

- Linux: download the `TupleDB_<version>_Linux_x86_64.AppImage` asset.
- macOS Apple Silicon: download `TupleDB_<version>_macOS_Apple_Silicon.dmg`.
- macOS Intel: download `TupleDB_<version>_macOS_Intel.dmg`.

The release may also include `*_updater.app.tar.gz` assets. Those are used by
the built-in updater and are not the normal first-install download.

macOS builds are not notarized yet, so Gatekeeper may warn on first launch.

## Tech Stack

- Tauri 2
- Vue 3
- TypeScript
- Rust
- SQLx
- CodeMirror
- Vitest and Rust integration tests

## Development

Requirements:

- Node.js 18 or newer
- npm
- Stable Rust toolchain
- Platform dependencies required by Tauri

Install dependencies:

```bash
npm install
```

Run the desktop app in development:

```bash
npm run dev:app
```

Build the frontend:

```bash
npm run build
```

Build the desktop app without updater artifacts:

```bash
npm run build:app
```

The app commands are convenience aliases:

- `npm run dev:app` runs `npm run tauri:dev`.
- `npm run build:app` runs `npm run tauri:build`.

Release-only commands live under `release:*` and are normally run by GitHub
Actions.

## Testing

Run the component test suite:

```bash
npm run test:component
```

Run the TypeScript/unit test command:

```bash
npm run test:unit
```

Some Rust integration tests require a real MySQL server and are ignored by
default. Set `TUPLEDB_TEST_MYSQL_URL` when running those tests manually.

Example:

```bash
TUPLEDB_TEST_MYSQL_URL='mysql://root:password@127.0.0.1:3306/mysql' \
  cargo test --manifest-path src-tauri/Cargo.toml --test mysql_integration -- --ignored
```

## Release Flow

The release workflow runs when a `v*` tag is pushed.

```bash
git tag v0.7.0
git push origin v0.7.0
```

The workflow builds Linux and macOS artifacts, then publishes a single GitHub
Release with:

- Human-readable installers for first-time downloads.
- Signed updater artifacts.
- A `latest.json` manifest for the Tauri updater.

The updater signing private key must be configured in GitHub Actions as
`TAURI_SIGNING_PRIVATE_KEY`. If the key has a password, also set
`TAURI_SIGNING_PRIVATE_KEY_PASSWORD`.

## Roadmap

TupleDB is currently focused on becoming a solid MySQL client. Support for other
database engines may come later, but MySQL quality takes priority for now.

## License

TupleDB is released under the [MIT License](LICENSE).
