use crate::core::models::{
    DatabaseSchema, IntrospectOptions, RoutineDetail, RoutineKind, TableDetail, TriggerDetail,
    TypeDetail, TypeKind, ViewDetail,
};
use async_trait::async_trait;

#[async_trait]
pub trait DatabaseIntrospector: Send + Sync {
    async fn introspect_database(&self, opts: IntrospectOptions) -> anyhow::Result<DatabaseSchema>;
    async fn list_tables(&self, schema: Option<&str>) -> anyhow::Result<Vec<(String, String)>>; // (schema, table)
    async fn get_table(&self, schema: &str, table: &str) -> anyhow::Result<TableDetail>;
    async fn list_views(&self, schema: Option<&str>) -> anyhow::Result<Vec<ViewDetail>>;
    async fn list_triggers(&self, schema: Option<&str>) -> anyhow::Result<Vec<TriggerDetail>>;
    async fn list_routines(
        &self,
        schema: Option<&str>,
        kind: Option<RoutineKind>,
    ) -> anyhow::Result<Vec<RoutineDetail>>;
    async fn list_types(
        &self,
        schema: Option<&str>,
        kind: Option<TypeKind>,
    ) -> anyhow::Result<Vec<TypeDetail>>;
}
