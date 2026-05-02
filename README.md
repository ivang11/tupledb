# DB Viewer

Cliente de MySQL de escritorio construido con:

- Tauri 2
- Vue 3
- Rust

La rama `main` es la base activa del proyecto.

## Stack

- Frontend: Vue 3, Pinia, Vue Router, Tailwind, CodeMirror
- Desktop shell: Tauri
- Backend local: Rust dentro de `src-tauri`

## Requisitos

- Node.js 18+
- npm
- Rust estable

En Linux, Tauri puede necesitar dependencias del sistema para WebKitGTK y empaquetado.

## Instalacion

```bash
npm install
```

## Desarrollo

Frontend solo:

```bash
npm run dev
```

App completa con Tauri:

```bash
npm run dev:app
```

## Build

Build web:

```bash
npm run build
```

Build de la app Tauri:

```bash
npm run build:app
```

## Releases y updater

La app usa el updater de Tauri para descargar nuevas versiones desde GitHub
Releases.

Guia paso a paso:

- [docs/UPDATER_RELEASE.md](./docs/UPDATER_RELEASE.md)

## Tests

Unit tests Rust:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

Tests de integración MySQL, ignorados por defecto:

```bash
DB_VIEWER_TEST_MYSQL_URL='mysql://root:password@127.0.0.1:3306/mysql' \
  cargo test --manifest-path src-tauri/Cargo.toml --test mysql_integration -- --ignored
```

Estos tests crean bases temporales `db_viewer_it_*` y las eliminan al terminar.

## Estructura

```text
.
├── src/                 # UI Vue
├── src-tauri/           # Backend Rust + shell Tauri
├── dist/                # Build web
├── package.json
└── src-tauri/tauri.conf.json
```

## Estado actual

La decision actual es seguir con Tauri y mejorar rendimiento sobre esta base antes de reabrir una migracion de stack.

## Prioridades de rendimiento

Los riesgos principales ahora mismo no vienen de Tauri, sino de como manejamos datasets grandes:

- `COUNT(*)` en cada carga de tabla
- paginacion por `LIMIT/OFFSET`
- queries libres que pueden acumular demasiadas filas en memoria
- exportaciones grandes

Resumen mas detallado:

- [ANALISIS_RENDIMIENTO_MAIN.md](./ANALISIS_RENDIMIENTO_MAIN.md)
