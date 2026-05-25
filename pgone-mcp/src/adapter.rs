use crate::core::models::{
    Column, DatabaseSchema, ForeignKey, Index, IntrospectOptions, PrimaryKey, RoutineDetail,
    RoutineKind, TableDetail, TriggerDetail, TypeDetail, TypeKind, ViewDetail,
};
use pgone_sql::Session;
use pgone_sql::models::{
    ColumnDetail as SqlColumnDetail, ForeignKeyDetail as SqlForeignKeyDetail,
    FunctionInfo as SqlFunctionInfo, IndexInfo as SqlIndexInfo,
    PrimaryKeyDetail as SqlPrimaryKeyDetail, TableDetail as SqlTableDetail,
    TriggerInfo as SqlTriggerInfo, ViewInfo as SqlViewInfo,
};

/// 将 pgone-sql 的模型转换为 core::models
pub struct ModelAdapter;

impl ModelAdapter {
    /// 将 SqlTableDetail 转换为 TableDetail
    pub fn table_detail(sql: SqlTableDetail, indexes: Vec<SqlIndexInfo>) -> TableDetail {
        TableDetail {
            schema: sql.schema,
            name: sql.name,
            comment: sql.comment,
            columns: sql.columns.into_iter().map(Self::column).collect(),
            primary_key: sql.primary_key.map(Self::primary_key),
            foreign_keys: sql
                .foreign_keys
                .into_iter()
                .map(Self::foreign_key)
                .collect(),
            indexes: indexes.into_iter().map(Self::index).collect(),
        }
    }

    /// 将 SqlColumnDetail 转换为 Column
    pub fn column(sql: SqlColumnDetail) -> Column {
        Column {
            name: sql.name,
            data_type: sql.data_type,
            udt_name: sql.udt_name,
            nullable: sql.nullable,
            default: sql.default,
            character_maximum_length: sql.character_maximum_length,
            numeric_precision: sql.numeric_precision,
            numeric_scale: sql.numeric_scale,
            comment: sql.comment,
        }
    }

    /// 将 SqlPrimaryKeyDetail 转换为 PrimaryKey
    pub fn primary_key(sql: SqlPrimaryKeyDetail) -> PrimaryKey {
        PrimaryKey {
            columns: sql.columns,
        }
    }

    /// 将 SqlForeignKeyDetail 转换为 ForeignKey
    pub fn foreign_key(sql: SqlForeignKeyDetail) -> ForeignKey {
        ForeignKey {
            columns: sql.columns,
            ref_table: sql.ref_table,
            ref_columns: sql.ref_columns,
            on_update: sql.on_update,
            on_delete: sql.on_delete,
        }
    }

    /// 将 SqlIndexInfo 转换为 Index
    pub fn index(sql: SqlIndexInfo) -> Index {
        Index {
            name: sql.name,
            unique: sql.unique,
            columns: sql.columns,
            include: Vec::new(), // pgone-sql 的 IndexInfo 没有 include 字段
            definition: sql.definition,
        }
    }

    /// 将 SqlViewInfo 转换为 ViewDetail
    pub fn view_detail(sql: SqlViewInfo) -> ViewDetail {
        ViewDetail {
            schema: sql.schema,
            name: sql.name,
            definition: sql.definition,
            comment: sql.description,
        }
    }

    /// 将 SqlTriggerInfo 转换为 TriggerDetail
    pub fn trigger_detail(sql: SqlTriggerInfo) -> TriggerDetail {
        TriggerDetail {
            schema: sql.schema,
            name: sql.name,
            table_schema: sql.table_schema,
            table_name: sql.table_name,
            timing: sql.timing,
            events: sql.events,
            function_name: sql.function_name,
        }
    }

    /// 将 SqlFunctionInfo 转换为 RoutineDetail
    /// 注意：pgone-sql 的 FunctionInfo 不包含参数信息，需要单独查询
    pub fn routine_detail(sql: SqlFunctionInfo) -> RoutineDetail {
        RoutineDetail {
            schema: sql.schema,
            name: sql.name,
            kind: RoutineKind::Function, // 默认，需要根据实际情况判断
            language: sql.language,
            return_type: sql.return_type,
            params: Vec::new(), // 需要单独查询参数
            definition: sql.definition,
            comment: sql.description,
        }
    }
}

/// 使用 pgone-sql::Session 实现数据库自省
pub struct SqlSessionIntrospector {
    session: Session,
}

