use eframe::egui;
use egui_plot::{Bar, BarChart, Plot};
use poll_promise::Promise;
use sqlx::Row;
use sqlx::postgres::PgPoolOptions;

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
    fn load_data(&mut self, dsn: Option<&str>) {
        if self.promise.is_some() {
            return;
        }

        let Some(dsn) = dsn else {
            self.error = Some("未选择数据库".to_string());
            return;
        };

        let dsn = dsn.to_string();
        let (sender, promise) = Promise::new();
        self.promise = Some(promise);

        crate::futures::spawn(async move {
            let result = async {
                let pool = PgPoolOptions::new()
                    .max_connections(1)
                    .connect(&dsn)
                    .await
                    .map_err(|e| format!("连接失败: {}", e))?;

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
                .map_err(|e| format!("查询失败: {}", e))?;

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

    fn ui(&mut self, ui: &mut egui::Ui, dsn: Option<&str>) {
        // 检查Promise状态
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

        // 如果没有数据且没有错误，开始加载
        if self.data.is_empty() && self.error.is_none() && self.promise.is_none() {
            self.load_data(dsn);
        }

        // 显示加载状态
        if self.promise.is_some() {
            ui.centered_and_justified(|ui| {
                ui.spinner();
                ui.label("加载中...");
            });
            return;
        }

        // 显示错误
        if let Some(err) = &self.error {
            ui.colored_label(egui::Color32::RED, format!("错误: {}", err));
            if ui.button("重试").clicked() {
                self.error = None;
                self.data.clear();
            }
            return;
        }

        // 显示数据
        if self.data.is_empty() {
            ui.label("没有数据");
            return;
        }

        ui.horizontal(|ui| {
            if ui.button("刷新").clicked() {
                self.data.clear();
                self.error = None;
            }
        });

        ui.separator();

        // 显示表格
        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::Grid::new("activity_grid")
                .num_columns(2)
                .spacing([40.0, 4.0])
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("状态").strong());
                    ui.label(egui::RichText::new("数量").strong());
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

        // 显示柱状图
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

            let chart = BarChart::new("连接状态分布", bars);

            Plot::new("activity_plot")
                .height(300.0)
                .show(ui, |plot_ui| {
                    plot_ui.bar_chart(chart);
                });
        }
    }
}

static MONITOR: std::sync::Mutex<Option<ActivityMonitor>> = std::sync::Mutex::new(None);

pub fn show(ui: &mut egui::Ui, dsn: Option<&str>) {
    let mut monitor = MONITOR.lock().unwrap();
    if monitor.is_none() {
        *monitor = Some(ActivityMonitor::default());
    }
    if let Some(ref mut m) = *monitor {
        m.ui(ui, dsn);
    }
}
