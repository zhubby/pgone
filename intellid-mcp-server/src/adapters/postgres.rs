use sqlx::{Pool, Postgres, Row};
use crate::core::introspector::DatabaseIntrospector;
use crate::core::models::*;
use async_trait::async_trait;

#[derive(Clone)]
pub struct PostgresIntrospector {
    pool: Pool<Postgres>,
}

impl PostgresIntrospector {
    pub fn new(pool: Pool<Postgres>) -> Self { Self { pool } }
}

#[async_trait]
impl DatabaseIntrospector for PostgresIntrospector {
    async fn introspect_database(&self, _opts: IntrospectOptions) -> anyhow::Result<DatabaseSchema> {
        let database: String = sqlx::query_scalar("SELECT current_database()")
            .fetch_one(&self.pool).await?;

        let tables = self.list_tables(None).await?;

        let mut schemas_map: std::collections::BTreeMap<String, Vec<TableDetail>> = std::collections::BTreeMap::new();
        for (schema, table) in tables {
            let td = self.get_table(&schema, &table).await?;
            schemas_map.entry(schema).or_default().push(td);
        }

        let mut schemas_vec: Vec<Schema> = Vec::new();
        for (schema_name, tables) in schemas_map {
            let views = self.list_views(Some(&schema_name)).await.unwrap_or_default();
            schemas_vec.push(Schema { name: schema_name, tables, views });
        }

        Ok(DatabaseSchema { database, schemas: schemas_vec })
    }

    async fn list_tables(&self, schema: Option<&str>) -> anyhow::Result<Vec<(String, String)>> {
        let rows = if let Some(s) = schema {
            sqlx::query(
                "SELECT table_schema, table_name FROM information_schema.tables \
                 WHERE table_type = 'BASE TABLE' AND table_schema = $1 \
                 AND table_schema NOT IN ('pg_catalog','information_schema') ORDER BY 1,2"
            ).bind(s).fetch_all(&self.pool).await?
        } else {
            sqlx::query(
                "SELECT table_schema, table_name FROM information_schema.tables \
                 WHERE table_type = 'BASE TABLE' \
                 AND table_schema NOT IN ('pg_catalog','information_schema') ORDER BY 1,2"
            ).fetch_all(&self.pool).await?
        };

        Ok(rows.into_iter().map(|r| (r.get::<String, _>(0), r.get::<String, _>(1))).collect())
    }

