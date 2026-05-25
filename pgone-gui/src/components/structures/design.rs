use super::types::EditableColumn;
use pgone_sql::{ColumnDetail, TableDetail};

/// 生成 ALTER TABLE 语句列表，对比原始结构和修改后的结构
pub(super) fn generate_alter_statements(
    schema: &str,
    table_name: &str,
    original: &TableDetail,
    modified: &[EditableColumn],
) -> Vec<String> {
    let mut statements = Vec::new();

    // 创建列名到原始列的映射
    let original_columns_map: std::collections::HashMap<String, &ColumnDetail> = original
        .columns
        .iter()
        .map(|col| (col.name.clone(), col))
        .collect();

    // 处理删除的列
    for col in modified {
        if col.is_deleted {
            let key = col.original_name.as_ref().unwrap_or(&col.name);
            if let Some(original_col) = original_columns_map.get(key) {
                statements.push(format!(
                    "ALTER TABLE {}.{} DROP COLUMN {}",
                    quote_ident(schema),
                    quote_ident(table_name),
                    quote_ident(&original_col.name)
                ));
            }
        }
    }

    // 处理新增的列
    for col in modified {
        if col.is_new && !col.is_deleted {
            statements.push(format!(
                "ALTER TABLE {}.{} ADD COLUMN {} {}",
                quote_ident(schema),
                quote_ident(table_name),
                quote_ident(&col.name),
                build_column_type(&col)
            ));

            // 设置可空性
            if !col.nullable {
                statements.push(format!(
                    "ALTER TABLE {}.{} ALTER COLUMN {} SET NOT NULL",
                    quote_ident(schema),
                    quote_ident(table_name),
                    quote_ident(&col.name)
                ));
            }

            // 设置默认值
            if let Some(ref default) = col.default {
                if !default.trim().is_empty() {
                    statements.push(format!(
                        "ALTER TABLE {}.{} ALTER COLUMN {} SET DEFAULT {}",
                        quote_ident(schema),
                        quote_ident(table_name),
                        quote_ident(&col.name),
                        default
                    ));
                }
            }

            // 设置注释
            if let Some(ref comment) = col.comment {
                if !comment.trim().is_empty() {
                    statements.push(format!(
                        "COMMENT ON COLUMN {}.{}.{} IS {}",
                        quote_ident(schema),
                        quote_ident(table_name),
                        quote_ident(&col.name),
                        quote_literal(comment)
                    ));
                }
            }
        }
    }

    // 处理修改的列
    for col in modified {
        if !col.is_new && !col.is_deleted {
            let original_name = col.original_name.as_ref().unwrap_or(&col.name);
            if let Some(original_col) = original_columns_map.get(original_name) {
                // 检查列名是否改变
                if col.name != original_col.name {
                    statements.push(format!(
                        "ALTER TABLE {}.{} RENAME COLUMN {} TO {}",
                        quote_ident(schema),
                        quote_ident(table_name),
                        quote_ident(&original_col.name),
                        quote_ident(&col.name)
                    ));
                }

                // 检查类型是否改变
                let original_type = build_column_type_from_detail(original_col);
                let new_type = build_column_type(col);
                if original_type != new_type {
                    statements.push(format!(
                        "ALTER TABLE {}.{} ALTER COLUMN {} TYPE {}",
                        quote_ident(schema),
                        quote_ident(table_name),
                        quote_ident(&col.name),
                        new_type
                    ));
                }

                // 检查可空性是否改变
                if col.nullable != original_col.nullable {
                    if col.nullable {
                        statements.push(format!(
                            "ALTER TABLE {}.{} ALTER COLUMN {} DROP NOT NULL",
                            quote_ident(schema),
                            quote_ident(table_name),
                            quote_ident(&col.name)
                        ));
                    } else {
                        statements.push(format!(
                            "ALTER TABLE {}.{} ALTER COLUMN {} SET NOT NULL",
                            quote_ident(schema),
                            quote_ident(table_name),
                            quote_ident(&col.name)
                        ));
                    }
                }

                // 检查默认值是否改变
                let original_default = original_col
                    .default
                    .as_ref()
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty());
                let new_default = col
                    .default
                    .as_ref()
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty());
                if original_default != new_default {
                    if let Some(ref default) = new_default {
                        statements.push(format!(
                            "ALTER TABLE {}.{} ALTER COLUMN {} SET DEFAULT {}",
                            quote_ident(schema),
                            quote_ident(table_name),
                            quote_ident(&col.name),
                            default
                        ));
                    } else {
                        statements.push(format!(
                            "ALTER TABLE {}.{} ALTER COLUMN {} DROP DEFAULT",
                            quote_ident(schema),
                            quote_ident(table_name),
                            quote_ident(&col.name)
                        ));
                    }
                }

                // 检查注释是否改变
                let original_comment = original_col
                    .comment
                    .as_ref()
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty());
                let new_comment = col
                    .comment
                    .as_ref()
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty());
                if original_comment != new_comment {
                    if let Some(ref comment) = new_comment {
                        statements.push(format!(
                            "COMMENT ON COLUMN {}.{}.{} IS {}",
                            quote_ident(schema),
                            quote_ident(table_name),
                            quote_ident(&col.name),
                            quote_literal(comment)
                        ));
                    } else {
                        statements.push(format!(
                            "COMMENT ON COLUMN {}.{}.{} IS NULL",
                            quote_ident(schema),
                            quote_ident(table_name),
                            quote_ident(&col.name)
                        ));
                    }
                }
            }
        }
    }

    statements
}

