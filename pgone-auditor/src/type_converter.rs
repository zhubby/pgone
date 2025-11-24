use pgwire::api::Type as PgWireType;

/// 将tokio-postgres的Type转换为pgwire的Type
pub fn convert_pg_type(pg_type: &tokio_postgres::types::Type) -> PgWireType {
    match *pg_type {
        tokio_postgres::types::Type::BOOL => PgWireType::BOOL,
        tokio_postgres::types::Type::BYTEA => PgWireType::BYTEA,
        tokio_postgres::types::Type::CHAR => PgWireType::CHAR,
        tokio_postgres::types::Type::INT8 => PgWireType::INT8,
        tokio_postgres::types::Type::INT2 => PgWireType::INT2,
        tokio_postgres::types::Type::INT4 => PgWireType::INT4,
        tokio_postgres::types::Type::TEXT => PgWireType::TEXT,
        tokio_postgres::types::Type::VARCHAR => PgWireType::VARCHAR,
        tokio_postgres::types::Type::FLOAT4 => PgWireType::FLOAT4,
        tokio_postgres::types::Type::FLOAT8 => PgWireType::FLOAT8,
        tokio_postgres::types::Type::DATE => PgWireType::DATE,
        tokio_postgres::types::Type::TIME => PgWireType::TIME,
        tokio_postgres::types::Type::TIMESTAMP => PgWireType::TIMESTAMP,
        tokio_postgres::types::Type::TIMESTAMPTZ => PgWireType::TIMESTAMPTZ,
        tokio_postgres::types::Type::UUID => PgWireType::UUID,
        tokio_postgres::types::Type::JSON => PgWireType::JSON,
        tokio_postgres::types::Type::JSONB => PgWireType::JSONB,
        _ => PgWireType::TEXT, // 默认使用TEXT类型
    }
}

