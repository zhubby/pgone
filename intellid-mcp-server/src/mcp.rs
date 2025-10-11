use crate::adapters::postgres::PostgresIntrospector;
use crate::core::introspector::DatabaseIntrospector;
use crate::core::models::{IntrospectOptions, RoutineKind, TypeKind};
use crate::formatters::markdown;
use crate::registry::{ConnectionConfig, ConnectionRegistry, DatabaseEngine};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};

#[derive(Debug, Deserialize)]
struct RpcRequest {
    pub id: Value,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Serialize)]
struct RpcError {
    code: i32,
    message: String,
}

#[derive(Debug, Serialize)]
struct RpcResponse {
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<RpcError>,
}

fn ok(id: Value, result: Value) -> RpcResponse {
    RpcResponse {
        id,
        result: Some(result),
        error: None,
    }
}

fn err(id: Value, code: i32, message: impl Into<String>) -> RpcResponse {
    RpcResponse {
        id,
        result: None,
        error: Some(RpcError {
            code,
            message: message.into(),
        }),
    }
}

pub async fn run_stdio(registry: ConnectionRegistry) -> anyhow::Result<()> {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let mut reader = tokio::io::BufReader::new(stdin).lines();
    let mut writer = tokio::io::BufWriter::new(stdout);

    while let Some(line) = reader.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }
        let req: Result<RpcRequest, _> = serde_json::from_str(&line);
        let response = match req {
            Ok(r) => handle_request(r, &registry)
                .await
                .unwrap_or_else(|e| err(json!(null), -32000, e.to_string())),
            Err(e) => err(json!(null), -32700, format!("Parse error: {}", e)),
        };
        let text = serde_json::to_string(&response)?;
        writer.write_all(text.as_bytes()).await?;
        writer.write_all(b"\n").await?;
        writer.flush().await?;
    }

    Ok(())
}

