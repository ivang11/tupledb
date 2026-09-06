use super::*;

pub(super) fn read_u32_wkb(data: &[u8], le: bool) -> u32 {
    let b = [data[0], data[1], data[2], data[3]];
    if le {
        u32::from_le_bytes(b)
    } else {
        u32::from_be_bytes(b)
    }
}

pub(super) fn read_f64_wkb(data: &[u8], le: bool) -> f64 {
    let b = [
        data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
    ];
    if le {
        f64::from_le_bytes(b)
    } else {
        f64::from_be_bytes(b)
    }
}

pub(super) fn wkb_parse(data: &[u8]) -> Option<String> {
    if data.len() < 5 {
        return None;
    }
    let le = data[0] == 1;
    let geom_type = read_u32_wkb(&data[1..5], le);
    let payload = &data[5..];
    match geom_type {
        1 => {
            // Point
            if payload.len() < 16 {
                return None;
            }
            let x = read_f64_wkb(&payload[0..8], le);
            let y = read_f64_wkb(&payload[8..16], le);
            Some(format!("POINT({} {})", x, y))
        }
        2 => {
            // LineString
            if payload.len() < 4 {
                return None;
            }
            let n = read_u32_wkb(&payload[0..4], le) as usize;
            let coords = &payload[4..];
            if coords.len() < n * 16 {
                return None;
            }
            let pts: Vec<String> = (0..n)
                .map(|i| {
                    let x = read_f64_wkb(&coords[i * 16..i * 16 + 8], le);
                    let y = read_f64_wkb(&coords[i * 16 + 8..i * 16 + 16], le);
                    format!("{} {}", x, y)
                })
                .collect();
            Some(format!("LINESTRING({})", pts.join(", ")))
        }
        3 => {
            // Polygon
            if payload.len() < 4 {
                return None;
            }
            let n_rings = read_u32_wkb(&payload[0..4], le) as usize;
            let mut offset = 4usize;
            let mut rings = Vec::new();
            for _ in 0..n_rings {
                if payload.len() < offset + 4 {
                    return None;
                }
                let n_pts = read_u32_wkb(&payload[offset..offset + 4], le) as usize;
                offset += 4;
                if payload.len() < offset + n_pts * 16 {
                    return None;
                }
                let pts: Vec<String> = (0..n_pts)
                    .map(|i| {
                        let x = read_f64_wkb(&payload[offset + i * 16..offset + i * 16 + 8], le);
                        let y =
                            read_f64_wkb(&payload[offset + i * 16 + 8..offset + i * 16 + 16], le);
                        format!("{} {}", x, y)
                    })
                    .collect();
                offset += n_pts * 16;
                rings.push(format!("({})", pts.join(", ")));
            }
            Some(format!("POLYGON({})", rings.join(", ")))
        }
        _ => None,
    }
}

pub(super) fn mysql_wkb_to_wkt(data: &[u8]) -> String {
    // MySQL spatial columns have a 4-byte SRID prefix before the WKB
    if data.len() > 4 {
        if let Some(wkt) = wkb_parse(&data[4..]) {
            return wkt;
        }
    }
    let hex: String = data.iter().map(|b| format!("{:02x}", b)).collect();
    format!("0x{}", hex)
}