    async fn get_table(&self, schema: &str, table: &str) -> anyhow::Result<TableDetail> {
        // columns + comments
        let cols = sqlx::query(
            "SELECT c.column_name, c.is_nullable, c.data_type, c.udt_name, \
                    c.character_maximum_length, c.numeric_precision, c.numeric_scale, \
                    c.column_default, pgd.description AS column_comment \
             FROM information_schema.columns c \
             LEFT JOIN pg_class pc ON pc.relname = c.table_name \
             LEFT JOIN pg_namespace pn ON pn.nspname = c.table_schema AND pn.oid = pc.relnamespace \
             LEFT JOIN pg_attribute pa ON pa.attrelid = pc.oid AND pa.attname = c.column_name \
             LEFT JOIN pg_description pgd ON pgd.objoid = pc.oid AND pgd.objsubid = pa.attnum \
             WHERE c.table_schema = $1 AND c.table_name = $2 \
             ORDER BY c.ordinal_position"
        ).bind(schema).bind(table).fetch_all(&self.pool).await?;

        let columns: Vec<Column> = cols.into_iter().map(|r| Column {
            name: r.get(0),
            nullable: matches!(r.get::<String, _>(1).as_str(), "YES"),
            data_type: r.get(2),
            udt_name: r.try_get(3).ok(),
            character_maximum_length: r.try_get::<Option<i32>, _>(4).ok().flatten(),
            numeric_precision: r.try_get::<Option<i32>, _>(5).ok().flatten(),
            numeric_scale: r.try_get::<Option<i32>, _>(6).ok().flatten(),
            default: r.try_get(7).ok(),
            comment: r.try_get(8).ok(),
        }).collect();

        // table comment
        let table_comment: Option<String> = sqlx::query_scalar(
            "SELECT obj_description(pc.oid) \
             FROM pg_class pc \
             JOIN pg_namespace pn ON pn.oid = pc.relnamespace \
             WHERE pn.nspname = $1 AND pc.relname = $2"
        ).bind(schema).bind(table).fetch_optional(&self.pool).await?;

        // primary key
        let pk_cols: Vec<String> = sqlx::query(
            "SELECT kcu.column_name \
             FROM information_schema.table_constraints tc \
             JOIN information_schema.key_column_usage kcu \
               ON tc.constraint_name = kcu.constraint_name AND tc.table_schema = kcu.table_schema \
             WHERE tc.constraint_type = 'PRIMARY KEY' AND tc.table_schema = $1 AND tc.table_name = $2 \
             ORDER BY kcu.ordinal_position"
        ).bind(schema).bind(table).fetch_all(&self.pool).await?
        .into_iter().map(|r| r.get(0)).collect();
        let primary_key = if pk_cols.is_empty() { None } else { Some(PrimaryKey { columns: pk_cols }) };

        // foreign keys
        let fk_rows = sqlx::query(
            "SELECT kcu.constraint_name, kcu.column_name, ccu.table_schema, ccu.table_name, ccu.column_name, rc.update_rule, rc.delete_rule \
             FROM information_schema.table_constraints tc \
             JOIN information_schema.key_column_usage kcu \
               ON tc.constraint_name = kcu.constraint_name AND tc.table_schema = kcu.table_schema \
             JOIN information_schema.referential_constraints rc \
               ON rc.constraint_name = tc.constraint_name AND rc.constraint_schema = tc.table_schema \
             JOIN information_schema.constraint_column_usage ccu \
               ON ccu.constraint_name = rc.unique_constraint_name AND ccu.constraint_schema = rc.unique_constraint_schema \
             WHERE tc.constraint_type = 'FOREIGN KEY' AND tc.table_schema = $1 AND tc.table_name = $2 \
             ORDER BY kcu.ordinal_position"
        ).bind(schema).bind(table).fetch_all(&self.pool).await?;

        // group by constraint_name
        let mut fk_map: std::collections::BTreeMap<String, (Vec<String>, (String, Vec<String>), Option<String>, Option<String>)> = std::collections::BTreeMap::new();
        for r in fk_rows {
            let cname: String = r.get(0);
            let col: String = r.get(1);
            let ref_schema: String = r.get(2);
            let ref_table: String = r.get(3);
            let ref_col: String = r.get(4);
            let on_update: Option<String> = r.try_get(5).ok();
            let on_delete: Option<String> = r.try_get(6).ok();
            let entry = fk_map.entry(cname).or_insert((Vec::new(), (format!("{}.{}", ref_schema, ref_table), Vec::new()), None, None));
            entry.0.push(col);
            entry.1.1.push(ref_col);
            entry.2 = on_update;
            entry.3 = on_delete;
        }
        let foreign_keys: Vec<ForeignKey> = fk_map.into_values().map(|(cols, (ref_table, ref_cols), on_update, on_delete)| ForeignKey {
            columns: cols,
            ref_table,
            ref_columns: ref_cols,
            on_update,
            on_delete,
        }).collect();

        // indexes (basic)
        let idx_rows = sqlx::query(
            "SELECT indexname, indexdef FROM pg_indexes WHERE schemaname = $1 AND tablename = $2"
        ).bind(schema).bind(table).fetch_all(&self.pool).await?;
        let mut indexes: Vec<Index> = Vec::new();
        for r in idx_rows {
            let name: String = r.get(0);
            let def: String = r.get(1);
            let upper = def.to_uppercase();
            let unique = upper.contains(" UNIQUE ");
            // 提取括号内列
            let cols: Vec<String> = def.split('(').nth(1).and_then(|s| s.split(')').next())
                .map(|s| s.split(',').map(|c| c.trim().trim_matches('"').to_string()).collect())
                .unwrap_or_default();
            // INCLUDE 提取
            let include: Vec<String> = if let Some(pos) = upper.find(" INCLUDE (") {
                let rest = &def[pos..];
                rest.split('(').nth(1).and_then(|s| s.split(')').next())
                    .map(|s| s.split(',').map(|c| c.trim().trim_matches('"').to_string()).collect())
                    .unwrap_or_default()
            } else { Vec::new() };
            indexes.push(Index { name, unique, columns: cols, include, definition: Some(def) });
        }

        Ok(TableDetail {
            schema: schema.to_string(),
            name: table.to_string(),
            comment: table_comment,
            columns,
            primary_key,
            foreign_keys,
            indexes,
        })
    }

    async fn list_views(&self, schema: Option<&str>) -> anyhow::Result<ViewDetailVec> {
        // 包含物化视图
        let rows = if let Some(s) = schema {
            sqlx::query(
                "SELECT table_schema, table_name, view_definition \
                 FROM information_schema.views WHERE table_schema = $1"
            ).bind(s).fetch_all(&self.pool).await?
        } else {
            sqlx::query(
                "SELECT table_schema, table_name, view_definition \
                 FROM information_schema.views WHERE table_schema NOT IN ('pg_catalog','information_schema')"
            ).fetch_all(&self.pool).await?
        };
        let mut views: Vec<ViewDetail> = rows.into_iter().map(|r| ViewDetail {
            schema: r.get(0),
            name: r.get(1),
            definition: r.try_get(2).ok(),
            comment: None,
        }).collect();

        // 追加物化视图
        let mat_rows = if let Some(s) = schema {
            sqlx::query("SELECT schemaname, matviewname, definition FROM pg_matviews WHERE schemaname = $1").bind(s).fetch_all(&self.pool).await?
        } else {
            sqlx::query("SELECT schemaname, matviewname, definition FROM pg_matviews").fetch_all(&self.pool).await?
        };
        for r in mat_rows { views.push(ViewDetail { schema: r.get(0), name: r.get(1), definition: r.try_get(2).ok(), comment: None }); }
        Ok(views)
    }

