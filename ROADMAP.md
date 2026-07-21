# Roadmap: TupleDB

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
- **Protección de `Delete`/`Backspace` durante edición inline** — Al editar una celda o una fila nueva, `Delete` y `Backspace` ya no se propagan al grid y no disparan borrado de fila accidental.
- **Indicador `EMPTY` en celdas** — Las celdas con string vacío (`""`) muestran la etiqueta `EMPTY` en cursiva para distinguirlas visualmente de `NULL`.
- **Virtualización del DataGrid** — Implementado con `@tanstack/vue-virtual` para manejar datasets grandes eficientemente
- **Menú contextual de fila** — Clic derecho sobre una fila del grid abre un menú con acciones rápidas: borrar fila inmediatamente y duplicar fila (precarga el formulario de inserción con los valores de la fila).
- **Selección múltiple de filas en el grid** — `Ctrl+Click` / `⌘Click` para alternar selección individual; `Shift+Click` para selección por rango; las filas seleccionadas pueden marcarse para borrado en lote con un solo gesto.
- **Selección de filas en tablas sin clave primaria** — Las tablas sin PK usan el índice de fila (`__row_index:N`) como clave de selección, habilitando el panel de detalle y el menú contextual también en vistas y tablas sin PK.
- Sistema de tabs con estado independiente por tab (página, filtros, sort, cambios pendientes)
- **Cierre rápido de tabs** — Soporte para cerrar pestañas haciendo clic con el botón central (rueda) del ratón
- Sidebar jerárquico: Conexión → Base de datos → Tabla
- **Rediseño visual del sidebar** — Chips de entorno con color y anillo por ambiente (PRODUCTION/STAGING/DEV), tipografía más legible, iconos actualizados y densidad de información mejorada.
- **Icono de vista en el sidebar** — Las tablas de tipo `VIEW` muestran un icono de ojo en lugar del icono de tabla para distinguirlas visualmente.
- Búsqueda en sidebar (tablas y bases de datos)
- **Creación de base de datos desde modal** — La acción `New Database` del menú contextual de conexión abre un diálogo dedicado en vez de insertar un formulario inline dentro del sidebar.
- Panel de detalle de fila
- FilterBar visual tipo TablePlus
- Paginación con selector de page size
- **Conteo aproximado con UI** — Cuando el total es una estimación, la paginación muestra el prefijo `~` y el sufijo `+`; un botón `Exact` permite solicitar el conteo real bajo demanda sin recargar los datos.
- Indicadores visuales de entorno (borde superior por color de environment)
- Resize de paneles (sidebar y panel de detalle) con drag
- Resize de columnas drag-to-resize con persistencia por tabla en localStorage

### Exploración de esquemas

- Estructura de tabla (columnas, tipos, constraints, extras)
- Claves foráneas con navegación relacional (click en FK abre tabla relacionada con filtro)
- Índices de tabla
- **Refresco manual de esquema por base de datos** — Acción `Refresh Schema` desde el menú contextual de la base de datos para recargar tablas y metadatos de pestañas abiertas sin reconectar la app.
- Context menu en sidebar (crear DB desde context menu de conexión, drop table, truncate, importar SQL, exportar)

### Operaciones de base de datos

