use crate::error::{Result, VectorStoreError};
use crate::table::{create_or_open_table, records_to_batch};
use crate::types::{ChatVectorRecord, QueryOptions, QueryResult};
use arrow::array::{FixedSizeListArray, Int64Array, StringArray};
use arrow::record_batch::RecordBatch;
use lancedb::query::Query;
use lancedb::{Database, Table};
use pgone_storage::models::Role;
use std::path::Path;
use std::sync::Arc;

/// 聊天向量存储
pub struct ChatVectorStore {
    /// LanceDB 数据库连接
    db: Database,
    /// 向量维度
    vector_dimension: usize,
    /// 表名
    table_name: String,
}

impl ChatVectorStore {
    /// 创建新的向量存储实例
    ///
    /// # 参数
    /// - `db_path`: 数据库路径
    /// - `vector_dimension`: 向量维度
    ///
    /// # 返回
    /// 返回初始化好的 `ChatVectorStore` 实例
    pub async fn new<P: AsRef<Path>>(db_path: P, vector_dimension: usize) -> Result<Self> {
        let db_path_str = db_path.as_ref().to_string_lossy().to_string();
        let db = Database::connect(&db_path_str)
            .await
            .map_err(|e| VectorStoreError::Connection(format!("连接数据库失败: {}", e)))?;

        // 创建或打开表
        create_or_open_table(&db, vector_dimension).await?;

        Ok(Self {
            db,
            vector_dimension,
            table_name: "chat_vectors".to_string(),
        })
    }

    /// 获取表引用
    async fn get_table(&self) -> Result<Table> {
        self.db
            .open_table(&self.table_name)
            .await
            .map_err(|e| VectorStoreError::TableOperation(format!("打开表失败: {}", e)))
    }

    /// 插入单条记录
    pub async fn insert(&self, record: ChatVectorRecord) -> Result<()> {
        self.insert_batch(vec![record]).await
    }

    /// 批量插入记录
    pub async fn insert_batch(&self, records: Vec<ChatVectorRecord>) -> Result<()> {
        if records.is_empty() {
            return Ok(());
        }

        // 验证向量维度
        for record in &records {
            if record.vector.len() != self.vector_dimension {
                return Err(VectorStoreError::DimensionMismatch {
                    expected: self.vector_dimension,
                    actual: record.vector.len(),
                });
            }
        }

        let table = self.get_table().await?;
        let batch = records_to_batch(&records, self.vector_dimension)?;

        table
            .add(Arc::new(batch))
            .await
            .map_err(|e| VectorStoreError::TableOperation(format!("插入数据失败: {}", e)))?;

        Ok(())
    }

