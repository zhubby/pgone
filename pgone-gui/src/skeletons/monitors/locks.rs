use eframe::egui;
use poll_promise::Promise;
use sqlx::Row;

#[derive(Clone)]
struct LockData {
    locktype: String,
    database: Option<i32>, // OID type (decoded as i32 by sqlx)
    relation: Option<i32>, // OID type (decoded as i32 by sqlx)
    page: Option<i32>,
    tuple: Option<i16>,
    virtualxid: Option<String>,
    transactionid: Option<i32>,
    classid: Option<i32>, // OID type (decoded as i32 by sqlx)
    objid: Option<i32>,   // OID type (decoded as i32 by sqlx)
    objsubid: Option<i16>,
    virtualtransaction: String,
    pid: Option<i32>,
    mode: String,
    granted: bool,
    fastpath: bool,
}

pub struct LocksMonitor {
    promise: Option<Promise<Result<Vec<LockData>, String>>>,
    data: Vec<LockData>,
    error: Option<String>,
}

impl Default for LocksMonitor {
    fn default() -> Self {
        Self {
            promise: None,
            data: Vec::new(),
            error: None,
        }
    }
}

impl LocksMonitor {
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
                        locktype,
                        database,
                        relation,
                        page,
                        tuple,
                        virtualxid,
                        transactionid,
                        classid,
                        objid,
                        objsubid,
                        virtualtransaction,
                        pid,
                        mode,
                        granted,
                        fastpath
                    FROM pg_locks
                    ORDER BY granted, locktype, mode
                    "#,
                )
                .fetch_all(&pool)
                .await
                .map_err(|e| format!("Query failed: {}", e))?;

                let mut data = Vec::new();
                for row in rows {
                    data.push(LockData {
                        locktype: row.get(0),
                        database: row.get(1),
                        relation: row.get(2),
                        page: row.get(3),
                        tuple: row.get(4),
                        virtualxid: row.get(5),
                        transactionid: row.get(6),
                        classid: row.get(7),
                        objid: row.get(8),
                        objsubid: row.get(9),
                        virtualtransaction: row.get(10),
                        pid: row.get(11),
                        mode: row.get(12),
                        granted: row.get(13),
                        fastpath: row.get(14),
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

        // Statistics
        let granted_count = self.data.iter().filter(|l| l.granted).count();
        let waiting_count = self.data.len() - granted_count;

        ui.horizontal(|ui| {
            ui.label(format!("Total locks: {}", self.data.len()));
            ui.label(format!("Granted: {}", granted_count));
            ui.colored_label(
                if waiting_count > 0 {
                    egui::Color32::RED
                } else {
                    egui::Color32::GREEN
                },
                format!("Waiting: {}", waiting_count),
            );
            if ui.button("Refresh").clicked() {
                self.data.clear();
                self.error = None;
            }
        });

        ui.separator();

        // Show table
        egui::ScrollArea::both().show(ui, |ui| {
            egui::Grid::new("locks_grid")
                .num_columns(8)
                .spacing([20.0, 4.0])
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("Lock type").strong());
                    ui.label(egui::RichText::new("Mode").strong());
                    ui.label(egui::RichText::new("PID").strong());
                    ui.label(egui::RichText::new("Virtual XID").strong());
                    ui.label(egui::RichText::new("Database").strong());
                    ui.label(egui::RichText::new("Relation").strong());
                    ui.label(egui::RichText::new("Transaction ID").strong());
                    ui.label(egui::RichText::new("Status").strong());
                    ui.end_row();

                    for item in &self.data {
                        ui.label(&item.locktype);
                        ui.label(&item.mode);
                        ui.label(
                            item.pid
                                .map(|p| p.to_string())
                                .unwrap_or_else(|| "-".to_string()),
                        );
                        ui.label(&item.virtualtransaction);
                        ui.label(
                            item.database
                                .map(|d| d.to_string())
                                .unwrap_or_else(|| "-".to_string()),
                        );
                        ui.label(
                            item.relation
                                .map(|r| r.to_string())
                                .unwrap_or_else(|| "-".to_string()),
                        );
                        ui.label(
                            item.transactionid
                                .map(|t| t.to_string())
                                .unwrap_or_else(|| "-".to_string()),
                        );
                        ui.colored_label(
                            if item.granted {
                                egui::Color32::GREEN
                            } else {
                                egui::Color32::RED
                            },
                            if item.granted { "Granted" } else { "Waiting" },
                        );
                        ui.end_row();
                    }
                });
        });
    }
}

static MONITOR: std::sync::Mutex<Option<LocksMonitor>> = std::sync::Mutex::new(None);

pub fn show(
    ui: &mut egui::Ui,
    pools: crate::components::db_manager::PoolRegistry,
    dsn: Option<&str>,
) {
    let mut monitor = MONITOR.lock().unwrap();
    if monitor.is_none() {
        *monitor = Some(LocksMonitor::default());
    }
    if let Some(ref mut m) = *monitor {
        m.ui(ui, pools, dsn);
    }
}
