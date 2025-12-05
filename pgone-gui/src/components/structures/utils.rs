pub fn quote_ident(ident: &str) -> String {
    format!("\"{}\"", ident.replace('"', "\"\""))
}

/// 转义 SQL 字符串字面量
pub fn quote_literal(literal: &str) -> String {
    format!("'{}'", literal.replace('\'', "''"))
}

/// 从 ColumnDetail 构建列类型字符串
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
        "numeric" | "decimal" => {
            match (col.numeric_precision, col.numeric_scale) {
                (Some(precision), Some(scale)) => format!("NUMERIC({},{})", precision, scale),
                (Some(precision), None) => format!("NUMERIC({})", precision),
                _ => "NUMERIC".to_string(),
            }
        }
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
        "interval" => {
            match (col.numeric_precision, col.numeric_scale) {
                (Some(precision), Some(scale)) => format!("INTERVAL({},{})", precision, scale),
                (Some(precision), None) => format!("INTERVAL({})", precision),
                _ => "INTERVAL".to_string(),
            }
        }
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

/// 生成完整的表DDL语句
pub fn generate_table_ddl(
    schema: &str,
    table_name: &str,
    table_detail: &pgone_sql::TableDetail,
    indexes: &[pgone_sql::IndexInfo],
) -> String {
    let mut ddl = String::new();
    
    // 生成 CREATE TABLE 语句
    ddl.push_str(&format!("CREATE TABLE {}.{} (\n", quote_ident(schema), quote_ident(table_name)));
    
    // 生成列定义
    let mut column_defs = Vec::new();
    for col in &table_detail.columns {
        let mut col_def = format!("    {} {}", quote_ident(&col.name), build_column_type_from_detail(col));
        
        // 添加 NOT NULL 约束
        if !col.nullable {
            col_def.push_str(" NOT NULL");
        }
        
        // 添加默认值
        if let Some(ref default) = col.default {
            if !default.trim().is_empty() {
                col_def.push_str(&format!(" DEFAULT {}", default));
            }
        }
        
        column_defs.push(col_def);
    }
    
    // 添加主键约束
    if let Some(ref pk) = table_detail.primary_key {
        if !pk.columns.is_empty() {
            let pk_cols: Vec<String> = pk.columns.iter().map(|c| quote_ident(c)).collect();
            column_defs.push(format!("    PRIMARY KEY ({})", pk_cols.join(", ")));
        }
    }
    
    ddl.push_str(&column_defs.join(",\n"));
    ddl.push_str("\n);\n\n");
    
    // 添加表注释
    if let Some(ref comment) = table_detail.comment {
        if !comment.trim().is_empty() {
            ddl.push_str(&format!(
                "COMMENT ON TABLE {}.{} IS {};\n\n",
                quote_ident(schema),
                quote_ident(table_name),
                quote_literal(comment)
            ));
        }
    }
    
    // 添加列注释
    for col in &table_detail.columns {
        if let Some(ref comment) = col.comment {
            if !comment.trim().is_empty() {
                ddl.push_str(&format!(
                    "COMMENT ON COLUMN {}.{}.{} IS {};\n",
                    quote_ident(schema),
                    quote_ident(table_name),
                    quote_ident(&col.name),
                    quote_literal(comment)
                ));
            }
        }
    }
    
    if !table_detail.columns.iter().any(|c| c.comment.is_some() && !c.comment.as_ref().unwrap().trim().is_empty()) {
        // 如果没有列注释，移除最后一个换行
        if ddl.ends_with("\n\n") {
            ddl.pop();
        }
    } else {
        ddl.push('\n');
    }
    
    // 添加外键约束
    for fk in &table_detail.foreign_keys {
        let fk_cols: Vec<String> = fk.columns.iter().map(|c| quote_ident(c)).collect();
        let ref_cols: Vec<String> = fk.ref_columns.iter().map(|c| quote_ident(c)).collect();
        
        ddl.push_str(&format!(
            "ALTER TABLE {}.{} ADD CONSTRAINT {} FOREIGN KEY ({}) REFERENCES {}({})",
            quote_ident(schema),
            quote_ident(table_name),
            quote_ident(&format!("fk_{}_{}", table_name, fk.columns.join("_"))),
            fk_cols.join(", "),
            quote_ident(&fk.ref_table),
            ref_cols.join(", ")
        ));
        
        if let Some(ref on_update) = fk.on_update {
            if on_update != "NO ACTION" {
                ddl.push_str(&format!(" ON UPDATE {}", on_update));
            }
        }
        
        if let Some(ref on_delete) = fk.on_delete {
            if on_delete != "NO ACTION" {
                ddl.push_str(&format!(" ON DELETE {}", on_delete));
            }
        }
        
        ddl.push_str(";\n");
    }
    
    if !table_detail.foreign_keys.is_empty() {
        ddl.push('\n');
    }
    
    // 添加索引（排除主键索引，因为主键会自动创建索引）
    for idx in indexes {
        // 跳过主键索引
        if let Some(ref pk) = table_detail.primary_key {
            if pk.columns.len() == idx.columns.len() 
                && pk.columns.iter().all(|col| idx.columns.contains(col)) {
                continue;
            }
        }
        
        let idx_cols: Vec<String> = idx.columns.iter().map(|c| quote_ident(c)).collect();
        
        if let Some(ref definition) = idx.definition {
            // 如果有定义，直接使用定义
            ddl.push_str(&format!("CREATE {}INDEX {} ON {}.{} {};\n",
                if idx.unique { "UNIQUE " } else { "" },
                quote_ident(&idx.name),
                quote_ident(schema),
                quote_ident(table_name),
                definition
            ));
        } else {
            // 否则使用列名构建
            ddl.push_str(&format!("CREATE {}INDEX {} ON {}.{} ({})",
                if idx.unique { "UNIQUE " } else { "" },
                quote_ident(&idx.name),
                quote_ident(schema),
                quote_ident(table_name),
                idx_cols.join(", ")
            ));
            
            ddl.push_str(";\n");
        }
        
        // 添加索引注释
        if let Some(ref desc) = idx.description {
            if !desc.trim().is_empty() {
                ddl.push_str(&format!(
                    "COMMENT ON INDEX {}.{} IS {};\n",
                    quote_ident(schema),
                    quote_ident(&idx.name),
                    quote_literal(desc)
                ));
            }
        }
    }
    
    ddl
}

