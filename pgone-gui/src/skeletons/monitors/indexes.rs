use eframe::egui;
use egui_plot::{Bar, BarChart, Plot};
use poll_promise::Promise;
use sqlx::Row;
use sqlx::postgres::PgPoolOptions;

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
                .map_err(|e| format!("查询失败: {}", e))?;

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
            egui::Grid::new("indexes_grid")
                .num_columns(6)
                .spacing([20.0, 4.0])
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("Schema").strong());
                    ui.label(egui::RichText::new("Table").strong());
                    ui.label(egui::RichText::new("Index").strong());
                    ui.label(egui::RichText::new("索引扫描").strong());
                    ui.label(egui::RichText::new("读取行数").strong());
                    ui.label(egui::RichText::new("获取行数").strong());
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

        // 显示柱状图 - TOP 10
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

            let chart = BarChart::new("索引扫描次数 TOP 10", bars);

            Plot::new("indexes_plot").height(300.0).show(ui, |plot_ui| {
                plot_ui.bar_chart(chart);
            });
        }
    }
}

static MONITOR: std::sync::Mutex<Option<IndexesMonitor>> = std::sync::Mutex::new(None);

pub fn show(ui: &mut egui::Ui, dsn: Option<&str>) {
    let mut monitor = MONITOR.lock().unwrap();
    if monitor.is_none() {
        *monitor = Some(IndexesMonitor::default());
    }
    if let Some(ref mut m) = *monitor {
        m.ui(ui, dsn);
    }
}