    async fn list_triggers(&self, schema: Option<&str>) -> anyhow::Result<Vec<TriggerDetail>> {
        let rows = if let Some(s) = schema {
            sqlx::query(
                "SELECT event_object_schema AS table_schema, event_object_table AS table_name, trigger_schema, trigger_name, action_timing, event_manipulation, action_statement \
                 FROM information_schema.triggers WHERE trigger_schema = $1 ORDER BY 1,2,4"
            ).bind(s).fetch_all(&self.pool).await?
        } else {
            sqlx::query(
                "SELECT event_object_schema AS table_schema, event_object_table AS table_name, trigger_schema, trigger_name, action_timing, event_manipulation, action_statement \
                 FROM information_schema.triggers ORDER BY 1,2,4"
            ).fetch_all(&self.pool).await?
        };

        // 聚合同名 trigger 的多个事件
        use std::collections::BTreeMap;
        let mut map: BTreeMap<(String,String,String), TriggerDetail> = BTreeMap::new();
        for r in rows {
            let table_schema: String = r.get(0);
            let table_name: String = r.get(1);
            let trigger_schema: String = r.get(2);
            let trigger_name: String = r.get(3);
            let timing: String = r.get(4);
            let event: String = r.get(5);
            let action_stmt: Option<String> = r.try_get(6).ok();
            let key = (trigger_schema.clone(), trigger_name.clone(), timing.clone());
            let entry = map.entry(key).or_insert(TriggerDetail {
                schema: trigger_schema.clone(),
                name: trigger_name.clone(),
                table_schema: table_schema.clone(),
                table_name: table_name.clone(),
                timing,
                events: Vec::new(),
                function_name: action_stmt.as_ref().and_then(|s| parse_trigger_function_name(s.as_str())),
            });
            if !entry.events.contains(&event) { entry.events.push(event); }
        }
        Ok(map.into_values().collect())
    }

    async fn list_routines(&self, schema: Option<&str>, kind: Option<RoutineKind>) -> anyhow::Result<Vec<RoutineDetail>> {
        // 仅函数/过程，聚合暂略（可通过 pg_proc.prokind = 'a' 查询）
        let mut sql = String::from(
            "SELECT n.nspname AS schema, p.proname AS name, p.prokind, l.lanname AS language, \
                    pg_get_function_result(p.oid) AS return_type, \
                    pg_get_functiondef(p.oid) AS definition, \
                    pg_catalog.obj_description(p.oid) AS comment, \
                    p.proargnames, p.proargtypes::regtype[] \
             FROM pg_proc p \
             JOIN pg_namespace n ON n.oid = p.pronamespace \
             JOIN pg_language l ON l.oid = p.prolang \
             WHERE n.nspname NOT IN ('pg_catalog','information_schema')"
        );
        if let Some(_s) = schema { sql.push_str(" AND n.nspname = $1"); }
        sql.push_str(" ORDER BY 1,2");

        let rows = if let Some(s) = schema {
            sqlx::query(&sql).bind(s).fetch_all(&self.pool).await?
        } else {
            sqlx::query(&sql).fetch_all(&self.pool).await?
        };

        let mut out: Vec<RoutineDetail> = Vec::new();
        for r in rows {
            let prokind: String = r.get::<String,_>(2); // 'f'|'p'|'a'
            let k = match prokind.as_str() { "f" => RoutineKind::Function, "p" => RoutineKind::Procedure, "a" => RoutineKind::Aggregate, _ => RoutineKind::Function };
            if let Some(filter) = kind.as_ref() { if &k != filter { continue; } }
            let names: Option<Vec<String>> = r.try_get::<Option<Vec<String>>,_>(7).ok().flatten();
            let types: Option<Vec<String>> = r.try_get::<Option<Vec<String>>,_>(8).ok().flatten();
            // 读取参数模式（可能为空）
            let modes: Option<Vec<String>> = sqlx::query_scalar(
                "SELECT p.proargmodes FROM pg_proc p WHERE p.proname = $1 AND p.pronamespace = (SELECT n.oid FROM pg_namespace n WHERE n.nspname = $2)"
            ).bind(r.get::<String,_>(1)).bind(r.get::<String,_>(0)).fetch_optional(&self.pool).await?;
            let mut params = Vec::new();
            if let Some(ts) = types {
                for (idx, t) in ts.into_iter().enumerate() {
                    let name = names.as_ref().and_then(|ns| ns.get(idx).cloned());
                    let mode = modes.as_ref().and_then(|ms| ms.get(idx).cloned()).and_then(|m| match m.as_str() { "i" => Some(ParamMode::In), "o" => Some(ParamMode::Out), "b" => Some(ParamMode::InOut), "v" => Some(ParamMode::Variadic), "t" => Some(ParamMode::Table), _ => None });
                    params.push(RoutineParam { name, data_type: t, mode });
                }
            }
            out.push(RoutineDetail {
                schema: r.get(0),
                name: r.get(1),
                kind: k,
                language: r.try_get(3).ok(),
                return_type: r.try_get(4).ok(),
                params,
                definition: r.try_get(5).ok(),
                comment: r.try_get(6).ok(),
            });
        }
        Ok(out)
    }

