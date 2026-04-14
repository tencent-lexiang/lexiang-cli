use serde_json::Value;

/// Fields hidden by default in table/CSV/markdown output to reduce noise.
const DEFAULT_HIDDEN_FIELDS: &[&str] = &["cover", "created_by", "updated_by"];

/// Options for controlling which fields appear in table output.
pub struct FieldFilter {
    /// If set, only these fields are shown (comma-separated from --fields).
    pub fields: Option<Vec<String>>,
    /// If true, show all fields including normally hidden ones (--all-fields).
    pub all_fields: bool,
}

impl FieldFilter {
    pub fn new(fields: Option<Vec<String>>, all_fields: bool) -> Self {
        Self { fields, all_fields }
    }

    /// Filter columns: apply --fields whitelist or remove `DEFAULT_HIDDEN_FIELDS`.
    pub fn filter_columns<'a>(&self, columns: Vec<&'a str>) -> Vec<&'a str> {
        if let Some(ref fields) = self.fields {
            // --fields: only show specified fields that exist in data
            columns
                .into_iter()
                .filter(|c| fields.iter().any(|f| f == c))
                .collect()
        } else if self.all_fields {
            // --all-fields: show everything
            columns
        } else {
            // Default: hide noisy fields
            columns
                .into_iter()
                .filter(|c| !DEFAULT_HIDDEN_FIELDS.contains(c))
                .collect()
        }
    }
}

pub fn print_table(value: &Value, filter: &FieldFilter) {
    let data = value.get("data").unwrap_or(value);

    match data {
        Value::Array(arr) => print_array_table(arr, filter),
        Value::Object(obj) => {
            for (key, val) in obj {
                if let Value::Array(arr) = val {
                    if !arr.is_empty() {
                        println!("{}:", key);
                        print_array_table(arr, filter);
                        return;
                    }
                }
            }
            for (key, val) in obj {
                println!("{}: {}", key, format_value(val));
            }
        }
        _ => println!("{}", format_value(data)),
    }
}

fn print_array_table(arr: &[Value], filter: &FieldFilter) {
    if arr.is_empty() {
        println!("(empty)");
        return;
    }

    if let Some(Value::Object(first)) = arr.first() {
        let all_columns: Vec<&str> = first.keys().map(std::string::String::as_str).collect();
        let columns = filter.filter_columns(all_columns);

        let mut widths: Vec<usize> = columns.iter().map(|c| c.len()).collect();
        for item in arr {
            if let Value::Object(obj) = item {
                for (i, col) in columns.iter().enumerate() {
                    let val_len = format_value(obj.get(*col).unwrap_or(&Value::Null)).len();
                    if val_len > widths[i] {
                        widths[i] = val_len.min(40);
                    }
                }
            }
        }

        let header: Vec<String> = columns
            .iter()
            .zip(&widths)
            .map(|(c, w)| format!("{:<width$}", c, width = *w))
            .collect();
        println!("{}", header.join(" | "));
        println!(
            "{}",
            widths
                .iter()
                .map(|w| "-".repeat(*w))
                .collect::<Vec<_>>()
                .join("-+-")
        );

        for item in arr {
            if let Value::Object(obj) = item {
                let row: Vec<String> = columns
                    .iter()
                    .zip(&widths)
                    .map(|(col, w)| {
                        let val = format_value(obj.get(*col).unwrap_or(&Value::Null));
                        if val.len() > *w {
                            format!("{:.width$}", val, width = w - 1).to_string() + "…"
                        } else {
                            format!("{:<width$}", val, width = *w)
                        }
                    })
                    .collect();
                println!("{}", row.join(" | "));
            }
        }
    } else {
        for item in arr {
            println!("{}", format_value(item));
        }
    }
}

pub fn print_csv(value: &Value, filter: &FieldFilter) {
    let data = value.get("data").unwrap_or(value);

    if let Value::Object(obj) = data {
        for (_, val) in obj {
            if let Value::Array(arr) = val {
                if !arr.is_empty() {
                    if let Some(Value::Object(first)) = arr.first() {
                        let all_columns: Vec<&str> =
                            first.keys().map(std::string::String::as_str).collect();
                        let columns = filter.filter_columns(all_columns);
                        println!("{}", columns.join(","));

                        for item in arr {
                            if let Value::Object(obj) = item {
                                let row: Vec<String> = columns
                                    .iter()
                                    .map(|col| {
                                        let val =
                                            format_value(obj.get(*col).unwrap_or(&Value::Null));
                                        if val.contains(',') || val.contains('"') {
                                            format!("\"{}\"", val.replace('"', "\"\""))
                                        } else {
                                            val
                                        }
                                    })
                                    .collect();
                                println!("{}", row.join(","));
                            }
                        }
                        return;
                    }
                }
            }
        }
    }

    println!("{}", serde_json::to_string(value).unwrap_or_default());
}

pub fn print_markdown(value: &Value, filter: &FieldFilter) {
    let data = value.get("data").unwrap_or(value);

    if let Value::Object(obj) = data {
        for (key, val) in obj {
            if let Value::Array(arr) = val {
                if !arr.is_empty() {
                    println!("## {}\n", key);
                    if let Some(Value::Object(first)) = arr.first() {
                        let all_columns: Vec<&str> =
                            first.keys().map(std::string::String::as_str).collect();
                        let columns = filter.filter_columns(all_columns);
                        println!("| {} |", columns.join(" | "));
                        println!(
                            "| {} |",
                            columns
                                .iter()
                                .map(|_| "---")
                                .collect::<Vec<_>>()
                                .join(" | ")
                        );

                        for item in arr {
                            if let Value::Object(obj) = item {
                                let row: Vec<String> = columns
                                    .iter()
                                    .map(|col| format_value(obj.get(*col).unwrap_or(&Value::Null)))
                                    .collect();
                                println!("| {} |", row.join(" | "));
                            }
                        }
                        return;
                    }
                }
            }
        }
    }

    println!(
        "```json\n{}\n```",
        serde_json::to_string_pretty(value).unwrap_or_default()
    );
}

fn format_value(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => {
            // 自动检测 Unix 时间戳（10 位数字，合理范围 2001-2040）
            if let Some(ts) = n.as_i64() {
                if ts > 1_000_000_000 && ts < 2_200_000_000 {
                    if let Some(dt) = chrono::DateTime::from_timestamp(ts, 0) {
                        return dt
                            .with_timezone(&chrono::Local)
                            .format("%Y-%m-%d %H:%M")
                            .to_string();
                    }
                }
            }
            n.to_string()
        }
        Value::String(s) => s.clone(),
        Value::Array(arr) => {
            let items: Vec<String> = arr.iter().map(format_value).collect();
            format!("[{}]", items.join(", "))
        }
        Value::Object(obj) => {
            if let Some(name) = obj.get("name").and_then(|v| v.as_str()) {
                name.to_string()
            } else if let Some(id) = obj.get("id").and_then(|v| v.as_str()) {
                id.to_string()
            } else {
                serde_json::to_string(obj).unwrap_or_else(|_| "{...}".to_string())
            }
        }
    }
}
