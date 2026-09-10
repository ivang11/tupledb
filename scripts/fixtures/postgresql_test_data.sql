-- TupleDB: banco de pruebas de PostgreSQL (validar en una base desechable).
-- Objetivo: PostgreSQL 14+; probado en PostgreSQL 17.
-- Referencia de tipos: https://www.postgresql.org/docs/17/datatype.html
--
-- USO EN TUPLEDB:
-- 1. Conecta a una base de pruebas y activa la escritura de la conexion.
-- 2. Menu contextual de la base > Import SQL > selecciona este archivo.
--    Alternativa: pega y ejecuta TODO el archivo como una unica sentencia DO.
-- 3. Refresca los esquemas: apareceran tupledb_test y tupledb_test_alt.
-- No requiere superusuario ni extensiones; si permisos CREATE en la base.
-- Alternativa: psql -X -v ON_ERROR_STOP=1 -d TU_BASE -f postgresql_test_data.sql
--
-- SEGURIDAD: no borra ni modifica objetos existentes. Si alguno de los dos
-- esquemas ya existe, falla y revierte toda la creacion. Para repetir, utiliza
-- otra base de pruebas. No se incluye un DROP SCHEMA CASCADE automatico.
--
-- COBERTURA: tipos nativos de uso general, todos los rangos/multirrangos
-- incorporados, enum, dominio, compuesto, arrays y tipos de identificadores.
-- No pretende cubrir tipos internos/pseudotipos (void, record, anyelement...),
-- todas las combinaciones posibles, ni extensiones (PostGIS, vector, hstore...).
-- smallserial/serial/bigserial son formas de definir columnas autoincrementales;
-- PostgreSQL las mostrara como smallint/integer/bigint con un default nextval.
--
-- DATOS: ficticios. Incluye NULL, vacio, Unicode, comillas, saltos de linea,
-- limites enteros, decimales exactos, NaN, infinitos y fechas especiales.
-- Las representaciones de money, interval y timestamptz dependen de la sesion.