    async fn list_types(&self, schema: Option<&str>, kind: Option<TypeKind>) -> anyhow::Result<Vec<TypeDetail>> {
        let mut out: Vec<TypeDetail> = Vec::new();
        // ENUM
        let enum_sql = "SELECT n.nspname, t.typname, 'enum' AS kind, NULL AS base, array_agg(e.enumlabel ORDER BY e.enumsortorder) AS labels, NULL AS def, pg_catalog.obj_description(t.oid) AS comment \
                        FROM pg_type t JOIN pg_enum e ON e.enumtypid = t.oid JOIN pg_namespace n ON n.oid = t.typnamespace \
                        WHERE ($1::text IS NULL OR n.nspname = $1) GROUP BY 1,2,3,4,6,7";
        let rows = sqlx::query(enum_sql).bind(schema).fetch_all(&self.pool).await?;
        for r in rows {
            let td = TypeDetail { schema: r.get(0), name: r.get(1), kind: TypeKind::Enum, base_type: None, enum_labels: r.try_get(4).ok(), definition: None, comment: r.try_get(6).ok() };
            if kind.as_ref().map(|k| matches!(k, TypeKind::Enum)).unwrap_or(true) { out.push(td); }
        }
        // DOMAIN
        let dom_sql = "SELECT n.nspname, t.typname, 'domain', format_type(t.typbasetype, t.typtypmod), NULL, NULL, pg_catalog.obj_description(t.oid) \
                       FROM pg_type t JOIN pg_namespace n ON n.oid = t.typnamespace \
                       WHERE t.typtype = 'd' AND ($1::text IS NULL OR n.nspname = $1)";
        let rows = sqlx::query(dom_sql).bind(schema).fetch_all(&self.pool).await?;
        for r in rows { let td = TypeDetail { schema: r.get(0), name: r.get(1), kind: TypeKind::Domain, base_type: r.try_get(3).ok(), enum_labels: None, definition: None, comment: r.try_get(6).ok() }; if kind.as_ref().map(|k| matches!(k, TypeKind::Domain)).unwrap_or(true) { out.push(td); } }
        // COMPOSITE
        let comp_sql = "SELECT n.nspname, t.typname, 'composite', NULL, NULL, NULL, pg_catalog.obj_description(t.oid) \
                        FROM pg_type t JOIN pg_namespace n ON n.oid = t.typnamespace \
                        WHERE t.typtype = 'c' AND ($1::text IS NULL OR n.nspname = $1)";
        let rows = sqlx::query(comp_sql).bind(schema).fetch_all(&self.pool).await?;
        for r in rows { let td = TypeDetail { schema: r.get(0), name: r.get(1), kind: TypeKind::Composite, base_type: None, enum_labels: None, definition: None, comment: r.try_get(6).ok() }; if kind.as_ref().map(|k| matches!(k, TypeKind::Composite)).unwrap_or(true) { out.push(td); } }
        // BASE (内建/别名展示)
        // 简化处理：不逐一列举所有 base 类型；可选实现
        Ok(out)
    }
}

type ViewDetailVec = Vec<ViewDetail>;

fn parse_trigger_function_name(stmt: &str) -> Option<String> {
    // 尝试从 "EXECUTE FUNCTION schema.func(args)" 或 "EXECUTE PROCEDURE ..." 中提取标识
    let upper = stmt.to_uppercase();
    let key = if let Some(pos) = upper.find("EXECUTE FUNCTION") { pos + "EXECUTE FUNCTION".len() } else if let Some(pos) = upper.find("EXECUTE PROCEDURE") { pos + "EXECUTE PROCEDURE".len() } else { return None };
    let rest = &stmt[key..].trim();
    let name = rest.split('(').next()?.trim().trim_matches('"');
    if name.is_empty() { None } else { Some(name.to_string()) }
}