impl SqlSessionIntrospector {
    pub fn new(session: Session) -> Self {
        Self { session }
    }

    /// 自省整个数据库
    pub async fn introspect_database(
        &self,
        opts: IntrospectOptions,
    ) -> anyhow::Result<DatabaseSchema> {
        let db_name = self.session.current_database().await?;

        let schemas_to_query = if let Some(ref schemas) = opts.schemas {
            schemas.clone()
        } else {
            // 获取所有 schema
            let all_tables = self.session.list_tables(None).await?;
            let mut schema_set = std::collections::BTreeSet::new();
            for table in all_tables {
                schema_set.insert(table.schema);
            }
            schema_set.into_iter().collect()
        };

        let mut schemas_vec = Vec::new();
        for schema_name in schemas_to_query {
            let tables = if opts.with_indexes {
                // 需要获取详细的表信息（包含索引）
                let table_infos = self.session.list_tables(Some(&schema_name)).await?;
                let mut table_details = Vec::new();
                for table_info in table_infos {
                    let detail = self
                        .session
                        .get_table_detail(&table_info.schema, &table_info.name)
                        .await?;
                    let indexes = self
                        .session
                        .list_table_indexes(&table_info.schema, &table_info.name)
                        .await
                        .unwrap_or_default();
                    table_details.push(ModelAdapter::table_detail(detail, indexes));
                }
                table_details
            } else {
                let table_infos = self.session.list_tables(Some(&schema_name)).await?;
                let mut table_details = Vec::new();
                for table_info in table_infos {
                    let detail = self
                        .session
                        .get_table_detail(&table_info.schema, &table_info.name)
                        .await?;
                    table_details.push(ModelAdapter::table_detail(detail, Vec::new()));
                }
                table_details
            };

            let views = self
                .session
                .list_views(Some(&schema_name))
                .await
                .unwrap_or_default();

            schemas_vec.push(crate::core::models::Schema {
                name: schema_name,
                tables,
                views: views.into_iter().map(ModelAdapter::view_detail).collect(),
            });
        }

        Ok(DatabaseSchema {
            database: db_name,
            schemas: schemas_vec,
        })
    }

    /// 列出表
    pub async fn list_tables(&self, schema: Option<&str>) -> anyhow::Result<Vec<(String, String)>> {
        let tables = self.session.list_tables(schema).await?;
        Ok(tables.into_iter().map(|t| (t.schema, t.name)).collect())
    }

    /// 获取表详情
    pub async fn get_table(&self, schema: &str, table: &str) -> anyhow::Result<TableDetail> {
        let detail = self.session.get_table_detail(schema, table).await?;
        let indexes = self
            .session
            .list_table_indexes(schema, table)
            .await
            .unwrap_or_default();
        Ok(ModelAdapter::table_detail(detail, indexes))
    }

    /// 列出视图
    pub async fn list_views(&self, schema: Option<&str>) -> anyhow::Result<Vec<ViewDetail>> {
        let views = self.session.list_views(schema).await?;
        Ok(views.into_iter().map(ModelAdapter::view_detail).collect())
    }

    /// 列出触发器
    pub async fn list_triggers(&self, schema: Option<&str>) -> anyhow::Result<Vec<TriggerDetail>> {
        let triggers = self.session.list_triggers(schema).await?;
        Ok(triggers
            .into_iter()
            .map(ModelAdapter::trigger_detail)
            .collect())
    }

    /// 列出例程（函数/过程）
    /// 注意：pgone-sql 只有 list_functions，需要适配为 routines
    pub async fn list_routines(
        &self,
        schema: Option<&str>,
        kind: Option<RoutineKind>,
    ) -> anyhow::Result<Vec<RoutineDetail>> {
        let functions = self.session.list_functions(schema).await?;
        let routines: Vec<RoutineDetail> = functions
            .into_iter()
            .map(ModelAdapter::routine_detail)
            .filter(|r| {
                if let Some(ref k) = kind {
                    r.kind == *k
                } else {
                    true
                }
            })
            .collect();
        Ok(routines)
    }

    /// 列出类型
    /// 注意：pgone-sql 可能没有直接的类型查询方法，需要实现
    pub async fn list_types(
        &self,
        _schema: Option<&str>,
        _kind: Option<TypeKind>,
    ) -> anyhow::Result<Vec<TypeDetail>> {
        // TODO: 实现类型查询
        // 这可能需要直接使用 SQL 查询 pg_type 等系统表
        Ok(Vec::new())
    }
}
