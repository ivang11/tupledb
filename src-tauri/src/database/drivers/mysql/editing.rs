use super::*;

#[async_trait]
impl EditDriver for MySqlDriver {
    // --- Mutations ---

    async fn apply_table_changes(
        &self,
        target: &TableRef,
        updates: Vec<RowChange>,
        deletions: Vec<RowDeletion>,
        disable_fk_checks: bool,
    ) -> Result<(), String> {
        let (database, table) = mysql_table_parts(target)?;
        let pk = self.get_primary_key_columns(target).await?;
        let structure = self.get_table_structure(target).await?;
        for update in &updates {
            validate_row_key(&update.key, &pk)?;
            let mut seen = std::collections::HashSet::new();
            for change in &update.changes {
                if !seen.insert(&change.column) {
                    return Err("Duplicate changed column".into());
                }
                let column = structure
                    .iter()
                    .find(|c| c.field == change.column)
                    .ok_or("Unknown changed column")?;
                if column.is_generated {
                    return Err("Generated columns cannot be edited".into());
                }
                validate_scalar(&change.value)?;
            }
        }
        for deletion in &deletions {
            validate_row_key(&deletion.key, &pk)?;
        }
        let relation = format!(
            "{}.{}",
            quote_identifier(database)?,
            quote_identifier(table)?
        );
        let mut conn = self.pool.acquire().await.map_err(|e| e.to_string())?;
        // Session settings must never leak into the pool on errors or cancellation.
        if disable_fk_checks {
            conn.close_on_drop();
        }
        use sqlx::Acquire;
        let mut tx = conn.begin().await.map_err(|e| e.to_string())?;
        if disable_fk_checks {
            sqlx::query("SET FOREIGN_KEY_CHECKS = 0")
                .execute(&mut *tx)
                .await
                .map_err(|e| e.to_string())?;
        }
        for update in updates {
            if update.changes.is_empty() {
                continue;
            }
            let mut query = sqlx::QueryBuilder::<MySql>::new(format!("UPDATE {} SET ", relation));
            for (i, change) in update.changes.iter().enumerate() {
                if i > 0 {
                    query.push(", ");
                }
                query.push(quote_identifier(&change.column)?).push(" = ");
                bind_scalar(&mut query, &change.value)?;
            }
            append_row_key(&mut query, &update.key)?;
            query
                .build()
                .execute(&mut *tx)
                .await
                .map_err(|e| e.to_string())?;
        }
        for deletion in deletions {
            let mut query = sqlx::QueryBuilder::<MySql>::new(format!("DELETE FROM {}", relation));
            append_row_key(&mut query, &deletion.key)?;
            query
                .build()
                .execute(&mut *tx)
                .await
                .map_err(|e| e.to_string())?;
        }
        tx.commit().await.map_err(|e| e.to_string())
    }

    async fn insert_row(
        &self,
        target: &TableRef,
        values: Vec<TableChange>,
        disable_fk_checks: bool,
    ) -> Result<(), String> {
        let (database, table) = mysql_table_parts(target)?;
        let structure = self.get_table_structure(target).await?;
        let mut seen = std::collections::HashSet::new();
        for value in &values {
            if !seen.insert(&value.column) {
                return Err("Duplicate inserted column".into());
            }
            let column = structure
                .iter()
                .find(|c| c.field == value.column)
                .ok_or("Unknown inserted column")?;
            if column.is_generated {
                return Err("Generated columns cannot be inserted".into());
            }
            validate_scalar(&value.value)?;
        }
        let relation = format!(
            "{}.{}",
            quote_identifier(database)?,
            quote_identifier(table)?
        );
        let mut query = sqlx::QueryBuilder::<MySql>::new(format!("INSERT INTO {} (", relation));
        for (i, value) in values.iter().enumerate() {
            if i > 0 {
                query.push(", ");
            }
            query.push(quote_identifier(&value.column)?);
        }
        query.push(") VALUES (");
        for (i, value) in values.iter().enumerate() {
            if i > 0 {
                query.push(", ");
            }
            // Text such as NOW() or NULL is data, never an implicit expression.
            bind_scalar(&mut query, &value.value)?;
        }
        query.push(")");
        let mut conn = self.pool.acquire().await.map_err(|e| e.to_string())?;
        if disable_fk_checks {
            conn.close_on_drop();
        }
        use sqlx::Acquire;
        let mut tx = conn.begin().await.map_err(|e| e.to_string())?;
        if disable_fk_checks {
            sqlx::query("SET FOREIGN_KEY_CHECKS = 0")
                .execute(&mut *tx)
                .await
                .map_err(|e| e.to_string())?;
        }
        query
            .build()
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
        tx.commit().await.map_err(|e| e.to_string())
    }

