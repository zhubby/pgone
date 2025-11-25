use crate::components::{SqlCtx, ResultsTable};
use sqlx::postgres::{PgPoolOptions, PgRow};
use sqlx::{Column, Row};
use std::collections::HashSet;

#[derive(Default)]
pub struct SqlPanel {
    pub sql_input: String,
    pub sql_error: Option<String>,
    pub query_columns: Vec<String>,
    pub query_rows: Vec<Vec<String>>,
    pub results_table: ResultsTable,
    pub primary_key_columns: HashSet<String>,
}

// Default is derived

impl SqlPanel {
    pub fn ui_editor(&mut self, ctxs: &mut SqlCtx, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading(format!("{} SQL Editor", egui_phosphor::regular::QUESTION));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(
                        egui::Button::new(egui_phosphor::regular::PLAY)
                            .min_size(egui::vec2(28.0, 28.0)),
                    )
                    .clicked()
                {
                    self.run_sql(ctxs);
                }
                ui.add_space(8.0);
                if ui
                    .add(
                        egui::Button::new(egui_phosphor::regular::CHECK)
                            .min_size(egui::vec2(28.0, 28.0)),
                    )
                    .clicked()
                {
                    self.check_sql();
                }
            });
        });
        ui.separator();
        let current_sql = self.sql_input.clone();
        let available_height = ui.available_height() - 10.0;
        let editor = ui.add(
            egui::TextEdit::multiline(&mut self.sql_input)
                .desired_rows((available_height / 20.0) as usize)
                .layouter(&mut move |ui, _text, wrap_width| {
                    let mut job = crate::sql::highlight_sql(&current_sql, ui.visuals());
                    job.wrap.max_width = wrap_width;
                    ui.fonts(|f| f.layout_job(job))
                }),
        );
        if let Some(err) = &self.sql_error {
            ui.colored_label(egui::Color32::RED, err);
        }
        if editor.changed() {
            self.sql_error = None;
        }
    }

    pub fn ui_results(&mut self, ui: &mut egui::Ui, mut ctxs: Option<&mut SqlCtx>) {
        let show_refresh = ctxs.is_some();
        
        // Check if refresh was requested
        if self.results_table.refresh_requested {
            self.results_table.refresh_requested = false;
            if let Some(ctxs) = ctxs.as_mut() {
                self.run_sql(ctxs);
            }
        }
        
        let pk_cols = if self.primary_key_columns.is_empty() {
            None
        } else {
            Some(&self.primary_key_columns)
        };
        self.results_table.ui(ui, &self.query_columns, &self.query_rows, pk_cols, show_refresh, Some(&self.sql_input));
    }

    pub fn check_sql(&mut self) {
        self.sql_error = None;
        let dialect = sqlparser::dialect::PostgreSqlDialect {};
        match sqlparser::parser::Parser::parse_sql(&dialect, &self.sql_input) {
            Ok(_) => {
                self.sql_error = None;
            }
            Err(e) => {
                self.sql_error = Some(format!("{}", e));
            }
        }
    }

    pub fn run_sql(&mut self, ctxs: &mut SqlCtx) {
        self.sql_error = None;
        self.primary_key_columns.clear();
        
        let Some(sess) = ctxs.state.sessions.get(ctxs.state.current_index).cloned() else {
            self.sql_error = Some("No active session".into());
            return;
        };
        let dsn = sess.db.dsn.clone();
        if dsn.trim().is_empty() {
            self.sql_error = Some("DSN is empty".into());
            return;
        }
        let sql = self.sql_input.clone();
        let pool_opt = ctxs.db.pools.get(&sess.id).cloned();
        
        // Try to detect primary key columns from SQL query
        let pk_cols = self.detect_primary_keys(&sql, &dsn, &pool_opt);
        
        let res: Result<(Vec<String>, Vec<Vec<String>>), String> = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
            let pool = match pool_opt {
                Some(p) => p,
                None => PgPoolOptions::new()
                    .max_connections(1)
                    .connect(&dsn)
                    .await
                    .map_err(|e| e.to_string())?,
            };
            let rows: Vec<PgRow> = sqlx::query(&sql)
                .fetch_all(&pool)
                .await
                .map_err(|e| e.to_string())?;
            let mut cols: Vec<String> = Vec::new();
            let mut data: Vec<Vec<String>> = Vec::new();
            if let Some(first) = rows.first() {
                for c in first.columns() {
                    cols.push(c.name().to_string());
                }
            }
            for row in rows.into_iter().take(100) {
                let mut r: Vec<String> = Vec::new();
                let n = if cols.is_empty() {
                    row.len()
                } else {
                    cols.len()
                };
                for i in 0..n {
                    r.push(crate::sql::format_cell(&row, i));
                }
                data.push(r);
            }
            Ok((cols, data))
            })
        });
        match res {
            Ok((cols, rows)) => {
                self.query_columns = cols;
                self.query_rows = rows;
                // Update primary key columns if detected
                if let Some(pk) = pk_cols {
                    self.primary_key_columns = pk;
                }
            }
            Err(e) => {
                self.sql_error = Some(e);
            }
        }
    }
    
    fn detect_primary_keys(&self, sql: &str, dsn: &str, pool_opt: &Option<sqlx::PgPool>) -> Option<HashSet<String>> {
        // Parse SQL to extract table names
        let dialect = sqlparser::dialect::PostgreSqlDialect {};
        let ast = sqlparser::parser::Parser::parse_sql(&dialect, sql).ok()?;
        
        // Extract table names from SELECT statements
        let mut table_names = Vec::new();
        for stmt in ast {
            if let sqlparser::ast::Statement::Query(query) = stmt {
                // Extract from SetExpr::Select
                if let sqlparser::ast::SetExpr::Select(select) = &*query.body {
                    for table_with_joins in &select.from {
                        if let sqlparser::ast::TableFactor::Table { name, .. } = &table_with_joins.relation {
                            let schema = name.0.first().map(|i| i.value.clone());
                            let table = name.0.last().map(|i| i.value.clone());
                            match (schema, table) {
                                (Some(s), Some(t)) => {
                                    table_names.push((s, t));
                                }
                                (None, Some(t)) => {
                                    // Default to public schema if no schema specified
                                    table_names.push(("public".to_string(), t));
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
        
        if table_names.is_empty() {
            return None;
        }
        
        // Query primary key information for the first table (simple case)
        // For JOIN queries, we only check the first table
        if let Some((schema, table)) = table_names.first() {
            let pk_result = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    let pool = match pool_opt {
                        Some(p) => p.clone(),
                        None => PgPoolOptions::new()
                            .max_connections(1)
                            .connect(dsn)
                            .await
                            .ok()?,
                    };
                    
                    let pk_query = "SELECT kcu.column_name \
                        FROM information_schema.table_constraints tc \
                        JOIN information_schema.key_column_usage kcu \
                          ON tc.constraint_name = kcu.constraint_name AND tc.table_schema = kcu.table_schema \
                        WHERE tc.constraint_type = 'PRIMARY KEY' AND tc.table_schema = $1 AND tc.table_name = $2 \
                        ORDER BY kcu.ordinal_position";
                    
                    let rows: Result<Vec<sqlx::postgres::PgRow>, _> = sqlx::query(pk_query)
                        .bind(schema)
                        .bind(table)
                        .fetch_all(&pool)
                        .await;
                    
                    rows.ok().map(|rows| {
                        rows.into_iter()
                            .map(|r| r.get::<String, _>(0))
                            .collect::<HashSet<String>>()
                    })
                })
            });
            
            pk_result
        } else {
            None
        }
    }

}
