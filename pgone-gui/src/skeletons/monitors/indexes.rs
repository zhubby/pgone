use eframe::egui;
use egui_plot::{Bar, BarChart, Plot};
use poll_promise::Promise;
use sqlx::Row;

#[derive(Clone)]
struct IndexData {
    schemaname: String,
    tablename: String,
    indexname: String,
    idx_scan: i64,
    idx_tup_read: i64,
    idx_tup_fetch: i64,
}

pub struct IndexesMonitor {
    promise: Option<Promise<Result<Vec<IndexData>, String>>>,
    data: Vec<IndexData>,
    error: Option<String>,
}

impl Default for IndexesMonitor {
    fn default() -> Self {
        Self {
            promise: None,
            data: Vec::new(),
            error: None,
        }
    }
}

impl IndexesMonitor {
    fn load_data(&mut self, pools: crate::components::db_manager::PoolRegistry, dsn: Option<&str>) {
        if self.promise.is_some() {
            return;
        }

        let Some(dsn) = dsn else {
            self.error = Some("No database selected".to_string());
            return;
        };

        let dsn = dsn.to_string();
        let (sender, promise) = Promise::new();
        self.promise = Some(promise);

        crate::futures::spawn(async move {
            let result = async {
                let pool = pools
                    .get_or_create_pool(&dsn)
                    .await
                    .map_err(|e| format!("Connection failed: {}", e))?;

                let rows = sqlx::query(
                    r#"
                    SELECT 
                        schemaname,
                        tablename,
                        indexname,
                        idx_scan,
                        idx_tup_read,
                        idx_tup_fetch
                    FROM pg_stat_user_indexes
                    ORDER BY idx_scan DESC
                    LIMIT 50
                    "#,
                )
                .fetch_all(&pool)
                .await
                .map_err(|e| format!("Query failed: {}", e))?;

                let mut data = Vec::new();
                for row in rows {
                    data.push(IndexData {
                        schemaname: row.get(0),
                        tablename: row.get(1),
                        indexname: row.get(2),
                        idx_scan: row.get(3),
                        idx_tup_read: row.get(4),
                        idx_tup_fetch: row.get(5),
                    });
                }

                Ok::<_, String>(data)
            }
            .await;

            sender.send(result);
        });
    }

    fn ui(
        &mut self,
        ui: &mut egui::Ui,
        pools: crate::components::db_manager::PoolRegistry,
        dsn: Option<&str>,
    ) {
        // Check Promise status
        if let Some(ref promise) = self.promise {
            if let Some(result) = promise.ready() {
                match result {
                    Ok(data) => {
                        self.data = data.clone();
                        self.error = None;
                    }
                    Err(e) => {
                        self.error = Some(e.clone());
                    }
                }
                self.promise = None;
            }
        }

        // If no data and no error, start loading
        if self.data.is_empty() && self.error.is_none() && self.promise.is_none() {
            self.load_data(pools.clone(), dsn);
        }

        // Show loading state
        if self.promise.is_some() {
            ui.centered_and_justified(|ui| {
                ui.spinner();
                ui.label("Loading...");
            });
            return;
        }

        // Show error
        if let Some(err) = &self.error {
            ui.colored_label(egui::Color32::RED, format!("Error: {}", err));
            if ui.button("Retry").clicked() {
                self.error = None;
                self.data.clear();
            }
            return;
        }

        // Show data
        if self.data.is_empty() {
            ui.label("No data");
            return;
        }

        ui.horizontal(|ui| {
            if ui.button("Refresh").clicked() {
                self.data.clear();
                self.error = None;
            }
        });

        ui.separator();

        // Show table
        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::Grid::new("indexes_grid")
                .num_columns(6)
                .spacing([20.0, 4.0])
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("Schema").strong());
                    ui.label(egui::RichText::new("Table").strong());
                    ui.label(egui::RichText::new("Index").strong());
                    ui.label(egui::RichText::new("Index scans").strong());
                    ui.label(egui::RichText::new("Rows read").strong());
                    ui.label(egui::RichText::new("Rows fetched").strong());
                    ui.end_row();

                    for item in &self.data {
                        ui.label(&item.schemaname);
                        ui.label(&item.tablename);
                        ui.label(&item.indexname);
                        ui.label(item.idx_scan.to_string());
                        ui.label(item.idx_tup_read.to_string());
                        ui.label(item.idx_tup_fetch.to_string());
                        ui.end_row();
                    }
                });
        });

        ui.separator();

        // Show bar chart - TOP 10
        if !self.data.is_empty() {
            let bars: Vec<Bar> = self
                .data
                .iter()
                .take(10)
                .enumerate()
                .map(|(i, item)| {
                    Bar::new(i as f64, item.idx_scan as f64)
                        .width(0.6)
                        .name(format!("{}.{}", item.schemaname, item.indexname))
                })
                .collect();

            let chart = BarChart::new("Index scan count TOP 10", bars);

            Plot::new("indexes_plot").height(300.0).show(ui, |plot_ui| {
                plot_ui.bar_chart(chart);
            });
        }
    }
}

static MONITOR: std::sync::Mutex<Option<IndexesMonitor>> = std::sync::Mutex::new(None);

pub fn show(
    ui: &mut egui::Ui,
    pools: crate::components::db_manager::PoolRegistry,
    dsn: Option<&str>,
) {
    let mut monitor = MONITOR.lock().unwrap();
    if monitor.is_none() {
        *monitor = Some(IndexesMonitor::default());
    }
    if let Some(ref mut m) = *monitor {
        m.ui(ui, pools, dsn);
    }
}
