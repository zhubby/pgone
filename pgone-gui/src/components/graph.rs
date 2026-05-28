use crate::components::db_manager::PoolRegistry;
use pgone_sql::{Session, TableDetail};
use poll_promise::Promise;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TableNode {
    pub table_name: String,
    pub color: egui::Color32,
}

impl TableNode {
    fn new(table_name: &str) -> Self {
        let mut hasher = DefaultHasher::new();
        table_name.hash(&mut hasher);
        let hash = hasher.finish();

        // Generate a color based on hash - use RGB
        let r = ((hash >> 0) & 0xFF) as u8;
        let g = ((hash >> 8) & 0xFF) as u8;
        let b = ((hash >> 16) & 0xFF) as u8;
        // Ensure minimum brightness
        let r = r.max(100);
        let g = g.max(100);
        let b = b.max(100);
        let color = egui::Color32::from_rgb(r, g, b);

        Self {
            table_name: table_name.to_string(),
            color,
        }
    }
}

pub struct SchemaGraph {
    tables: Vec<TableDetail>,
    schema_name: String,
    database_name: String,
    loading: bool,
    error: Option<String>,
    promise: Option<Promise<Result<Vec<TableDetail>, String>>>,
    initialized: bool,
}

impl Default for SchemaGraph {
    fn default() -> Self {
        Self {
            tables: Vec::new(),
            schema_name: String::new(),
            database_name: String::new(),
            loading: false,
            error: None,
            promise: None,
            initialized: false,
        }
    }
}

impl SchemaGraph {
    pub fn new(database_name: String, schema_name: String) -> Self {
        Self {
            tables: Vec::new(),
            schema_name,
            database_name,
            loading: false,
            error: None,
            promise: None,
            initialized: false,
        }
    }

    pub fn load_data(&mut self, pools: PoolRegistry, dsn: &str) {
        if self.loading || self.promise.is_some() {
            return;
        }

        self.loading = true;
        self.error = None;
        let schema_name = self.schema_name.clone();
        let dsn = dsn.to_string();

        let (sender, promise) = Promise::new();
        self.promise = Some(promise);

        crate::futures::spawn(async move {
            let result: Result<Vec<TableDetail>, String> = async move {
                let pool = pools.get_or_create_pool(&dsn).await?;
                let session = Session::from_pool(pool);
                session
                    .list_table_details(&schema_name)
                    .await
                    .map_err(|e| e.to_string())
            }
            .await;

            sender.send(result);
        });
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, pools: PoolRegistry, dsn: Option<&str>) {
        // Check promise status
        if let Some(ref promise) = self.promise {
            if let Some(result) = promise.ready() {
                self.loading = false;
                match result {
                    Ok(tables) => {
                        self.tables = tables.clone();
                        self.initialized = true;
                    }
                    Err(e) => {
                        self.error = Some(e.clone());
                    }
                }
                self.promise = None;
            }
        }

        // Start loading if needed
        if !self.initialized && dsn.is_some() && self.promise.is_none() && !self.loading {
            self.load_data(pools, dsn.unwrap());
        }

        if self.loading {
            ui.centered_and_justified(|ui| {
                ui.spinner();
                ui.label("加载表信息中...");
            });
            return;
        }

        if let Some(err) = &self.error {
            ui.colored_label(egui::Color32::RED, format!("错误: {}", err));
            if ui.button("重试").clicked() {
                self.error = None;
                self.initialized = false;
            }
            return;
        }

        if self.tables.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.label("没有找到表");
            });
            return;
        }

        self.show_schema_overview(ui);
    }

    fn show_schema_overview(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading(format!("{}.{}", self.database_name, self.schema_name));
            ui.label(
                egui::RichText::new(format!("{} tables", self.tables.len()))
                    .small()
                    .weak(),
            );
        });
        ui.separator();

        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                self.show_relationships(ui);
                ui.add_space(8.0);
                ui.columns(2, |columns| {
                    for (index, table) in self.tables.iter().enumerate() {
                        columns[index % 2].group(|ui| {
                            self.show_table_card(ui, table);
                        });
                    }
                });
            });
    }

    fn show_relationships(&self, ui: &mut egui::Ui) {
        let relationship_count: usize = self
            .tables
            .iter()
            .map(|table| table.foreign_keys.len())
            .sum();

        ui.collapsing(format!("Relationships ({})", relationship_count), |ui| {
            if relationship_count == 0 {
                ui.label(egui::RichText::new("No foreign key relationships").weak());
                return;
            }

            for table in &self.tables {
                for fk in &table.foreign_keys {
                    let source = fk.columns.join(", ");
                    let target = fk.ref_columns.join(", ");
                    ui.horizontal_wrapped(|ui| {
                        ui.monospace(format!("{}.{}", table.name, source));
                        ui.label(egui_phosphor::regular::ARROW_RIGHT);
                        ui.monospace(format!("{}.{}", fk.ref_table, target));
                        if let Some(on_delete) = &fk.on_delete {
                            ui.label(
                                egui::RichText::new(format!("ON DELETE {}", on_delete)).weak(),
                            );
                        }
                        if let Some(on_update) = &fk.on_update {
                            ui.label(
                                egui::RichText::new(format!("ON UPDATE {}", on_update)).weak(),
                            );
                        }
                    });
                }
            }
        });
    }

    fn show_table_card(&self, ui: &mut egui::Ui, table: &TableDetail) {
        let node = TableNode::new(&table.name);
        ui.horizontal_wrapped(|ui| {
            ui.colored_label(node.color, egui_phosphor::regular::TABLE);
            ui.strong(&node.table_name);
            if let Some(pk) = &table.primary_key {
                ui.label(
                    egui::RichText::new(format!("PK {}", pk.columns.join(", ")))
                        .small()
                        .weak(),
                );
            }
        });

        if let Some(comment) = &table.comment {
            ui.label(egui::RichText::new(comment).italics().weak());
        }

        ui.separator();
        egui::Grid::new(("schema_graph_table", &table.name))
            .num_columns(3)
            .striped(true)
            .min_col_width(72.0)
            .show(ui, |ui| {
                for column in &table.columns {
                    ui.horizontal(|ui| {
                        if table
                            .primary_key
                            .as_ref()
                            .is_some_and(|pk| pk.columns.contains(&column.name))
                        {
                            ui.label(egui_phosphor::regular::KEY);
                        }
                        if table
                            .foreign_keys
                            .iter()
                            .any(|fk| fk.columns.contains(&column.name))
                        {
                            ui.label(egui_phosphor::regular::LINK);
                        }
                        ui.monospace(&column.name);
                    });
                    ui.label(&column.data_type);
                    ui.horizontal_wrapped(|ui| {
                        if !column.nullable {
                            ui.label(egui::RichText::new("NOT NULL").small().weak());
                        }
                        if let Some(comment) = &column.comment {
                            ui.label(egui::RichText::new(comment).small().weak());
                        }
                    });
                    ui.end_row();
                }
            });
    }
}
