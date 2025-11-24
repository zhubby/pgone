use sqlparser::dialect::PostgreSqlDialect;
use sqlparser::parser::Parser;
use tracing::{info, warn};

/// 解析SQL语句并输出AST日志
pub fn parse_and_log_sql(sql: &str) {
    let dialect = PostgreSqlDialect {};
    let ast_result = Parser::parse_sql(&dialect, sql);
    
    match &ast_result {
        Ok(statements) => {
            info!(
                statement_count = statements.len(),
                statements = ?statements,
                "SQL parsed successfully"
            );
            // 输出AST的调试格式用于日志
            for (idx, stmt) in statements.iter().enumerate() {
                info!(
                    statement_index = idx,
                    statement = ?stmt,
                    "AST statement"
                );
            }
        }
        Err(e) => {
            warn!(
                error = ?e,
                sql = sql,
                "Failed to parse SQL"
            );
            // 即使解析失败，也继续执行（可能是某些特殊SQL）
        }
    }
}

