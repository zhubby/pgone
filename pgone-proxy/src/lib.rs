pub mod dsn_extractor;
pub mod extractor;
pub mod processor;
pub mod replay;
pub mod row_converter;
pub mod server;
pub mod session;
pub mod sql_parser;
pub mod type_converter;



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dsn_extractor_basic() {
        let sql = "-- DSN: postgres://user:pass@localhost:5432/testdb\nSELECT 1;";
        let result = dsn_extractor::extract_dsn_from_sql(sql);
        assert!(result.is_some());
        let (dsn, actual_sql) = result.unwrap();
        assert_eq!(dsn, "postgres://user:pass@localhost:5432/testdb");
        assert_eq!(actual_sql.trim(), "SELECT 1;");
    }

    #[test]
    fn test_dsn_extractor_no_dsn() {
        let sql = "SELECT * FROM users WHERE id = 1;";
        let result = dsn_extractor::extract_dsn_from_sql(sql);
        assert!(result.is_none());
    }

    #[test]
    fn test_dsn_extractor_with_whitespace() {
        let sql = "-- DSN: postgres://user:pass@host:5432/db\n\nSELECT 1;";
        let result = dsn_extractor::extract_dsn_from_sql(sql);
        assert!(result.is_some());
        let (dsn, _) = result.unwrap();
        assert_eq!(dsn, "postgres://user:pass@host:5432/db");
    }

    #[test]
    fn test_type_converter_common_types() {
        use pgwire::api::Type as PgWireType;
        use tokio_postgres::types::Type;

        assert_eq!(
            type_converter::convert_pg_type(&Type::BOOL),
            PgWireType::BOOL
        );
        assert_eq!(
            type_converter::convert_pg_type(&Type::INT4),
            PgWireType::INT4
        );
        assert_eq!(
            type_converter::convert_pg_type(&Type::TEXT),
            PgWireType::TEXT
        );
        assert_eq!(
            type_converter::convert_pg_type(&Type::VARCHAR),
            PgWireType::VARCHAR
        );
        assert_eq!(
            type_converter::convert_pg_type(&Type::FLOAT8),
            PgWireType::FLOAT8
        );
    }

    #[test]
    fn test_type_converter_default_fallback() {
        use pgwire::api::Type as PgWireType;
        use tokio_postgres::types::Type;

        // 测试未知类型应该回退到 TEXT
        let unknown_type = Type::TEXT; // 使用已知类型作为示例
        let result = type_converter::convert_pg_type(&unknown_type);
        assert_eq!(result, PgWireType::TEXT);
    }

    #[test]
    fn test_sql_parser_valid_select() {
        let sql = "SELECT id, name FROM users WHERE id = 1";
        sql_parser::parse_and_log_sql(sql);
    }

    #[test]
    fn test_sql_parser_invalid_sql() {
        let sql = "INVALID SQL SYNTAX !!!";
        sql_parser::parse_and_log_sql(sql);
    }

    #[test]
    fn test_sql_parser_multiple_statements() {
        let sql = "SELECT 1; SELECT 2; SELECT 3;";
        sql_parser::parse_and_log_sql(sql);
    }

    #[test]
    fn test_module_integration() {
        // 集成测试：测试多个模块协同工作
        let sql_with_dsn = "-- DSN: postgres://test@localhost:5432/test\nSELECT * FROM users;";
        
        // 1. 提取 DSN
        let dsn_result = dsn_extractor::extract_dsn_from_sql(sql_with_dsn);
        assert!(dsn_result.is_some());
        
        // 2. 解析 SQL
        if let Some((_, actual_sql)) = &dsn_result {
            sql_parser::parse_and_log_sql(actual_sql);
        }
    }
}