async fn handle_request(
    req: RpcRequest,
    registry: &ConnectionRegistry,
) -> anyhow::Result<RpcResponse> {
    match req.method.as_str() {
        "register_connection" => {
            #[derive(Deserialize)]
            struct P {
                id: String,
                engine: String,
                dsn: String,
            }
            let p: P = serde_json::from_value(req.params)?;
            let engine = parse_engine(&p.engine)?;
            registry
                .register(ConnectionConfig {
                    id: p.id,
                    engine,
                    dsn: p.dsn,
                    default_schemas: None,
                    include_system: Some(false),
                })
                .await?;
            Ok(ok(req.id, json!({"ok": true})))
        }
        "list_connections" => {
            let items = registry
                .list()
                .await
                .into_iter()
                .map(|(id, engine)| json!({"id": id, "engine": engine_to_str(engine)}))
                .collect::<Vec<_>>();
            Ok(ok(req.id, json!(items)))
        }
        "remove_connection" => {
            #[derive(Deserialize)]
            struct P {
                id: String,
            }
            let p: P = serde_json::from_value(req.params)?;
            let removed = registry.remove(&p.id).await;
            Ok(ok(req.id, json!({"removed": removed})))
        }
        "health_check" => {
            #[derive(Deserialize)]
            struct P {
                id: String,
            }
            let p: P = serde_json::from_value(req.params)?;
            let Some(handle) = registry.get(&p.id).await else {
                return Ok(err(req.id, 404, "Unknown connectionId"));
            };
            let v: i32 = sqlx::query_scalar("SELECT 1")
                .fetch_one(&handle.pool)
                .await?;
            Ok(ok(req.id, json!({"ok": v == 1})))
        }
        "introspect_all" => {
            #[derive(Deserialize)]
            struct P {
                #[serde(rename = "connectionId")]
                connection_id: String,
                schemas: Option<Vec<String>>,
                #[serde(rename = "withIndexes")]
                with_indexes: Option<bool>,
                #[serde(rename = "withRoutines")]
                with_routines: Option<bool>,
                #[serde(rename = "withTypes")]
                with_types: Option<bool>,
                #[serde(rename = "withTriggers")]
                with_triggers: Option<bool>,
                page: Option<u32>,
                #[serde(rename = "pageSize")]
                page_size: Option<u32>,
                format: Option<String>,
            }
            let p: P = serde_json::from_value(req.params)?;
            let Some(handle) = registry.get(&p.connection_id).await else {
                return Ok(err(req.id, 404, "Unknown connectionId"));
            };
            match handle.engine {
                DatabaseEngine::Postgres => {
                    let pg = PostgresIntrospector::new(handle.pool.clone());
                    let want_markdown = matches!(p.format.as_deref(), Some("markdown"));

                    // 判断是否分页
                    let is_paged = p.page.is_some() || p.page_size.is_some();
                    if is_paged {
                        // 真实分页：按表分页
                        let page = p.page.unwrap_or(1).max(1);
                        let page_size = p.page_size.unwrap_or(100).max(1);
                        // 聚合待分页表列表
                        let mut pairs: Vec<(String, String)> = Vec::new();
                        if let Some(schemas) = &p.schemas {
                            for s in schemas {
                                let mut v = pg.list_tables(Some(s.as_str())).await?;
                                pairs.append(&mut v);
                            }
                        } else {
                            pairs = pg.list_tables(None).await?;
                        }
                        pairs.sort();
                        pairs.dedup();
                        let total = pairs.len() as u32;
                        let start = (page - 1) * page_size;
                        let end = (start + page_size).min(total);
                        let slice = if (start as usize) < pairs.len() {
                            &pairs[start as usize..end as usize]
                        } else {
                            &[][..]
                        };

                        use std::collections::BTreeMap;
                        let mut by_schema: BTreeMap<String, Vec<crate::core::models::TableDetail>> =
                            BTreeMap::new();
                        for (schema, table) in slice {
                            let td = pg.get_table(schema, table).await?;
                            by_schema.entry(schema.clone()).or_default().push(td);
                        }

                        // 构造 DatabaseSchema 结构（附当前 schemas 的视图）
                        let dbname: String = sqlx::query_scalar("SELECT current_database()")
                            .fetch_one(&handle.pool)
                            .await?;
                        let mut schemas_vec = Vec::new();
                        for (schema_name, tables) in by_schema {
                            let views = pg.list_views(Some(&schema_name)).await.unwrap_or_default();
                            schemas_vec.push(crate::core::models::Schema {
                                name: schema_name,
                                tables,
                                views,
                            });
                        }
                        let db = crate::core::models::DatabaseSchema {
                            database: dbname,
                            schemas: schemas_vec,
                        };

                        if want_markdown {
                            let mut md = markdown::render_overview(&db);
                            if p.with_triggers.unwrap_or(false) {
                                let tg = pg.list_triggers(None).await.unwrap_or_default();
                                if !tg.is_empty() {
                                    md.push_str("\n触发器：\n");
                                    for t in tg {
                                        md.push_str(&format!(
                                            "- {}.{} [{} {}]\n",
                                            t.schema,
                                            t.name,
                                            t.timing,
                                            t.events.join("/")
                                        ));
                                    }
                                }
                            }
                            if p.with_routines.unwrap_or(false) {
                                let rt = pg.list_routines(None, None).await.unwrap_or_default();
                                if !rt.is_empty() {
                                    md.push_str("\n例程：\n");
                                    for r in rt {
                                        md.push_str(&format!(
                                            "- {}.{} ({:?})\n",
                                            r.schema, r.name, r.kind
                                        ));
                                    }
                                }
                            }
                            if p.with_types.unwrap_or(false) {
                                let tp = pg.list_types(None, None).await.unwrap_or_default();
                                if !tp.is_empty() {
                                    md.push_str("\n类型：\n");
                                    for t in tp {
                                        md.push_str(&format!(
                                            "- {}.{} ({:?})\n",
                                            t.schema, t.name, t.kind
                                        ));
                                    }
                                }
                            }
                            return Ok(ok(
                                req.id,
                                json!({
                                    "markdown": md,
                                    "pageInfo": {"page": page, "pageSize": page_size, "total": total}
                                }),
                            ));
                        }
                        return Ok(ok(
                            req.id,
                            json!({
                                "database": db.database,
                                "schemas": db.schemas,
                                "pageInfo": {"page": page, "pageSize": page_size, "total": total}
                            }),
                        ));
                    }

                    // 非分页：尝试缓存
                    let mut key_parts = vec!["introspect_all".to_string()];
                    if let Some(s) = &p.schemas {
                        let mut s2 = s.clone();
                        s2.sort();
                        key_parts.push(format!("schemas={}", s2.join(",")));
                    }
                    key_parts.push(format!("idx={}", p.with_indexes.unwrap_or(true)));
                    key_parts.push(format!("rtn={}", p.with_routines.unwrap_or(false)));
                    key_parts.push(format!("typ={}", p.with_types.unwrap_or(false)));
                    key_parts.push(format!("trg={}", p.with_triggers.unwrap_or(false)));
                    let cache_key = key_parts.join("|");
                    if !want_markdown && let Some(v) = handle.cache.get(&cache_key).await {
                        return Ok(ok(req.id, v));
                    }

                    let out = pg
                        .introspect_database(IntrospectOptions {
                            schemas: p.schemas,
                            with_indexes: p.with_indexes.unwrap_or(true),
                            with_routines: p.with_routines.unwrap_or(false),
                            with_types: p.with_types.unwrap_or(false),
                            with_triggers: p.with_triggers.unwrap_or(false),
                            page: p.page,
                            page_size: p.page_size,
                        })
                        .await?;
                    if want_markdown {
                        let mut md = markdown::render_overview(&out);
                        if p.with_triggers.unwrap_or(false) {
                            let tg = pg.list_triggers(None).await.unwrap_or_default();
                            if !tg.is_empty() {
                                md.push_str("\n触发器：\n");
                                for t in tg {
                                    md.push_str(&format!(
                                        "- {}.{} [{} {}]\n",
                                        t.schema,
                                        t.name,
                                        t.timing,
                                        t.events.join("/")
                                    ));
                                }
                            }
                        }
                        if p.with_routines.unwrap_or(false) {
                            let rt = pg.list_routines(None, None).await.unwrap_or_default();
                            if !rt.is_empty() {
                                md.push_str("\n例程：\n");
                                for r in rt {
                                    md.push_str(&format!(
                                        "- {}.{} ({:?})\n",
                                        r.schema, r.name, r.kind
                                    ));
                                }
                            }
                        }
                        if p.with_types.unwrap_or(false) {
                            let tp = pg.list_types(None, None).await.unwrap_or_default();
                            if !tp.is_empty() {
                                md.push_str("\n类型：\n");
                                for t in tp {
                                    md.push_str(&format!(
                                        "- {}.{} ({:?})\n",
                                        t.schema, t.name, t.kind
                                    ));
                                }
                            }
                        }
                        Ok(ok(req.id, json!({"markdown": md})))
                    } else {
                        let v = serde_json::to_value(&out)?;
                        handle.cache.insert(cache_key, v.clone()).await;
                        Ok(ok(req.id, v))
                    }
                }
            }
        }
        "refresh_cache" => {
            #[derive(Deserialize)]
            struct P {
                id: String,
                scope: Option<String>,
                schema: Option<String>,
                table: Option<String>,
            }
            let p: P = serde_json::from_value(req.params)?;
            let Some(handle) = registry.get(&p.id).await else {
                return Ok(err(req.id, 404, "Unknown connectionId"));
            };
            match p.scope.as_deref() {
                Some("table") => {
                    if let (Some(s), Some(t)) = (p.schema.as_deref(), p.table.as_deref()) {
                        let key = format!("get_table|{}.{}", s, t);
                        handle.cache.invalidate(&key).await;
                    }
                }
                _ => handle.cache.invalidate_all(),
            }
            Ok(ok(req.id, json!({"refreshed": true})))
        }
        "reload_connections" => {
            #[derive(Deserialize)]
            struct P {
                path: String,
            }
            let p: P = serde_json::from_value(req.params)?;
            let conns = crate::config::load_connections_from_path(&p.path)?;
            for c in conns {
                let _ = registry.register(c).await;
            }
            Ok(ok(req.id, json!({"ok": true})))
        }
        "list_triggers" => {
            #[derive(Deserialize)]
            struct P {
                #[serde(rename = "connectionId")]
                connection_id: String,
                schema: Option<String>,
            }
            let p: P = serde_json::from_value(req.params)?;
            let Some(handle) = registry.get(&p.connection_id).await else {
                return Ok(err(req.id, 404, "Unknown connectionId"));
            };
            let pg = PostgresIntrospector::new(handle.pool.clone());
            let out = pg.list_triggers(p.schema.as_deref()).await?;
            Ok(ok(req.id, serde_json::to_value(out)?))
        }
        "list_routines" => {
            #[derive(Deserialize)]
            struct P {
                #[serde(rename = "connectionId")]
                connection_id: String,
                schema: Option<String>,
                kind: Option<String>,
            }
            let p: P = serde_json::from_value(req.params)?;
            let Some(handle) = registry.get(&p.connection_id).await else {
                return Ok(err(req.id, 404, "Unknown connectionId"));
            };
            let kind = match p.kind.as_deref() {
                Some("function") => Some(RoutineKind::Function),
                Some("procedure") => Some(RoutineKind::Procedure),
                Some("aggregate") => Some(RoutineKind::Aggregate),
                _ => None,
            };
            let pg = PostgresIntrospector::new(handle.pool.clone());
            let out = pg.list_routines(p.schema.as_deref(), kind).await?;
            Ok(ok(req.id, serde_json::to_value(out)?))
        }
        "list_types" => {
            #[derive(Deserialize)]
            struct P {
                #[serde(rename = "connectionId")]
                connection_id: String,
                schema: Option<String>,
                kind: Option<String>,
            }
            let p: P = serde_json::from_value(req.params)?;
            let Some(handle) = registry.get(&p.connection_id).await else {
                return Ok(err(req.id, 404, "Unknown connectionId"));
            };
            let kind = match p.kind.as_deref() {
                Some("enum") => Some(TypeKind::Enum),
                Some("domain") => Some(TypeKind::Domain),
                Some("composite") => Some(TypeKind::Composite),
                Some("base") => Some(TypeKind::Base),
                _ => None,
            };
            let pg = PostgresIntrospector::new(handle.pool.clone());
            let out = pg.list_types(p.schema.as_deref(), kind).await?;
            Ok(ok(req.id, serde_json::to_value(out)?))
        }
        "render_er" => {
            #[derive(Deserialize)]
            struct P {
                #[serde(rename = "connectionId")]
                connection_id: String,
                schemas: Option<Vec<String>>,
            }
            let p: P = serde_json::from_value(req.params)?;
            let Some(handle) = registry.get(&p.connection_id).await else {
                return Ok(err(req.id, 404, "Unknown connectionId"));
            };
            let pg = PostgresIntrospector::new(handle.pool.clone());
            let db = pg
                .introspect_database(crate::core::models::IntrospectOptions {
                    schemas: p.schemas,
                    with_indexes: false,
                    with_routines: false,
                    with_types: false,
                    with_triggers: false,
                    page: None,
                    page_size: None,
                })
                .await?;
            let er = crate::formatters::mermaid::render_er(&db);
            Ok(ok(req.id, json!({"mermaid": er})))
        }
        "render_dbml" => {
            #[derive(Deserialize)]
            struct P {
                #[serde(rename = "connectionId")]
                connection_id: String,
                schemas: Option<Vec<String>>,
            }
            let p: P = serde_json::from_value(req.params)?;
            let Some(handle) = registry.get(&p.connection_id).await else {
                return Ok(err(req.id, 404, "Unknown connectionId"));
            };
            let pg = PostgresIntrospector::new(handle.pool.clone());
            let db = pg
                .introspect_database(crate::core::models::IntrospectOptions {
                    schemas: p.schemas,
                    with_indexes: false,
                    with_routines: false,
                    with_types: false,
                    with_triggers: false,
                    page: None,
                    page_size: None,
                })
                .await?;
            let text = crate::formatters::dbml::render_dbml(&db);
            Ok(ok(req.id, json!({"dbml": text})))
        }
        "get_table" => {
            #[derive(Deserialize)]
            struct P {
                #[serde(rename = "connectionId")]
                connection_id: String,
                schema: String,
                table: String,
                format: Option<String>,
            }
            let p: P = serde_json::from_value(req.params)?;
            let Some(handle) = registry.get(&p.connection_id).await else {
                return Ok(err(req.id, 404, "Unknown connectionId"));
            };
            match handle.engine {
                DatabaseEngine::Postgres => {
                    let pg = PostgresIntrospector::new(handle.pool.clone());
                    let cache_key = format!("get_table|{}.{}", p.schema, p.table);
                    if !matches!(p.format.as_deref(), Some("markdown"))
                        && let Some(v) = handle.cache.get(&cache_key).await
                    {
                        return Ok(ok(req.id, v));
                    }
                    let td = pg.get_table(&p.schema, &p.table).await?;
                    if matches!(p.format.as_deref(), Some("markdown")) {
                        let md = crate::formatters::markdown::render_table_detail(&td);
                        Ok(ok(req.id, json!({"markdown": md})))
                    } else {
                        let v = serde_json::to_value(&td)?;
                        handle.cache.insert(cache_key, v.clone()).await;
                        Ok(ok(req.id, v))
                    }
                }
            }
        }
        _ => Ok(err(req.id, -32601, "Method not found")),
    }
}

fn parse_engine(s: &str) -> anyhow::Result<DatabaseEngine> {
    match s.to_lowercase().as_str() {
        "postgres" | "postgresql" | "pg" => Ok(DatabaseEngine::Postgres),
        _ => anyhow::bail!("Unsupported engine: {}", s),
    }
}

fn engine_to_str(e: DatabaseEngine) -> &'static str {
    match e {
        DatabaseEngine::Postgres => "postgres",
    }
}