- Crear base de datos desde la UI
- DROP TABLE / TRUNCATE TABLE con confirmación fuerte
- **Borrado múltiple de tablas optimizado** — Las operaciones de `Delete tables` y selección múltiple agrupan tablas por conexión/base de datos y las borran en lote desde backend, evitando una llamada Tauri y una reconfiguración de sesión por tabla.
- Importar ficheros `.sql`
- **Importación SQL incremental por streaming** — El import ya no espera a leer y trocear todo el dump en memoria antes de empezar; ahora parsea el fichero por streaming y ejecuta statements por lotes durante la lectura.
- **Sesión de importación persistente** — La importación reutiliza una conexión dedicada durante todo el proceso en vez de reacquirir y reconfigurar sesión MySQL en cada batch.
- **Batching adaptativo de imports** — El tamaño del bloque SQL ahora se ajusta al `max_allowed_packet` del servidor y agrupa por bytes reales además de por número de statements para aprovechar mejor conexiones SSH.
- **Compactación conservadora de `INSERT`s pequeños** — Cuando el dump trae muchos `INSERT ... VALUES (...)` contiguos y compatibles, el importador los fusiona en bloques mayores antes de enviarlos a MySQL para reducir round-trips sobre SSH.
- **Cancelación de importaciones SQL** — La importación en curso puede cancelarse desde la barra de estado; la app marca el import como cancelado y corta el lote MySQL activo cuando es posible.
- **Métricas de importación por fase** — El resultado del import ahora separa tiempos de lectura, procesado/compactación y ejecución, además de batches y bloques SQL, para perfilar dumps reales con datos concretos.
- **Perfilado de importación SQL grande por SSH** — Baseline corregido con dump real: 1.235.527 statements importados correctamente en 1m 39s; lectura 450ms, procesado/compactación 28.4s, ejecución MySQL/SSH 1m 10s, 10 batches y 1.235.252 `INSERT`s compactados en 275 bloques SQL. Se probó y descartó el fast path de sentencia por línea porque empeoró a 2m 34s y confirmó que el cuello dominante queda en ejecución MySQL/SSH.
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

### Rendimiento sobre `main`

- **Instrumentación de carga de tablas** — `get_table_data` separa tiempos de `COUNT(*)`, lectura de página y total; el query log muestra las fases principales para perfilar tablas grandes.
- **Conteo exacto por defecto con soporte de estimación** — La UI mantiene el total real de registros por UX, y el backend conserva soporte para conteo estimado bajo demanda si se decide usarlo más adelante.
- **Carga lazy de metadatos** — Al abrir tabla se cargan datos y estructura básica; índices, claves foráneas y DDL se cargan al entrar en `Structure`.
- **Límite de filas retenidas en Query Editor** — Los SELECT masivos mantienen el contador en vivo, pero el renderer retiene solo las primeras 5000 filas y muestra aviso de truncado.
- **Exportación completa en streaming** — `export_database` escribe incrementalmente con `BufWriter` y consume filas por streaming, evitando construir un SQL gigante en memoria.
- **Keyset pagination con fallback** — La navegación secuencial `next/prev` usa cursor por primary key cuando es seguro; offset exacto, cambios de límite, filtros y ordenaciones arbitrarias siguen usando `OFFSET`.

### Tests

Checklist de cobertura actual y recomendada:

