use super::types::EditableColumn;
use pgone_sql::{ColumnDetail, TableDetail};

/// Generate ALTER TABLE statement list, comparing original and modified structures
pub(super) fn generate_alter_statements(
    schema: &str,
    table_name: &str,
    original: &TableDetail,
    modified: &[EditableColumn],
) -> Vec<String> {
    let mut statements = Vec::new();

    // Create a mapping from column names to original columns
    let original_columns_map: std::collections::HashMap<String, &ColumnDetail> = original
        .columns
        .iter()
        .map(|col| (col.name.clone(), col))
        .collect();

    // Handle deleted columns
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

    // Handle new columns
    for col in modified {
        if col.is_new && !col.is_deleted {
            statements.push(format!(
                "ALTER TABLE {}.{} ADD COLUMN {} {}",
                quote_ident(schema),
                quote_ident(table_name),
                quote_ident(&col.name),
                build_column_type(&col)
            ));

            // Set nullability
            if !col.nullable {
                statements.push(format!(
                    "ALTER TABLE {}.{} ALTER COLUMN {} SET NOT NULL",
                    quote_ident(schema),
                    quote_ident(table_name),
                    quote_ident(&col.name)
                ));
            }

            // Set default value
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

            // Set comment
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

    // Handle modified columns
    for col in modified {
        if !col.is_new && !col.is_deleted {
            let original_name = col.original_name.as_ref().unwrap_or(&col.name);
            if let Some(original_col) = original_columns_map.get(original_name) {
                // Check if column name changed
                if col.name != original_col.name {
                    statements.push(format!(
                        "ALTER TABLE {}.{} RENAME COLUMN {} TO {}",
                        quote_ident(schema),
                        quote_ident(table_name),
                        quote_ident(&original_col.name),
                        quote_ident(&col.name)
                    ));
                }

                // Check if type changed
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

                // Check if nullability changed
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

                // Check if default value changed
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

                // Check if comment changed
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

/// Build column type string from EditableColumn
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
            // For other types, try using the UDT name or original type
            col.data_type.to_uppercase()
        }
    }
}

/// Build column type string from ColumnDetail (used for comparison)
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
            // For other types, try using the UDT name or original type
            if let Some(ref udt_name) = col.udt_name {
                udt_name.to_uppercase()
            } else {
                col.data_type.to_uppercase()
            }
        }
    }
}

/// Escape SQL identifiers
fn quote_ident(ident: &str) -> String {
    format!("\"{}\"", ident.replace('"', "\"\""))
}

/// Escape SQL string literals
fn quote_literal(literal: &str) -> String {
    format!("'{}'", literal.replace('\'', "''"))
}
