use eframe::egui;
use egui_plot::{Bar, BarChart, Plot};
use poll_promise::Promise;
use sqlx::Row;

#[derive(Clone)]
struct TableData {
    schemaname: String,
    tablename: String,
    seq_scan: i64,
    seq_tup_read: i64,
    idx_scan: i64,
    idx_tup_fetch: i64,
    n_tup_ins: i64,
    n_tup_upd: i64,
    n_tup_del: i64,
}

pub struct TablesMonitor {
    promise: Option<Promise<Result<Vec<TableData>, String>>>,
    data: Vec<TableData>,
    error: Option<String>,
    sort_by: SortBy,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SortBy {
    SeqScan,
    SeqTupRead,
    IdxScan,
    NtupIns,
    NtupUpd,
    NtupDel,
}

impl Default for TablesMonitor {
    fn default() -> Self {
        Self {
            promise: None,
            data: Vec::new(),
            error: None,
            sort_by: SortBy::SeqScan,
        }
    }
}

impl TablesMonitor {
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
                let pool = pools
                    .get_or_create_pool(&dsn)
                    .await
                    .map_err(|e| format!("连接失败: {}", e))?;

                let rows = sqlx::query(
                    r#"
                    SELECT 
                        schemaname,
                        tablename,
                        seq_scan,
                        seq_tup_read,
                        idx_scan,
                        idx_tup_fetch,
                        n_tup_ins,
                        n_tup_upd,
                        n_tup_del
                    FROM pg_stat_user_tables
                    ORDER BY seq_scan DESC
                    LIMIT 50
                    "#,
                )
                .fetch_all(&pool)
                .await
                .map_err(|e| format!("查询失败: {}", e))?;

                let mut data = Vec::new();
                for row in rows {
                    data.push(TableData {
                        schemaname: row.get(0),
                        tablename: row.get(1),
                        seq_scan: row.get(2),
                        seq_tup_read: row.get(3),
                        idx_scan: row.get(4),
                        idx_tup_fetch: row.get(5),
                        n_tup_ins: row.get(6),
                        n_tup_upd: row.get(7),
                        n_tup_del: row.get(8),
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
            ui.label("排序方式:");
            ui.selectable_value(&mut self.sort_by, SortBy::SeqScan, "顺序扫描");
            ui.selectable_value(&mut self.sort_by, SortBy::SeqTupRead, "顺序读取行数");
            ui.selectable_value(&mut self.sort_by, SortBy::IdxScan, "索引扫描");
            ui.selectable_value(&mut self.sort_by, SortBy::NtupIns, "插入行数");
            ui.selectable_value(&mut self.sort_by, SortBy::NtupUpd, "更新行数");
            ui.selectable_value(&mut self.sort_by, SortBy::NtupDel, "删除行数");
            if ui.button("刷新").clicked() {
                self.data.clear();
                self.error = None;
            }
        });

        ui.separator();

        // 排序数据
        let mut sorted_data = self.data.clone();
        sorted_data.sort_by(|a, b| {
            let a_val = match self.sort_by {
                SortBy::SeqScan => a.seq_scan,
                SortBy::SeqTupRead => a.seq_tup_read,
                SortBy::IdxScan => a.idx_scan,
                SortBy::NtupIns => a.n_tup_ins,
                SortBy::NtupUpd => a.n_tup_upd,
                SortBy::NtupDel => a.n_tup_del,
            };
            let b_val = match self.sort_by {
                SortBy::SeqScan => b.seq_scan,
                SortBy::SeqTupRead => b.seq_tup_read,
                SortBy::IdxScan => b.idx_scan,
                SortBy::NtupIns => b.n_tup_ins,
                SortBy::NtupUpd => b.n_tup_upd,
                SortBy::NtupDel => b.n_tup_del,
            };
            b_val.cmp(&a_val)
        });

        // 显示表格
        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::Grid::new("tables_grid")
                .num_columns(9)
                .spacing([20.0, 4.0])
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("Schema").strong());
                    ui.label(egui::RichText::new("Table").strong());
                    ui.label(egui::RichText::new("顺序扫描").strong());
                    ui.label(egui::RichText::new("顺序读取行").strong());
                    ui.label(egui::RichText::new("索引扫描").strong());
                    ui.label(egui::RichText::new("插入").strong());
                    ui.label(egui::RichText::new("更新").strong());
                    ui.label(egui::RichText::new("删除").strong());
                    ui.end_row();

                    for item in sorted_data.iter().take(30) {
                        ui.label(&item.schemaname);
                        ui.label(&item.tablename);
                        ui.label(item.seq_scan.to_string());
                        ui.label(item.seq_tup_read.to_string());
                        ui.label(item.idx_scan.to_string());
                        ui.label(item.n_tup_ins.to_string());
                        ui.label(item.n_tup_upd.to_string());
                        ui.label(item.n_tup_del.to_string());
                        ui.end_row();
                    }
                });
        });

        ui.separator();

        // 显示柱状图 - TOP 10
        if !sorted_data.is_empty() {
            let bars: Vec<Bar> = sorted_data
                .iter()
                .take(10)
                .enumerate()
                .map(|(i, item)| {
                    let value = match self.sort_by {
                        SortBy::SeqScan => item.seq_scan as f64,
                        SortBy::SeqTupRead => item.seq_tup_read as f64,
                        SortBy::IdxScan => item.idx_scan as f64,
                        SortBy::NtupIns => item.n_tup_ins as f64,
                        SortBy::NtupUpd => item.n_tup_upd as f64,
                        SortBy::NtupDel => item.n_tup_del as f64,
                    };
                    Bar::new(i as f64, value)
                        .width(0.6)
                        .name(format!("{}.{}", item.schemaname, item.tablename))
                })
                .collect();

            let chart_name = match self.sort_by {
                SortBy::SeqScan => "顺序扫描 TOP 10",
                SortBy::SeqTupRead => "顺序读取行数 TOP 10",
                SortBy::IdxScan => "索引扫描 TOP 10",
                SortBy::NtupIns => "插入行数 TOP 10",
                SortBy::NtupUpd => "更新行数 TOP 10",
                SortBy::NtupDel => "删除行数 TOP 10",
            };

            let chart = BarChart::new(chart_name, bars);

            Plot::new("tables_plot").height(300.0).show(ui, |plot_ui| {
                plot_ui.bar_chart(chart);
            });
        }
    }
}

static MONITOR: std::sync::Mutex<Option<TablesMonitor>> = std::sync::Mutex::new(None);

pub fn show(
    ui: &mut egui::Ui,
    pools: crate::components::db_manager::PoolRegistry,
    dsn: Option<&str>,
) {
    let mut monitor = MONITOR.lock().unwrap();
    if monitor.is_none() {
        *monitor = Some(TablesMonitor::default());
    }
    if let Some(ref mut m) = *monitor {
        m.ui(ui, pools, dsn);
    }
}
