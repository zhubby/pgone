use eframe::egui;
use poll_promise::Promise;
use sqlx::Row;
use sqlx::postgres::PgPoolOptions;

#[derive(Clone)]
struct ReplicationData {
    pid: Option<i32>,
    usesysid: Option<i32>,
    usename: Option<String>,
    application_name: Option<String>,
    client_addr: Option<String>,
    client_hostname: Option<String>,
    client_port: Option<i32>,
    backend_start: Option<chrono::NaiveDateTime>,
    state: Option<String>,
    sent_lsn: Option<String>,
    write_lsn: Option<String>,
    flush_lsn: Option<String>,
    replay_lsn: Option<String>,
    sync_priority: Option<i32>,
    sync_state: Option<String>,
}

pub struct ReplicationMonitor {
    promise: Option<Promise<Result<Vec<ReplicationData>, String>>>,
    data: Vec<ReplicationData>,
    error: Option<String>,
}

impl Default for ReplicationMonitor {
    fn default() -> Self {
        Self {
            promise: None,
            data: Vec::new(),
            error: None,
        }
    }
}

impl ReplicationMonitor {
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
                        pid,
                        usesysid,
                        usename,
                        application_name,
                        client_addr,
                        client_hostname,
                        client_port,
                        backend_start,
                        state,
                        sent_lsn,
                        write_lsn,
                        flush_lsn,
                        replay_lsn,
                        sync_priority,
                        sync_state
                    FROM pg_stat_replication
                    "#,
                )
                .fetch_all(&pool)
                .await
                .map_err(|e| format!("查询失败: {}", e))?;

                let mut data = Vec::new();
                for row in rows {
                    data.push(ReplicationData {
                        pid: row.get(0),
                        usesysid: row.get(1),
                        usename: row.get(2),
                        application_name: row.get(3),
                        client_addr: row.get(4),
                        client_hostname: row.get(5),
                        client_port: row.get(6),
                        backend_start: {
                            let ts_str: Option<String> = row.get(7);
                            ts_str.and_then(|s| {
                                chrono::NaiveDateTime::parse_from_str(&s, "%Y-%m-%d %H:%M:%S%.f")
                                    .or_else(|_| {
                                        chrono::NaiveDateTime::parse_from_str(
                                            &s,
                                            "%Y-%m-%dT%H:%M:%S%.f",
                                        )
                                    })
                                    .ok()
                            })
                        },
                        state: row.get(8),
                        sent_lsn: row.get(9),
                        write_lsn: row.get(10),
                        flush_lsn: row.get(11),
                        replay_lsn: row.get(12),
                        sync_priority: row.get(13),
                        sync_state: row.get(14),
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
            ui.label("没有复制连接（可能是单机模式）");
            return;
        }

        ui.horizontal(|ui| {
            ui.label(format!("复制连接数: {}", self.data.len()));
            if ui.button("刷新").clicked() {
                self.data.clear();
                self.error = None;
            }
        });

        ui.separator();

        // 显示表格
        egui::ScrollArea::both().show(ui, |ui| {
            for (idx, item) in self.data.iter().enumerate() {
                ui.group(|ui| {
                    ui.heading(format!("复制连接 #{}", idx + 1));

                    egui::Grid::new(format!("replication_grid_{}", idx))
                        .num_columns(2)
                        .spacing([40.0, 4.0])
                        .show(ui, |ui| {
                            if let Some(pid) = item.pid {
                                ui.label("PID:");
                                ui.label(pid.to_string());
                                ui.end_row();
                            }

                            if let Some(ref usename) = item.usename {
                                ui.label("用户名:");
                                ui.label(usename);
                                ui.end_row();
                            }

                            if let Some(ref app_name) = item.application_name {
                                ui.label("应用名称:");
                                ui.label(app_name);
                                ui.end_row();
                            }

                            if let Some(ref client_addr) = item.client_addr {
                                ui.label("客户端地址:");
                                ui.label(client_addr);
                                ui.end_row();
                            }

                            if let Some(ref state) = item.state {
                                ui.label("状态:");
                                ui.label(state);
                                ui.end_row();
                            }

                            if let Some(ref sent_lsn) = item.sent_lsn {
                                ui.label("发送LSN:");
                                ui.label(sent_lsn);
                                ui.end_row();
                            }

                            if let Some(ref write_lsn) = item.write_lsn {
                                ui.label("写入LSN:");
                                ui.label(write_lsn);
                                ui.end_row();
                            }

                            if let Some(ref flush_lsn) = item.flush_lsn {
                                ui.label("刷新LSN:");
                                ui.label(flush_lsn);
                                ui.end_row();
                            }

                            if let Some(ref replay_lsn) = item.replay_lsn {
                                ui.label("重放LSN:");
                                ui.label(replay_lsn);
                                ui.end_row();
                            }

                            if let Some(sync_priority) = item.sync_priority {
                                ui.label("同步优先级:");
                                ui.label(sync_priority.to_string());
                                ui.end_row();
                            }

                            if let Some(ref sync_state) = item.sync_state {
                                ui.label("同步状态:");
                                ui.label(sync_state);
                                ui.end_row();
                            }

                            if let Some(backend_start) = item.backend_start {
                                ui.label("后端启动时间:");
                                ui.label(backend_start.to_string());
                                ui.end_row();
                            }
                        });
                });
                ui.add_space(10.0);
            }
        });
    }
}

static MONITOR: std::sync::Mutex<Option<ReplicationMonitor>> = std::sync::Mutex::new(None);

pub fn show(ui: &mut egui::Ui, dsn: Option<&str>) {
    let mut monitor = MONITOR.lock().unwrap();
    if monitor.is_none() {
        *monitor = Some(ReplicationMonitor::default());
    }
    if let Some(ref mut m) = *monitor {
        m.ui(ui, dsn);
    }
}
