use super::*;

#[async_trait]
impl QueryDriver for MySqlDriver {
    async fn get_table_data(
        &self,
        target: &TableRef,
        page: u32,
        page_size: u32,
        filters: Option<FilterSet>,
        sort_column: Option<String>,
        sort_desc: Option<bool>,
        exact_count: bool,
        keyset: Option<KeysetPage>,
    ) -> Result<QueryResult, String> {
        let (database, table) = mysql_table_parts(target)?;
        let (where_clause, params) = if let Some(f) = filters {
            query_builder::build_where_clause(&f)
        } else {
            ("".to_string(), vec![])
        };

        let primary_key = self.get_primary_key_columns(target).await?;
        if let Some(cursor) = &keyset {
            if primary_key.len() != 1
                || primary_key[0] != cursor.column
                || sort_column.is_some()
                || (cursor.direction != "next" && cursor.direction != "prev")
            {
                return Err(
                    "Keyset pagination requires the single-column primary key in natural order"
                        .into(),
                );
            }
        }
        let offset = u64::from(page) * u64::from(page_size);
        let keyset_desc_query = keyset
            .as_ref()
            .map(|k| k.direction == "prev")
            .unwrap_or(false);
        let data_where_clause = keyset
            .as_ref()
            .map(|k| append_keyset_predicate(&where_clause, k, keyset_desc_query))
            .unwrap_or_else(|| where_clause.clone());
        let total_timer = std::time::Instant::now();
        let count_timer = std::time::Instant::now();
        let should_use_exact_count = exact_count || !where_clause.is_empty();
        let total_count_is_estimate = !should_use_exact_count;

        let mut total_count = if should_use_exact_count {
            let count_query = format!(
                "SELECT COUNT(*) as total FROM {}.{} {}",
                quote_identifier(database)?,
                quote_identifier(table)?,
                where_clause
            );
            let mut q = sqlx::query_as::<_, (i64,)>(&count_query);
            for p in &params {
                q = q.bind(p);
            }
            let (total_count,) = q
                .fetch_one(&self.pool)
                .await
                .map_err(|e| format!("Failed to fetch total count: {}", e))?;
            total_count
        } else {
            self.get_estimated_row_count(target).await?
        };
        let count_ms = count_timer.elapsed().as_millis() as u64;

        let mut order = Vec::new();
        if let Some(cursor) = &keyset {
            order.push(format!(
                "{} {}",
                quote_identifier(&cursor.column)?,
                if keyset_desc_query { "DESC" } else { "ASC" }
            ));
        } else {
            if let Some(column) = &sort_column {
                order.push(format!(
                    "{} {}",
                    quote_identifier(column)?,
                    if sort_desc.unwrap_or(false) {
                        "DESC"
                    } else {
                        "ASC"
                    }
                ));
            }
            // A unique tie-breaker makes OFFSET pages stable for composite keys.
            for column in &primary_key {
                if sort_column.as_ref() != Some(column) {
                    order.push(format!("{} ASC", quote_identifier(column)?));
                }
            }
        }
        let order_sql = if order.is_empty() {
            String::new()
        } else {
            format!(" ORDER BY {}", order.join(", "))
        };

        let data_query = if keyset.is_some() {
            format!(
                "SELECT * FROM {}.{} {}{} LIMIT {}",
                quote_identifier(database)?,
                quote_identifier(table)?,
                data_where_clause,
                order_sql,
                page_size
            )
        } else {
            format!(
                "SELECT * FROM {}.{} {}{} LIMIT {} OFFSET {}",
                quote_identifier(database)?,
                quote_identifier(table)?,
                data_where_clause,
                order_sql,
                page_size,
                offset
            )
        };
        let mut q = sqlx::query(&data_query);
        for p in &params {
            q = q.bind(p);
        }
        let select_timer = std::time::Instant::now();
        let rows = q
            .fetch_all(&self.pool)
            .await
            .map_err(|e| format!("Failed to fetch data: {}", e))?;
        let select_ms = select_timer.elapsed().as_millis() as u64;

        let (columns, mut result_rows) = rows_to_parsed(rows);
        if keyset_desc_query {
            result_rows.reverse();
        }
        if total_count_is_estimate {
            let visible_minimum = offset as i64 + result_rows.len() as i64;
            total_count = total_count.max(visible_minimum);
        }
        Ok(QueryResult {
            columns,
            rows: result_rows,
            total_count,
            total_count_is_estimate,
            timings: Some(TableDataTimings {
                count_ms,
                select_ms,
                total_ms: total_timer.elapsed().as_millis() as u64,
            }),
        })
    }