    /// 向量相似度搜索
    pub async fn search(
        &self,
        query_vector: Vec<f32>,
        options: QueryOptions,
    ) -> Result<Vec<QueryResult>> {
        // 验证查询向量维度
        if query_vector.len() != self.vector_dimension {
            return Err(VectorStoreError::DimensionMismatch {
                expected: self.vector_dimension,
                actual: query_vector.len(),
            });
        }

        let table = self.get_table().await?;

        // 构建查询
        let mut query = table
            .query()
            .nearest(query_vector.as_slice())
            .map_err(|e| VectorStoreError::Query(format!("构建查询失败: {}", e)))?;

        // 应用过滤条件
        if let Some(session_id) = &options.session_id {
            query = query
                .filter(&format!("session_id = '{}'", session_id))
                .map_err(|e| VectorStoreError::Query(format!("添加会话ID过滤失败: {}", e)))?;
        }

        if let Some(role) = &options.role {
            let role_str = format!("{:?}", role);
            query = query
                .filter(&format!("role = '{}'", role_str))
                .map_err(|e| VectorStoreError::Query(format!("添加角色过滤失败: {}", e)))?;
        }

        if let Some(min_ts) = options.min_timestamp {
            query = query
                .filter(&format!("timestamp >= {}", min_ts))
                .map_err(|e| VectorStoreError::Query(format!("添加最小时间戳过滤失败: {}", e)))?;
        }

        if let Some(max_ts) = options.max_timestamp {
            query = query
                .filter(&format!("timestamp <= {}", max_ts))
                .map_err(|e| VectorStoreError::Query(format!("添加最大时间戳过滤失败: {}", e)))?;
        }

        // 设置返回数量限制
        let limit = options.limit.unwrap_or(10);
        query = query
            .limit(limit)
            .map_err(|e| VectorStoreError::Query(format!("设置限制失败: {}", e)))?;

        // 执行查询
        let results = query
            .execute()
            .await
            .map_err(|e| VectorStoreError::Query(format!("执行查询失败: {}", e)))?;

        // 转换结果
        let mut query_results = Vec::new();
        for batch_result in results {
            let batch = batch_result
                .map_err(|e| VectorStoreError::Query(format!("读取查询结果失败: {}", e)))?;

            // 从 RecordBatch 中提取数据
            let schema = batch.schema();
            let num_rows = batch.num_rows();

            // 获取各列的索引
            let id_idx = schema.column_with_name("id")
                .map(|(idx, _)| idx)
                .ok_or_else(|| VectorStoreError::Query("缺少 id 列".to_string()))?;
            let session_id_idx = schema.column_with_name("session_id")
                .map(|(idx, _)| idx)
                .ok_or_else(|| VectorStoreError::Query("缺少 session_id 列".to_string()))?;
            let role_idx = schema.column_with_name("role")
                .map(|(idx, _)| idx)
                .ok_or_else(|| VectorStoreError::Query("缺少 role 列".to_string()))?;
            let content_idx = schema.column_with_name("content")
                .map(|(idx, _)| idx)
                .ok_or_else(|| VectorStoreError::Query("缺少 content 列".to_string()))?;
            let vector_idx = schema.column_with_name("vector")
                .map(|(idx, _)| idx)
                .ok_or_else(|| VectorStoreError::Query("缺少 vector 列".to_string()))?;
            let timestamp_idx = schema.column_with_name("timestamp")
                .map(|(idx, _)| idx)
                .ok_or_else(|| VectorStoreError::Query("缺少 timestamp 列".to_string()))?;
            let embedding_model_idx = schema.column_with_name("embedding_model")
                .map(|(idx, _)| idx);

            // 提取距离列（lancedb 查询结果中通常包含 _distance 列）
            let distance_idx = schema.column_with_name("_distance")
                .or_else(|| schema.column_with_name("distance"))
                .map(|(idx, _)| idx);

            // 解析每一行
            for row_idx in 0..num_rows {
                // 提取基本字段
                let id = extract_string_from_batch(&batch, id_idx, row_idx)?;
                let session_id = extract_string_from_batch(&batch, session_id_idx, row_idx)?;
                let role_str = extract_string_from_batch(&batch, role_idx, row_idx)?;
                let content = extract_string_from_batch(&batch, content_idx, row_idx)?;
                let timestamp = extract_i64_from_batch(&batch, timestamp_idx, row_idx)?;
                let embedding_model = embedding_model_idx
                    .and_then(|idx| extract_string_from_batch(&batch, idx, row_idx).ok());

                // 解析角色
                let role = match role_str.as_str() {
                    "User" => Role::User,
                    "Assistant" => Role::Assistant,
                    "System" => Role::System,
                    _ => return Err(VectorStoreError::Query(format!("未知角色: {}", role_str))),
                };

                // 提取向量
                let vector = extract_vector_from_batch(&batch, vector_idx, row_idx)?;

                // 提取距离
                let distance = if let Some(idx) = distance_idx {
                    extract_f32_from_batch(&batch, idx, row_idx).unwrap_or(0.0)
                } else {
                    0.0 // 如果没有距离列，使用默认值
                };

                let record = ChatVectorRecord {
                    id,
                    session_id,
                    role,
                    content,
                    vector,
                    timestamp,
                    embedding_model,
                };

                query_results.push(QueryResult { record, distance });
            }
        }

        Ok(query_results)
    }