pub(super) fn parse_mysql_row(row: &sqlx::mysql::MySqlRow) -> Value {
    let mut map = Map::new();
    // Decode by ordinal, not by column name. MySQL can return fresh column
    // metadata while a pooled/prepared statement still has a stale name index
    // after a schema change. Mixing `row.columns()` with name-based lookups can
    // then read a value from a different column until another connection is
    // used. The ordinal and its metadata always come from the same result row.
    for (index, col) in row.columns().iter().enumerate() {
        let col_name = col.name();
        let type_name = col.type_info().name().to_uppercase();
        let value: Value = match row.try_get_raw(index) {
            Ok(raw) if raw.is_null() => Value::Null,
            _ => match type_name.as_str() {
                "TINYINT(1)" | "BOOLEAN" | "BOOL" => row
                    .try_get::<i8, _>(index)
                    .map(|v| Value::Bool(v != 0))
                    .unwrap_or(Value::Null),
                t if t == "BIT" || t.starts_with("BIT(") => {
                    let width: u32 = t
                        .trim_start_matches("BIT(")
                        .trim_end_matches(')')
                        .parse()
                        .unwrap_or(1);
                    let to_bin =
                        |n: u64| Value::String(format!("{:0>width$b}", n, width = width as usize));
                    row.try_get::<u64, _>(index)
                        .map(&to_bin)
                        .or_else(|_| {
                            row.try_get::<Vec<u8>, _>(index).map(|b| {
                                let n = b.iter().fold(0u64, |acc, &x| (acc << 8) | x as u64);
                                to_bin(n)
                            })
                        })
                        .unwrap_or(Value::Null)
                }
                t if t.contains("INT") => row
                    .try_get::<i64, _>(index)
                    .map(|v| {
                        if (-9_007_199_254_740_991..=9_007_199_254_740_991).contains(&v) {
                            Value::Number(v.into())
                        } else {
                            Value::String(v.to_string())
                        }
                    })
                    .or_else(|_| {
                        row.try_get::<u64, _>(index).map(|v| {
                            if v <= 9_007_199_254_740_991 {
                                Value::Number(v.into())
                            } else {
                                Value::String(v.to_string())
                            }
                        })
                    })
                    .unwrap_or(Value::Null),
                t if t == "DOUBLE"
                    || t == "FLOAT"
                    || t.starts_with("DOUBLE")
                    || t.starts_with("FLOAT") =>
                {
                    row.try_get::<f64, _>(index)
                        .ok()
                        .and_then(serde_json::Number::from_f64)
                        .map(Value::Number)
                        .unwrap_or(Value::Null)
                }
                t if t.starts_with("DECIMAL") || t.starts_with("NUMERIC") || t == "NEWDECIMAL" => {
                    // MySQL sends DECIMAL as decimal text in both protocols.
                    // Decode directly to preserve its full 65-digit range,
                    // beyond rust_decimal's precision and JavaScript numbers.
                    row.try_get_unchecked::<String, _>(index)
                        .map(Value::String)
                        .unwrap_or(Value::Null)
                }
                "YEAR" => row
                    .try_get::<u16, _>(index)
                    .map(|v| Value::Number(v.into()))
                    .or_else(|_| row.try_get::<String, _>(index).map(Value::String))
                    .unwrap_or(Value::Null),
                "DATE" => row
                    .try_get::<chrono::NaiveDate, _>(index)
                    .map(|d| Value::String(d.to_string()))
                    .or_else(|_| row.try_get::<String, _>(index).map(Value::String))
                    .unwrap_or(Value::Null),
                t if t == "TIME" || t.starts_with("TIME(") => row
                    .try_get::<chrono::NaiveTime, _>(index)
                    .map(|t| {
                        let base = t.format("%H:%M:%S").to_string();
                        if t.nanosecond() == 0 {
                            base
                        } else {
                            let frac = format!("{:.6}", t.nanosecond() as f64 / 1_000_000_000.0);
                            format!(
                                "{}{}",
                                base,
                                frac.trim_start_matches('0').trim_end_matches('0')
                            )
                        }
                    })
                    .map(Value::String)
                    .or_else(|_| row.try_get::<String, _>(index).map(Value::String))
                    .unwrap_or(Value::Null),
                t if t == "DATETIME" || t.starts_with("DATETIME(") => row
                    .try_get::<chrono::NaiveDateTime, _>(index)
                    .map(|dt| {
                        let base = dt.format("%Y-%m-%d %H:%M:%S").to_string();
                        if dt.nanosecond() == 0 {
                            base
                        } else {
                            let frac = format!("{:.6}", dt.nanosecond() as f64 / 1_000_000_000.0);
                            format!(
                                "{}{}",
                                base,
                                frac.trim_start_matches('0').trim_end_matches('0')
                            )
                        }
                    })
                    .map(Value::String)
                    .or_else(|_| row.try_get::<String, _>(index).map(Value::String))
                    .unwrap_or(Value::Null),
                t if t == "TIMESTAMP" || t.starts_with("TIMESTAMP(") => {
                    let s = row
                        .try_get::<chrono::NaiveDateTime, _>(index)
                        .map(|dt| {
                            let base = dt.format("%Y-%m-%d %H:%M:%S").to_string();
                            if dt.nanosecond() == 0 {
                                base
                            } else {
                                let frac =
                                    format!("{:.6}", dt.nanosecond() as f64 / 1_000_000_000.0);
                                format!(
                                    "{}{}",
                                    base,
                                    frac.trim_start_matches('0').trim_end_matches('0')
                                )
                            }
                        })
                        .or_else(|_| {
                            row.try_get::<chrono::DateTime<chrono::Utc>, _>(index)
                                .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                        })
                        .or_else(|_| row.try_get::<String, _>(index));
                    s.map(Value::String).unwrap_or(Value::Null)
                }
                t if t.contains("BLOB") || t == "BINARY" || t.starts_with("VARBINARY") => row
                    .try_get::<Vec<u8>, _>(index)
                    .map(|b| {
                        let trimmed: Vec<u8> = b
                            .iter()
                            .copied()
                            .rev()
                            .skip_while(|&x| x == 0)
                            .collect::<Vec<_>>()
                            .into_iter()
                            .rev()
                            .collect();
                        match String::from_utf8(trimmed) {
                            Ok(s) => Value::String(s),
                            Err(_) => {
                                let hex: String =
                                    b.iter().map(|byte| format!("{:02x}", byte)).collect();
                                Value::String(format!("0x{}", hex))
                            }
                        }
                    })
                    .unwrap_or(Value::Null),
                t if t == "GEOMETRY"
                    || t == "POINT"
                    || t == "LINESTRING"
                    || t == "POLYGON"
                    || t.starts_with("MULTI")
                    || t == "GEOMETRYCOLLECTION" =>
                {
                    row.try_get_unchecked::<Vec<u8>, _>(index)
                        .map(|b| Value::String(mysql_wkb_to_wkt(&b)))
                        .or_else(|_| row.try_get_unchecked::<String, _>(index).map(Value::String))
                        .unwrap_or_else(|_| Value::String(format!("<{}>", type_name)))
                }
                t if t == "JSON" || t.contains("JSON") => row
                    .try_get_unchecked::<String, _>(index)
                    .or_else(|_| {
                        row.try_get_unchecked::<Vec<u8>, _>(index)
                            .map(|b| String::from_utf8_lossy(&b).to_string())
                    })
                    .map(Value::String)
                    .unwrap_or_else(|_| Value::String(format!("<{}>", type_name))),
                _ => row
                    .try_get::<String, _>(index)
                    .map(Value::String)
                    .or_else(|_| {
                        row.try_get::<Vec<u8>, _>(index)
                            .map(|b| Value::String(String::from_utf8_lossy(&b).to_string()))
                    })
                    .unwrap_or_else(|_| Value::String(format!("<{}>", type_name))),
            },
        };
        map.insert(col_name.to_string(), value);
    }
    Value::Object(map)
}