    async fn get_all_rows(
        &self,
        target: &TableRef,
    ) -> Result<(Vec<ColumnInfo>, Vec<Value>), String> {
        let (database, table) = mysql_table_parts(target)?;
        let query = format!(
            "SELECT * FROM {}.{}",
            quote_identifier(database)?,
            quote_identifier(table)?
        );
        let mut conn = self.pool.acquire().await.map_err(|e| e.to_string())?;
        if self.no_group_by_check {
            sqlx::query(
                "SET SESSION sql_mode=(SELECT REPLACE(@@SESSION.sql_mode,'ONLY_FULL_GROUP_BY',''))",
            )
            .execute(&mut *conn)
            .await
            .map_err(|e| format!("Failed to set sql_mode: {}", e))?;
        }
        let mut stream = sqlx::query(&query).fetch(&mut *conn);
        let mut columns: Vec<ColumnInfo> = Vec::new();
        let mut result_rows: Vec<Value> = Vec::new();
        while let Some(row_result) = stream.next().await {
            let row = row_result.map_err(|e| format!("Failed to fetch data: {}", e))?;
            if columns.is_empty() {
                for col in row.columns() {
                    columns.push(ColumnInfo {
                        name: col.name().to_string(),
                        type_name: col.type_info().name().to_string(),
                    });
                }
            }
            result_rows.push(parse_mysql_row(&row));
        }
        Ok((columns, result_rows))
    }