    /// 删除指定会话的所有记录
    pub async fn delete_by_session(&self, session_id: &str) -> Result<()> {
        let table = self.get_table().await?;
        
        table
            .delete(&format!("session_id = '{}'", session_id))
            .await
            .map_err(|e| VectorStoreError::TableOperation(format!("删除会话记录失败: {}", e)))?;

        Ok(())
    }

    /// 删除指定ID的记录
    pub async fn delete_by_id(&self, id: &str) -> Result<()> {
        let table = self.get_table().await?;
        
        table
            .delete(&format!("id = '{}'", id))
            .await
            .map_err(|e| VectorStoreError::TableOperation(format!("删除记录失败: {}", e)))?;

        Ok(())
    }

    /// 获取向量维度
    pub fn vector_dimension(&self) -> usize {
        self.vector_dimension
    }
}

/// 从 RecordBatch 中提取字符串值
fn extract_string_from_batch(batch: &RecordBatch, col_idx: usize, row_idx: usize) -> Result<String> {
    let column = batch.column(col_idx);
    let string_array = column.as_any().downcast_ref::<StringArray>()
        .ok_or_else(|| VectorStoreError::Query("列类型不是字符串".to_string()))?;
    
    if row_idx >= string_array.len() {
        return Err(VectorStoreError::Query("行索引超出范围".to_string()));
    }
    
    if string_array.is_null(row_idx) {
        return Err(VectorStoreError::Query("字符串值为空".to_string()));
    }
    
    Ok(string_array.value(row_idx).to_string())
}

/// 从 RecordBatch 中提取 i64 值
fn extract_i64_from_batch(batch: &RecordBatch, col_idx: usize, row_idx: usize) -> Result<i64> {
    let column = batch.column(col_idx);
    let int_array = column.as_any().downcast_ref::<Int64Array>()
        .ok_or_else(|| VectorStoreError::Query("列类型不是 i64".to_string()))?;
    
    if row_idx >= int_array.len() {
        return Err(VectorStoreError::Query("行索引超出范围".to_string()));
    }
    
    Ok(int_array.value(row_idx))
}

/// 从 RecordBatch 中提取 f32 值
fn extract_f32_from_batch(batch: &RecordBatch, col_idx: usize, row_idx: usize) -> Result<f32> {
    use arrow::array::Float32Array;
    let column = batch.column(col_idx);
    let float_array = column.as_any().downcast_ref::<Float32Array>()
        .ok_or_else(|| VectorStoreError::Query("列类型不是 f32".to_string()))?;
    
    if row_idx >= float_array.len() {
        return Err(VectorStoreError::Query("行索引超出范围".to_string()));
    }
    
    Ok(float_array.value(row_idx))
}

/// 从 RecordBatch 中提取向量值
fn extract_vector_from_batch(batch: &RecordBatch, col_idx: usize, row_idx: usize) -> Result<Vec<f32>> {
    let column = batch.column(col_idx);
    let list_array = column.as_any().downcast_ref::<FixedSizeListArray>()
        .ok_or_else(|| VectorStoreError::Query("列类型不是固定大小列表".to_string()))?;
    
    if row_idx >= list_array.len() {
        return Err(VectorStoreError::Query("行索引超出范围".to_string()));
    }
    
    if list_array.is_null(row_idx) {
        return Err(VectorStoreError::Query("向量值为空".to_string()));
    }
    
    let start = list_array.value_offset(row_idx) as usize;
    let length = list_array.value_length() as usize;
    let values = list_array.values();
    
    let float_array = values.as_any().downcast_ref::<arrow::array::Float32Array>()
        .ok_or_else(|| VectorStoreError::Query("向量值类型不是 Float32".to_string()))?;
    
    let mut vector = Vec::with_capacity(length);
    for i in start..(start + length) {
        vector.push(float_array.value(i));
    }
    
    Ok(vector)
}

