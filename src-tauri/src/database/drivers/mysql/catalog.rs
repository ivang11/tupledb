use super::*;

#[async_trait]
impl CatalogDriver for MySqlDriver {
    async fn get_databases(&self) -> Result<Vec<String>, String> {
        let query = "SELECT schema_name FROM information_schema.schemata ORDER BY schema_name ASC";
        let rows = match sqlx::query(query).fetch_all(&self.pool).await {
            Ok(rows) => rows,
            Err(first_error) if is_early_connection_close(&first_error) => {
                println!(
                    "  -> MySQL closed connection while fetching databases, retrying once: {}",
                    first_error
                );
                sqlx::query(query)
                    .fetch_all(&self.pool)
                    .await
                    .map_err(|e| format!("Failed to fetch databases: {}", e))?
            }
            Err(e) => return Err(format!("Failed to fetch databases: {}", e)),
        };
        Ok(rows.iter().map(|row| get_str_lossy(row, 0)).collect())
    }

    async fn get_database_creation_options(&self) -> Result<DatabaseCreationOptions, String> {
        let defaults = sqlx::query("SELECT @@character_set_server, @@collation_server")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| format!("Failed to fetch database defaults: {}", e))?;
        let collations = match sqlx::query(
            "SELECT COLLATION_NAME, CHARACTER_SET_NAME, IS_DEFAULT FROM information_schema.COLLATIONS ORDER BY CHARACTER_SET_NAME, COLLATION_NAME",
        )
        .fetch_all(&self.pool)
        .await
        {
            Ok(rows) => rows
                .iter()
                .map(|row| DatabaseCollation {
                    name: get_str_lossy(row, 0),
                    character_set: get_str_lossy(row, 1),
                    is_default: get_str_lossy(row, 2).eq_ignore_ascii_case("yes"),
                })
                .collect(),
            Err(information_schema_error) => {
                // Some managed MySQL-compatible services restrict information_schema
                // while still exposing the equivalent SHOW command.
                let rows = sqlx::query("SHOW COLLATION")
                    .fetch_all(&self.pool)
                    .await
                    .map_err(|show_error| {
                        format!(
                            "Failed to fetch available collations (information_schema: {}; SHOW COLLATION: {})",
                            information_schema_error, show_error
                        )
                    })?;
                rows.iter()
                    .map(|row| DatabaseCollation {
                        name: get_str_lossy(row, 0),
                        character_set: get_str_lossy(row, 1),
                        is_default: get_str_lossy(row, 3).eq_ignore_ascii_case("yes"),
                    })
                    .collect()
            }
        };

