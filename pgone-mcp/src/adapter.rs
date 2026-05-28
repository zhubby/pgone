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

/// Converts pgone-sql models to core::models
pub struct ModelAdapter;

impl ModelAdapter {
    /// Converts SqlTableDetail to TableDetail
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

    /// Converts SqlColumnDetail to Column
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

    /// Converts SqlPrimaryKeyDetail to PrimaryKey
    pub fn primary_key(sql: SqlPrimaryKeyDetail) -> PrimaryKey {
        PrimaryKey {
            columns: sql.columns,
        }
    }

    /// Converts SqlForeignKeyDetail to ForeignKey
    pub fn foreign_key(sql: SqlForeignKeyDetail) -> ForeignKey {
        ForeignKey {
            columns: sql.columns,
            ref_table: sql.ref_table,
            ref_columns: sql.ref_columns,
            on_update: sql.on_update,
            on_delete: sql.on_delete,
        }
    }

    /// Converts SqlIndexInfo to Index
    pub fn index(sql: SqlIndexInfo) -> Index {
        Index {
            name: sql.name,
            unique: sql.unique,
            columns: sql.columns,
            include: Vec::new(), // pgone-sql's IndexInfo has no include field
            definition: sql.definition,
        }
    }

    /// Converts SqlViewInfo to ViewDetail
    pub fn view_detail(sql: SqlViewInfo) -> ViewDetail {
        ViewDetail {
            schema: sql.schema,
            name: sql.name,
            definition: sql.definition,
            comment: sql.description,
        }
    }

    /// Converts SqlTriggerInfo to TriggerDetail
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

    /// Converts SqlFunctionInfo to RoutineDetail
    /// Note: pgone-sql's FunctionInfo does not include parameter info, requires separate query
    pub fn routine_detail(sql: SqlFunctionInfo) -> RoutineDetail {
        RoutineDetail {
            schema: sql.schema,
            name: sql.name,
            kind: RoutineKind::Function, // Default, needs to be determined based on actual situation
            language: sql.language,
            return_type: sql.return_type,
            params: Vec::new(), // Parameters need to be queried separately
            definition: sql.definition,
            comment: sql.description,
        }
    }
}

/// Database introspector implementation using pgone-sql::Session
pub struct SqlSessionIntrospector {
    session: Session,
}

impl SqlSessionIntrospector {
    pub fn new(session: Session) -> Self {
        Self { session }
    }

    /// Introspects the entire database
    pub async fn introspect_database(
        &self,
        opts: IntrospectOptions,
    ) -> anyhow::Result<DatabaseSchema> {
        let db_name = self.session.current_database().await?;

        let schemas_to_query = if let Some(ref schemas) = opts.schemas {
            schemas.clone()
        } else {
            // Get all schemas
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
                // Need to get detailed table info (including indexes)
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

    /// Lists tables
    pub async fn list_tables(&self, schema: Option<&str>) -> anyhow::Result<Vec<(String, String)>> {
        let tables = self.session.list_tables(schema).await?;
        Ok(tables.into_iter().map(|t| (t.schema, t.name)).collect())
    }

    /// Gets table details
    pub async fn get_table(&self, schema: &str, table: &str) -> anyhow::Result<TableDetail> {
        let detail = self.session.get_table_detail(schema, table).await?;
        let indexes = self
            .session
            .list_table_indexes(schema, table)
            .await
            .unwrap_or_default();
        Ok(ModelAdapter::table_detail(detail, indexes))
    }

    /// Lists views
    pub async fn list_views(&self, schema: Option<&str>) -> anyhow::Result<Vec<ViewDetail>> {
        let views = self.session.list_views(schema).await?;
        Ok(views.into_iter().map(ModelAdapter::view_detail).collect())
    }

    /// Lists triggers
    pub async fn list_triggers(&self, schema: Option<&str>) -> anyhow::Result<Vec<TriggerDetail>> {
        let triggers = self.session.list_triggers(schema).await?;
        Ok(triggers
            .into_iter()
            .map(ModelAdapter::trigger_detail)
            .collect())
    }

    /// Lists routines (functions/procedures)
    /// Note: pgone-sql only has list_functions, needs to be adapted to routines
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

    /// Lists types
    /// Note: pgone-sql may not have a direct type query method, needs implementation
    pub async fn list_types(
        &self,
        _schema: Option<&str>,
        _kind: Option<TypeKind>,
    ) -> anyhow::Result<Vec<TypeDetail>> {
        // TODO: Implement type query
        // This may require directly querying system tables like pg_type via SQL
        Ok(Vec::new())
    }
}
