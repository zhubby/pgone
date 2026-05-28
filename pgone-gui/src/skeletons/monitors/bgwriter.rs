use eframe::egui;
use egui_plot::{Bar, BarChart, Plot};
use poll_promise::Promise;
use sqlx::Row;

#[derive(Clone)]
struct BgwriterData {
    checkpoints_timed: i64,
    checkpoints_req: i64,
    checkpoint_write_time: f64,
    checkpoint_sync_time: f64,
    buffers_checkpoint: i64,
    buffers_clean: i64,
    maxwritten_clean: i64,
    buffers_backend: i64,
    buffers_backend_fsync: i64,
    buffers_alloc: i64,
    stats_reset: Option<chrono::NaiveDateTime>,
}

pub struct BgwriterMonitor {
    promise: Option<Promise<Result<BgwriterData, String>>>,
    data: Option<BgwriterData>,
    error: Option<String>,
}

impl Default for BgwriterMonitor {
    fn default() -> Self {
        Self {
            promise: None,
            data: None,
            error: None,
        }
    }
}

impl BgwriterMonitor {
    fn load_data(&mut self, pools: crate::components::db_manager::PoolRegistry, dsn: Option<&str>) {
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
                let pool = pools.get_or_create_pool(&dsn).await.map_err(|e| format!("连接失败: {}", e))?;

                let row = sqlx::query(
                    r#"
                    SELECT 
                        checkpoints_timed,
                        checkpoints_req,
                        checkpoint_write_time,
                        checkpoint_sync_time,
                        buffers_checkpoint,
                        buffers_clean,
                        maxwritten_clean,
                        buffers_backend,
                        buffers_backend_fsync,
                        buffers_alloc,
                        CASE WHEN stats_reset IS NULL THEN NULL ELSE to_char(stats_reset, 'YYYY-MM-DD HH24:MI:SS.US') END as stats_reset
                    FROM pg_stat_bgwriter
                    "#,
                )
                .fetch_one(&pool)
                .await
                .map_err(|e| format!("查询失败: {}", e))?;

                let stats_reset_str: Option<String> = row.get(10);
                let stats_reset = stats_reset_str.and_then(|s| {
                    chrono::NaiveDateTime::parse_from_str(&s, "%Y-%m-%d %H:%M:%S%.f")
                        .or_else(|_| chrono::NaiveDateTime::parse_from_str(&s, "%Y-%m-%dT%H:%M:%S%.f"))
                        .ok()
                });

                Ok::<_, String>(BgwriterData {
                    checkpoints_timed: row.get(0),
                    checkpoints_req: row.get(1),
                    checkpoint_write_time: row.get::<Option<f64>, _>(2).unwrap_or(0.0),
                    checkpoint_sync_time: row.get::<Option<f64>, _>(3).unwrap_or(0.0),
                    buffers_checkpoint: row.get(4),
                    buffers_clean: row.get(5),
                    maxwritten_clean: row.get(6),
                    buffers_backend: row.get(7),
                    buffers_backend_fsync: row.get(8),
                    buffers_alloc: row.get(9),
                    stats_reset,
                })
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
        // 检查Promise状态
        if let Some(ref promise) = self.promise {
            if let Some(result) = promise.ready() {
                match result {
                    Ok(data) => {
                        self.data = Some(data.clone());
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
        if self.data.is_none() && self.error.is_none() && self.promise.is_none() {
            self.load_data(pools.clone(), dsn);
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
                self.data = None;
            }
            return;
        }

        // 显示数据
        let reset_time = self.data.as_ref().and_then(|d| d.stats_reset);
        let mut should_refresh = false;
        ui.horizontal(|ui| {
            if ui.button("刷新").clicked() {
                should_refresh = true;
            }
            if let Some(reset_time) = reset_time {
                ui.label(format!("统计重置时间: {}", reset_time));
            }
        });

        if should_refresh {
            self.data = None;
            self.error = None;
            return;
        }

        let Some(ref data) = self.data else {
            ui.label("没有数据");
            return;
        };

        ui.separator();

        // 显示表格
        egui::Grid::new("bgwriter_grid")
            .num_columns(2)
            .spacing([40.0, 4.0])
            .show(ui, |ui| {
                ui.label(egui::RichText::new("指标").strong());
                ui.label(egui::RichText::new("值").strong());
                ui.end_row();

                ui.label("定时检查点");
                ui.label(data.checkpoints_timed.to_string());
                ui.end_row();

                ui.label("请求检查点");
                ui.label(data.checkpoints_req.to_string());
                ui.end_row();

                ui.label("检查点写入时间(ms)");
                ui.label(format!("{:.2}", data.checkpoint_write_time));
                ui.end_row();

                ui.label("检查点同步时间(ms)");
                ui.label(format!("{:.2}", data.checkpoint_sync_time));
                ui.end_row();

                ui.label("检查点缓冲区");
                ui.label(data.buffers_checkpoint.to_string());
                ui.end_row();

                ui.label("清理缓冲区");
                ui.label(data.buffers_clean.to_string());
                ui.end_row();

                ui.label("最大清理写入");
                ui.label(data.maxwritten_clean.to_string());
                ui.end_row();

                ui.label("后端缓冲区");
                ui.label(data.buffers_backend.to_string());
                ui.end_row();

                ui.label("后端fsync");
                ui.label(data.buffers_backend_fsync.to_string());
                ui.end_row();

                ui.label("分配缓冲区");
                ui.label(data.buffers_alloc.to_string());
                ui.end_row();
            });

        ui.separator();

        // 显示柱状图
        let bars = vec![
            Bar::new(0.0, data.checkpoints_timed as f64)
                .width(0.6)
                .name("定时检查点"),
            Bar::new(1.0, data.checkpoints_req as f64)
                .width(0.6)
                .name("请求检查点"),
            Bar::new(2.0, data.buffers_checkpoint as f64)
                .width(0.6)
                .name("检查点缓冲区"),
            Bar::new(3.0, data.buffers_clean as f64)
                .width(0.6)
                .name("清理缓冲区"),
            Bar::new(4.0, data.buffers_backend as f64)
                .width(0.6)
                .name("后端缓冲区"),
            Bar::new(5.0, data.buffers_alloc as f64)
                .width(0.6)
                .name("分配缓冲区"),
        ];

        let chart = BarChart::new("后台写入器统计", bars);

        Plot::new("bgwriter_plot")
            .height(300.0)
            .show(ui, |plot_ui| {
                plot_ui.bar_chart(chart);
            });
    }
}

static MONITOR: std::sync::Mutex<Option<BgwriterMonitor>> = std::sync::Mutex::new(None);

pub fn show(
    ui: &mut egui::Ui,
    pools: crate::components::db_manager::PoolRegistry,
    dsn: Option<&str>,
) {
    let mut monitor = MONITOR.lock().unwrap();
    if monitor.is_none() {
        *monitor = Some(BgwriterMonitor::default());
    }
    if let Some(ref mut m) = *monitor {
        m.ui(ui, pools, dsn);
    }
}
