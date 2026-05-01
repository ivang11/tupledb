use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FilterSet {
    pub match_all: bool, // AND if true, OR if false
    pub rows: Vec<FilterRow>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FilterRow {
    pub active: bool,
    pub column: String,
    pub operator: Operator,
    pub value: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum Operator {
    // Text
    Equals,
    NotEquals,
    Contains,
    StartsWith,
    EndsWith,
    In,
    NotIn,
    IsNull,
    IsNotNull,
    // Number
    GreaterThan,
    GreaterOrEqual,
    LessThan,
    LessOrEqual,
    Between,
    // Date
    Before,
    After,
    // Boolean
    True,
    False,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_frontend_operator_names() {
        let json = r#"{
            "match_all": false,
            "rows": [
                {
                    "active": true,
                    "column": "name",
                    "operator": "starts_with",
                    "value": "A"
                },
                {
                    "active": true,
                    "column": "deleted_at",
                    "operator": "is_null",
                    "value": ""
                }
            ]
        }"#;

        let filters: FilterSet = serde_json::from_str(json).expect("valid filter set");

        assert!(!filters.match_all);
        assert_eq!(filters.rows.len(), 2);
        assert!(matches!(filters.rows[0].operator, Operator::StartsWith));
        assert!(matches!(filters.rows[1].operator, Operator::IsNull));
    }

    #[test]
    fn serializes_operator_names_for_frontend_contract() {
        let row = FilterRow {
            active: true,
            column: "created_at".to_string(),
            operator: Operator::GreaterOrEqual,
            value: "2026-01-01".to_string(),
        };

        let value = serde_json::to_value(row).expect("serializable filter row");

        assert_eq!(value["operator"], "greater_or_equal");
    }
}
