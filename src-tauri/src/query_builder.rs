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