        Ok(DatabaseCreationOptions {
            default_character_set: get_str_lossy(&defaults, 0),
            default_collation: get_str_lossy(&defaults, 1),
            collations,
        })
    }

    async fn create_database(
        &self,
        name: &str,
        character_set: Option<&str>,
        collation: Option<&str>,
    ) -> Result<(), String> {
        let mut query = format!("CREATE DATABASE {}", quote_identifier(name)?);

        if character_set.is_some() || collation.is_some() {
            let options = self.get_database_creation_options().await?;
            let selected_collation = collation.and_then(|value| {
                options
                    .collations
                    .iter()
                    .find(|option| option.name == value)
            });

            if let Some(value) = character_set {
                let is_valid = options
                    .collations
                    .iter()
                    .any(|option| option.character_set == value);
                if !is_valid {
                    return Err(format!("Unsupported character set: {}", value));
                }
                query.push_str(&format!(" CHARACTER SET {}", value));
            }

            if let Some(value) = collation {
                let option = selected_collation
                    .ok_or_else(|| format!("Unsupported collation: {}", value))?;
                if let Some(selected_character_set) = character_set {
                    if option.character_set != selected_character_set {
                        return Err(format!(
                            "Collation {} does not belong to character set {}",
                            value, selected_character_set
                        ));
                    }
                }
                query.push_str(&format!(" COLLATE {}", value));
            }
        }

        sqlx::query(&query)
            .execute(&self.pool)
            .await
            .map_err(|e| format!("Failed to create database: {}", e))?;
        Ok(())
    }

    async fn drop_database(&self, name: &str) -> Result<(), String> {
        sqlx::query(&format!("DROP DATABASE {}", quote_identifier(name)?))
            .execute(&self.pool)
            .await
            .map_err(|e| format!("Failed to drop database: {}", e))?;
        Ok(())
    }

    async fn get_tables(&self, database: &str) -> Result<Vec<Table>, String> {
        let query = format!("SHOW FULL TABLES FROM {}", quote_identifier(database)?);
        let rows = sqlx::query(&query)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| {
                println!("  -> Error fetching tables: {}", e);
                format!("Failed to fetch tables: {}", e)
            })?;
        let mut tables: Vec<Table> = rows
            .iter()
            .map(|row| Table {
                reference: TableRef::new(database, &get_str_lossy(row, 0)),
                name: get_str_lossy(row, 0),
                table_type: get_str_lossy(row, 1),
            })
            .collect();
        tables.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        Ok(tables)
    }

    async fn get_table_structure(&self, target: &TableRef) -> Result<Vec<ColumnStructure>, String> {
        let (database, table) = mysql_table_parts(target)?;
        let rows = sqlx::query(
            "SELECT COLUMN_NAME, COLUMN_TYPE, IS_NULLABLE, COLUMN_KEY, COLUMN_DEFAULT, EXTRA
             FROM information_schema.COLUMNS WHERE TABLE_SCHEMA = ? AND TABLE_NAME = ? ORDER BY ORDINAL_POSITION")
            .bind(database).bind(table).fetch_all(&self.pool).await.map_err(|e| e.to_string())?;
        let primary_key = self.get_primary_key_columns(target).await?;
        Ok(rows
            .iter()
            .map(|row| {
                let field = get_str_lossy(row, 0);
                let field_type = get_str_lossy(row, 1);
                let extra = get_str_lossy(row, 5);
                ColumnStructure {
                    primary_key_position: primary_key
                        .iter()
                        .position(|name| *name == field)
                        .map(|i| i + 1),
                    value_kind: mysql_value_kind(&field_type),
                    is_identity: extra.to_ascii_uppercase().contains("AUTO_INCREMENT"),
                    is_generated: extra.to_ascii_uppercase().contains("VIRTUAL GENERATED")
                        || extra.to_ascii_uppercase().contains("STORED GENERATED"),
                    field,
                    field_type,
                    nullable: get_str_lossy(row, 2) == "YES",
                    key: get_str_lossy(row, 3),
                    default_value: get_optional_str_lossy(row, 4),
                    extra,
                }
            })
            .collect())
    }

    async fn get_table_ddl(&self, target: &TableRef) -> Result<String, String> {
        let (database, table) = mysql_table_parts(target)?;
        let query = format!(
            "SHOW CREATE TABLE {}.{}",
            quote_identifier(database)?,
            quote_identifier(table)?
        );
        let row = sqlx::query(&query)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| format!("Failed to get DDL for {}: {}", table, e))?;
        Ok(get_str_lossy(&row, 1))
    }

    async fn get_foreign_keys(&self, target: &TableRef) -> Result<Vec<ForeignKey>, String> {
        let (database, table) = mysql_table_parts(target)?;
        let query = format!(
            "SELECT COLUMN_NAME, REFERENCED_TABLE_NAME, REFERENCED_COLUMN_NAME, CONSTRAINT_NAME, ORDINAL_POSITION, REFERENCED_TABLE_SCHEMA \
             FROM INFORMATION_SCHEMA.KEY_COLUMN_USAGE \
             WHERE TABLE_SCHEMA = '{}' AND TABLE_NAME = '{}' AND REFERENCED_TABLE_NAME IS NOT NULL ORDER BY CONSTRAINT_NAME, ORDINAL_POSITION",
            database.replace('\'', "\\'"),
            table.replace('\'', "\\'")
        );
        let rows = sqlx::query(&query)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| format!("Failed to fetch foreign keys: {}", e))?;
        Ok(rows
            .iter()
            .map(|row| ForeignKey {
                constraint_name: get_str_lossy(row, 3),
                position: get_str_lossy(row, 4).parse().unwrap_or(1),
                referenced: TableRef::new(&get_str_lossy(row, 5), &get_str_lossy(row, 1)),
                column: get_str_lossy(row, 0),
                referenced_table: get_str_lossy(row, 1),
                referenced_column: get_str_lossy(row, 2),
            })
            .collect())
    }

    async fn get_table_indexes(&self, target: &TableRef) -> Result<Vec<TableIndex>, String> {
        let (database, table) = mysql_table_parts(target)?;
        let query = format!(
            "SHOW INDEX FROM {}.{}",
            quote_identifier(database)?,
            quote_identifier(table)?
        );
        let rows = sqlx::query(&query)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| format!("Failed to fetch indexes: {}", e))?;
        Ok(rows
            .iter()
            .map(|row| {
                let non_unique: bool = row
                    .try_get::<i64, _>(1)
                    .or_else(|_| row.try_get::<u64, _>(1).map(|v| v as i64))
                    .map(|v| v != 0)
                    .unwrap_or_else(|_| get_str_lossy(row, 1) != "0");
                let seq: u64 = get_str_lossy(row, 3).parse().unwrap_or(1);
                TableIndex {
                    key_name: get_str_lossy(row, 2),
                    non_unique,
                    column_name: get_str_lossy(row, 4),
                    seq_in_index: seq,
                    index_type: get_str_lossy(row, 10),
                    nullable: get_str_lossy(row, 9) == "YES",
                    comment: get_str_lossy(row, 11),
                }
            })
            .collect())
    }

    async fn get_primary_key_columns(&self, target: &TableRef) -> Result<Vec<String>, String> {
        let (database, table) = mysql_table_parts(target)?;
        let query = format!(
            "SELECT column_name FROM information_schema.statistics \
             WHERE table_schema='{}' AND table_name='{}' AND index_name='PRIMARY' \
             ORDER BY seq_in_index ASC",
            database.replace('\'', "\\'"),
            table.replace('\'', "\\'")
        );
        let rows = sqlx::query(&query)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| format!("Failed to fetch primary key: {}", e))?;
        Ok(rows.iter().map(|r| get_str_lossy(r, 0)).collect())
    }

    async fn get_estimated_row_count(&self, target: &TableRef) -> Result<i64, String> {
        let (database, table) = mysql_table_parts(target)?;
        let query = format!(
            "SELECT table_rows FROM information_schema.TABLES \
             WHERE TABLE_SCHEMA='{}' AND TABLE_NAME='{}'",
            database.replace('\'', "\\'"),
            table.replace('\'', "\\'")
        );
        let row = sqlx::query(&query)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| format!("Failed to fetch estimated count: {}", e))?;
        Ok(row
            .map(|r| get_str_lossy(&r, 0).parse().unwrap_or(0))
            .unwrap_or(0))
    }

    // --- Data ---
}

fn mysql_value_kind(sql_type: &str) -> ValueKind {
    let t = sql_type.to_ascii_lowercase();
    if t == "tinyint(1)" || t == "boolean" || t == "bool" {
        ValueKind::Boolean
    } else if t.contains("int") {
        ValueKind::Integer
    } else if t.starts_with("decimal") || t.starts_with("numeric") {
        ValueKind::Decimal
    } else if t.starts_with("float") || t.starts_with("double") {
        ValueKind::Float
    } else if t == "json" {
        ValueKind::Json
    } else if t.contains("blob") || t.contains("binary") || t.starts_with("bit") {
        ValueKind::Binary
    } else if t.starts_with("date") || t.starts_with("time") || t == "year" {
        ValueKind::DateTime
    } else if t.contains("char")
        || t.contains("text")
        || t.starts_with("enum")
        || t.starts_with("set")
    {
        ValueKind::Text
    } else {
        ValueKind::Other
    }
}
