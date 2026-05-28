use eframe::egui;
use egui_plot::{Bar, BarChart, Plot};
use poll_promise::Promise;
use sqlx::Row;

#[derive(Clone)]
struct ActivityData {
    state: Option<String>,
    count: i64,
}

pub struct ActivityMonitor {
    promise: Option<Promise<Result<Vec<ActivityData>, String>>>,
    data: Vec<ActivityData>,
    error: Option<String>,
}

impl Default for ActivityMonitor {
    fn default() -> Self {
        Self {
            promise: None,
            data: Vec::new(),
            error: None,
        }
    }
}

impl ActivityMonitor {
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
                    SELECT state, COUNT(*) as count
                    FROM pg_stat_activity
                    GROUP BY state
                    ORDER BY count DESC
                    "#,
                )
                .fetch_all(&pool)
                .await
                .map_err(|e| format!("Query failed: {}", e))?;

                let mut data = Vec::new();
                for row in rows {
                    let state: Option<String> = row.get(0);
                    let count: i64 = row.get(1);
                    data.push(ActivityData { state, count });
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
            egui::Grid::new("activity_grid")
                .num_columns(2)
                .spacing([40.0, 4.0])
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("Status").strong());
                    ui.label(egui::RichText::new("Count").strong());
                    ui.end_row();

                    for item in &self.data {
                        let state_display = item.state.as_deref().unwrap_or("NULL");
                        ui.label(state_display);
                        ui.label(item.count.to_string());
                        ui.end_row();
                    }
                });
        });

        ui.separator();

        // Show bar chart
        if !self.data.is_empty() {
            let bars: Vec<Bar> = self
                .data
                .iter()
                .enumerate()
                .map(|(i, item)| {
                    let name = item.state.as_deref().unwrap_or("NULL");
                    Bar::new(i as f64, item.count as f64).width(0.6).name(name)
                })
                .collect();

            let chart = BarChart::new("Connection status distribution", bars);

            Plot::new("activity_plot")
                .height(300.0)
                .show(ui, |plot_ui| {
                    plot_ui.bar_chart(chart);
                });
        }
    }
}

static MONITOR: std::sync::Mutex<Option<ActivityMonitor>> = std::sync::Mutex::new(None);

pub fn show(
    ui: &mut egui::Ui,
    pools: crate::components::db_manager::PoolRegistry,
    dsn: Option<&str>,
) {
    let mut monitor = MONITOR.lock().unwrap();
    if monitor.is_none() {
        *monitor = Some(ActivityMonitor::default());
    }
    if let Some(ref mut m) = *monitor {
        m.ui(ui, pools, dsn);
    }
}
