use thiserror::Error;

/// 向量存储错误类型
#[derive(Debug, Error)]
pub enum VectorStoreError {
    #[error("数据库连接错误: {0}")]
    Connection(String),

    #[error("表操作错误: {0}")]
    TableOperation(String),

    #[error("向量维度不匹配: 期望 {expected}, 实际 {actual}")]
    DimensionMismatch { expected: usize, actual: usize },

    #[error("查询错误: {0}")]
    Query(String),

    #[error("序列化错误: {0}")]
    Serialization(String),

    #[error("记录未找到: {0}")]
    NotFound(String),

    #[error("IO错误: {0}")]
    Io(#[from] std::io::Error),
}

/// 结果类型别名
pub type Result<T> = std::result::Result<T, VectorStoreError>;