- [x] **`query_builder.rs`** — Generación de `WHERE`, `AND/OR`, params bindados, booleanos normalizados, `NULL` y filas inactivas.
- [x] **`filters.rs`** — Contrato serde de operadores `snake_case` entre frontend y backend.
- [x] **`security.rs`** — Modo read-only en producción y validación anti-injection de columnas para `ORDER BY`.
- [x] **Helpers MySQL puros** — Escape CSV, literales SQL para keyset, predicados de cursor y WKB `POINT` con fallback hexadecimal.
- [x] **Export SQL helpers** — Formateo de valores `NULL`, strings escapados, booleanos y numeros para exportacion.
- [x] **Import SQL splitter** — Separacion incremental de statements, flush final, `;` dentro de strings/backticks, comentarios de linea/bloque y tokens tipo comentario dentro de strings.
- [x] **Compactacion de `INSERT`s** — Parsing de inserts compactables, rechazo de statements no compatibles y merge conservador de inserts con el mismo prefijo.
- [x] **Integracion MySQL opcional** — Suite ignorada por defecto con `TUPLEDB_TEST_MYSQL_URL`; cubre lectura real con `sqlx`, `NULL`, string vacio, decimal, boolean, JSON, filtros/sort, keyset, vistas y estructura.
- [x] **Import session contra MySQL real** — Ejecutar statements representativos con comments, strings con `;` y multiples batches sobre una sesion persistente; validar filas importadas.
- [x] **Operaciones destructivas** — Tests de `drop_table`, `drop_tables`, `truncate_table` y `drop_database` contra una DB temporal, incluyendo nombres raros y FK checks.
- [x] **Aplicacion de cambios** — Tests de `insert_row` y `apply_table_changes` para UPDATE/DELETE transaccional, `NULL`, empty string, booleanos y expresiones SQL permitidas.
- [x] **Tipos MySQL raros** — Cobertura adicional con `BIT`, `BLOB/VARBINARY`, `DATE/TIME/DATETIME/TIMESTAMP`, `YEAR`, `ENUM/SET`, `JSON` y geometria.
- [x] **Streaming/cancelacion de queries en backend** — `execute_query` valida chunks de 500 filas, columnas en el primer chunk, `KILL QUERY` sobre una query larga y limpieza del tracking de query incluso en error.
- [x] **Import cancelable en backend** — Test de cancelacion durante un batch largo con sesion dedicada, `KILL CONNECTION` y limpieza del tracking de import.
- [x] **Import end-to-end desde fichero** — Ejecutar un dump representativo desde fichero con comments, strings con `;`, inserts compactables y multiples batches; validar filas importadas, progreso y metricas.
- [x] **Export de tabla end-to-end contra MySQL real** — Exportar una tabla temporal en CSV, JSON y SQL; validar contenido, escapes, progreso y que el SQL exportado pueda reimportarse.
- [x] **Export de base end-to-end contra MySQL real** — Exportar una base temporal completa en modo full; validar contenido, escapes, progreso y que el SQL exportado pueda reimportarse.
- [x] **Streaming de queries en frontend** — Tests unitarios de la retencion maxima de 5000 filas, contador total de filas y truncado de resultados buffered/streaming.
- [x] **Estado de cancelacion en Query Editor** — Tests unitarios del boton de cancelacion, bloqueo de doble cancelacion y supresion del error backend durante cancelacion intencional.
- [x] **Edicion de celdas en frontend** — Tests unitarios de `normalizeInsertValue`, `normalizeChangeValue`, `coercePkValue` y `computeCellEditValue` extraidos a `src/lib/tableEditing.ts`; cubre null, empty string, booleanos, numericos, pending changes y tablas sin PK.
- [x] **Gestion de tabs en frontend** — Tests unitarios de `findTabInsertIndex` (agrupacion por conexion+BD) y `findNextActiveIndex` (tab a activar al cerrar); extraidos a `src/lib/tabManagement.ts`.
- [x] **Seleccion de filas en frontend** — Tests unitarios de `buildSortPayload`, `resolveKeysetColumn` y `computeRowClickSelection` (Ctrl+Click, Shift+Click, clic simple, sin PK); extraidos a `src/lib/rowSelection.ts`.
- [x] **Smoke visual de cancelacion en UI** — `QueryCancelHarness` con Vitest + Vue Test Utils: boton Cancel aparece al correr query, muestra "Cancelling..." deshabilitado tras click, error backend suprimido si el usuario cancelo, error surfaceado si falla de otro modo.
- [x] **SSH/import remoto** — Suite de integracion en `src-tauri/tests/ssh_integration.rs`; se salta silenciosamente si las env vars no estan; cubre tunel SSH, lectura/escritura basica y batching/compactacion de imports con 500 filas.
- [x] **Frontend smoke tests** — Smoke tests de `PaginationFooter` (rango, conteo aproximado, paginacion, toggle de vista) y `DataGrid` (empty state, NULL/EMPTY, seleccion, multi-seleccion, borrado pendiente, sin PK, sort) con Vitest + happy-dom.

---

## Pendiente

### Baja prioridad / v2

- **Evaluar vía de importación masiva alternativa** — Siguiente salto probable tras el perfilado: medir variabilidad de ejecución por batch y estudiar una vía tipo carga masiva si MySQL/SSH sigue siendo el cuello dominante.

- **Soporte PostgreSQL** — Una vez implementado el trait `DatabaseDriver`, añadir el driver de Postgres (`sqlx` ya lo soporta).

- **Soporte SQLite** — Útil para uso local/offline. Driver más simple de implementar.

- **Auto-updater** — Tauri v2 incluye el plugin `tauri-plugin-updater`. Requiere infraestructura de releases (GitHub Releases o servidor propio).

- **Sistema de plugins** — Arquitectura de drivers externos vía JSON-RPC para que terceros puedan añadir soporte de nuevas bases de datos sin modificar el binario. Alto valor diferenciador, alta complejidad. Diseñar la interfaz antes de implementar.

- **Cache local** — SQLite local para cachear esquemas y reducir latencia en reconexiones frecuentes.

---

_Última actualización: Abril 2026 — v0.6.4_
