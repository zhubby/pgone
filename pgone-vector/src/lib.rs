pub mod error;
pub mod store;
pub mod table;
pub mod types;

pub use error::{Result, VectorStoreError};
pub use store::ChatVectorStore;
pub use types::{ChatVectorRecord, QueryOptions, QueryResult};

/// 默认向量数据库路径
#[deprecated(note = "use vector_database_path() for the user-local vector storage path")]
pub const VECTOR_DATABASE_PATH: &str = "vector.db";

#[must_use]
pub fn vector_database_path() -> std::path::PathBuf {
    pgone_storage::vector_database_path()
}
