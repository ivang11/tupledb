use super::*;
use crate::database::results::retained_row_limit;
use crate::filters::{FilterSet, Operator};
use futures::StreamExt;
use serde_json::Value;
use sqlx::{Column, Either, Executor, Postgres, QueryBuilder, TypeInfo};
use std::{sync::Arc, time::Instant};

pub(super) fn bind_value(
    query: &mut QueryBuilder<'_, Postgres>,
    value: &Value,
    column: &PgColumn,
) -> Result<(), String> {
    let text = match value {
        Value::Null => None,
        Value::String(s) => Some(s.clone()),
        Value::Bool(_) | Value::Number(_) => Some(value.to_string()),
        _ if matches!(column.value_kind, ValueKind::Json) => Some(value.to_string()),
        _ => return Err("Use the PostgreSQL text representation for this value".into()),
    };
    if column.record_input {
        // An array of array-valued domains is not a multidimensional array:
        // retain its element domains and use their input functions (which check
        // lengths) rather than explicit casts (which can silently truncate).
        query.push("(SELECT value FROM pg_catalog.json_to_record(pg_catalog.json_build_object('value', ")
            .push_bind(text)
            .push(")) AS tupledb_input(value ")
            .push(&column.input_type)
            .push("))");
        return Ok(());
    }
    query
        .push("CAST(")
        .push_bind(text)
        .push(" AS ")
        .push(&column.input_type)
        .push(")");
    Ok(())
}

pub(super) fn column<'a>(columns: &'a [PgColumn], name: &str) -> Result<&'a PgColumn, String> {
    columns
        .iter()
        .find(|c| c.field == name)
        .ok_or_else(|| format!("Unknown column: {name}"))
}

fn where_clause(
    query: &mut QueryBuilder<'_, Postgres>,
    filters: Option<&FilterSet>,
    columns: &[PgColumn],
) -> Result<(), String> {
    let Some(filters) = filters else {
        return Ok(());
    };
    let rows: Vec<_> = filters.rows.iter().filter(|r| r.active).collect();
    if rows.is_empty() {
        return Ok(());
    }
    query.push(" WHERE (");
    for (i, row) in rows.iter().enumerate() {
        if i > 0 {
            query.push(if filters.match_all { " AND " } else { " OR " });
        }
        let metadata = column(columns, &row.column)?;
        query.push(sql::quote_identifier(&row.column)?);
        match row.operator {
            Operator::IsNull => {
                query.push(" IS NULL");
            }
            Operator::IsNotNull => {
                query.push(" IS NOT NULL");
            }
            Operator::True | Operator::False => {
                query.push(if matches!(row.operator, Operator::True) {
                    " IS TRUE"
                } else {
                    " IS FALSE"
                });
            }
            Operator::Contains | Operator::StartsWith | Operator::EndsWith => {
                let escaped = row
                    .value
                    .replace('\\', "\\\\")
                    .replace('%', "\\%")
                    .replace('_', "\\_");
                let value = match row.operator {
                    Operator::Contains => format!("%{escaped}%"),
                    Operator::StartsWith => format!("{escaped}%"),
                    _ => format!("%{escaped}"),
                };
                query
                    .push("::text LIKE ")
                    .push_bind(value)
                    .push(" ESCAPE E'\\\\'");
            }
            Operator::In | Operator::NotIn | Operator::Between => {
                let parts: Vec<_> = row.value.split(',').map(str::trim).collect();
                if matches!(row.operator, Operator::Between) {
                    if parts.len() != 2 {
                        return Err("Between expects two comma-separated values".into());
                    }
                    query.push(" BETWEEN ");
                    bind_value(query, &Value::String(parts[0].into()), metadata)?;
                    query.push(" AND ");
                    bind_value(query, &Value::String(parts[1].into()), metadata)?;
                } else {
                    query.push(if matches!(row.operator, Operator::In) {
                        " IN ("
                    } else {
                        " NOT IN ("
                    });
                    for (j, part) in parts.iter().enumerate() {
                        if j > 0 {
                            query.push(", ");
                        }
                        bind_value(query, &Value::String((*part).into()), metadata)?;
                    }
                    query.push(")");
                }
            }
            _ => {
                query.push(match row.operator {
                    Operator::Equals => " = ",
                    Operator::NotEquals => " <> ",
                    Operator::GreaterThan | Operator::After => " > ",
                    Operator::GreaterOrEqual => " >= ",
                    Operator::LessThan | Operator::Before => " < ",
                    Operator::LessOrEqual => " <= ",
                    _ => return Err("Unsupported filter operator".into()),
                });
                bind_value(query, &Value::String(row.value.clone()), metadata)?;
            }
        }
    }
    query.push(")");
    Ok(())
}

