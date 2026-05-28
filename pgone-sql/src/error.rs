use thiserror::Error;

#[derive(Debug, Error)]
pub enum SqlError {
    #[error("Database connection error: {0}")]
    Connection(#[from] sqlx::Error),

    #[error("SQL execution error: {0}")]
    Execution(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Object not found: {0}")]
    NotFound(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Transaction error: {0}")]
    Transaction(String),
}

pub type Result<T> = std::result::Result<T, SqlError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sql_error_display() {
        let err = SqlError::Execution("Test error".to_string());
        assert!(err.to_string().contains("Test error"));

        let err = SqlError::PermissionDenied("Access denied".to_string());
        assert!(err.to_string().contains("Permission denied"));
        assert!(err.to_string().contains("Access denied"));

        let err = SqlError::NotFound("Object not found".to_string());
        assert!(err.to_string().contains("Object not found"));

        let err = SqlError::InvalidInput("Invalid input".to_string());
        assert!(err.to_string().contains("Invalid input"));
    }

    #[test]
    fn test_sql_error_from_sqlx_error() {
        let _result: Result<()> = Err(SqlError::Connection(sqlx::Error::RowNotFound));
    }
}