/// 从 EditableColumn 构建列类型字符串
fn build_column_type(col: &EditableColumn) -> String {
    let base_type = col.data_type.to_lowercase();

    match base_type.as_str() {
        "character varying" | "varchar" => {
            if let Some(len) = col.character_maximum_length {
                format!("VARCHAR({})", len)
            } else {
                "VARCHAR".to_string()
            }
        }
        "character" | "char" => {
            if let Some(len) = col.character_maximum_length {
                format!("CHAR({})", len)
            } else {
                "CHAR".to_string()
            }
        }
        "numeric" | "decimal" => match (col.numeric_precision, col.numeric_scale) {
            (Some(precision), Some(scale)) => format!("NUMERIC({},{})", precision, scale),
            (Some(precision), None) => format!("NUMERIC({})", precision),
            _ => "NUMERIC".to_string(),
        },
        "timestamp without time zone" | "timestamp" => {
            if let Some(precision) = col.numeric_precision {
                format!("TIMESTAMP({})", precision)
            } else {
                "TIMESTAMP".to_string()
            }
        }
        "timestamp with time zone" | "timestamptz" => {
            if let Some(precision) = col.numeric_precision {
                format!("TIMESTAMP({}) WITH TIME ZONE", precision)
            } else {
                "TIMESTAMP WITH TIME ZONE".to_string()
            }
        }
        "time without time zone" | "time" => {
            if let Some(precision) = col.numeric_precision {
                format!("TIME({})", precision)
            } else {
                "TIME".to_string()
            }
        }
        "time with time zone" | "timetz" => {
            if let Some(precision) = col.numeric_precision {
                format!("TIME({}) WITH TIME ZONE", precision)
            } else {
                "TIME WITH TIME ZONE".to_string()
            }
        }
        "interval" => match (col.numeric_precision, col.numeric_scale) {
            (Some(precision), Some(scale)) => format!("INTERVAL({},{})", precision, scale),
            (Some(precision), None) => format!("INTERVAL({})", precision),
            _ => "INTERVAL".to_string(),
        },
        _ => {
            // 对于其他类型，尝试使用 UDT 名称或原始类型
            col.data_type.to_uppercase()
        }
    }
}

/// 从 ColumnDetail 构建列类型字符串（用于对比）
fn build_column_type_from_detail(col: &pgone_sql::ColumnDetail) -> String {
    let base_type = col.data_type.to_lowercase();

    match base_type.as_str() {
        "character varying" | "varchar" => {
            if let Some(len) = col.character_maximum_length {
                format!("VARCHAR({})", len)
            } else {
                "VARCHAR".to_string()
            }
        }
        "character" | "char" => {
            if let Some(len) = col.character_maximum_length {
                format!("CHAR({})", len)
            } else {
                "CHAR".to_string()
            }
        }
        "numeric" | "decimal" => match (col.numeric_precision, col.numeric_scale) {
            (Some(precision), Some(scale)) => format!("NUMERIC({},{})", precision, scale),
            (Some(precision), None) => format!("NUMERIC({})", precision),
            _ => "NUMERIC".to_string(),
        },
        "timestamp without time zone" | "timestamp" => {
            if let Some(precision) = col.numeric_precision {
                format!("TIMESTAMP({})", precision)
            } else {
                "TIMESTAMP".to_string()
            }
        }
        "timestamp with time zone" | "timestamptz" => {
            if let Some(precision) = col.numeric_precision {
                format!("TIMESTAMP({}) WITH TIME ZONE", precision)
            } else {
                "TIMESTAMP WITH TIME ZONE".to_string()
            }
        }
        "time without time zone" | "time" => {
            if let Some(precision) = col.numeric_precision {
                format!("TIME({})", precision)
            } else {
                "TIME".to_string()
            }
        }
        "time with time zone" | "timetz" => {
            if let Some(precision) = col.numeric_precision {
                format!("TIME({}) WITH TIME ZONE", precision)
            } else {
                "TIME WITH TIME ZONE".to_string()
            }
        }
        "interval" => match (col.numeric_precision, col.numeric_scale) {
            (Some(precision), Some(scale)) => format!("INTERVAL({},{})", precision, scale),
            (Some(precision), None) => format!("INTERVAL({})", precision),
            _ => "INTERVAL".to_string(),
        },
        _ => {
            // 对于其他类型，尝试使用 UDT 名称或原始类型
            if let Some(ref udt_name) = col.udt_name {
                udt_name.to_uppercase()
            } else {
                col.data_type.to_uppercase()
            }
        }
    }
}

/// 转义 SQL 标识符
fn quote_ident(ident: &str) -> String {
    format!("\"{}\"", ident.replace('"', "\"\""))
}

/// 转义 SQL 字符串字面量
fn quote_literal(literal: &str) -> String {
    format!("'{}'", literal.replace('\'', "''"))
}