#[async_trait]
impl QueryDriver for PostgreSqlDriver {
    async fn get_table_data(
        &self,
        table: &TableRef,
        page: u32,
        page_size: u32,
        filters: Option<FilterSet>,
        sort_column: Option<String>,
        sort_desc: Option<bool>,
        exact_count: bool,
        keyset: Option<KeysetPage>,
    ) -> Result<QueryResult, String> {
        let started = Instant::now();
        let relation = self.relation(table)?;
        let pool = self.pool_for(&table.catalog).await?;
        let structure = self.column_metadata(table).await?;
        let mut pk: Vec<_> = structure
            .iter()
            .filter(|c| c.primary_key_position.is_some())
            .collect();
        pk.sort_by_key(|c| c.primary_key_position);
        let has_filters = filters
            .as_ref()
            .is_some_and(|f| f.rows.iter().any(|r| r.active));
        let estimated = !exact_count && !has_filters;
        let count_timer = Instant::now();
        let mut total_count = if estimated {
            self.get_estimated_row_count(table).await?
        } else {
            let mut query = QueryBuilder::new(format!("SELECT COUNT(*) FROM {relation}"));
            where_clause(&mut query, filters.as_ref(), &structure)?;
            query
                .build_query_scalar::<i64>()
                .fetch_one(&pool)
                .await
                .map_err(|e| e.to_string())?
        };
        let count_ms = count_timer.elapsed().as_millis() as u64;
        let columns: Vec<_> = structure
            .iter()
            .map(|c| ColumnInfo {
                name: c.field.clone(),
                type_name: c.field_type.clone(),
            })
            .collect();
        let projection = structure
            .iter()
            .map(|c| Ok(format!("{}::text", sql::quote_identifier(&c.field)?)))
            .collect::<Result<Vec<_>, String>>()?
            .join(", ");
        let mut query = QueryBuilder::new(format!(
            "SELECT {} FROM {relation} AS tupledb_row",
            if projection.is_empty() {
                "*"
            } else {
                &projection
            }
        ));
        where_clause(&mut query, filters.as_ref(), &structure)?;
        let previous = keyset.as_ref().is_some_and(|k| k.direction == "prev");
        if let Some(cursor) = &keyset {
            if pk.len() != 1
                || pk[0].field != cursor.column
                || sort_column.is_some()
                || !["next", "prev"].contains(&cursor.direction.as_str())
                || cursor.value.is_null()
            {
                return Err(
                    "Keyset pagination requires a single primary key in natural order".into(),
                );
            }
            query
                .push(if has_filters { " AND " } else { " WHERE " })
                .push(sql::quote_identifier(&cursor.column)?)
                .push(if previous { " < " } else { " > " });
            bind_value(&mut query, &cursor.value, pk[0])?;
        }
        let mut order = Vec::new();
        if let Some(sort) = &sort_column {
            column(&structure, sort)?;
            order.push(format!(
                "tupledb_row.{} {}",
                sql::quote_identifier(sort)?,
                if sort_desc.unwrap_or(false) {
                    "DESC"
                } else {
                    "ASC"
                }
            ));
        }
        for key in &pk {
            if sort_column.as_ref() != Some(&key.field) {
                order.push(format!(
                    "tupledb_row.{} {}",
                    sql::quote_identifier(&key.field)?,
                    if previous { "DESC" } else { "ASC" }
                ));
            }
        }
        if !order.is_empty() {
            query.push(" ORDER BY ").push(order.join(", "));
        }
        query.push(" LIMIT ").push_bind(i64::from(page_size));
        if keyset.is_none() {
            query
                .push(" OFFSET ")
                .push_bind(i64::from(page) * i64::from(page_size));
        }
        let select_timer = Instant::now();
        let rows = query
            .build()
            .fetch_all(&pool)
            .await
            .map_err(|e| e.to_string())?;
        let mut rows = rows
            .iter()
            .map(|r| values::parse_text_row(r, &columns, false))
            .collect::<Result<Vec<_>, _>>()?;
        if previous {
            rows.reverse();
        }
        if estimated {
            total_count =
                total_count.max(i64::from(page) * i64::from(page_size) + rows.len() as i64);
        }
        Ok(QueryResult {
            columns,
            rows,
            total_count,
            total_count_is_estimate: estimated,
            timings: Some(TableDataTimings {
                count_ms,
                select_ms: select_timer.elapsed().as_millis() as u64,
                total_ms: started.elapsed().as_millis() as u64,
            }),
        })
    }

