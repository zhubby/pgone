use pgone_storage::models::Role;
use serde::{Deserialize, Serialize};

/// 聊天向量记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatVectorRecord {
    /// 消息ID
    pub id: String,
    /// 会话ID
    pub session_id: String,
    /// 角色
    pub role: Role,
    /// 消息内容
    pub content: String,
    /// 向量数据
    pub vector: Vec<f32>,
    /// 时间戳
    pub timestamp: i64,
    /// 使用的 embedding 模型名称（可选）
    pub embedding_model: Option<String>,
}

/// 查询选项
#[derive(Debug, Clone, Default)]
pub struct QueryOptions {
    /// 返回结果数量限制
    pub limit: Option<usize>,
    /// 可选的会话ID过滤
    pub session_id: Option<String>,
    /// 可选的角色过滤
    pub role: Option<Role>,
    /// 可选的最小时间戳
    pub min_timestamp: Option<i64>,
    /// 可选的最大时间戳
    pub max_timestamp: Option<i64>,
}

impl QueryOptions {
    /// 创建默认查询选项
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置结果数量限制
    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    /// 设置会话ID过滤
    pub fn with_session_id(mut self, session_id: String) -> Self {
        self.session_id = Some(session_id);
        self
    }

    /// 设置角色过滤
    pub fn with_role(mut self, role: Role) -> Self {
        self.role = Some(role);
        self
    }

    /// 设置时间范围过滤
    pub fn with_time_range(mut self, min: Option<i64>, max: Option<i64>) -> Self {
        self.min_timestamp = min;
        self.max_timestamp = max;
        self
    }
}

/// 查询结果
#[derive(Debug, Clone)]
pub struct QueryResult {
    /// 向量记录
    pub record: ChatVectorRecord,
    /// 相似度距离（越小越相似）
    pub distance: f32,
}
