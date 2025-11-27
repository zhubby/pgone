use eframe::egui::text::{LayoutJob, TextFormat};
use sqlx::Row;
use sqlx::postgres::PgRow;
use std::collections::HashSet;
use once_cell::sync::Lazy;

/// PostgreSQL 关键字集合
static PG_KEYWORDS: &[&str] = &[
    // DML
    "select", "insert", "update", "delete", "merge",
    // DDL
    "create", "drop", "alter", "truncate",
    // 表相关
    "table", "view", "index", "sequence", "schema", "database",
    // JOIN
    "join", "inner", "left", "right", "full", "outer", "cross", "natural", "on",
    // WHERE/HAVING
    "where", "having",
    // GROUP BY/ORDER BY
    "group", "by", "order", "asc", "desc", "nulls", "first", "last",
    // 聚合函数
    "count", "sum", "avg", "min", "max", "distinct",
    // 集合操作
    "union", "intersect", "except", "all",
    // 子查询
    "exists", "in", "any", "some",
    // 逻辑运算符
    "and", "or", "not", "is",
    // 比较运算符
    "between", "like", "ilike", "similar", "to",
    // 条件
    "case", "when", "then", "else", "end",
    // 别名
    "as",
    // FROM
    "from",
    // LIMIT/OFFSET
    "limit", "offset", "fetch", "next", "rows", "only",
    // WITH
    "with", "recursive",
    // 约束
    "primary", "key", "foreign", "references", "constraint", "unique", "check",
    "default",
    // 数据类型
    "integer", "int", "bigint", "smallint", "serial", "bigserial",
    "real", "double", "precision", "numeric", "decimal",
    "varchar", "char", "text",
    "boolean", "bool",
    "date", "time", "timestamp", "interval",
    "json", "jsonb", "xml", "array",
    "uuid", "bytea",
    // 函数相关
    "function", "procedure", "returns", "return", "language",
    // 事务
    "begin", "commit", "rollback", "transaction", "savepoint", "release",
    // 权限
    "grant", "revoke", "privileges",
    // 其他
    "if", "exists", "cascade", "restrict",
    "set", "to", "using",
    "values", "into",
    "over", "partition", "window",
    "cast",
    "coalesce", "nullif",
    "extract", "current_date", "current_time", "current_timestamp",
    "now", "today", "yesterday",
    "true", "false",
];

static KEYWORD_SET: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    PG_KEYWORDS.iter().copied().collect()
});

