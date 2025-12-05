pub mod error;
pub mod store;
pub mod table;
pub mod types;

pub use error::{Result, VectorStoreError};
pub use store::ChatVectorStore;
pub use types::{ChatVectorRecord, QueryOptions, QueryResult};

/// 默认向量数据库路径
pub const VECTOR_DATABASE_PATH: &str = "vector.db";
