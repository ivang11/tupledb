use crate::filters::{FilterSet, Operator};

pub fn build_where_clause(filters: &FilterSet) -> (String, Vec<String>) {
    let active_rows: Vec<_> = filters.rows.iter().filter(|r| r.active).collect();

    if active_rows.is_empty() {
        return ("".to_string(), vec![]);
    }

    let mut sql = " WHERE ".to_string();
    let mut params = Vec::new();
    let joiner = if filters.match_all { " AND " } else { " OR " };

    for (i, row) in active_rows.iter().enumerate() {
        if i > 0 {
            sql.push_str(joiner);
        }

        let mut value = row.value.clone();

        // Normalize boolean strings for MySQL
        if value.to_lowercase() == "true" {
            value = "1".to_string();
        } else if value.to_lowercase() == "false" {
            value = "0".to_string();
        }

        let condition = match row.operator {
            Operator::Equals => {
                params.push(value);
                format!("`{}` = ?", row.column)
            }
            Operator::NotEquals => {
                params.push(value);
                format!("`{}` != ?", row.column)
            }
            Operator::Contains => {
                params.push(format!("%{}%", row.value));
                format!("`{}` LIKE ?", row.column)
            }
            Operator::StartsWith => {
                params.push(format!("{}%", row.value));
                format!("`{}` LIKE ?", row.column)
            }
            Operator::EndsWith => {
                params.push(format!("%{}", row.value));
                format!("`{}` LIKE ?", row.column)
            }
            Operator::IsNull => format!("`{}` IS NULL", row.column),
            Operator::IsNotNull => format!("`{}` IS NOT NULL", row.column),
            Operator::GreaterThan => {
                params.push(row.value.clone());
                format!("`{}` > ?", row.column)
            }
            Operator::GreaterOrEqual => {
                params.push(row.value.clone());
                format!("`{}` >= ?", row.column)
            }
            Operator::LessThan => {
                params.push(row.value.clone());
                format!("`{}` < ?", row.column)
            }
            Operator::LessOrEqual => {
                params.push(row.value.clone());
                format!("`{}` <= ?", row.column)
            }
            Operator::True => format!("`{}` = 1", row.column),
            Operator::False => format!("`{}` = 0", row.column),
            _ => "1=1".to_string(), // Fallback
        };

        sql.push_str(&condition);
    }

    (sql, params)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filters::FilterRow;

    fn row(column: &str, operator: Operator, value: &str) -> FilterRow {
        FilterRow {
            active: true,
            column: column.to_string(),
            operator,
            value: value.to_string(),
        }
    }

    #[test]
    fn returns_empty_clause_when_no_rows_are_active() {
        let filters = FilterSet {
            match_all: true,
            rows: vec![FilterRow {
                active: false,
                column: "name".to_string(),
                operator: Operator::Equals,
                value: "Ada".to_string(),
            }],
        };

        let (sql, params) = build_where_clause(&filters);

        assert_eq!(sql, "");
        assert!(params.is_empty());
    }

    #[test]
    fn builds_and_clause_with_bound_params() {
        let filters = FilterSet {
            match_all: true,
            rows: vec![
                row("name", Operator::Contains, "ada"),
                row("age", Operator::GreaterOrEqual, "18"),
            ],
        };

        let (sql, params) = build_where_clause(&filters);

        assert_eq!(sql, " WHERE `name` LIKE ? AND `age` >= ?");
        assert_eq!(params, vec!["%ada%".to_string(), "18".to_string()]);
    }

    #[test]
    fn builds_or_clause_and_skips_inactive_rows() {
        let filters = FilterSet {
            match_all: false,
            rows: vec![
                row("status", Operator::Equals, "active"),
                FilterRow {
                    active: false,
                    column: "deleted_at".to_string(),
                    operator: Operator::IsNull,
                    value: String::new(),
                },
                row("role", Operator::Equals, "admin"),
            ],
        };

        let (sql, params) = build_where_clause(&filters);

        assert_eq!(sql, " WHERE `status` = ? OR `role` = ?");
        assert_eq!(params, vec!["active".to_string(), "admin".to_string()]);
    }

    #[test]
    fn normalizes_boolean_strings_for_equality() {
        let filters = FilterSet {
            match_all: true,
            rows: vec![
                row("is_enabled", Operator::Equals, "true"),
                row("is_archived", Operator::NotEquals, "FALSE"),
            ],
        };

        let (sql, params) = build_where_clause(&filters);

        assert_eq!(sql, " WHERE `is_enabled` = ? AND `is_archived` != ?");
        assert_eq!(params, vec!["1".to_string(), "0".to_string()]);
    }

    #[test]
    fn null_and_boolean_operators_do_not_bind_values() {
        let filters = FilterSet {
            match_all: true,
            rows: vec![
                row("deleted_at", Operator::IsNull, ""),
                row("verified", Operator::True, ""),
                row("blocked", Operator::False, ""),
            ],
        };

        let (sql, params) = build_where_clause(&filters);

        assert_eq!(
            sql,
            " WHERE `deleted_at` IS NULL AND `verified` = 1 AND `blocked` = 0"
        );
        assert!(params.is_empty());
    }
}
