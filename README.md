# TupleDB

TupleDB is a desktop MySQL client built with Tauri, Vue, and Rust.

It is designed for everyday database work: saving connections, browsing
databases and tables, editing rows, running SQL queries, and importing or
exporting data.

## Installation

Requirements:

- Node.js 18+
- npm
- Stable Rust toolchain

Install dependencies:

```bash
npm install
```

Run the desktop app in development:

```bash
npm run dev:app
```

Build the desktop app:

```bash
npm run build:app
```

## Features

- Saved MySQL connections with direct or SSH-tunneled access.
- Connection testing, environment labels, configurable timeouts, and optional
  read-only mode.
- Import and export saved connections.
- Multi-connection workspace with tabs, split panes, draggable tabs, resizable
  panels, and a connection/database rail.
- Database explorer for opening and refreshing databases, tables, and views.
- Create and drop databases.
- Drop or truncate tables.
- Browse table data with pagination, keyset pagination, sorting, filtering,
  virtualized rows, and optional exact row counts.
- View table structure, DDL, indexes, and foreign-key based navigation.
- Insert, edit, duplicate, and delete rows with pending changes before applying.
- Run SQL queries with cancellation, execution timing, streamed results,
  formatting, autocomplete, local history, and saved queries.
- Import SQL files with batching, progress, metrics, and cancellation.
- Export databases or selected tables as SQL, CSV, or JSON, with optional gzip
  compression.
- Status bar with query log, import/export progress, toast notifications,
  configurable keyboard shortcuts, custom window controls, and updater flow.