    async fn stream_all_rows(
        &self,
        target: &TableRef,
        tx: tokio::sync::mpsc::Sender<(Option<Vec<ColumnInfo>>, Value)>,
    ) -> Result<(), String> {
        let (database, table) = mysql_table_parts(target)?;
        let query = format!(
            "SELECT * FROM {}.{}",
            quote_identifier(database)?,
            quote_identifier(table)?
        );
        let mut conn = self.pool.acquire().await.map_err(|e| e.to_string())?;
        if self.no_group_by_check {
            sqlx::query(
                "SET SESSION sql_mode=(SELECT REPLACE(@@SESSION.sql_mode,'ONLY_FULL_GROUP_BY',''))",
            )
            .execute(&mut *conn)
            .await
            .map_err(|e| format!("Failed to set sql_mode: {}", e))?;
        }
        let mut columns: Vec<ColumnInfo> = Vec::new();
        {
            let mut stream = sqlx::query(&query).fetch(&mut *conn);
            while let Some(row_result) = stream.next().await {
                let row = row_result.map_err(|e| format!("Failed to stream data: {}", e))?;
                let col_opt = if columns.is_empty() {
                    for col in row.columns() {
                        columns.push(ColumnInfo {
                            name: col.name().to_string(),
                            type_name: col.type_info().name().to_string(),
                        });
                    }
                    Some(columns.clone())
                } else {
                    None
                };
                if tx.send((col_opt, parse_mysql_row(&row))).await.is_err() {
                    break; // receiver dropped (export cancelled)
                }
            }
        }
        // Empty tables never populate `columns` above; describe the query so
        // callers still get header metadata (matches the PostgreSQL driver).
        if columns.is_empty() {
            use sqlx::Executor;
            let description = (&mut *conn)
                .describe(&query)
                .await
                .map_err(|e| e.to_string())?;
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
        const CHUNK_SIZE: usize = 500;

        let trimmed = sql.trim().to_uppercase();
        let is_select = trimmed.starts_with("SELECT")
            || trimmed.starts_with("SHOW")
            || trimmed.starts_with("DESCRIBE")
            || trimmed.starts_with("DESC")
            || trimmed.starts_with("EXPLAIN")
            || trimmed.starts_with("WITH");

        let mut conn = self.pool.acquire().await.map_err(|e| e.to_string())?;

        use sqlx::Executor;

        // Register the MySQL thread id so cancel_query can KILL it.
        if let Some(qid) = query_id {
            let thread_id: u64 = sqlx::query_scalar("SELECT CONNECTION_ID()")
                .fetch_one(&mut *conn)
                .await
                .map_err(|e| e.to_string())?;
            self.running_queries
                .write()
                .insert(qid.to_string(), thread_id);
        }

        let result = async {
            // USE and SHOW/DESCRIBE require the simple query protocol (not prepared).
            // Passing &str (not a prepared query) uses the simple protocol.
            if let Some(db) = database {
                if !db.is_empty() {
                    let use_stmt = format!("USE `{}`", db);
                    conn.execute(use_stmt.as_str())
                        .await
                        .map_err(|e| e.to_string())?;
                }
            }

            if is_select {
                let mut stream = conn.fetch(sql);
                let mut columns: Vec<ColumnInfo> = Vec::new();
                let mut row_count: u64 = 0;

                if on_chunk.is_some() {
                    // ── Streaming mode: flush rows in chunks ──────────────────────
                    let mut chunk_buf: Vec<Value> = Vec::with_capacity(CHUNK_SIZE);
                    let mut first_chunk = true;

                    while let Some(row_result) = stream.next().await {
                        let row = row_result.map_err(|e| format!("Query error: {}", e))?;
                        if columns.is_empty() {
                            for col in row.columns() {
                                columns.push(ColumnInfo {
                                    name: col.name().to_string(),
                                    type_name: col.type_info().name().to_string(),
                                });
                            }
                        }
                        if (row_count as usize)
                            < retained_row_limit(columns.len(), max_retained_cells)
                        {
                            chunk_buf.push(parse_mysql_row(&row));
                        }
                        row_count += 1;

                        if chunk_buf.len() >= CHUNK_SIZE {
                            if let Some(ref cb) = on_chunk {
                                let cols = if first_chunk {
                                    Some(columns.clone())
                                } else {
                                    None
                                };
                                cb(
                                    cols,
                                    std::mem::replace(
                                        &mut chunk_buf,
                                        Vec::with_capacity(CHUNK_SIZE),
                                    ),
                                );
                                first_chunk = false;
                            }
                            if let Some(ref cb) = on_progress {
                                cb(row_count);
                            }
                        } else if row_count.is_multiple_of(1000) {
                            if let Some(ref cb) = on_progress {
                                cb(row_count);
                            }
                        }
                    }
                    // Flush remaining rows
                    if !chunk_buf.is_empty() {
                        if let Some(ref cb) = on_chunk {
                            let cols = if first_chunk {
                                Some(columns.clone())
                            } else {
                                None
                            };
                            cb(cols, chunk_buf);
                        }
                    }
                    if let Some(ref cb) = on_progress {
                        cb(row_count);
                    }
                    Ok(RawQueryResult {
                        columns,
                        rows: vec![], // rows were streamed via on_chunk
                        rows_affected: row_count,
                        is_select: true,
                    })
                } else {
                    // ── Buffered mode (legacy): accumulate all rows ───────────────
                    let mut result_rows: Vec<Value> = Vec::new();
                    while let Some(row_result) = stream.next().await {
                        let row = row_result.map_err(|e| format!("Query error: {}", e))?;
                        if columns.is_empty() {
                            for col in row.columns() {
                                columns.push(ColumnInfo {
                                    name: col.name().to_string(),
                                    type_name: col.type_info().name().to_string(),
                                });
                            }
                        }
                        result_rows.push(parse_mysql_row(&row));
                        row_count += 1;
                        if let Some(ref cb) = on_progress {
                            if row_count.is_multiple_of(1000) {
                                cb(row_count);
                            }
                        }
                    }
                    Ok(RawQueryResult {
                        columns,
                        rows: result_rows,
                        rows_affected: row_count,
                        is_select: true,
                    })
                }
            } else {
                let result: sqlx::mysql::MySqlQueryResult = conn
                    .execute(sql)
                    .await
                    .map_err(|e| format!("Query error: {}", e))?;
                Ok(RawQueryResult {
                    columns: vec![],
                    rows: vec![],
                    rows_affected: result.rows_affected(),
                    is_select: false,
                })
            }
        }
        .await;

        if let Some(qid) = query_id {
            self.running_queries.write().remove(qid);
        }

        result
    }
}
