use sea_query::{Expr, Order, Query, SqliteQueryBuilder};

/// Options for querying the messages table
#[derive(Debug, Clone, Default)]
pub struct MessagesQueryOptions {
    /// Filter by session_id
    pub session_id: Option<String>,
    /// Filter by role
    pub role: Option<String>,
    /// Filter by kind
    pub kind: Option<String>,
    /// Timestamp range: start time
    pub timestamp_from: Option<i64>,
    /// Timestamp range: end time
    pub timestamp_to: Option<i64>,
    /// Order by field, defaults to timestamp
    pub order_by: Option<String>,
    /// Order direction, defaults to ASC
    pub order: Option<Order>,
    /// Limit the number of returned records
    pub limit: Option<u64>,
    /// Offset for pagination
    pub offset: Option<u64>,
}

/// Build SQL query for the messages table
pub fn build_messages_query(options: MessagesQueryOptions) -> (String, Vec<sea_query::Value>) {
    let mut query = Query::select();

    // Select all columns
    query
        .column("id")
        .column("session_id")
        .column("role")
        .column("timestamp")
        .column("kind")
        .column("content_markdown")
        .column("image_path")
        .column("image_w")
        .column("image_h")
        .column("video_path")
        .column("video_duration_ms")
        .from("messages");

    // Build WHERE conditions and parameters
    let mut where_parts = Vec::new();
    let mut params = Vec::new();

    if let Some(session_id) = &options.session_id {
        where_parts.push("session_id = ?".to_string());
        params.push(sea_query::Value::String(Some(session_id.clone())));
    }

    if let Some(role) = &options.role {
        where_parts.push("role = ?".to_string());
        params.push(sea_query::Value::String(Some(role.clone())));
    }

    if let Some(kind) = &options.kind {
        where_parts.push("kind = ?".to_string());
        params.push(sea_query::Value::String(Some(kind.clone())));
    }

    if let Some(from) = options.timestamp_from {
        where_parts.push("timestamp >= ?".to_string());
        params.push(sea_query::Value::BigInt(Some(from)));
    }

    if let Some(to) = options.timestamp_to {
        where_parts.push("timestamp <= ?".to_string());
        params.push(sea_query::Value::BigInt(Some(to)));
    }

    // Apply WHERE conditions
    if !where_parts.is_empty() {
        let where_clause = where_parts.join(" AND ");
        query.cond_where(Expr::cust(where_clause));
    }

    // Ordering
    let order = options.order.unwrap_or(Order::Asc);
    let order_by_col = options.order_by.unwrap_or_else(|| "timestamp".to_string());
    query.order_by(order_by_col, order);

    // Limit and offset
    if let Some(limit) = options.limit {
        query.limit(limit);
    }

    if let Some(offset) = options.offset {
        query.offset(offset);
    }

    // Generate SQL
    let (sql, _values) = query.build(SqliteQueryBuilder);

    // Note: Since we use Expr::cust to build the WHERE clause, sea-query does not
    // automatically bind parameters, so we return the parameter list manually.
    // Ensure the ? placeholders in the SQL match the parameter order at call sites.

    (sql, params)
}

/// Query messages by id and session_id, ordered by creation time (newest first), returning up to 10 rows.
///
/// # Parameters
/// - `id`: message id (optional; if None, no id filter is applied)
/// - `session_id`: session id (required)
///
/// # Returns
/// The generated SQL statement and parameter list.
///
/// # Ordering
/// Ordered by timestamp descending (newest first).
pub fn build_messages_query_by_id_and_session(
    id: Option<String>,
    session_id: String,
) -> (String, Vec<sea_query::Value>) {
    let mut query = Query::select();

    // Select all columns
    query
        .column("id")
        .column("session_id")
        .column("role")
        .column("timestamp")
        .column("kind")
        .column("content_markdown")
        .column("image_path")
        .column("image_w")
        .column("image_h")
        .column("video_path")
        .column("video_duration_ms")
        .from("messages");

    // Build WHERE conditions and parameters
    let mut where_parts = Vec::new();
    let mut params = Vec::new();

    // session_id is required
    where_parts.push("session_id = ?".to_string());
    params.push(sea_query::Value::String(Some(session_id)));

    // id is optional
    if let Some(msg_id) = id {
        where_parts.push("id = ?".to_string());
        params.push(sea_query::Value::String(Some(msg_id)));
    }

    // Apply WHERE conditions
    if !where_parts.is_empty() {
        let where_clause = where_parts.join(" AND ");
        query.cond_where(Expr::cust(where_clause));
    }

    // Order by creation time (timestamp), newest first (descending)
    query.order_by("timestamp", Order::Desc);

    // Limit to 10 rows
    query.limit(10);

    // Generate SQL
    let (sql, _values) = query.build(SqliteQueryBuilder);

    (sql, params)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_query() {
        let options = MessagesQueryOptions::default();
        let (sql, params) = build_messages_query(options);
        assert!(sql.contains("SELECT"));
        assert!(sql.contains("messages"));
        assert_eq!(params.len(), 0);
    }

    #[test]
    fn test_query_with_session_id() {
        let options = MessagesQueryOptions {
            session_id: Some("test-session".to_string()),
            ..Default::default()
        };
        let (sql, params) = build_messages_query(options);
        assert!(sql.contains("session_id"));
        assert_eq!(params.len(), 1);
        match &params[0] {
            sea_query::Value::String(Some(s)) => {
                assert_eq!(s, "test-session");
            }
            _ => {
                panic!("Expected string value");
            }
        }
    }

    #[test]
    fn test_query_with_filters() {
        let options = MessagesQueryOptions {
            session_id: Some("session-1".to_string()),
            role: Some("User".to_string()),
            kind: Some("Markdown".to_string()),
            limit: Some(10),
            ..Default::default()
        };
        let (sql, params) = build_messages_query(options);
        assert!(sql.contains("WHERE"));
        assert!(sql.contains("LIMIT"));
        assert_eq!(params.len(), 3);
    }

    #[test]
    fn test_query_by_id_and_session() {
        let (sql, params) = build_messages_query_by_id_and_session(
            Some("msg-123".to_string()),
            "session-456".to_string(),
        );
        assert!(sql.contains("SELECT"));
        assert!(sql.contains("messages"));
        assert!(sql.contains("session_id"));
        assert!(sql.contains("id"));
        assert!(sql.contains("timestamp"));
        assert!(sql.contains("LIMIT"));
        assert_eq!(params.len(), 2);

        // Verify parameter order: session_id first, id second
        match &params[0] {
            sea_query::Value::String(Some(s)) => {
                assert_eq!(s, "session-456");
            }
            _ => panic!("Expected session_id string"),
        }
        match &params[1] {
            sea_query::Value::String(Some(s)) => {
                assert_eq!(s, "msg-123");
            }
            _ => panic!("Expected id string"),
        }
    }

    #[test]
    fn test_query_by_session_only() {
        let (sql, params) = build_messages_query_by_id_and_session(None, "session-789".to_string());
        assert!(sql.contains("SELECT"));
        assert!(sql.contains("messages"));
        assert!(sql.contains("session_id"));
        assert!(sql.contains("LIMIT"));
        assert_eq!(params.len(), 1);

        match &params[0] {
            sea_query::Value::String(Some(s)) => {
                assert_eq!(s, "session-789");
            }
            _ => panic!("Expected session_id string"),
        }
    }
}
