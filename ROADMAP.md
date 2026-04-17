# Roadmap: DB Viewer

Cliente de bases de datos tipo TablePlus construido con Tauri v2, Rust y Vue 3.

---

## Completado

### Arquitectura base

- Tauri v2 como runtime de escritorio multiplataforma
- Separación estricta en capas: Frontend (Vue) / Backend (Rust) / DB drivers
- Comunicación exclusivamente vía `invoke()` y eventos Tauri
- Estado global de Rust con `parking_lot::RwLock` (sin contención)
- Configuración persistida en disco (`connections.json`)

### Capa de conexión

- MySQL con `sqlx` y pool de conexiones persistente por sesión
- Túneles SSH con autenticación por password y por clave privada
- Gestión de sesiones activas en `AppState`
- `test_connection` con feedback inmediato (usa la contraseña almacenada si la conexión ya existe)
- Múltiples conexiones simultáneas e independientes
- **Timeout configurable por conexión** — Campo en la configuración de conexión; fallback a 30s si no se especifica

### Procesado de datos en backend

- Paginación obligatoria con `LIMIT / OFFSET` en Rust
- Ordenación en backend (validación de nombre de columna anti-injection)
- Filtros avanzados construidos en Rust (`query_builder.rs`): `=`, `!=`, `contains`, `starts_with`, `is_null`, `>`, `<`, etc.
- Lógica global `AND / OR` en filtros
- Soporte completo de tipos MySQL: enteros, decimales, fechas, timestamps, blobs, JSON, geometría WKB→WKT, BIT, BOOLEAN, YEAR, ENUM, SET

### Seguridad

- Modo solo lectura estricto en entorno `PRODUCTION` (bloqueo en Rust, no en frontend)
- Validación de columnas de ordenación contra injection
- Passwords almacenados en keychain del sistema (no en disco en texto plano)
- Frontend nunca recibe passwords
- Confirmaciones adicionales para operaciones destructivas (DROP, TRUNCATE)

### UI y UX

- Data grid con scroll, edición inline de celdas y marcado de filas para borrado
- **Virtualización del DataGrid** — Implementado con `@tanstack/vue-virtual` para manejar datasets grandes eficientemente
- Sistema de tabs con estado independiente por tab (página, filtros, sort, cambios pendientes)
- **Cierre rápido de tabs** — Soporte para cerrar pestañas haciendo clic con el botón central (rueda) del ratón
- Sidebar jerárquico: Conexión → Base de datos → Tabla
- Búsqueda en sidebar (tablas y bases de datos)
- Panel de detalle de fila
- FilterBar visual tipo TablePlus
- Paginación con selector de page size
- Indicadores visuales de entorno (borde superior por color de environment)
- Resize de paneles (sidebar y panel de detalle) con drag
- Resize de columnas drag-to-resize con persistencia por tabla en localStorage

### Exploración de esquemas

- Estructura de tabla (columnas, tipos, constraints, extras)
- Claves foráneas con navegación relacional (click en FK abre tabla relacionada con filtro)
- Índices de tabla
- Context menu en sidebar (crear DB desde context menu de conexión, drop table, truncate, importar SQL, exportar)

### Operaciones de base de datos

- Crear base de datos
- DROP TABLE / TRUNCATE TABLE con confirmación fuerte
- Importar ficheros `.sql`
- Exportar tabla a CSV, JSON o SQL
- Exportar base de datos completa con selección de tablas y modo (estructura, datos o ambos); orden de inserción resuelto con `SET FOREIGN_KEY_CHECKS=0/1`
- Eliminar base de datos completa desde la UI con confirmación fuerte
- Inserción de filas con soporte para expresiones SQL (`NOW()`, `UUID()`, etc.)
- Aplicar cambios en bloque (UPDATE/DELETE en transacción única)
- Opción para desactivar FK checks al guardar

### Query Editor

- Editor SQL con textarea monospace
- Selector de base de datos de contexto
- Resultados en grid para SELECT; contador de filas afectadas para DML
- Historial persistido en localStorage (últimas 100 queries con tiempo de ejecución)
- Panel de historial con carga rápida
- Respeta modo solo lectura en producción
- Integrado en el sistema de tabs
- **Cancelación de queries** — Comando `cancel_query` que ejecuta `KILL QUERY` para interrumpir queries en curso
- **Autocompletado en el Query Editor** — Completado básico de nombres de tablas y columnas usando el esquema ya cargado en memoria.

### Atajos de teclado

- `⌘K` / `Ctrl+K` — Búsqueda en sidebar
- `⌘R` / `Ctrl+R` — Refrescar tabla
- `⌘Enter` / `Ctrl+Enter` — Aplicar filtros / Ejecutar query
- `⌘W` / `Ctrl+W` — Cerrar pestaña activa
- `⌘⇧F` / `Ctrl+Shift+F` — Formatear código SQL (Query Editor)
- `Click central` en pestaña — Cerrar pestaña
- `⌘Click` / `Ctrl+Click` en tabla (Sidebar) — Seleccionar múltiples tablas para exportación en bloque
- **Keybindings configurables** — Todos los atajos configurables vía JSON (`localStorage`) con UI visual en la barra de título; soporte de Shift+Click para selección por rango en sidebar

### Queries guardadas

- Favoritos persistidos en disco con nombre y descripción
- Asociación opcional a una conexión concreta
- Panel lateral con búsqueda y carga rápida desde el Query Editor

### Arquitectura

- **Arquitectura multi-DB (`DatabaseDriver` trait)** — Trait abstracto `DatabaseDriver` con métodos comunes (`get_tables`, `get_table_data`, `execute_query`, etc.). Implementado con `MySqlDriver` para preparar soporte de PostgreSQL y SQLite.

### Streaming de resultados

- **Streaming chunked en Query Editor** — `execute_query` emite lotes de 500 filas vía `query-chunk:{id}` en cuanto llegan de MySQL. El grid muestra las primeras filas en ~12ms independientemente del tamaño total. Los exports escriben a disco sin cargar todo en memoria.

---

## Pendiente

### Media prioridad

- **Tests en Rust** — Los módulos `query_builder.rs`, `filters.rs` y `security.rs` son candidatos directos a unit tests. Cero tests actualmente.

### Baja prioridad / v2

- **Soporte PostgreSQL** — Una vez implementado el trait `DatabaseDriver`, añadir el driver de Postgres (`sqlx` ya lo soporta).

- **Soporte SQLite** — Útil para uso local/offline. Driver más simple de implementar.

- **Auto-updater** — Tauri v2 incluye el plugin `tauri-plugin-updater`. Requiere infraestructura de releases (GitHub Releases o servidor propio).

- **Sistema de plugins** — Arquitectura de drivers externos vía JSON-RPC para que terceros puedan añadir soporte de nuevas bases de datos sin modificar el binario. Alto valor diferenciador, alta complejidad. Diseñar la interfaz antes de implementar.

- **Cache local** — SQLite local para cachear esquemas y reducir latencia en reconexiones frecuentes.

- **Historial de conexiones recientes** — Mostrar las últimas N conexiones usadas en la pantalla de inicio para acceso rápido.

---

_Última actualización: Abril 2026 — v0.2_
