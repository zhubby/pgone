use eframe::egui;
use egui_plot::{Bar, BarChart, Plot};
use poll_promise::Promise;
use sqlx::Row;

#[derive(Clone)]
struct StatementData {
    query: String,
    calls: i64,
    total_time: f64,
    mean_time: f64,
}

pub struct StatementsMonitor {
    promise: Option<Promise<Result<Vec<StatementData>, String>>>,
    data: Vec<StatementData>,
    error: Option<String>,
    limit: usize,
}

impl Default for StatementsMonitor {
    fn default() -> Self {
        Self {
            promise: None,
            data: Vec::new(),
            error: None,
            limit: 20,
        }
    }
}

impl StatementsMonitor {
    fn load_data(&mut self, pools: crate::components::db_manager::PoolRegistry, dsn: Option<&str>) {
        if self.promise.is_some() {
            return;
        }

        let Some(dsn) = dsn else {
            self.error = Some("No database selected".to_string());
            return;
        };

        let dsn = dsn.to_string();
        let limit = self.limit;
        let (sender, promise) = Promise::new();
        self.promise = Some(promise);

        crate::futures::spawn(async move {
            let result = async {
                let pool = pools
                    .get_or_create_pool(&dsn)
                    .await
                    .map_err(|e| format!("Connection failed: {}", e))?;

                // Check if extension is enabled
                let ext_check: Option<(bool,)> = sqlx::query_as(
                    "SELECT EXISTS(SELECT 1 FROM pg_extension WHERE extname = 'pg_stat_statements')"
                )
                .fetch_optional(&pool)
                .await
                .map_err(|e| format!("Failed to check extension: {}", e))?
                .map(|row: (bool,)| row);

                if ext_check.map(|(exists,)| exists) != Some(true) {
                    return Err("pg_stat_statements extension is not enabled".to_string());
                }

                let rows = sqlx::query(
                    r#"
                    SELECT 
                        LEFT(query, 100) as query,
                        calls,
                        total_exec_time as total_time,
                        mean_exec_time as mean_time
                    FROM pg_stat_statements
                    ORDER BY total_exec_time DESC
                    LIMIT $1
                    "#,
                )
                .bind(limit as i64)
                .fetch_all(&pool)
                .await
                .map_err(|e| format!("Query failed: {}", e))?;

                let mut data = Vec::new();
                for row in rows {
                    let query: String = row.get(0);
                    let calls: i64 = row.get(1);
                    let total_time: Option<f64> = row.get(2);
                    let mean_time: Option<f64> = row.get(3);
                    data.push(StatementData {
                        query,
                        calls,
                        total_time: total_time.unwrap_or(0.0),
                        mean_time: mean_time.unwrap_or(0.0),
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
            ui.label("Display count:");
            ui.add(egui::Slider::new(&mut self.limit, 10..=100).text(""));
            if ui.button("Refresh").clicked() {
                self.data.clear();
                self.error = None;
            }
        });

        ui.separator();

        // Show table
        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::Grid::new("statements_grid")
                .num_columns(4)
                .spacing([40.0, 4.0])
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("Query").strong());
                    ui.label(egui::RichText::new("Calls").strong());
                    ui.label(egui::RichText::new("Total time (ms)").strong());
                    ui.label(egui::RichText::new("Mean time (ms)").strong());
                    ui.end_row();

                    for item in &self.data {
                        ui.label(egui::RichText::new(&item.query).small());
                        ui.label(item.calls.to_string());
                        ui.label(format!("{:.2}", item.total_time));
                        ui.label(format!("{:.2}", item.mean_time));
                        ui.end_row();
                    }
                });
        });

        ui.separator();

        // Show bar chart - sorted by total time
        if !self.data.is_empty() {
            let bars: Vec<Bar> = self
                .data
                .iter()
                .take(10)
                .enumerate()
                .map(|(i, item)| {
                    Bar::new(i as f64, item.total_time)
                        .width(0.6)
                        .name(format!("Query {}", i + 1))
                })
                .collect();

            let chart = BarChart::new("Query total time TOP 10", bars);

            Plot::new("statements_plot")
                .height(300.0)
                .show(ui, |plot_ui| {
                    plot_ui.bar_chart(chart);
                });
        }
    }
}

static MONITOR: std::sync::Mutex<Option<StatementsMonitor>> = std::sync::Mutex::new(None);

pub fn show(
    ui: &mut egui::Ui,
    pools: crate::components::db_manager::PoolRegistry,
    dsn: Option<&str>,
) {
    let mut monitor = MONITOR.lock().unwrap();
    if monitor.is_none() {
        *monitor = Some(StatementsMonitor::default());
    }
    if let Some(ref mut m) = *monitor {
        m.ui(ui, pools, dsn);
    }
}
