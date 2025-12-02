use sea_query::{Expr, Order, Query, SqliteQueryBuilder};

/// 查询 messages 表的选项
#[derive(Debug, Clone, Default)]
pub struct MessagesQueryOptions {
    /// 按 session_id 过滤
    pub session_id: Option<String>,
    /// 按 role 过滤
    pub role: Option<String>,
    /// 按 kind 过滤
    pub kind: Option<String>,
    /// 时间戳范围：起始时间
    pub timestamp_from: Option<i64>,
    /// 时间戳范围：结束时间
    pub timestamp_to: Option<i64>,
    /// 排序字段，默认为 timestamp
    pub order_by: Option<String>,
    /// 排序方向，默认为 ASC
    pub order: Option<Order>,
    /// 限制返回的记录数
    pub limit: Option<u64>,
    /// 偏移量，用于分页
    pub offset: Option<u64>,
}

/// 生成查询 messages 表的 SQL 语句
pub fn build_messages_query(options: MessagesQueryOptions) -> (String, Vec<sea_query::Value>) {
    let mut query = Query::select();
    
    // 选择所有列
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

    // 构建 WHERE 条件和参数
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

    // 应用 WHERE 条件
    if !where_parts.is_empty() {
        let where_clause = where_parts.join(" AND ");
        query.cond_where(Expr::cust(where_clause));
    }

    // 排序
    let order = options.order.unwrap_or(Order::Asc);
    let order_by_col = options.order_by.unwrap_or_else(|| "timestamp".to_string());
    query.order_by(order_by_col, order);

    // 限制和偏移
    if let Some(limit) = options.limit {
        query.limit(limit);
    }

    if let Some(offset) = options.offset {
        query.offset(offset);
    }

    // 生成 SQL
    let (sql, _values) = query.build(SqliteQueryBuilder);
    
    // 注意：由于我们使用 Expr::cust 构建 WHERE 子句，sea-query 不会自动绑定参数
    // 所以我们需要手动返回参数列表
    // 在实际使用时，需要确保 SQL 中的 ? 占位符与参数顺序匹配
    
    (sql, params)
}

/// 查询 messages 表中指定 id 和 session_id 的记录，按创建时间从近到远排序，返回前 10 条
/// 
/// # 参数
/// - `id`: message 的 id（可选，如果为 None 则不按 id 过滤）
/// - `session_id`: session 的 id（必需）
/// 
/// # 返回
/// 返回生成的 SQL 语句和参数列表
/// 
/// # 排序
/// 按 timestamp 降序排列，最新的记录在前
pub fn build_messages_query_by_id_and_session(
    id: Option<String>,
    session_id: String,
) -> (String, Vec<sea_query::Value>) {
    let mut query = Query::select();
    
    // 选择所有列
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

    // 构建 WHERE 条件和参数
    let mut where_parts = Vec::new();
    let mut params = Vec::new();

    // session_id 是必需的
    where_parts.push("session_id = ?".to_string());
    params.push(sea_query::Value::String(Some(session_id)));

    // id 是可选的
    if let Some(msg_id) = id {
        where_parts.push("id = ?".to_string());
        params.push(sea_query::Value::String(Some(msg_id)));
    }

    // 应用 WHERE 条件
    if !where_parts.is_empty() {
        let where_clause = where_parts.join(" AND ");
        query.cond_where(Expr::cust(where_clause));
    }

    // 按创建时间（timestamp）排序，从近到远（降序）
    query.order_by("timestamp", Order::Desc);

    // 限制返回 10 条数据
    query.limit(10);

    // 生成 SQL
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
        
        // 验证参数顺序：session_id 在前，id 在后
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
        let (sql, params) = build_messages_query_by_id_and_session(
            None,
            "session-789".to_string(),
        );
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