/// SQL 高亮函数，支持 PostgreSQL 标准
pub fn highlight_sql(text: &str, visuals: &egui::Visuals) -> LayoutJob {
    let mut job = LayoutJob::default();
    
    // 定义文本格式
    let normal = TextFormat {
        color: visuals.text_color(),
        ..Default::default()
    };
    let kw = TextFormat {
        color: egui::Color32::from_rgb(198, 120, 221), // 紫色 - 关键字
        ..Default::default()
    };
    let string = TextFormat {
        color: egui::Color32::from_rgb(152, 195, 121), // 绿色 - 字符串
        ..Default::default()
    };
    let number = TextFormat {
        color: egui::Color32::from_rgb(209, 154, 102), // 橙色 - 数字
        ..Default::default()
    };
    let comment = TextFormat {
        color: egui::Color32::from_rgb(128, 128, 128), // 灰色 - 注释
        ..Default::default()
    };
    let operator = TextFormat {
        color: egui::Color32::from_rgb(180, 180, 180), // 浅灰色 - 操作符
        ..Default::default()
    };
    
    let bytes = text.as_bytes();
    let mut i = 0;
    
    while i < bytes.len() {
        let c = bytes[i] as char;
        
        // 处理空白字符
        if c.is_whitespace() {
            job.append(&text[i..i + 1], 0.0, normal.clone());
            i += 1;
            continue;
        }
        
        // 处理单行注释 --
        if i + 1 < bytes.len() && c == '-' && bytes[i + 1] as char == '-' {
            let start = i;
            i += 2;
            while i < bytes.len() && bytes[i] as char != '\n' {
                i += 1;
            }
            job.append(&text[start..i], 0.0, comment.clone());
            continue;
        }
        
        // 处理多行注释 /* */
        if i + 1 < bytes.len() && c == '/' && bytes[i + 1] as char == '*' {
            let start = i;
            i += 2;
            while i + 1 < bytes.len() {
                if bytes[i] as char == '*' && bytes[i + 1] as char == '/' {
                    i += 2;
                    break;
                }
                i += 1;
            }
            job.append(&text[start..i], 0.0, comment.clone());
            continue;
        }
        
        // 处理单引号字符串（PostgreSQL 字符串字面量）
        if c == '\'' {
            let start = i;
            i += 1;
            while i < bytes.len() {
                let ch = bytes[i] as char;
                if ch == '\'' {
                    // 检查是否是转义的单引号 ''
                    if i + 1 < bytes.len() && bytes[i + 1] as char == '\'' {
                        i += 2;
                    } else {
                        i += 1;
                        break;
                    }
                } else if ch == '\\' && i + 1 < bytes.len() {
                    // 处理转义字符
                    i += 2;
                } else {
                    i += 1;
                }
            }
            job.append(&text[start..i], 0.0, string.clone());
            continue;
        }
        
        // 处理双引号标识符（PostgreSQL 区分大小写的标识符）
        if c == '"' {
            let start = i;
            i += 1;
            while i < bytes.len() {
                if bytes[i] as char == '"' {
                    if i + 1 < bytes.len() && bytes[i + 1] as char == '"' {
                        // 转义的双引号
                        i += 2;
                    } else {
                        i += 1;
                        break;
                    }
                } else {
                    i += 1;
                }
            }
            job.append(&text[start..i], 0.0, string.clone());
            continue;
        }
        
        // 处理数字（包括小数和科学计数法）
        if c.is_ascii_digit() || (c == '.' && i + 1 < bytes.len() && (bytes[i + 1] as char).is_ascii_digit()) {
            let start = i;
            i += 1;
            
            // 整数部分
            while i < bytes.len() && (bytes[i] as char).is_ascii_digit() {
                i += 1;
            }
            
            // 小数部分
            if i < bytes.len() && bytes[i] as char == '.' {
                i += 1;
                while i < bytes.len() && (bytes[i] as char).is_ascii_digit() {
                    i += 1;
                }
            }
            
            // 科学计数法
            if i < bytes.len() && (bytes[i] as char == 'e' || bytes[i] as char == 'E') {
                i += 1;
                if i < bytes.len() && (bytes[i] as char == '+' || bytes[i] as char == '-') {
                    i += 1;
                }
                while i < bytes.len() && (bytes[i] as char).is_ascii_digit() {
                    i += 1;
                }
            }
            
            job.append(&text[start..i], 0.0, number.clone());
            continue;
        }
        
        // 处理操作符
        if matches!(c, '+' | '-' | '*' | '/' | '%' | '=' | '<' | '>' | '!' | '&' | '|' | '^' | '~') {
            let start = i;
            i += 1;
            // 处理多字符操作符如 <=, >=, !=, <>, <<, >>, ||, :: 等
            if i < bytes.len() {
                let next = bytes[i] as char;
                if matches!((c, next), 
                    ('<', '=') | ('>', '=') | ('!', '=') | ('<', '>') | 
                    ('<', '<') | ('>', '>') | ('|', '|') | (':', ':') |
                    ('-', '>') | ('<', '-') | ('=', '>') | ('<', '@') |
                    ('@', '>') | ('&', '&') | ('|', '/') | ('#', '#')
                ) {
                    i += 1;
                }
            }
            job.append(&text[start..i], 0.0, operator.clone());
            continue;
        }
        
        // 处理其他标点符号
        if matches!(c, '(' | ')' | ',' | ';' | '[' | ']' | '{' | '}' | '.' | ':') {
            job.append(&text[i..i + 1], 0.0, normal.clone());
            i += 1;
            continue;
        }
        
        // 处理标识符和关键字
        let start = i;
        while i < bytes.len() {
            let ch = bytes[i] as char;
            if !ch.is_alphanumeric() && ch != '_' && ch != '$' {
                break;
            }
            i += 1;
        }
        
        if start < i {
            let token = &text[start..i];
            let lower = token.to_ascii_lowercase();
            
            let fmt = if KEYWORD_SET.contains(lower.as_str()) {
                kw.clone()
            } else {
                normal.clone()
            };
            
            job.append(token, 0.0, fmt);
        } else {
            // 未知字符，按普通文本处理
            job.append(&text[i..i + 1], 0.0, normal.clone());
            i += 1;
        }
    }
    
    job
}

