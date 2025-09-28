use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseSchema {
    pub database: String,
    pub schemas: Vec<Schema>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schema {
    pub name: String,
    pub tables: Vec<TableDetail>,
    pub views: Vec<ViewDetail>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableDetail {
    pub schema: String,
    pub name: String,
    pub comment: Option<String>,
    pub columns: Vec<Column>,
    pub primary_key: Option<PrimaryKey>,
    pub foreign_keys: Vec<ForeignKey>,
    pub indexes: Vec<Index>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Column {
    pub name: String,
    pub data_type: String,
    pub udt_name: Option<String>,
    pub nullable: bool,
    pub default: Option<String>,
    pub character_maximum_length: Option<i32>,
    pub numeric_precision: Option<i32>,
    pub numeric_scale: Option<i32>,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrimaryKey {
    pub columns: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForeignKey {
    pub columns: Vec<String>,
    pub ref_table: String,
    pub ref_columns: Vec<String>,
    pub on_update: Option<String>,
    pub on_delete: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Index {
    pub name: String,
    pub unique: bool,
    pub columns: Vec<String>,
    pub include: Vec<String>,
    pub definition: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewDetail {
    pub schema: String,
    pub name: String,
    pub definition: Option<String>,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerDetail {
    pub schema: String,
    pub name: String,
    pub table_schema: String,
    pub table_name: String,
    pub timing: String, // BEFORE | AFTER | INSTEAD OF
    pub events: Vec<String>, // INSERT/UPDATE/DELETE/TRUNCATE
    pub function_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RoutineKind { Function, Procedure, Aggregate }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutineParam {
    pub name: Option<String>,
    pub data_type: String,
    pub mode: Option<ParamMode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutineDetail {
    pub schema: String,
    pub name: String,
    pub kind: RoutineKind,
    pub language: Option<String>,
    pub return_type: Option<String>,
    pub params: Vec<RoutineParam>,
    pub definition: Option<String>,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ParamMode { In, Out, InOut, Variadic, Table }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TypeKind { Enum, Domain, Composite, Base }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeDetail {
    pub schema: String,
    pub name: String,
    pub kind: TypeKind,
    pub base_type: Option<String>,
    pub enum_labels: Option<Vec<String>>, // when enum
    pub definition: Option<String>,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntrospectOptions {
    pub schemas: Option<Vec<String>>, 
    pub with_indexes: bool,
    pub with_routines: bool,
    pub with_types: bool,
    pub with_triggers: bool,
    pub page: Option<u32>,
    pub page_size: Option<u32>,
}