/// 生成表的 DML (INSERT) 语句
pub fn generate_table_dml(
    schema: &str,
    table_name: &str,
    columns: &[String],
    rows: &[Vec<String>],
) -> String {
    if rows.is_empty() {
        return String::new();
    }

    let mut dml = String::new();
    let quoted_table = format!("{}.{}", quote_ident(schema), quote_ident(table_name));
    let quoted_columns: Vec<String> = columns.iter().map(|c| quote_ident(c)).collect();
    let columns_str = quoted_columns.join(", ");

    // 批量生成 INSERT 语句，每100条一个批次
    const BATCH_SIZE: usize = 100;
    
    for batch_start in (0..rows.len()).step_by(BATCH_SIZE) {
        let batch_end = (batch_start + BATCH_SIZE).min(rows.len());
        let batch = &rows[batch_start..batch_end];
        
        if batch_start == 0 {
            dml.push_str(&format!("INSERT INTO {} ({}) VALUES\n", quoted_table, columns_str));
        } else {
            dml.push_str(&format!("\nINSERT INTO {} ({}) VALUES\n", quoted_table, columns_str));
        }

        for (idx, row) in batch.iter().enumerate() {
            let values: Vec<String> = row.iter().map(|val| {
                if val.trim().is_empty() || val == "NULL" {
                    "NULL".to_string()
                } else {
                    // 转义单引号并添加引号
                    quote_literal(val)
                }
            }).collect();
            
            dml.push_str("    (");
            dml.push_str(&values.join(", "));
            dml.push_str(")");
            
            if idx < batch.len() - 1 {
                dml.push_str(",\n");
            } else {
                dml.push_str(";\n");
            }
        }
    }

    dml
}

/// Replace database name in DSN while preserving password and other parameters
pub fn replace_database_in_dsn(dsn: &str, new_database: &str) -> Option<String> {
    // Try to parse as URL first - this preserves password and all query parameters
    if let Ok(mut url) = url::Url::parse(dsn) {
        // Set the new database path (url::Url handles encoding automatically)
        url.set_path(&format!("/{}", new_database));
        return Some(url.to_string());
    }
    
    // Fallback: try manual parsing for postgresql:// URLs
    // This handles cases where URL parsing fails but DSN format is still valid
    if dsn.starts_with("postgresql://") || dsn.starts_with("postgres://") {
        // Find the last '/' before query parameters
        if let Some(db_start) = dsn.rfind('/') {
            if let Some(query_start) = dsn[db_start..].find('?') {
                // Has query parameters - preserve them
                let base = &dsn[..db_start];
                let query = &dsn[db_start + query_start..];
                return Some(format!("{}/{}{}", base, new_database, query));
            } else {
                // No query parameters
                return Some(format!("{}/{}", &dsn[..db_start], new_database));
            }
        }
    }
    
    None
}