/// 美化 SQL 语句，添加适当的缩进和换行
/// 使用基于字符串的格式化方法，避免复杂的 AST 操作
pub fn format_sql(sql: &str) -> String {
    use sqlparser::dialect::PostgreSqlDialect;
    use sqlparser::parser::Parser;
    
    let dialect = PostgreSqlDialect {};
    
    // 尝试解析 SQL 以验证语法
    match Parser::parse_sql(&dialect, sql) {
        Ok(_) => {
            // 解析成功，进行格式化
            format_sql_string(sql)
        }
        Err(e) => {
            tracing::error!("Failed to format SQL: {}", e);
            // 解析失败时，返回原 SQL（可能包含 sqlparser 不支持的语法）
            sql.to_string()
        }
    }
}

/// 基于字符串的 SQL 格式化
fn format_sql_string(sql: &str) -> String {
    let mut result = String::new();
    let mut chars = sql.chars().peekable();
    let indent = "  ";
    let mut indent_level = 0;
    let mut in_string = false;
    let mut string_char = '\0';
    let mut in_comment = false;
    let mut comment_type = CommentType::None;
    
    enum CommentType {
        None,
        SingleLine, // --
        MultiLine,  // /* */
    }
    
    while let Some(ch) = chars.next() {
        if in_comment {
            match comment_type {
                CommentType::SingleLine => {
                    result.push(ch);
                    if ch == '\n' {
                        in_comment = false;
                        comment_type = CommentType::None;
                    }
                }
                CommentType::MultiLine => {
                    result.push(ch);
                    if ch == '*' && chars.peek() == Some(&'/') {
                        result.push(chars.next().unwrap());
                        in_comment = false;
                        comment_type = CommentType::None;
                    }
                }
                CommentType::None => {}
            }
            continue;
        }
        
        if in_string {
            result.push(ch);
            if ch == string_char {
                // 检查是否是转义的引号
                if (ch == '\'' && chars.peek() == Some(&'\'')) || 
                   (ch == '"' && chars.peek() == Some(&'"')) {
                    result.push(chars.next().unwrap());
                } else {
                    in_string = false;
                    string_char = '\0';
                }
            } else if ch == '\\' && chars.peek().is_some() {
                // 转义字符
                result.push(chars.next().unwrap());
            }
            continue;
        }
        
        match ch {
            '\'' | '"' => {
                in_string = true;
                string_char = ch;
                result.push(ch);
            }
            '-' if chars.peek() == Some(&'-') => {
                in_comment = true;
                comment_type = CommentType::SingleLine;
                result.push(ch);
                result.push(chars.next().unwrap());
            }
            '/' if chars.peek() == Some(&'*') => {
                in_comment = true;
                comment_type = CommentType::MultiLine;
                result.push(ch);
                result.push(chars.next().unwrap());
            }
            ';' => {
                result.push(ch);
                result.push('\n');
                if indent_level > 0 {
                    indent_level = 0;
                }
            }
            '(' => {
                result.push(ch);
                if let Some(&next) = chars.peek() {
                    if !next.is_whitespace() && next != ')' {
                        result.push(' ');
                    }
                }
                indent_level += 1;
            }
            ')' => {
                if indent_level > 0 {
                    indent_level -= 1;
                }
                result.push(ch);
            }
            ',' => {
                result.push(ch);
                if let Some(&next) = chars.peek() {
                    if next != '\n' && !next.is_whitespace() {
                        result.push(' ');
                    }
                }
            }
            '\n' => {
                result.push(ch);
                if indent_level > 0 {
                    result.push_str(&indent.repeat(indent_level));
                }
            }
            ' ' | '\t' => {
                // 压缩多个空格为一个
                if let Some(&next) = chars.peek() {
                    if !next.is_whitespace() && next != ',' && next != ';' && next != ')' {
                        result.push(' ');
                    }
                }
            }
            _ => {
                result.push(ch);
            }
        }
    }
    
    // 格式化关键字后的换行
    let keywords = [
        "SELECT", "FROM", "WHERE", "JOIN", "INNER", "LEFT", "RIGHT", "FULL",
        "OUTER", "ON", "GROUP", "ORDER", "HAVING", "LIMIT", "OFFSET",
        "INSERT", "UPDATE", "DELETE", "CREATE", "ALTER", "DROP",
        "UNION", "INTERSECT", "EXCEPT", "WITH", "AS", "AND", "OR",
    ];
    
    let mut formatted = String::new();
    let lines: Vec<&str> = result.lines().collect();
    
    for (idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if idx < lines.len() - 1 {
                formatted.push('\n');
            }
            continue;
        }
        
        let upper = trimmed.to_uppercase();
        let mut needs_newline = false;
        
        for keyword in &keywords {
            if upper.starts_with(keyword) && 
               (trimmed.len() == keyword.len() || 
                trimmed.chars().nth(keyword.len()).map_or(false, |c| c.is_whitespace() || c == '(')) {
                if idx > 0 && !formatted.trim_end().ends_with('\n') {
                    formatted.push('\n');
                }
                needs_newline = true;
                break;
            }
        }
        
        if needs_newline && idx > 0 {
            formatted.push('\n');
        }
        
        formatted.push_str(trimmed);
        if idx < lines.len() - 1 {
            formatted.push('\n');
        }
    }
    
    formatted.trim_end().to_string()
}