pub(super) fn rows_to_parsed(rows: Vec<sqlx::mysql::MySqlRow>) -> (Vec<ColumnInfo>, Vec<Value>) {
    let mut columns = Vec::new();
    if let Some(first_row) = rows.first() {
        for col in first_row.columns() {
            columns.push(ColumnInfo {
                name: col.name().to_string(),
                type_name: col.type_info().name().to_string(),
            });
        }
    }
    let result_rows = rows.iter().map(parse_mysql_row).collect();
    (columns, result_rows)
}

// --------------------------------------------------------------------------
// Row helper: try String, fallback to bytes
// --------------------------------------------------------------------------

pub(super) fn get_str_lossy(row: &sqlx::mysql::MySqlRow, index: usize) -> String {
    get_optional_str_lossy(row, index).unwrap_or_else(|| "unknown".to_string())
}

pub(super) fn get_optional_str_lossy(row: &sqlx::mysql::MySqlRow, index: usize) -> Option<String> {
    if row
        .try_get_raw(index)
        .ok()
        .is_some_and(|value| value.is_null())
    {
        return None;
    }
    row.try_get::<String, _>(index)
        .ok()
        .or_else(|| row.try_get::<i64, _>(index).ok().map(|n| n.to_string()))
        .or_else(|| row.try_get::<u64, _>(index).ok().map(|n| n.to_string()))
        .or_else(|| {
            row.try_get::<Vec<u8>, _>(index)
                .ok()
                .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
        })
}
