use std::sync::Arc;

use pgwire::api::results::{DataRowEncoder, FieldInfo};
use pgwire::error::{ErrorInfo, PgWireError};
use tokio_postgres::Row;

/// 将tokio-postgres的Row转换为pgwire的DataRowEncoder
pub fn convert_row_to_data_row(
    row: Row,
    schema: &Arc<Vec<FieldInfo>>,
) -> Result<DataRowEncoder, PgWireError> {
    let mut encoder = DataRowEncoder::new(schema.clone());
    for (i, _field) in schema.iter().enumerate() {
        // 直接从row获取类型信息
        let column = row.columns().get(i).ok_or_else(|| {
            PgWireError::UserError(Box::new(ErrorInfo::new(
                "ERROR".to_owned(),
                "22023".to_owned(),
                format!("Column index {} out of bounds", i),
            )))
        })?;
        let pg_type = column.type_();

        match *pg_type {
            tokio_postgres::types::Type::BOOL => {
                let val: Option<bool> = row.get(i);
                encoder.encode_field(&val)?;
            }
            tokio_postgres::types::Type::INT2 => {
                let val: Option<i16> = row.get(i);
                encoder.encode_field(&val)?;
            }
            tokio_postgres::types::Type::INT4 => {
                let val: Option<i32> = row.get(i);
                encoder.encode_field(&val)?;
            }
            tokio_postgres::types::Type::INT8 => {
                let val: Option<i64> = row.get(i);
                encoder.encode_field(&val)?;
            }
            tokio_postgres::types::Type::FLOAT4 => {
                let val: Option<f32> = row.get(i);
                encoder.encode_field(&val)?;
            }
            tokio_postgres::types::Type::FLOAT8 => {
                let val: Option<f64> = row.get(i);
                encoder.encode_field(&val)?;
            }
            tokio_postgres::types::Type::TEXT | tokio_postgres::types::Type::VARCHAR => {
                let val: Option<String> = row.get(i);
                encoder.encode_field(&val)?;
            }
            _ => {
                // 对于其他类型，尝试作为字符串处理
                let val: Option<String> = row.get(i);
                encoder.encode_field(&val)?;
            }
        }
    }
    Ok(encoder)
}