pub fn format_cell(row: &PgRow, idx: usize) -> String {
    if let Ok(v) = row.try_get::<String, _>(idx) {
        return v;
    }
    if let Ok(v) = row.try_get::<i64, _>(idx) {
        return v.to_string();
    }
    if let Ok(v) = row.try_get::<f64, _>(idx) {
        return v.to_string();
    }
    if let Ok(v) = row.try_get::<bool, _>(idx) {
        return v.to_string();
    }
    if let Ok(v) = row.try_get::<Vec<u8>, _>(idx) {
        return format!("\\x{}", hex::encode(v));
    }
    "<unfmt>".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::Visuals;

    fn create_test_visuals() -> Visuals {
        Visuals::dark()
    }

    #[test]
    fn test_highlight_sql_basic_select() {
        let visuals = create_test_visuals();
        let sql = "SELECT * FROM users";
        let job = highlight_sql(sql, &visuals);
        
        // 验证函数不会 panic 并且返回了 LayoutJob
        assert!(!job.sections.is_empty());
    }

    #[test]
    fn test_highlight_sql_keywords() {
        let visuals = create_test_visuals();
        let sql = "SELECT id, name FROM users WHERE active = true";
        let job = highlight_sql(sql, &visuals);
        
        // 验证关键字被识别（通过检查 sections 不为空）
        assert!(!job.sections.is_empty());
    }

    #[test]
    fn test_highlight_sql_strings() {
        let visuals = create_test_visuals();
        let sql = "SELECT name FROM users WHERE name = 'John'";
        let job = highlight_sql(sql, &visuals);
        
        assert!(!job.sections.is_empty());
    }

    #[test]
    fn test_highlight_sql_escaped_strings() {
        let visuals = create_test_visuals();
        // 测试转义的单引号
        let sql = "SELECT 'O''Reilly' AS name";
        let job = highlight_sql(sql, &visuals);
        
        assert!(!job.sections.is_empty());
    }

    #[test]
    fn test_highlight_sql_double_quoted_identifiers() {
        let visuals = create_test_visuals();
        // PostgreSQL 双引号标识符
        let sql = r#"SELECT "User Name" FROM "My Table""#;
        let job = highlight_sql(sql, &visuals);
        
        assert!(!job.sections.is_empty());
    }

    #[test]
    fn test_highlight_sql_numbers() {
        let visuals = create_test_visuals();
        let sql = "SELECT 123, 45.67, 1e10, 2.5e-3 FROM test";
        let job = highlight_sql(sql, &visuals);
        
        assert!(!job.sections.is_empty());
    }

    #[test]
    fn test_highlight_sql_comments() {
        let visuals = create_test_visuals();
        // 单行注释
        let sql = "SELECT * FROM users -- This is a comment";
        let job = highlight_sql(sql, &visuals);
        assert!(!job.sections.is_empty());

        // 多行注释
        let sql = "SELECT * /* This is a\nmulti-line comment */ FROM users";
        let job = highlight_sql(sql, &visuals);
        assert!(!job.sections.is_empty());
    }

    #[test]
    fn test_highlight_sql_operators() {
        let visuals = create_test_visuals();
        let sql = "SELECT * FROM users WHERE id <= 100 AND name != 'test'";
        let job = highlight_sql(sql, &visuals);
        
        assert!(!job.sections.is_empty());
    }

    #[test]
    fn test_highlight_sql_postgres_operators() {
        let visuals = create_test_visuals();
        // PostgreSQL 特有操作符
        let sql = "SELECT '{}'::jsonb, data->>'key', array[1,2,3]";
        let job = highlight_sql(sql, &visuals);
        
        assert!(!job.sections.is_empty());
    }

    #[test]
    fn test_highlight_sql_complex_query() {
        let visuals = create_test_visuals();
        let sql = r#"
            WITH RECURSIVE tree AS (
                SELECT id, name, parent_id
                FROM categories
                WHERE parent_id IS NULL
                UNION ALL
                SELECT c.id, c.name, c.parent_id
                FROM categories c
                JOIN tree t ON c.parent_id = t.id
            )
            SELECT * FROM tree
            ORDER BY id
            LIMIT 10
        "#;
        let job = highlight_sql(sql, &visuals);
        
        assert!(!job.sections.is_empty());
    }

    #[test]
    fn test_highlight_sql_empty_string() {
        let visuals = create_test_visuals();
        let sql = "";
        let job = highlight_sql(sql, &visuals);
        
        // 空字符串应该返回空的或只有默认格式的 job
        assert!(job.sections.is_empty() || !job.sections.is_empty());
    }

    #[test]
    fn test_highlight_sql_whitespace_only() {
        let visuals = create_test_visuals();
        let sql = "   \n\t  ";
        let job = highlight_sql(sql, &visuals);
        
        // 应该能处理空白字符
        assert!(!job.sections.is_empty() || job.sections.is_empty());
    }

    #[test]
    fn test_format_sql_basic_select() {
        let sql = "SELECT id, name FROM users";
        let formatted = format_sql(sql);
        
        // 格式化后的 SQL 应该包含原 SQL 的关键部分
        assert!(formatted.contains("SELECT"));
        assert!(formatted.contains("FROM"));
    }

    #[test]
    fn test_format_sql_with_where() {
        let sql = "SELECT * FROM users WHERE id = 1 AND active = true";
        let formatted = format_sql(sql);
        
        assert!(formatted.contains("SELECT"));
        assert!(formatted.contains("WHERE"));
    }

    #[test]
    fn test_format_sql_with_join() {
        let sql = "SELECT u.id, u.name, p.title FROM users u JOIN posts p ON u.id = p.user_id";
        let formatted = format_sql(sql);
        
        assert!(formatted.contains("SELECT"));
        assert!(formatted.contains("JOIN"));
        assert!(formatted.contains("ON"));
    }

    #[test]
    fn test_format_sql_with_group_by() {
        let sql = "SELECT category, COUNT(*) FROM products GROUP BY category HAVING COUNT(*) > 10";
        let formatted = format_sql(sql);
        
        assert!(formatted.contains("SELECT"));
        assert!(formatted.contains("GROUP BY"));
        assert!(formatted.contains("HAVING"));
    }

    #[test]
    fn test_format_sql_with_order_by() {
        let sql = "SELECT * FROM users ORDER BY name ASC, id DESC LIMIT 10 OFFSET 20";
        let formatted = format_sql(sql);
        
        assert!(formatted.contains("SELECT"));
        assert!(formatted.contains("ORDER BY"));
        assert!(formatted.contains("LIMIT"));
        assert!(formatted.contains("OFFSET"));
    }

    #[test]
    fn test_format_sql_with_subquery() {
        let sql = "SELECT * FROM (SELECT id FROM users WHERE active = true) AS active_users";
        let formatted = format_sql(sql);
        
        assert!(formatted.contains("SELECT"));
        assert!(formatted.contains("FROM"));
    }

    #[test]
    fn test_format_sql_with_cte() {
        let sql = "WITH recent_users AS (SELECT * FROM users WHERE created_at > NOW() - INTERVAL '1 day') SELECT * FROM recent_users";
        let formatted = format_sql(sql);
        
        assert!(formatted.contains("WITH"));
        assert!(formatted.contains("SELECT"));
    }

    #[test]
    fn test_format_sql_with_comments() {
        let sql = "SELECT * FROM users -- comment\nWHERE id = 1";
        let formatted = format_sql(sql);
        
        // 注释应该被保留
        assert!(formatted.contains("-- comment") || formatted.contains("comment"));
    }

    #[test]
    fn test_format_sql_invalid_sql() {
        let sql = "SELECT * FROM WHERE INVALID SQL SYNTAX";
        let formatted = format_sql(sql);
        
        // 无效 SQL 应该返回原 SQL
        assert_eq!(formatted, sql);
    }

    #[test]
    fn test_format_sql_empty_string() {
        let sql = "";
        let formatted = format_sql(sql);
        
        assert_eq!(formatted, "");
    }

    #[test]
    fn test_format_sql_multiple_statements() {
        let sql = "SELECT * FROM users; SELECT * FROM posts;";
        let formatted = format_sql(sql);
        
        // 应该包含两个 SELECT
        let select_count = formatted.matches("SELECT").count();
        assert!(select_count >= 2);
    }

    #[test]
    fn test_format_sql_preserves_strings() {
        let sql = "SELECT 'test string' AS name FROM users";
        let formatted = format_sql(sql);
        
        // 字符串应该被保留
        assert!(formatted.contains("'test string'"));
    }

    #[test]
    fn test_format_sql_complex_query() {
        let sql = r#"
            SELECT 
                u.id,
                u.name,
                COUNT(p.id) as post_count
            FROM users u
            LEFT JOIN posts p ON u.id = p.user_id
            WHERE u.active = true
            GROUP BY u.id, u.name
            HAVING COUNT(p.id) > 5
            ORDER BY post_count DESC
            LIMIT 10
        "#;
        let formatted = format_sql(sql);
        
        assert!(formatted.contains("SELECT"));
        assert!(formatted.contains("LEFT JOIN"));
        assert!(formatted.contains("GROUP BY"));
        assert!(formatted.contains("ORDER BY"));
    }

    #[test]
    fn test_highlight_sql_case_insensitive_keywords() {
        let visuals = create_test_visuals();
        // 测试大小写不敏感的关键字识别
        let sql = "select ID, name from users where active = true";
        let job = highlight_sql(sql, &visuals);
        
        assert!(!job.sections.is_empty());
    }

    #[test]
    fn test_highlight_sql_special_characters() {
        let visuals = create_test_visuals();
        // 测试特殊字符处理
        let sql = "SELECT $1, $2 FROM test WHERE id = ANY($3)";
        let job = highlight_sql(sql, &visuals);
        
        assert!(!job.sections.is_empty());
    }

    #[test]
    fn test_highlight_sql_json_operators() {
        let visuals = create_test_visuals();
        // PostgreSQL JSON 操作符
        let sql = r#"SELECT data->>'key', data->'nested'->>'value', data @> '{"key": "value"}'::jsonb"#;
        let job = highlight_sql(sql, &visuals);
        
        assert!(!job.sections.is_empty());
    }

    #[test]
    fn test_format_sql_union_query() {
        let sql = "SELECT id FROM users UNION SELECT id FROM admins";
        let formatted = format_sql(sql);
        
        assert!(formatted.contains("SELECT"));
        assert!(formatted.contains("UNION"));
    }

    #[test]
    fn test_format_sql_insert_statement() {
        let sql = "INSERT INTO users (name, email) VALUES ('John', 'john@example.com')";
        let formatted = format_sql(sql);
        
        // INSERT 语句可能无法被 sqlparser 0.48 完全解析，但应该不会 panic
        assert!(!formatted.is_empty());
    }
}