DO $tupledb_fixture$
BEGIN
    CREATE SCHEMA tupledb_test;
    CREATE SCHEMA tupledb_test_alt;
    PERFORM set_config('TimeZone', 'UTC', true);
    PERFORM set_config('DateStyle', 'ISO, YMD', true);

    -- 01. Tipos definidos por el usuario.
    CREATE TYPE tupledb_test.order_status AS ENUM ('pending', 'paid', 'shipped', 'cancelled');
    CREATE DOMAIN tupledb_test.email_address AS text
        CHECK (VALUE IS NULL OR VALUE ~ '^[^@[:space:]]+@[^@[:space:]]+[.][^@[:space:]]+$');
    CREATE DOMAIN tupledb_test.positive_amount AS numeric(20,6)
        CHECK (VALUE >= 0);
    CREATE TYPE tupledb_test.postal_address AS (
        street text, city text, postcode varchar(12), country char(2)
    );
    CREATE TYPE tupledb_test.temperature_range AS RANGE (subtype = double precision);

    -- 02. Numeros: no debe redondearse bigint ni numeric al cruzar JavaScript.
    CREATE TABLE tupledb_test.numeric_types (
        id integer PRIMARY KEY,
        scenario text NOT NULL,
        small_value smallint,
        integer_value integer,
        big_value bigint,
        exact_value numeric(65,30),
        unrestricted_numeric numeric,
        decimal_alias decimal(12,4),
        real_value real,
        double_value double precision,
        money_value money
    );
    INSERT INTO tupledb_test.numeric_types VALUES
        (1, 'maximos y precision', 32767, 2147483647, 9223372036854775807,
         12345678901234567890123456789012345.123456789012345678901234567890,
         1e100, 12345678.1234, 3.1415927, 3.141592653589793, 1234567.89::numeric::money),
        (2, 'minimos y negativos', -32768, -2147483648, -9223372036854775808,
         -0.000000000000000000000000000001, -1e100, -12345678.1234,
         -1.25e-30, -1.25e-200, (-42.50)::numeric::money),
        (3, 'ceros', 0, 0, 0, 0, 0, 0, 0, 0, 0::numeric::money),
        (4, 'NULL', NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL),
        (5, 'NaN', NULL, NULL, 9007199254740991, NULL, 'NaN', NULL, 'NaN', 'NaN', NULL),
        (6, 'infinito positivo / bigint fuera del rango seguro JS', NULL, NULL,
         9007199254740992, NULL, 'Infinity', NULL, 'Infinity', 'Infinity', NULL),
        (7, 'infinito negativo / bigint que no se debe redondear', NULL, NULL,
         9007199254740993, NULL, '-Infinity', NULL, '-Infinity', '-Infinity', NULL);
    COMMENT ON COLUMN tupledb_test.numeric_types.big_value IS
        '9223372036854775807 y 9007199254740993 deben conservarse exactamente.';

    -- 03. Texto y booleanos. Texto "NULL" no es SQL NULL; NOW() es texto literal.
    CREATE TABLE tupledb_test.text_and_boolean (
        id integer GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
        fixed_text char(8), short_text varchar(120), long_text text,
        enabled boolean, default_text text DEFAULT 'valor por defecto'
    );
    INSERT INTO tupledb_test.text_and_boolean (fixed_text, short_text, long_text, enabled) VALUES
        ('abc', 'Español: ñ, á, ü · 日本語 · العربية · 🐘', 'Texto normal', true),
        ('', '', '', false),
        (NULL, NULL, NULL, NULL),
        ('NULL', 'NULL', 'NOW()', true),
        ('quotes', 'O''Reilly, "comillas", 100% y guion_bajo',
         E'Primera linea\nSegunda linea\r\nTabulador:\tRuta: C:\\datos\\prueba', false),
        ('long', '<script>alert("solo texto")</script>', repeat('Texto largo ñ 🐘 · ', 1024), true),
        ('csv', '=1+1', '+SUM(A1:A2)', false);

    -- 04. Calendario, zonas horarias, microsegundos, intervalos e infinitos.
    CREATE TABLE tupledb_test.temporal_types (
        id integer PRIMARY KEY, scenario text,
        date_value date, time_value time(6), time_zone_value time(6) with time zone,
        timestamp_value timestamp(6), timestamp_zone_value timestamp(6) with time zone,
        interval_value interval, month_interval interval year to month
    );
    INSERT INTO tupledb_test.temporal_types VALUES
        (1, 'bisiesto y microsegundos', '2024-02-29', '23:59:59.123456', '23:59:59.123456+02',
         '2024-02-29 23:59:59.123456', '2024-02-29 23:59:59.123456+02',
         '1 year 2 mons 3 days 04:05:06.123456', '2 years 3 mons'),
        (2, 'cambio horario y duracion negativa', '2026-10-25', '00:00:00', '02:30:00+01',
         '2026-10-25 02:30:00', '2026-10-25 02:30:00+01', '-3 days -04:05:06', '-14 mons'),
        (3, 'infinito positivo', 'infinity', '24:00:00', '00:00:00-05:30',
         'infinity', 'infinity', '0', '0'),
        (4, 'infinito negativo', '-infinity', NULL, NULL, '-infinity', '-infinity', NULL, NULL),
        (5, 'antes de Cristo', '0044-03-15 BC', '12:00:00', '12:00:00+00',
         '0044-03-15 12:00:00 BC', '0044-03-15 12:00:00+00 BC', '1 microsecond', '1 mon'),
        (6, 'NULL', NULL, NULL, NULL, NULL, NULL, NULL, NULL);

    -- 05. Binario, bits y UUID: bytea incluye el byte cero, no permitido en text.
    CREATE TABLE tupledb_test.binary_types (
        id integer PRIMARY KEY, uuid_value uuid,
        binary_value bytea, fixed_bits bit(8), variable_bits bit varying(64)
    );
    INSERT INTO tupledb_test.binary_types VALUES
        (1, '123e4567-e89b-12d3-a456-426614174000', decode('0001027f80feff', 'hex'), B'10101010', B'101'),
        (2, '00000000-0000-0000-0000-000000000000', decode('', 'hex'), B'00000000', B''),
        (3, 'ffffffff-ffff-ffff-ffff-ffffffffffff', convert_to('Hola ñ 🐘', 'UTF8'), B'11111111', B'1111000011110000'),
        (4, NULL, NULL, NULL, NULL);

    -- 06. JSON textual conserva duplicados; JSONB normaliza claves. Numeros exactos.
    CREATE TABLE tupledb_test.document_types (
        id integer PRIMARY KEY, json_value json, jsonb_value jsonb,
        xml_value xml, path_value jsonpath,
        search_document tsvector, search_query tsquery
    );
    INSERT INTO tupledb_test.document_types VALUES
        (1, '{"duplicate":1,"duplicate":2,"big":9223372036854775807}',
         '{"name":"Ana 🐘","big":9223372036854775807,"decimal":123456789.123456789123456789,"active":true,"tags":["a","b"],"nested":{"empty":null}}',
         '<root lang="es"><name>Ana &amp; Luis</name><empty/></root>', '$.tags[*]',
         to_tsvector('simple', 'PostgreSQL tipos y pruebas'), to_tsquery('simple', 'postgresql & pruebas')),
        (2, '[]', '{}', '<root/>', '$.missing', ''::tsvector, ''::tsquery),
        (3, 'null', 'null', NULL, '$', NULL, NULL),
        (4, NULL, NULL, NULL, NULL, NULL, NULL),
        (5, '"texto JSON"', '[1,"dos",true,null,{"x":3}]',
         '<root><![CDATA[Texto <literal>]]></root>', '$[*] ? (@.x == 3)',
         to_tsvector('simple', 'Unicode ñ y espacios'), plainto_tsquery('simple', 'unicode'));
    CREATE INDEX document_jsonb_gin ON tupledb_test.document_types USING gin(jsonb_value);
    CREATE INDEX document_search_gin ON tupledb_test.document_types USING gin(search_document);

    -- 07. IPv4, IPv6, redes y direcciones MAC.
    CREATE TABLE tupledb_test.network_types (
        id integer PRIMARY KEY, address inet, network cidr, mac macaddr, extended_mac macaddr8
    );
    INSERT INTO tupledb_test.network_types VALUES
        (1, '192.0.2.10/24', '192.0.2.0/24', '08:00:2b:01:02:03', '08:00:2b:ff:fe:01:02:03'),
        (2, '2001:db8::42/64', '2001:db8::/32', 'ff:ff:ff:ff:ff:ff', 'ff:ff:ff:ff:ff:ff:ff:ff'),
        (3, '127.0.0.1', '0.0.0.0/0', '00:00:00:00:00:00', '00:00:00:00:00:00:00:00'),
        (4, NULL, NULL, NULL, NULL);

    -- 08. Geometria nativa (no PostGIS).
    CREATE TABLE tupledb_test.geometric_types (
        id integer PRIMARY KEY, point_value point, line_value line, segment_value lseg,
        box_value box, open_path path, closed_path path, polygon_value polygon, circle_value circle
    );
    INSERT INTO tupledb_test.geometric_types VALUES
        (1, '(1.5,-2.25)', '{1,-1,0}', '[(0,0),(3,4)]', '(3,4),(0,0)',
         '[(0,0),(1,2),(3,4)]', '((0,0),(1,0),(1,1))', '((0,0),(4,0),(4,4),(0,4))', '<(1,2),3.5>'),
        (2, '(0,0)', '{0,1,-2}', '[(0,0),(0,0)]', '(0,0),(0,0)',
         '[(0,0)]', '((0,0),(2,0),(0,2))', '((0,0),(2,0),(0,2))', '<(0,0),0>'),
        (3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL);
    CREATE INDEX geometric_point_gist ON tupledb_test.geometric_types USING gist(point_value);

    -- 09. Arrays: NULL, vacio, elemento NULL, dimensiones y limites no habituales.
    CREATE TABLE tupledb_test.array_types (
        id integer PRIMARY KEY, integers integer[], big_integers bigint[], texts text[],
        decimals numeric[], booleans boolean[], uuids uuid[], dates date[],
        documents jsonb[], matrix integer[][], custom_bounds integer[],
        statuses tupledb_test.order_status[]
    );
    INSERT INTO tupledb_test.array_types VALUES
        (1, ARRAY[1,2,NULL], ARRAY[9223372036854775807::bigint,9007199254740993::bigint],
         ARRAY['uno','dos, tres','NULL',NULL,'','ñ 🐘'],
         ARRAY[1.123456789123456789::numeric,0.000000000000000001::numeric],
         ARRAY[true,false,NULL], ARRAY['123e4567-e89b-12d3-a456-426614174000'::uuid],
         ARRAY['2024-02-29'::date,'infinity'::date], ARRAY['{"id":9223372036854775807}'::jsonb,'null'::jsonb],
         ARRAY[[1,2,3],[4,5,6]], '[0:2]={10,20,30}'::integer[],
         ARRAY['pending','paid']::tupledb_test.order_status[]),
        (2, '{}', '{}', '{}', '{}', '{}', '{}', '{}', '{}', '{}', '{}', '{}'),
        (3, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL);
    CREATE INDEX array_texts_gin ON tupledb_test.array_types USING gin(texts);

    -- 10. Los seis rangos y los seis multirrangos incorporados.
    CREATE TABLE tupledb_test.range_types (
        id integer PRIMARY KEY, integers int4range, big_integers int8range,
        decimals numrange, timestamps tsrange, zoned_timestamps tstzrange, dates daterange,
        multi_integers int4multirange, multi_big_integers int8multirange,
        multi_decimals nummultirange, multi_timestamps tsmultirange,
        multi_zoned_timestamps tstzmultirange, multi_dates datemultirange
    );
    INSERT INTO tupledb_test.range_types VALUES
        (1, '[1,10)', '[9007199254740993,9223372036854775807)', '[0.000000000000000001,99.99]',
         '[2026-01-01 00:00:00,2026-02-01 00:00:00)',
         '[2026-01-01 00:00:00+00,2026-02-01 00:00:00+00)', '[2024-02-28,2024-03-01)',
         '{[1,3),[7,10)}', '{[9007199254740993,9007199254740995)}', '{[0.1,0.2],[1.1,1.2]}',
         '{[2026-01-01,2026-01-02),[2026-02-01,2026-02-02)}',
         '{[2026-01-01 00:00:00+00,2026-01-02 00:00:00+00)}', '{[2026-01-01,2026-01-10),[2026-02-01,2026-02-10)}'),
        (2, 'empty', 'empty', 'empty', 'empty', 'empty', 'empty', '{}', '{}', '{}', '{}', '{}', '{}'),
        (3, '(,)', '(,)', '(,)', '(,)', '(,)', '(,)', '{(,)}', '{(,)}', '{(,)}', '{(,)}', '{(,)}', '{(,)}'),
        (4, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL);
    CREATE INDEX range_dates_gist ON tupledb_test.range_types USING gist(dates);

    -- 11. Dominios, enum, compuesto y rango personalizado como valores de columna.
    CREATE TABLE tupledb_test.custom_types (
        id integer PRIMARY KEY, status tupledb_test.order_status,
        email tupledb_test.email_address, amount tupledb_test.positive_amount,
        address tupledb_test.postal_address, temperatures tupledb_test.temperature_range
    );
    INSERT INTO tupledb_test.custom_types VALUES
        (1, 'pending', 'ana@example.test', 123.456789,
         ROW('Calle "Mayor", 1','Madrid','28001','ES')::tupledb_test.postal_address, '[-10.5,42.25]'),
        (2, 'shipped', 'test+tag@example.test', 0,
         ROW(NULL,'日本語',NULL,'JP')::tupledb_test.postal_address, 'empty'),
        (3, NULL, NULL, NULL, NULL, NULL);

    -- 12. Identificadores y snapshots. oid/reg* referencian objetos de esta base.
    CREATE TABLE tupledb_test.system_types (
        id integer PRIMARY KEY, object_id oid, table_ref regclass, type_ref regtype,
        function_ref regproc, signature_ref regprocedure, operator_ref regoper,
        operator_signature_ref regoperator, namespace_ref regnamespace, role_ref regrole,
        collation_ref regcollation, config_ref regconfig, dictionary_ref regdictionary,
        transaction_id xid, full_transaction_id xid8, command_id cid,
        tuple_id tid, wal_position pg_lsn, snapshot_value pg_snapshot,
        legacy_snapshot txid_snapshot
    );
    INSERT INTO tupledb_test.system_types VALUES
        (1, 'tupledb_test.numeric_types'::regclass::oid, 'tupledb_test.numeric_types', 'pg_catalog.int8',
         'pg_catalog.now', 'pg_catalog.abs(integer)', '0', '+(integer,integer)',
         'tupledb_test', current_user::regrole, 'pg_catalog."C"', 'pg_catalog.simple', 'pg_catalog.simple',
         '42', '9007199254740993', '0', '(42,7)', '16/B374D848', pg_current_snapshot(), txid_current_snapshot()),
        (2, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL,
         NULL, NULL, NULL, NULL, NULL, NULL, NULL);

    -- 13. Identidades, seriales, defaults y columna calculada almacenada.
    CREATE TABLE tupledb_test.generated_values (
        id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
        small_sequence smallserial, normal_sequence serial, big_sequence bigserial,
        optional_identity integer GENERATED BY DEFAULT AS IDENTITY,
        quantity integer NOT NULL DEFAULT 1 CHECK (quantity > 0),
        unit_price numeric(12,2) NOT NULL DEFAULT 9.99 CHECK (unit_price >= 0),
        total numeric(16,2) GENERATED ALWAYS AS (quantity * unit_price) STORED,
        created_at timestamptz NOT NULL DEFAULT CURRENT_TIMESTAMP,
        note text DEFAULT 'creado con defaults'
    );
    INSERT INTO tupledb_test.generated_values DEFAULT VALUES;
    INSERT INTO tupledb_test.generated_values (quantity, unit_price, note)
        VALUES (3, 12.50, 'total esperado: 37.50'), (2, 0, 'total esperado: 0.00');

    -- 14. Modelo relacional, FK entre esquemas y misma tabla en ambos esquemas.
    CREATE TABLE tupledb_test.users (
        id integer GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
        email varchar(120) NOT NULL UNIQUE, name text NOT NULL,
        active boolean NOT NULL DEFAULT true, created_at timestamptz DEFAULT CURRENT_TIMESTAMP
    );
    CREATE TABLE tupledb_test_alt.users (
        id integer PRIMARY KEY, name text NOT NULL,
        main_user_id integer REFERENCES tupledb_test.users(id) ON DELETE RESTRICT
    );
    INSERT INTO tupledb_test.users (email, name, active) VALUES
        ('ana@example.test', 'Ana', true), ('luis@example.test', 'Luis', false), ('zoe@example.test', 'Zoë 🐘', true);
    INSERT INTO tupledb_test_alt.users VALUES (1, 'OTRO ESQUEMA: no confundir con Ana', 1), (2, 'Relacion vacia', NULL);
    CREATE INDEX users_active_name ON tupledb_test.users (name) WHERE active;
    CREATE INDEX users_lower_name ON tupledb_test.users (lower(name));

    CREATE TABLE tupledb_test.orders (
        order_id integer NOT NULL, tenant_id integer NOT NULL, user_id integer NOT NULL,
        status tupledb_test.order_status NOT NULL DEFAULT 'pending',
        amount numeric(20,6) NOT NULL CHECK (amount >= 0), note text,
        CONSTRAINT orders_pk PRIMARY KEY (tenant_id, order_id),
        CONSTRAINT orders_user_fk FOREIGN KEY (user_id) REFERENCES tupledb_test.users(id)
    );
    CREATE TABLE tupledb_test_alt.order_items (
        tenant_id integer NOT NULL, order_id integer NOT NULL, line_number integer NOT NULL,
        product text NOT NULL, quantity integer NOT NULL CHECK (quantity > 0),
        PRIMARY KEY (tenant_id, order_id, line_number),
        FOREIGN KEY (tenant_id, order_id) REFERENCES tupledb_test.orders(tenant_id, order_id)
    );
    INSERT INTO tupledb_test.orders VALUES
        (1, 10, 1, 'paid', 19.990001, 'PK ordenada tenant_id, order_id; distinto al orden de columnas'),
        (1, 20, 2, 'pending', 5.00, 'mismo order_id, otro tenant'),
        (2, 10, 3, 'shipped', 0, NULL);
    INSERT INTO tupledb_test_alt.order_items VALUES
        (10,1,1,'Teclado',1), (10,1,2,'Cable',2), (20,1,1,'Adaptador',1);

    -- 15. Identificadores que requieren comillas: puntos, espacios, Unicode, SQL.
    CREATE TABLE tupledb_test."tabla.con ""comillas""" (
        "clave primaria" integer PRIMARY KEY, "select" text, "nombre con espacios" text,
        "a.b" numeric(30,10), "a""b" text, "a`b" text, "日本語" text
    );
    INSERT INTO tupledb_test."tabla.con ""comillas""" VALUES
        (1, 'No es una palabra SQL aqui', 'O''Reilly', 12345678901234567890.1234567890, 'comilla doble', 'backtick', 'こんにちは'),
        (2, NULL, '', NULL, 'NULL', '', '🐘');

    -- 16. PK de texto con caracteres especiales; pruebas de identidad al editar.
    CREATE TABLE tupledb_test.text_primary_keys (id text PRIMARY KEY, note text);
    INSERT INTO tupledb_test.text_primary_keys VALUES
        ('a:b', 'dos puntos'), ('a.b', 'punto'), ('a|b', 'barra'),
        ('O''Reilly', 'comilla'), ('日本語🐘', 'Unicode'), ('', 'PK de texto vacia valida');

    -- 17. Tablas vacias y sin PK. La de sin PK no debe permitir edicion por fila.
    CREATE TABLE tupledb_test.empty_table (id integer PRIMARY KEY, name text, amount numeric(40,20));
    CREATE TABLE tupledb_test.no_primary_key (label text, value integer);
    INSERT INTO tupledb_test.no_primary_key VALUES ('duplicado',1), ('duplicado',1), (NULL,NULL);

    -- 18. Paginacion, ordenacion, filtros, exportaciones y valores repetidos.
    CREATE TABLE tupledb_test.pagination (
        id bigint PRIMARY KEY, category varchar(12), label text,
        amount numeric(30,12), active boolean, created_at timestamptz, payload jsonb
    );
    INSERT INTO tupledb_test.pagination
    SELECT n, 'grupo_' || (n % 7),
        CASE WHEN n % 19 = 0 THEN NULL ELSE 'Fila ' || lpad(n::text, 5, '0') END,
        n::numeric / 8, n % 2 = 0,
        timestamptz '2026-01-01 00:00:00+00' + n * interval '1 minute',
        jsonb_build_object('sequence', n, 'exact_bigint', 9007199254740993::bigint, 'tags', jsonb_build_array('demo', n % 7))
    FROM generate_series(1, 5000) AS series(n);
    CREATE INDEX pagination_category ON tupledb_test.pagination(category, id);
    CREATE INDEX pagination_created_brin ON tupledb_test.pagination USING brin(created_at);

    -- 19. Tabla particionada y sus dos particiones.
    CREATE TABLE tupledb_test.events (
        id integer NOT NULL, occurred_on date NOT NULL, payload jsonb,
        PRIMARY KEY (occurred_on, id)
    ) PARTITION BY RANGE (occurred_on);
    CREATE TABLE tupledb_test.events_2026 PARTITION OF tupledb_test.events
        FOR VALUES FROM ('2026-01-01') TO ('2027-01-01');
    CREATE TABLE tupledb_test.events_other PARTITION OF tupledb_test.events DEFAULT;
    INSERT INTO tupledb_test.events VALUES
        (1,'2026-09-06','{"event":"inside partition"}'), (2,'2025-01-01','{"event":"default partition"}');

    -- 20. Vista normal y materializada; sin una PK editable expuesta en la vista.
    CREATE VIEW tupledb_test.user_orders AS
        SELECT u.id AS user_id, u.name, o.tenant_id, o.order_id, o.status, o.amount
        FROM tupledb_test.users u LEFT JOIN tupledb_test.orders o ON o.user_id = u.id;
    CREATE MATERIALIZED VIEW tupledb_test.order_summary AS
        SELECT tenant_id, count(*) AS order_count, sum(amount) AS total
        FROM tupledb_test.orders GROUP BY tenant_id;
    CREATE UNIQUE INDEX order_summary_tenant ON tupledb_test.order_summary(tenant_id);

    COMMENT ON SCHEMA tupledb_test IS 'Datos ficticios para probar el adaptador PostgreSQL de TupleDB';
    COMMENT ON SCHEMA tupledb_test_alt IS 'Segundo esquema: identidades duplicadas y FK cruzadas';
    COMMENT ON TABLE tupledb_test.empty_table IS 'CSV con cabecera; JSON sin filas fantasma';
    COMMENT ON TABLE tupledb_test.orders IS 'PK compuesta con orden distinto al de las columnas';
    ANALYZE tupledb_test.pagination;

    ASSERT (SELECT count(*) = 5000 FROM tupledb_test.pagination), 'Faltan filas de paginacion';
    ASSERT (SELECT big_value::text = '9223372036854775807' FROM tupledb_test.numeric_types WHERE id = 1), 'Bigint incorrecto';
    ASSERT (SELECT exact_value::text = '12345678901234567890123456789012345.123456789012345678901234567890'
            FROM tupledb_test.numeric_types WHERE id = 1), 'Numeric incorrecto';
    RAISE NOTICE 'Fixture creado. Refresca tupledb_test y tupledb_test_alt. pagination contiene 5000 filas.';
