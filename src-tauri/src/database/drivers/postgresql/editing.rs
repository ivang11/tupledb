use super::queries::{bind_value, column};
use super::*;
use sqlx::{Column, Executor, Postgres, QueryBuilder};
use std::collections::HashSet;

fn key_predicate(
    query: &mut QueryBuilder<'_, Postgres>,
    key: &[TableChange],
    columns: &[PgColumn],
) -> Result<(), String> {
    let pk: HashSet<_> = columns
        .iter()
        .filter(|c| c.primary_key_position.is_some())
        .map(|c| c.field.as_str())
        .collect();
    let provided: HashSet<_> = key.iter().map(|k| k.column.as_str()).collect();
    if pk.is_empty()
        || pk != provided
        || provided.len() != key.len()
        || key.iter().any(|k| k.value.is_null())
    {
        return Err("A complete, unique primary key is required to edit a row".into());
    }
    query.push(" WHERE ");
    for (i, part) in key.iter().enumerate() {
        if i > 0 {
            query.push(" AND ");
        }
        query.push(sql::quote_identifier(&part.column)?).push(" = ");
        bind_value(query, &part.value, column(columns, &part.column)?)?;
    }
    Ok(())
}

fn editable_column<'a>(columns: &'a [PgColumn], name: &str) -> Result<&'a PgColumn, String> {
    let col = column(columns, name)?;
    if col.is_generated || col.is_identity {
        return Err("Generated and identity columns cannot be written by the row editor".into());
    }
    Ok(col)
}

#[async_trait]
impl EditDriver for PostgreSqlDriver {
    async fn apply_table_changes(
        &self,
        table: &TableRef,
        updates: Vec<RowChange>,
        deletions: Vec<RowDeletion>,
        disable_fk_checks: bool,
    ) -> Result<(), String> {
        self.check_write(disable_fk_checks)?;
        let pool = self.pool_for(&table.catalog).await?;
        let relation = self.relation(table)?;
        let columns = self.column_metadata(table).await?;
        let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
        for update in updates {
            let mut query = QueryBuilder::new(format!("UPDATE {relation} SET "));
            let mut names = HashSet::new();
            for (i, change) in update.changes.iter().enumerate() {
                if !names.insert(&change.column) {
                    return Err("Duplicate changed column".into());
                }
                let col = editable_column(&columns, &change.column)?;
                if i > 0 {
                    query.push(", ");
                }
                query
                    .push(sql::quote_identifier(&change.column)?)
                    .push(" = ");
                bind_value(&mut query, &change.value, col)?;
            }
            key_predicate(&mut query, &update.key, &columns)?;
            if update.changes.is_empty() {
                continue;
            }
            let result = query
                .build()
                .execute(&mut *tx)
                .await
                .map_err(|e| e.to_string())?;
            if result.rows_affected() != 1 {
                return Err(
                    "The row changed or is no longer visible; refresh before editing".into(),
                );
            }
        }
        for deletion in deletions {
            let mut query = QueryBuilder::new(format!("DELETE FROM {relation}"));
            key_predicate(&mut query, &deletion.key, &columns)?;
            let result = query
                .build()
                .execute(&mut *tx)
                .await
                .map_err(|e| e.to_string())?;
            if result.rows_affected() != 1 {
                return Err(
                    "The row changed or is no longer visible; refresh before deleting".into(),
                );
            }
        }
        tx.commit().await.map_err(|e| e.to_string())
    }

