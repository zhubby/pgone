use eframe::egui;
use egui_plot::{Bar, BarChart, Plot};
use poll_promise::Promise;
use sqlx::postgres::PgPoolOptions;
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
    fn load_data(&mut self, dsn: Option<&str>) {
        if self.promise.is_some() {
            return;
        }

        let Some(dsn) = dsn else {
            self.error = Some("未选择数据库".to_string());
            return;
        };

        let dsn = dsn.to_string();
        let limit = self.limit;
        let (sender, promise) = Promise::new();
        self.promise = Some(promise);

        crate::futures::spawn(async move {
            let result = async {
                let pool = PgPoolOptions::new()
                    .max_connections(1)
                    .connect(&dsn)
                    .await
                    .map_err(|e| format!("连接失败: {}", e))?;

                // 检查扩展是否启用
                let ext_check: Option<(bool,)> = sqlx::query_as(
                    "SELECT EXISTS(SELECT 1 FROM pg_extension WHERE extname = 'pg_stat_statements')"
                )
                .fetch_optional(&pool)
                .await
                .map_err(|e| format!("检查扩展失败: {}", e))?
                .map(|row: (bool,)| row);

                if ext_check.map(|(exists,)| exists) != Some(true) {
                    return Err("pg_stat_statements 扩展未启用".to_string());
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
                .map_err(|e| format!("查询失败: {}", e))?;

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
            ui.label("显示数量:");
            ui.add(egui::Slider::new(&mut self.limit, 10..=100).text(""));
            if ui.button("刷新").clicked() {
                self.data.clear();
                self.error = None;
            }
        });

        ui.separator();

        // 显示表格
        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::Grid::new("statements_grid")
                .num_columns(4)
                .spacing([40.0, 4.0])
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("查询").strong());
                    ui.label(egui::RichText::new("调用次数").strong());
                    ui.label(egui::RichText::new("总时间(ms)").strong());
                    ui.label(egui::RichText::new("平均时间(ms)").strong());
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

        // 显示柱状图 - 按总时间排序
        if !self.data.is_empty() {
            let bars: Vec<Bar> = self
                .data
                .iter()
                .take(10)
                .enumerate()
                .map(|(i, item)| {
                    Bar::new(i as f64, item.total_time)
                        .width(0.6)
                        .name(format!("查询 {}", i + 1))
                })
                .collect();

            let chart = BarChart::new("查询总时间 TOP 10", bars);

            Plot::new("statements_plot")
                .height(300.0)
                .show(ui, |plot_ui| {
                    plot_ui.bar_chart(chart);
                });
        }
    }
}

static MONITOR: std::sync::Mutex<Option<StatementsMonitor>> = std::sync::Mutex::new(None);

pub fn show(ui: &mut egui::Ui, dsn: Option<&str>) {
    let mut monitor = MONITOR.lock().unwrap();
    if monitor.is_none() {
        *monitor = Some(StatementsMonitor::default());
    }
    if let Some(ref mut m) = *monitor {
        m.ui(ui, dsn);
    }
}