    async fn alter_table_column(
        &self,
        target: &TableRef,
        old_name: &str,
        new_name: &str,
        new_type: &str,
    ) -> Result<String, String> {
        let (database, table) = mysql_table_parts(target)?;
        let database_sql = quote_identifier(database)?;
        let table_sql = quote_identifier(table)?;
        let old_name_sql = quote_identifier(old_name)?;
        let new_name_sql = quote_identifier(new_name)?;
        let new_type = validate_column_type(new_type)?;

        let metadata = sqlx::query(
            "SELECT COLUMN_TYPE, IS_NULLABLE, COLUMN_DEFAULT, EXTRA, COLUMN_COMMENT, \
                    CHARACTER_SET_NAME, COLLATION_NAME, GENERATION_EXPRESSION \
             FROM information_schema.COLUMNS \
             WHERE TABLE_SCHEMA = ? AND TABLE_NAME = ? AND COLUMN_NAME = ?",
        )
        .bind(database)
        .bind(table)
        .bind(old_name)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| format!("Failed to read column definition: {}", e))?
        .ok_or_else(|| format!("Column '{}' no longer exists", old_name))?;

        let current_type = get_optional_str_lossy(&metadata, 0)
            .ok_or_else(|| "Failed to read column type from information_schema".to_string())?;
        let name_changed = old_name.trim() != new_name.trim();
        let type_changed =
            current_type.trim().to_ascii_lowercase() != new_type.trim().to_ascii_lowercase();

        if !name_changed && !type_changed {
            return Ok(String::new());
        }

        // RENAME COLUMN is the safest path when the type is unchanged: MySQL
        // retains every attribute without us having to rebuild the definition.
        let sql = if !type_changed {
            format!(
                "ALTER TABLE {}.{} RENAME COLUMN {} TO {}",
                database_sql, table_sql, old_name_sql, new_name_sql
            )
        } else {
            let nullable = get_optional_str_lossy(&metadata, 1).ok_or_else(|| {
                "Failed to read column nullability from information_schema".to_string()
            })?;
            let default_value = get_optional_str_lossy(&metadata, 2);
            let extra = get_optional_str_lossy(&metadata, 3).unwrap_or_default();
            let comment = get_optional_str_lossy(&metadata, 4).unwrap_or_default();
            let character_set = get_optional_str_lossy(&metadata, 5);
            let collation = get_optional_str_lossy(&metadata, 6);
            let generation = get_optional_str_lossy(&metadata, 7).unwrap_or_default();
            let extra_upper = extra.to_ascii_uppercase();

            let mut definition = new_type;
            if column_type_supports_charset(&definition) {
                if let Some(charset) = character_set {
                    definition.push_str(" CHARACTER SET ");
                    definition.push_str(&quote_identifier(&charset)?);
                }
                if let Some(collation) = collation {
                    definition.push_str(" COLLATE ");
                    definition.push_str(&quote_identifier(&collation)?);
                }
            }

            if !generation.trim().is_empty() {
                definition.push_str(" GENERATED ALWAYS AS (");
                definition.push_str(&generation);
                definition.push(')');
                if extra_upper.contains("STORED GENERATED") {
                    definition.push_str(" STORED");
                } else {
                    definition.push_str(" VIRTUAL");
                }
            } else {
                definition.push_str(if nullable == "YES" {
                    " NULL"
                } else {
                    " NOT NULL"
                });

                if let Some(default) = default_value {
                    let upper = default.trim().to_ascii_uppercase();
                    let temporal_expression = upper == "CURRENT_TIMESTAMP"
                        || upper.starts_with("CURRENT_TIMESTAMP(")
                        || upper == "CURRENT_DATE"
                        || upper.starts_with("CURRENT_DATE(")
                        || upper == "CURRENT_TIME"
                        || upper.starts_with("CURRENT_TIME(");
                    definition.push_str(" DEFAULT ");
                    if temporal_expression {
                        definition.push_str(default.trim());
                    } else if extra_upper.contains("DEFAULT_GENERATED") {
                        if default.trim().starts_with('(') {
                            definition.push_str(default.trim());
                        } else {
                            definition.push('(');
                            definition.push_str(default.trim());
                            definition.push(')');
                        }
                    } else {
                        definition.push_str(&quote_mysql_string(&default));
                    }
                } else if nullable == "YES" && !extra_upper.contains("AUTO_INCREMENT") {
                    definition.push_str(" DEFAULT NULL");
                }

                if extra_upper.contains("AUTO_INCREMENT") {
                    definition.push_str(" AUTO_INCREMENT");
                }
                if let Some(on_update) = extra_upper.find("ON UPDATE ") {
                    definition.push(' ');
                    definition.push_str(&extra[on_update..]);
                }
                if extra_upper.contains("INVISIBLE") {
                    definition.push_str(" INVISIBLE");
                }
            }

            if !comment.is_empty() {
                definition.push_str(" COMMENT ");
                definition.push_str(&quote_mysql_string(&comment));
            }

            format!(
                "ALTER TABLE {}.{} CHANGE COLUMN {} {} {}",
                database_sql, table_sql, old_name_sql, new_name_sql, definition
            )
        };