    async fn insert_row(
        &self,
        table: &TableRef,
        values: Vec<TableChange>,
        disable_fk_checks: bool,
    ) -> Result<(), String> {
        self.check_write(disable_fk_checks)?;
        let pool = self.pool_for(&table.catalog).await?;
        let relation = self.relation(table)?;
        let columns = self.column_metadata(table).await?;
        let mut query = QueryBuilder::new(format!("INSERT INTO {relation}"));
        if values.is_empty() {
            query.push(" DEFAULT VALUES");
        } else {
            let mut names = HashSet::new();
            query.push(" (");
            for (i, value) in values.iter().enumerate() {
                editable_column(&columns, &value.column)?;
                if !names.insert(&value.column) {
                    return Err("Duplicate inserted column".into());
                }
                if i > 0 {
                    query.push(", ");
                }
                query.push(sql::quote_identifier(&value.column)?);
            }
            query.push(") VALUES (");
            for (i, value) in values.iter().enumerate() {
                if i > 0 {
                    query.push(", ");
                }
                bind_value(&mut query, &value.value, column(&columns, &value.column)?)?;
            }
            query.push(")");
        }
        query
            .build()
            .execute(&pool)
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    async fn alter_table_column(
        &self,
        table: &TableRef,
        old_name: &str,
        new_name: &str,
        new_type: &str,
    ) -> Result<String, String> {
        self.check_write(false)?;
        let relation = self.relation(table)?;
        let old = sql::quote_identifier(old_name)?;
        let new = sql::quote_identifier(new_name)?;
        let ty = super::type_name::validate(new_type)?;
        let pool = self.pool_for(&table.catalog).await?;
        let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
        sqlx::query("SET LOCAL lock_timeout = '10s'")
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
        sqlx::query(&format!("LOCK TABLE {relation} IN ACCESS EXCLUSIVE MODE"))
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
        let (current_type, collation): (String, Option<String>) = sqlx::query_as(
            "SELECT format_type(atttypid,atttypmod), CASE WHEN attcollation<>0 THEN attcollation::regcollation::text END FROM pg_attribute WHERE attrelid=to_regclass($1) AND attname=$2 AND attnum>0 AND NOT attisdropped"
        ).bind(&relation).bind(old_name).fetch_one(&mut *tx).await.map_err(|e| e.to_string())?;
        let mut statements = Vec::new();
        if ty != current_type {
            // Resolve the validated type without executing user expressions.
            let description = (&mut *tx)
                .describe(&format!("SELECT NULL::{ty}"))
                .await
                .map_err(|e| e.to_string())?;
            let oid = description.columns()[0]
                .type_info()
                .oid()
                .ok_or("Cannot resolve PostgreSQL type")?;
            let collatable: bool =
                sqlx::query_scalar("SELECT typcollation<>0 FROM pg_type WHERE oid=$1")
                    .bind(oid)
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(|e| e.to_string())?;
            let collate = if collatable {
                collation
                    .map(|c| format!(" COLLATE {c}"))
                    .unwrap_or_default()
            } else {
                String::new()
            };
            let statement = format!("ALTER TABLE {relation} ALTER COLUMN {old} TYPE {ty}{collate}");
            sqlx::query(&statement).execute(&mut *tx).await.map_err(|e| format!("Cannot change column type: {e}. For conversions requiring USING, use the SQL editor."))?;
            statements.push(statement);
        }
        if old_name != new_name {
            let statement = format!("ALTER TABLE {relation} RENAME COLUMN {old} TO {new}");
            sqlx::query(&statement)
                .execute(&mut *tx)
                .await
                .map_err(|e| e.to_string())?;
            statements.push(statement);
        }
        tx.commit().await.map_err(|e| e.to_string())?;
        Ok(statements.join(";\n"))
    }

    async fn drop_table(&self, table: &TableRef, disable_fk_checks: bool) -> Result<(), String> {
        self.check_write(disable_fk_checks)?;
        let pool = self.pool_for(&table.catalog).await?;
        sqlx::query(&format!("DROP TABLE {}", self.relation(table)?))
            .execute(&pool)
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    async fn drop_tables(
        &self,
        database: &str,
        tables: &[TableRef],
        disable_fk_checks: bool,
    ) -> Result<(), String> {
        self.check_write(disable_fk_checks)?;
        self.check_catalog(database)?;
        if tables.iter().any(|table| table.catalog != database) {
            return Err("Mismatched catalog".into());
        }
        let pool = self.pool_for(database).await?;
        let relations = tables
            .iter()
            .map(|t| self.relation(t))
            .collect::<Result<Vec<_>, _>>()?;
        if relations.is_empty() {
            return Ok(());
        }
        sqlx::query(&format!("DROP TABLE {}", relations.join(", ")))
            .execute(&pool)
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    async fn truncate_table(
        &self,
        table: &TableRef,
        disable_fk_checks: bool,
    ) -> Result<(), String> {
        self.check_write(disable_fk_checks)?;
        let pool = self.pool_for(&table.catalog).await?;
        sqlx::query(&format!(
            "TRUNCATE TABLE {} RESTART IDENTITY",
            self.relation(table)?
        ))
        .execute(&pool)
        .await
        .map_err(|e| e.to_string())?;
        Ok(())
    }
}
