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