END
$tupledb_fixture$;

-- CONSULTAS MANUALES: ejecutar por separado DESPUES de cargar el bloque.
-- SELECT * FROM tupledb_test.numeric_types ORDER BY id;
-- SELECT * FROM tupledb_test.temporal_types ORDER BY id;
-- SELECT * FROM tupledb_test.pagination ORDER BY id LIMIT 100 OFFSET 4900;
-- SELECT * FROM tupledb_test.pagination WHERE category = 'grupo_3' AND active ORDER BY id;
-- SELECT * FROM tupledb_test.users;
-- SELECT * FROM tupledb_test_alt.users;
-- SELECT * FROM tupledb_test.user_orders;
-- SELECT * FROM tupledb_test."tabla.con ""comillas""";
-- SELECT 1 AS duplicada, 2 AS duplicada; -- dos columnas, no perder una
-- SELECT * FROM tupledb_test.empty_table; -- conservar metadatos aunque no haya filas
--
-- LISTA DE COMPROBACION:
-- [ ] Mostrar todos los tipos sin bloquear la tabla y conservar valores exactos.
-- [ ] Distinguir SQL NULL, texto 'NULL', texto vacio y JSON null.
-- [ ] Editar numeric/text/boolean y PK compuestas sin tocar otra fila/esquema.
-- [ ] Insertar en generated_values dejando que el servidor aplique defaults.
-- [ ] Rechazar quantity <= 0, emails duplicados, dominios invalidos y FK inexistentes.
-- [ ] Impedir borrar users referenciados por orders (no CASCADE implicito).
-- [ ] Mantener separadas las pestañas users de ambos esquemas.
-- [ ] Paginar 5000 filas sin duplicar/saltar IDs y exportar CSV/JSON completos.
-- [ ] Exportar empty_table con cabecera CSV y array JSON vacio.
-- [ ] Mostrar nombres especiales correctamente. Sirven tambien para detectar
--       limitaciones del cliente: actualmente el validador IPC de ordenacion
--       puede rechazar nombres de columnas con espacios o caracteres especiales.
-- [ ] Mostrar arrays, compuestos, rangos y geometria como texto nativo si no hay
--       editor especializado. No confundir esa representacion con un dato perdido.