    async fn get_all_rows(
        &self,
        table: &TableRef,
    ) -> Result<(Vec<ColumnInfo>, Vec<Value>), String> {
        let sql = format!("SELECT * FROM {}", self.relation(table)?);
        let pool = self.pool_for(&table.catalog).await?;
        let description = pool.describe(&sql).await.map_err(|e| e.to_string())?;
        let columns = description
            .columns()
            .iter()
            .map(|c| ColumnInfo {
                name: c.name().into(),
                type_name: c.type_info().name().into(),
            })
            .collect::<Vec<_>>();
        let rows = sqlx::raw_sql(&sql)
            .fetch_all(&pool)
            .await
            .map_err(|e| e.to_string())?;
        Ok((
            columns.clone(),
            rows.iter()
                .map(|r| values::parse_text_row(r, &columns, false))
                .collect::<Result<Vec<_>, _>>()?,
        ))
    }

    async fn stream_all_rows(
        &self,
        table: &TableRef,
        tx: tokio::sync::mpsc::Sender<(Option<Vec<ColumnInfo>>, Value)>,
    ) -> Result<(), String> {
        let sql = format!("SELECT * FROM {}", self.relation(table)?);
        let pool = self.pool_for(&table.catalog).await?;
        let mut stream = sqlx::raw_sql(&sql).fetch(&pool);
        let mut first = true;
        while let Some(row) = stream.next().await {
            let row = row.map_err(|e| e.to_string())?;
            let columns = values::columns(&row);
            let value = values::parse_text_row(&row, &columns, false)?;
            if tx
                .send((if first { Some(columns) } else { None }, value))
                .await
                .is_err()
            {
                break;
            }
            first = false;
        }
        drop(stream);
        if first {
            let description = pool.describe(&sql).await.map_err(|e| e.to_string())?;
            let columns = description
                .columns()
                .iter()
                .map(|c| ColumnInfo {
                    name: c.name().into(),
                    type_name: c.type_info().name().into(),
                })
                .collect();
            let _ = tx.send((Some(columns), Value::Null)).await;
        }
        Ok(())
    }