        sqlx::query(&sql)
            .execute(&self.pool)
            .await
            .map_err(|e| format!("Failed to alter column: {}", e))?;
        Ok(sql)
    }

    async fn drop_table(&self, target: &TableRef, disable_fk_checks: bool) -> Result<(), String> {
        let (database, table) = mysql_table_parts(target)?;
        let mut conn = self.pool.acquire().await.map_err(|e| e.to_string())?;
        if disable_fk_checks {
            sqlx::query("SET FOREIGN_KEY_CHECKS = 0")
                .execute(&mut *conn)
                .await
                .map_err(|e| e.to_string())?;
        }
        let res = sqlx::query(&format!(
            "DROP TABLE {}.{}",
            quote_identifier(database)?,
            quote_identifier(table)?
        ))
        .execute(&mut *conn)
        .await;
        if disable_fk_checks {
            let _ = sqlx::query("SET FOREIGN_KEY_CHECKS = 1")
                .execute(&mut *conn)
                .await;
        }
        res.map(|_| ()).map_err(|e| e.to_string())
    }

    async fn drop_tables(
        &self,
        database: &str,
        tables: &[TableRef],
        disable_fk_checks: bool,
    ) -> Result<(), String> {
        // Validate the entire selection before any destructive operation.
        for target in tables {
            mysql_table_parts(target)?;
            if target.catalog != database {
                return Err("Mismatched catalog".into());
            }
        }
        for target in tables {
            self.drop_table(target, disable_fk_checks).await?;
        }
        Ok(())
    }

    async fn truncate_table(
        &self,
        target: &TableRef,
        disable_fk_checks: bool,
    ) -> Result<(), String> {
        let (database, table) = mysql_table_parts(target)?;
        let mut conn = self.pool.acquire().await.map_err(|e| e.to_string())?;
        if disable_fk_checks {
            sqlx::query("SET FOREIGN_KEY_CHECKS = 0")
                .execute(&mut *conn)
                .await
                .map_err(|e| e.to_string())?;
            let res = sqlx::query(&format!(
                "DELETE FROM {}.{}",
                quote_identifier(database)?,
                quote_identifier(table)?
            ))
            .execute(&mut *conn)
            .await;
            // Simulate TRUNCATE by resetting auto-increment
            let _ = sqlx::query(&format!(
                "ALTER TABLE {}.{} AUTO_INCREMENT = 1",
                quote_identifier(database)?,
                quote_identifier(table)?
            ))
            .execute(&mut *conn)
            .await;
            let _ = sqlx::query("SET FOREIGN_KEY_CHECKS = 1")
                .execute(&mut *conn)
                .await;
            res.map(|_| ()).map_err(|e| e.to_string())
        } else {
            sqlx::query(&format!(
                "TRUNCATE TABLE {}.{}",
                quote_identifier(database)?,
                quote_identifier(table)?
            ))
            .execute(&mut *conn)
            .await
            .map(|_| ())
            .map_err(|e| e.to_string())
        }
    }

    // --- Import ---
}

fn validate_scalar(value: &Value) -> Result<(), String> {
    if value.is_array() || value.is_object() {
        Err("Expected a scalar value; encode JSON as text".into())
    } else {
        Ok(())
    }
}

fn validate_row_key(key: &[TableChange], primary_key: &[String]) -> Result<(), String> {
    let names: std::collections::HashSet<_> = key.iter().map(|part| &part.column).collect();
    if primary_key.is_empty()
        || key.len() != primary_key.len()
        || names.len() != key.len()
        || primary_key.iter().any(|name| !names.contains(name))
    {
        return Err("A complete, unique primary key is required to edit a row".into());
    }
    for part in key {
        if part.value.is_null() {
            return Err("Primary key values cannot be null".into());
        }
        validate_scalar(&part.value)?;
    }
    Ok(())
}

fn bind_scalar(query: &mut sqlx::QueryBuilder<'_, MySql>, value: &Value) -> Result<(), String> {
    match value {
        Value::Null => {
            query.push_bind(None::<String>);
        }
        Value::Bool(v) => {
            query.push_bind(*v);
        }
        Value::Number(v) => {
            if let Some(v) = v.as_i64() {
                query.push_bind(v);
            } else if let Some(v) = v.as_u64() {
                query.push_bind(v);
            } else {
                query.push_bind(v.as_f64().ok_or("Invalid number")?);
            }
        }
        Value::String(v) => {
            query.push_bind(v.clone());
        }
        _ => return Err("Unsupported value type".into()),
    };
    Ok(())
}

fn append_row_key(
    query: &mut sqlx::QueryBuilder<'_, MySql>,
    key: &[TableChange],
) -> Result<(), String> {
    query.push(" WHERE ");
    for (i, part) in key.iter().enumerate() {
        if i > 0 {
            query.push(" AND ");
        }
        query.push(quote_identifier(&part.column)?).push(" = ");
        bind_scalar(query, &part.value)?;
    }
    Ok(())
}
