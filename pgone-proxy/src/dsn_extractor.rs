/// 从SQL首行注释中提取DSN
/// 格式: -- DSN: postgres://user:password@host:port/database
pub fn extract_dsn_from_sql(sql: &str) -> Option<(String, String)> {
    let lines: Vec<&str> = sql.lines().collect();
    if lines.is_empty() {
        return None;
    }

    let first_line = lines[0].trim();
    if first_line.starts_with("--") {
        let comment_content = first_line.strip_prefix("--").unwrap_or("").trim();
        if let Some(dsn_part) = comment_content.strip_prefix("DSN:") {
            let dsn = dsn_part.trim().to_string();
            // 提取实际的SQL（去除首行注释）
            let actual_sql = lines[1..].join("\n").trim().to_string();
            return Some((dsn, actual_sql));
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_dsn() {
        let sql = "-- DSN: postgres://user:pass@localhost:5432/db\nSELECT * FROM users;";
        let result = extract_dsn_from_sql(sql);
        assert!(result.is_some());
        let (dsn, actual_sql) = result.unwrap();
        assert_eq!(dsn, "postgres://user:pass@localhost:5432/db");
        assert_eq!(actual_sql, "SELECT * FROM users;");
    }

    #[test]
    fn test_no_dsn() {
        let sql = "SELECT * FROM users;";
        let result = extract_dsn_from_sql(sql);
        assert!(result.is_none());
    }
}