    async fn execute_query(
        &self,
        database: Option<&str>,
        sql: &str,
        query_id: Option<&str>,
        on_progress: Option<Arc<dyn Fn(u64) + Send + Sync>>,
        on_chunk: Option<QueryChunkCallback>,
        max_retained_cells: Option<usize>,
    ) -> Result<RawQueryResult, String> {
        struct Registration<'a>(&'a PostgreSqlDriver, String);
        impl Drop for Registration<'_> {
            fn drop(&mut self) {
                self.0.running_queries.lock().remove(&self.1);
            }
        }
        let id = query_id
            .map(str::to_owned)
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let (tx, mut rx) = tokio::sync::watch::channel(false);
        {
            let mut active = self.running_queries.lock();
            if active.contains_key(&id) {
                return Err("Query ID is already active".into());
            }
            active.insert(id.clone(), tx);
        }
        let _registration = Registration(self, id);
        let database = database.unwrap_or(&self.catalog);
        let pool = tokio::select! {
            biased;
            _ = rx.changed() => return Err("Query cancelled".into()),
            pool = self.pool_for(database) => pool?,
        };
        let mut conn = tokio::select! {
            biased;
            _ = rx.changed() => return Err("Query cancelled".into()),
            conn = pool.acquire() => conn.map_err(|e| e.to_string())?,
        };
        // User SQL may change session state or leave a transaction open.
        conn.close_on_drop();
        let (pid, started): (i32, String) = sqlx::query_as(
            "SELECT pg_backend_pid(), backend_start::text FROM pg_stat_activity WHERE pid=pg_backend_pid()"
        ).fetch_one(&mut *conn).await.map_err(|e| e.to_string())?;
        let execution = async {
            if self.read_only {
                crate::security::is_query_safe(sql, crate::connections::Environment::Local, false)?;
                sqlx::query("BEGIN READ ONLY")
                    .execute(&mut *conn)
                    .await
                    .map_err(|e| e.to_string())?;
            }
            // Parse/describe enforces one statement and gives metadata for empty results.
            let description = conn
                .describe(sql)
                .await
                .map_err(|e| format!("PostgreSQL query (one statement per execution): {e}"))?;
            let columns = description
                .columns()
                .iter()
                .map(|c| ColumnInfo {
                    name: c.name().into(),
                    type_name: c.type_info().name().into(),
                })
                .collect::<Vec<_>>();
            let is_select = !columns.is_empty();
            let limit =
                retained_row_limit(columns.len(), Some(max_retained_cells.unwrap_or(300_000)));
            let mut rows = Vec::new();
            let mut chunk = Vec::new();
            let mut fetched = 0;
            let mut affected = 0;
            if let Some(callback) = &on_chunk {
                if is_select {
                    callback(Some(columns.clone()), vec![]);
                }
            }
            let mut stream = sqlx::raw_sql(sql).fetch_many(&mut *conn);
            while let Some(result) = stream.next().await {
                match result.map_err(|e| e.to_string())? {
                    Either::Left(result) => {
                        affected += result.rows_affected();
                    }
                    Either::Right(row) => {
                        fetched += 1;
                        if fetched <= limit as u64 {
                            let row = values::parse_text_row(&row, &columns, true)?;
                            if on_chunk.is_some() {
                                chunk.push(row);
                            } else {
                                rows.push(row);
                            }
                        }
                        if chunk.len() >= 256 {
                            if let Some(callback) = &on_chunk {
                                callback(None, std::mem::take(&mut chunk));
                            }
                        }
                        if fetched % 1000 == 0 {
                            if let Some(callback) = &on_progress {
                                callback(fetched);
                            }
                        }
                    }
                }
            }
            if !chunk.is_empty() {
                if let Some(callback) = &on_chunk {
                    callback(None, chunk);
                }
            }
            if let Some(callback) = &on_progress {
                callback(fetched);
            }
            Ok(RawQueryResult {
                columns,
                rows,
                rows_affected: if is_select { fetched } else { affected },
                is_select,
            })
        };
        let result = tokio::select! {
            biased;
            _ = rx.changed() => {
                // Hold the original session until the targeted cancel finishes;
                // a separate control connection also works with a one-slot pool.
                super::session::cancel_backend(&pool, database, pid, &started).await;
                Err("Query cancelled".into())
            },
            result = execution => result,
        };
        // Never reuse editor sessions, including canceled COPY/transaction SQL.
        let _ = tokio::time::timeout(std::time::Duration::from_secs(5), conn.close()).await;
        result
    }
}
