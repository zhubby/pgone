use crate::error::{Result, VectorStoreError};
use arrow::array::{ArrayRef, FixedSizeListArray, Float32Array, Int64Array, StringArray};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use lancedb::Connection;
use lancedb::table::Table;
use std::sync::Arc;

const TABLE_NAME: &str = "chat_vectors";

/// 创建或获取表结构
pub async fn create_or_open_table(db: &Connection, vector_dimension: usize) -> Result<Table> {
    // 尝试打开已存在的表
    if let Ok(table) = db.open_table(TABLE_NAME).execute().await {
        return Ok(table);
    }

    // 创建新表
    let table = db
        .create_empty_table(TABLE_NAME, Arc::new(create_schema(vector_dimension)?))
        .execute()
        .await
        .map_err(|e| VectorStoreError::TableOperation(format!("创建表失败: {}", e)))?;

    Ok(table)
}

/// 创建表结构
pub(crate) fn create_schema(vector_dimension: usize) -> Result<Schema> {
    let vector_dimension = i32::try_from(vector_dimension)
        .map_err(|_| VectorStoreError::Serialization("向量维度超过 i32 范围".to_string()))?;

    let fields = vec![
        Field::new("id", DataType::Utf8, false),
        Field::new("session_id", DataType::Utf8, false),
        Field::new("role", DataType::Utf8, false),
        Field::new("content", DataType::Utf8, false),
        Field::new(
            "vector",
            DataType::FixedSizeList(
                Arc::new(Field::new("item", DataType::Float32, true)),
                vector_dimension,
            ),
            false,
        ),
        Field::new("timestamp", DataType::Int64, false),
        Field::new("embedding_model", DataType::Utf8, true),
    ];

    Ok(Schema::new(fields))
}

/// 将 ChatVectorRecord 转换为 RecordBatch
pub fn records_to_batch(
    records: &[crate::types::ChatVectorRecord],
    vector_dimension: usize,
) -> Result<RecordBatch> {
    if records.is_empty() {
        return Err(VectorStoreError::TableOperation(
            "记录列表不能为空".to_string(),
        ));
    }

    // 验证所有向量的维度
    for record in records {
        if record.vector.len() != vector_dimension {
            return Err(VectorStoreError::DimensionMismatch {
                expected: vector_dimension,
                actual: record.vector.len(),
            });
        }
    }

    let ids: Vec<String> = records.iter().map(|r| r.id.clone()).collect();
    let session_ids: Vec<String> = records.iter().map(|r| r.session_id.clone()).collect();
    let roles: Vec<String> = records.iter().map(|r| format!("{:?}", r.role)).collect();
    let contents: Vec<String> = records.iter().map(|r| r.content.clone()).collect();
    let timestamps: Vec<i64> = records.iter().map(|r| r.timestamp).collect();
    let embedding_models: Vec<Option<String>> =
        records.iter().map(|r| r.embedding_model.clone()).collect();

    // 构建向量数组
    let mut vector_values = Vec::new();
    for record in records {
        vector_values.extend_from_slice(&record.vector);
    }

    let id_array = StringArray::from(ids);
    let session_id_array = StringArray::from(session_ids);
    let role_array = StringArray::from(roles);
    let content_array = StringArray::from(contents);
    let timestamp_array = Int64Array::from(timestamps);
    let embedding_model_array = StringArray::from(
        embedding_models
            .iter()
            .map(|opt| opt.as_ref().map(|s| s.as_str()))
            .collect::<Vec<_>>(),
    );

    // 创建固定大小的向量数组
    let float32_values = Float32Array::from(vector_values);
    let vector_array = FixedSizeListArray::try_new(
        Arc::new(Field::new("item", DataType::Float32, true)),
        vector_dimension as i32,
        Arc::new(float32_values) as ArrayRef,
        None,
    )
    .map_err(|e| VectorStoreError::Serialization(format!("创建向量数组失败: {}", e)))?;

    let schema = create_schema(vector_dimension)?;
    let batch = RecordBatch::try_new(
        Arc::new(schema),
        vec![
            Arc::new(id_array),
            Arc::new(session_id_array),
            Arc::new(role_array),
            Arc::new(content_array),
            Arc::new(vector_array),
            Arc::new(timestamp_array),
            Arc::new(embedding_model_array),
        ],
    )
    .map_err(|e| VectorStoreError::Serialization(format!("创建 RecordBatch 失败: {}", e)))?;

    Ok(batch)
}
