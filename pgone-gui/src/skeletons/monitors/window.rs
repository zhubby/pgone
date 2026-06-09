use super::MonitorPanel;
use crate::components::DbManager;
use eframe::egui::{self, Align2, Color32, Context, Id, RichText, Window};
use egui_extras::{Column, TableBuilder};
use egui_plot::{Line, Plot, PlotPoints};
use pgone_sql::{
    ActivityRow, BackgroundWriterStats, IndexHealthRow, IndexHealthSort, IoStats, LockRow,
    MaintenanceProgressRow, MonitorCapabilities, MonitorSnapshot, OptionalMonitorData,
    ReplicationRow, Session, StatementOptions, StatementRow, StatementSort, TableHealthRow,
    TableHealthSort, WalStats,
};
use poll_promise::Promise;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

const HISTORY_LIMIT: usize = 60;
const MONITOR_QUERY_TIMEOUT: Duration = Duration::from_secs(8);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RefreshCadence {
    Manual,
    Seconds5,
    Seconds10,
    Seconds30,
}

impl RefreshCadence {
    const ALL: [Self; 4] = [
        Self::Manual,
        Self::Seconds5,
        Self::Seconds10,
        Self::Seconds30,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Manual => "Manual",
            Self::Seconds5 => "5s",
            Self::Seconds10 => "10s",
            Self::Seconds30 => "30s",
        }
    }

    fn duration(self) -> Option<Duration> {
        match self {
            Self::Manual => None,
            Self::Seconds5 => Some(Duration::from_secs(5)),
            Self::Seconds10 => Some(Duration::from_secs(10)),
            Self::Seconds30 => Some(Duration::from_secs(30)),
        }
    }
}

#[derive(Debug, Clone)]
struct SnapshotPoint {
    active_connections: f64,
    waiting_locks: f64,
    cache_hit_ratio: f64,
}

#[derive(Debug, Clone)]
enum MonitorPayload {
    Overview,
    Activity(Vec<ActivityRow>),
    Queries(OptionalMonitorData<Vec<StatementRow>>),
    Storage {
        tables: Vec<TableHealthRow>,
        indexes: Vec<IndexHealthRow>,
    },
    Locks(Vec<LockRow>),
    Replication(Vec<ReplicationRow>),
    WalIo {
        wal: OptionalMonitorData<Option<WalStats>>,
        io: OptionalMonitorData<Option<IoStats>>,
        bgwriter: BackgroundWriterStats,
    },
    Maintenance(OptionalMonitorData<Vec<MaintenanceProgressRow>>),
}

#[derive(Debug, Clone)]
struct MonitorLoadResult {
    panel: MonitorPanel,
    capabilities: MonitorCapabilities,
    snapshot: MonitorSnapshot,
    payload: MonitorPayload,
}

pub struct MonitorWorkbench {
    open: bool,
    active_panel: MonitorPanel,
    refresh_cadence: RefreshCadence,
    last_refresh: Option<Instant>,
    current_dsn: Option<String>,
    refresh_requested: bool,
    promise: Option<Promise<Result<MonitorLoadResult, String>>>,
    loading_panel: Option<MonitorPanel>,
    error: Option<String>,
    capabilities: Option<MonitorCapabilities>,
    snapshot: Option<MonitorSnapshot>,
    history: VecDeque<SnapshotPoint>,
    activity: Vec<ActivityRow>,
    statements: OptionalMonitorData<Vec<StatementRow>>,
    tables: Vec<TableHealthRow>,
    indexes: Vec<IndexHealthRow>,
    locks: Vec<LockRow>,
    replication: Vec<ReplicationRow>,
    wal: OptionalMonitorData<Option<WalStats>>,
    io: OptionalMonitorData<Option<IoStats>>,
    bgwriter: Option<BackgroundWriterStats>,
    maintenance: OptionalMonitorData<Vec<MaintenanceProgressRow>>,
    statement_options: StatementOptions,
    table_sort: TableHealthSort,
    index_sort: IndexHealthSort,
    row_limit: i64,
}

impl Default for MonitorWorkbench {
    fn default() -> Self {
        Self {
            open: false,
            active_panel: MonitorPanel::Overview,
            refresh_cadence: RefreshCadence::Manual,
            last_refresh: None,
            current_dsn: None,
            refresh_requested: false,
            promise: None,
            loading_panel: None,
            error: None,
            capabilities: None,
            snapshot: None,
            history: VecDeque::new(),
            activity: Vec::new(),
            statements: OptionalMonitorData::default(),
            tables: Vec::new(),
            indexes: Vec::new(),
            locks: Vec::new(),
            replication: Vec::new(),
            wal: OptionalMonitorData::default(),
            io: OptionalMonitorData::default(),
            bgwriter: None,
            maintenance: OptionalMonitorData::default(),
            statement_options: StatementOptions::default(),
            table_sort: TableHealthSort::default(),
            index_sort: IndexHealthSort::default(),
            row_limit: 50,
        }
    }
}

impl MonitorWorkbench {
    pub fn open_at(&mut self, panel: MonitorPanel) {
        self.open = true;
        if self.active_panel != panel {
            self.active_panel = panel;
            self.error = None;
            self.refresh_requested = true;
        }
    }

    pub fn show(&mut self, ctx: &Context, db_manager: &mut DbManager) {
        self.poll_promise();

        if !self.open {
            return;
        }

        let dsn = db_manager.active_dsn();
        if self.current_dsn != dsn {
            self.current_dsn = dsn.clone();
            self.clear_data_for_new_connection();
            self.start_load(db_manager.pools.clone(), dsn.as_deref());
        }

        if self.snapshot.is_none() && self.promise.is_none() && self.error.is_none() {
            self.start_load(db_manager.pools.clone(), dsn.as_deref());
        }

        if self.refresh_requested && self.promise.is_none() {
            self.refresh_requested = false;
            self.start_load(db_manager.pools.clone(), dsn.as_deref());
        }

        if self.should_auto_refresh() {
            self.start_load(db_manager.pools.clone(), dsn.as_deref());
        }

        let mut open = self.open;
        Window::new("PostgreSQL Monitor")
            .id(Id::new("monitor_workbench_window"))
            .open(&mut open)
            .default_pos(screen_center(ctx))
            .pivot(Align2::CENTER_CENTER)
            .default_size([1120.0, 720.0])
            .min_size([860.0, 520.0])
            .show(ctx, |ui| {
                self.ui(ui, ctx, db_manager.pools.clone(), dsn.as_deref());
            });
        self.open = open;

        if self.promise.is_some() {
            ctx.request_repaint_after(Duration::from_millis(100));
        } else if let Some(interval) = self.refresh_cadence.duration() {
            ctx.request_repaint_after(interval);
        }
    }

    fn ui(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &Context,
        pools: crate::components::db_manager::PoolRegistry,
        dsn: Option<&str>,
    ) {
        self.show_header(ui, pools, dsn);
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.set_height(ui.available_height());
            self.show_navigation(ui);
            ui.separator();
            ui.vertical(|ui| {
                ui.set_width(ui.available_width());
                self.show_panel_header(ui);
                ui.add_space(8.0);

                if let Some(error) = &self.error {
                    error_banner(ui, error);
                    ui.add_space(8.0);
                }

                if self.promise.is_some() && self.snapshot.is_none() {
                    centered_status(ui, "Loading monitor data...");
                    return;
                }

                match self.active_panel {
                    MonitorPanel::Overview => self.show_overview(ui),
                    MonitorPanel::Activity => self.show_activity(ui),
                    MonitorPanel::Queries => self.show_queries(ui),
                    MonitorPanel::Storage => self.show_storage(ui),
                    MonitorPanel::Locks => self.show_locks(ui),
                    MonitorPanel::Replication => self.show_replication(ui),
                    MonitorPanel::WalIo => self.show_wal_io(ui),
                    MonitorPanel::Maintenance => self.show_maintenance(ui),
                }

                if self.promise.is_some() {
                    ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                        ui.label(
                            RichText::new(format!(
                                "{} Refreshing {}...",
                                egui_phosphor::regular::CIRCLE_NOTCH,
                                self.loading_panel
                                    .map(|panel| panel.title())
                                    .unwrap_or("monitor")
                            ))
                            .small()
                            .color(ui.visuals().weak_text_color()),
                        );
                    });
                    ctx.request_repaint_after(Duration::from_millis(100));
                }
            });
        });
    }

    fn show_header(
        &mut self,
        ui: &mut egui::Ui,
        pools: crate::components::db_manager::PoolRegistry,
        dsn: Option<&str>,
    ) {
        ui.horizontal(|ui| {
            ui.heading(format!(
                "{} PostgreSQL Monitor",
                egui_phosphor::regular::GAUGE
            ));
            if let Some(snapshot) = &self.snapshot {
                status_pill(ui, &snapshot.database_name, Tone::Info);
                ui.label(
                    RichText::new(&snapshot.server_version)
                        .small()
                        .color(ui.visuals().weak_text_color()),
                );
            } else {
                status_pill(ui, "No database", Tone::Warning);
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .button(egui_phosphor::regular::ARROW_CLOCKWISE)
                    .on_hover_text("Refresh monitor data")
                    .clicked()
                {
                    self.start_load(pools, dsn);
                }

                egui::ComboBox::from_id_salt("monitor_refresh_cadence")
                    .selected_text(self.refresh_cadence.label())
                    .width(86.0)
                    .show_ui(ui, |ui| {
                        for cadence in RefreshCadence::ALL {
                            ui.selectable_value(
                                &mut self.refresh_cadence,
                                cadence,
                                cadence.label(),
                            );
                        }
                    });
                ui.label(RichText::new("Refresh").small());

                let last_refresh = self
                    .last_refresh
                    .map(|instant| format!("updated {} ago", format_duration(instant.elapsed())))
                    .unwrap_or_else(|| "not loaded".to_string());
                ui.label(
                    RichText::new(last_refresh)
                        .small()
                        .color(ui.visuals().weak_text_color()),
                );
            });
        });
    }

    fn show_navigation(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            ui.set_width(168.0);
            ui.spacing_mut().item_spacing.y = 3.0;
            for panel in MonitorPanel::ALL {
                let selected = self.active_panel == panel;
                let response = ui
                    .selectable_label(selected, format!("{} {}", panel.icon(), panel.title()))
                    .on_hover_text(panel.subtitle());
                if response.clicked() {
                    self.active_panel = panel;
                    self.error = None;
                    self.promise = None;
                    self.refresh_requested = true;
                }
            }

            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                if let Some(capabilities) = &self.capabilities {
                    ui.label(
                        RichText::new("Capabilities")
                            .small()
                            .color(ui.visuals().weak_text_color()),
                    );
                    capability_label(ui, "statements", capabilities.pg_stat_statements);
                    capability_label(ui, "pg_stat_io", capabilities.pg_stat_io);
                    capability_label(ui, "pg_stat_wal", capabilities.pg_stat_wal);
                }
            });
        });
    }

    fn show_panel_header(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading(format!(
                "{} {}",
                self.active_panel.icon(),
                self.active_panel.title()
            ));
            ui.label(
                RichText::new(self.active_panel.subtitle())
                    .small()
                    .color(ui.visuals().weak_text_color()),
            );
        });
    }

    fn show_overview(&self, ui: &mut egui::Ui) {
        let Some(snapshot) = &self.snapshot else {
            centered_status(ui, "No monitor snapshot");
            return;
        };

        ui.columns(4, |columns| {
            metric_card(
                &mut columns[0],
                "Connections",
                &format!(
                    "{} / {}",
                    snapshot.total_connections, snapshot.max_connections
                ),
                &format!(
                    "{} active, {} idle",
                    snapshot.active_connections, snapshot.idle_connections
                ),
                if snapshot.total_connections * 100 / snapshot.max_connections.max(1) > 80 {
                    Tone::Warning
                } else {
                    Tone::Good
                },
            );
            metric_card(
                &mut columns[1],
                "Cache Hit",
                &snapshot
                    .cache_hit_ratio
                    .map(format_percent)
                    .unwrap_or_else(|| "n/a".to_string()),
                &format!(
                    "{} hits, {} reads",
                    format_count(snapshot.blks_hit),
                    format_count(snapshot.blks_read)
                ),
                if snapshot.cache_hit_ratio.unwrap_or(1.0) < 0.95 {
                    Tone::Warning
                } else {
                    Tone::Good
                },
            );
            metric_card(
                &mut columns[2],
                "Locks Waiting",
                &snapshot.waiting_lock_count.to_string(),
                &format!("{} locks total", snapshot.lock_count),
                if snapshot.waiting_lock_count > 0 {
                    Tone::Bad
                } else {
                    Tone::Good
                },
            );
            metric_card(
                &mut columns[3],
                "Replication",
                &snapshot.replication_clients.to_string(),
                "streaming clients",
                Tone::Info,
            );
        });

        ui.add_space(8.0);
        ui.columns(4, |columns| {
            metric_card(
                &mut columns[0],
                "Transactions",
                &format_count(snapshot.xact_commit + snapshot.xact_rollback),
                &format!(
                    "{} commits, {} rollbacks",
                    format_count(snapshot.xact_commit),
                    format_count(snapshot.xact_rollback)
                ),
                Tone::Neutral,
            );
            metric_card(
                &mut columns[1],
                "Deadlocks",
                &format_count(snapshot.deadlocks),
                "since stats reset",
                if snapshot.deadlocks > 0 {
                    Tone::Warning
                } else {
                    Tone::Good
                },
            );
            metric_card(
                &mut columns[2],
                "Temp Bytes",
                &format_bytes(snapshot.temp_bytes),
                "temp files written",
                Tone::Neutral,
            );
            metric_card(
                &mut columns[3],
                "WAL Bytes",
                &snapshot
                    .wal_bytes
                    .as_deref()
                    .map(format_numeric_bytes)
                    .unwrap_or_else(|| "n/a".to_string()),
                "pg_stat_wal",
                if snapshot.wal_bytes.is_some() {
                    Tone::Info
                } else {
                    Tone::Neutral
                },
            );
        });

        ui.add_space(12.0);
        if self.history.len() > 1 {
            ui.label(RichText::new("Short trend").strong());
            Plot::new("monitor_overview_trend")
                .height(220.0)
                .allow_scroll(false)
                .allow_zoom(false)
                .show(ui, |plot_ui| {
                    plot_ui.line(Line::new(
                        "Active connections",
                        history_points(&self.history, |point| point.active_connections),
                    ));
                    plot_ui.line(Line::new(
                        "Waiting locks",
                        history_points(&self.history, |point| point.waiting_locks),
                    ));
                    plot_ui.line(Line::new(
                        "Cache hit %",
                        history_points(&self.history, |point| point.cache_hit_ratio * 100.0),
                    ));
                });
        } else {
            centered_status(ui, "Refresh again to build a short trend");
        }
    }

    fn show_activity(&self, ui: &mut egui::Ui) {
        if self.activity.is_empty() {
            centered_status(ui, "No activity rows");
            return;
        }

        let active = self
            .activity
            .iter()
            .filter(|row| row.state.as_deref() == Some("active"))
            .count();
        let waiting = self
            .activity
            .iter()
            .filter(|row| row.wait_event_type.is_some())
            .count();
        ui.horizontal(|ui| {
            status_pill(ui, &format!("{} sessions", self.activity.len()), Tone::Info);
            status_pill(ui, &format!("{active} active"), Tone::Good);
            status_pill(
                ui,
                &format!("{waiting} waiting"),
                if waiting > 0 {
                    Tone::Warning
                } else {
                    Tone::Neutral
                },
            );
        });
        ui.add_space(8.0);

        table(ui, "monitor_activity_table", |table| {
            table
                .column(Column::initial(62.0).at_least(48.0))
                .column(Column::initial(96.0).at_least(72.0))
                .column(Column::initial(96.0).at_least(72.0))
                .column(Column::initial(130.0).at_least(96.0))
                .column(Column::initial(110.0).at_least(72.0))
                .column(Column::remainder().at_least(240.0))
                .header(22.0, |mut header| {
                    header_text(&mut header, "PID");
                    header_text(&mut header, "State");
                    header_text(&mut header, "Wait");
                    header_text(&mut header, "Age");
                    header_text(&mut header, "App");
                    header_text(&mut header, "Query");
                })
                .body(|mut body| {
                    for row in &self.activity {
                        body.row(24.0, |mut table_row| {
                            table_row.col(|ui| {
                                ui.label(row.pid.to_string());
                            });
                            table_row.col(|ui| state_label(ui, row.state.as_deref()));
                            table_row.col(|ui| {
                                ui.label(
                                    row.wait_event_type
                                        .as_deref()
                                        .or(row.wait_event.as_deref())
                                        .unwrap_or("-"),
                                );
                            });
                            table_row.col(|ui| {
                                ui.label(
                                    row.query_age_seconds
                                        .map(format_seconds)
                                        .unwrap_or_else(|| "-".to_string()),
                                );
                            });
                            table_row.col(|ui| {
                                clipped_label(ui, row.application_name.as_deref().unwrap_or("-"))
                            });
                            table_row.col(|ui| clipped_label(ui, &row.query));
                        });
                    }
                });
        });
    }

    fn show_queries(&mut self, ui: &mut egui::Ui) {
        let mut refresh = false;
        ui.horizontal(|ui| {
            ui.label("Sort");
            refresh |= ui
                .selectable_value(
                    &mut self.statement_options.sort,
                    StatementSort::TotalExecTime,
                    "Total time",
                )
                .clicked();
            refresh |= ui
                .selectable_value(
                    &mut self.statement_options.sort,
                    StatementSort::MeanExecTime,
                    "Mean time",
                )
                .clicked();
            refresh |= ui
                .selectable_value(
                    &mut self.statement_options.sort,
                    StatementSort::Calls,
                    "Calls",
                )
                .clicked();
            refresh |= ui
                .selectable_value(
                    &mut self.statement_options.sort,
                    StatementSort::Rows,
                    "Rows",
                )
                .clicked();
            refresh |= ui
                .selectable_value(
                    &mut self.statement_options.sort,
                    StatementSort::SharedReads,
                    "Reads",
                )
                .clicked();
            ui.separator();
            ui.label("Limit");
            refresh |= ui
                .add(egui::Slider::new(&mut self.statement_options.limit, 10..=100).text(""))
                .changed();
        });
        if refresh {
            self.promise = None;
            self.refresh_requested = true;
        }

        if !self.statements.available {
            optional_banner(ui, &self.statements);
            return;
        }
        if self.statements.data.is_empty() {
            centered_status(ui, "No pg_stat_statements rows");
            return;
        }

        ui.add_space(8.0);
        table(ui, "monitor_statements_table", |table| {
            table
                .column(Column::remainder().at_least(360.0))
                .column(Column::initial(84.0).at_least(64.0))
                .column(Column::initial(92.0).at_least(72.0))
                .column(Column::initial(108.0).at_least(84.0))
                .column(Column::initial(108.0).at_least(84.0))
                .column(Column::initial(94.0).at_least(72.0))
                .header(22.0, |mut header| {
                    header_text(&mut header, "Query");
                    header_text(&mut header, "Calls");
                    header_text(&mut header, "Rows");
                    header_text(&mut header, "Total ms");
                    header_text(&mut header, "Mean ms");
                    header_text(&mut header, "Reads");
                })
                .body(|mut body| {
                    for row in &self.statements.data {
                        body.row(28.0, |mut table_row| {
                            table_row.col(|ui| clipped_label(ui, &row.query));
                            table_row.col(|ui| {
                                ui.label(format_count(row.calls));
                            });
                            table_row.col(|ui| {
                                ui.label(format_count(row.rows));
                            });
                            table_row.col(|ui| {
                                ui.label(format!("{:.1}", row.total_exec_time_ms));
                            });
                            table_row.col(|ui| {
                                ui.label(format!("{:.2}", row.mean_exec_time_ms));
                            });
                            table_row.col(|ui| {
                                ui.label(format_count(row.shared_blks_read));
                            });
                        });
                    }
                });
        });
    }

    fn show_storage(&mut self, ui: &mut egui::Ui) {
        let mut refresh = false;
        ui.horizontal(|ui| {
            ui.label("Tables");
            refresh |= ui
                .selectable_value(&mut self.table_sort, TableHealthSort::Size, "Size")
                .clicked();
            refresh |= ui
                .selectable_value(
                    &mut self.table_sort,
                    TableHealthSort::DeadTuples,
                    "Dead rows",
                )
                .clicked();
            refresh |= ui
                .selectable_value(
                    &mut self.table_sort,
                    TableHealthSort::DeadTupleRatio,
                    "Dead %",
                )
                .clicked();
            refresh |= ui
                .selectable_value(&mut self.table_sort, TableHealthSort::SeqScan, "Seq scan")
                .clicked();
            refresh |= ui
                .selectable_value(&mut self.table_sort, TableHealthSort::Writes, "Writes")
                .clicked();
            ui.separator();
            ui.label("Indexes");
            refresh |= ui
                .selectable_value(
                    &mut self.index_sort,
                    IndexHealthSort::LeastScanned,
                    "Least used",
                )
                .clicked();
            refresh |= ui
                .selectable_value(
                    &mut self.index_sort,
                    IndexHealthSort::MostScanned,
                    "Most used",
                )
                .clicked();
            refresh |= ui
                .selectable_value(&mut self.index_sort, IndexHealthSort::Size, "Size")
                .clicked();
        });
        if refresh {
            self.promise = None;
            self.refresh_requested = true;
        }

        ui.add_space(8.0);
        ui.label(RichText::new("Tables").strong());
        if self.tables.is_empty() {
            ui.label(RichText::new("No user table stats").color(ui.visuals().weak_text_color()));
        } else {
            table(ui, "monitor_tables_table", |table| {
                table
                    .column(Column::initial(120.0).at_least(88.0))
                    .column(Column::remainder().at_least(180.0))
                    .column(Column::initial(90.0).at_least(70.0))
                    .column(Column::initial(92.0).at_least(70.0))
                    .column(Column::initial(86.0).at_least(70.0))
                    .column(Column::initial(86.0).at_least(70.0))
                    .column(Column::initial(94.0).at_least(72.0))
                    .header(22.0, |mut header| {
                        header_text(&mut header, "Schema");
                        header_text(&mut header, "Table");
                        header_text(&mut header, "Size");
                        header_text(&mut header, "Live");
                        header_text(&mut header, "Dead");
                        header_text(&mut header, "Dead %");
                        header_text(&mut header, "Seq scan");
                    })
                    .body(|mut body| {
                        for row in &self.tables {
                            body.row(24.0, |mut table_row| {
                                table_row.col(|ui| clipped_label(ui, &row.schema_name));
                                table_row.col(|ui| clipped_label(ui, &row.table_name));
                                table_row.col(|ui| {
                                    ui.label(&row.size_pretty);
                                });
                                table_row.col(|ui| {
                                    ui.label(format_count(row.live_tuples));
                                });
                                table_row.col(|ui| {
                                    ui.label(format_count(row.dead_tuples));
                                });
                                table_row.col(|ui| {
                                    ui.label(format_percent(row.dead_tuple_ratio));
                                });
                                table_row.col(|ui| {
                                    ui.label(format_count(row.seq_scan));
                                });
                            });
                        }
                    });
            });
        }

        ui.add_space(10.0);
        ui.label(RichText::new("Indexes").strong());
        if self.indexes.is_empty() {
            ui.label(RichText::new("No user index stats").color(ui.visuals().weak_text_color()));
        } else {
            table(ui, "monitor_indexes_table", |table| {
                table
                    .column(Column::initial(120.0).at_least(88.0))
                    .column(Column::initial(150.0).at_least(100.0))
                    .column(Column::remainder().at_least(180.0))
                    .column(Column::initial(86.0).at_least(70.0))
                    .column(Column::initial(88.0).at_least(70.0))
                    .column(Column::initial(84.0).at_least(64.0))
                    .header(22.0, |mut header| {
                        header_text(&mut header, "Schema");
                        header_text(&mut header, "Table");
                        header_text(&mut header, "Index");
                        header_text(&mut header, "Size");
                        header_text(&mut header, "Scans");
                        header_text(&mut header, "Status");
                    })
                    .body(|mut body| {
                        for row in &self.indexes {
                            body.row(24.0, |mut table_row| {
                                table_row.col(|ui| clipped_label(ui, &row.schema_name));
                                table_row.col(|ui| clipped_label(ui, &row.table_name));
                                table_row.col(|ui| clipped_label(ui, &row.index_name));
                                table_row.col(|ui| {
                                    ui.label(&row.size_pretty);
                                });
                                table_row.col(|ui| {
                                    ui.label(format_count(row.idx_scan));
                                });
                                table_row.col(|ui| {
                                    let label = if row.valid { "valid" } else { "invalid" };
                                    status_pill(
                                        ui,
                                        label,
                                        if row.valid { Tone::Good } else { Tone::Bad },
                                    );
                                });
                            });
                        }
                    });
            });
        }
    }

    fn show_locks(&self, ui: &mut egui::Ui) {
        if self.locks.is_empty() {
            centered_status(ui, "No lock rows");
            return;
        }

        let waiting = self.locks.iter().filter(|row| !row.granted).count();
        ui.horizontal(|ui| {
            status_pill(ui, &format!("{} locks", self.locks.len()), Tone::Info);
            status_pill(
                ui,
                &format!("{waiting} waiting"),
                if waiting > 0 { Tone::Bad } else { Tone::Good },
            );
        });
        ui.add_space(8.0);

        table(ui, "monitor_locks_table", |table| {
            table
                .column(Column::initial(120.0).at_least(90.0))
                .column(Column::initial(160.0).at_least(110.0))
                .column(Column::initial(72.0).at_least(56.0))
                .column(Column::initial(110.0).at_least(80.0))
                .column(Column::initial(130.0).at_least(90.0))
                .column(Column::remainder().at_least(220.0))
                .header(22.0, |mut header| {
                    header_text(&mut header, "Type");
                    header_text(&mut header, "Mode");
                    header_text(&mut header, "PID");
                    header_text(&mut header, "Status");
                    header_text(&mut header, "Relation");
                    header_text(&mut header, "Query");
                })
                .body(|mut body| {
                    for row in &self.locks {
                        body.row(26.0, |mut table_row| {
                            table_row.col(|ui| clipped_label(ui, &row.lock_type));
                            table_row.col(|ui| clipped_label(ui, &row.mode));
                            table_row.col(|ui| {
                                ui.label(
                                    row.pid
                                        .map(|pid| pid.to_string())
                                        .unwrap_or_else(|| "-".into()),
                                );
                            });
                            table_row.col(|ui| {
                                status_pill(
                                    ui,
                                    if row.granted { "granted" } else { "waiting" },
                                    if row.granted { Tone::Good } else { Tone::Bad },
                                );
                            });
                            table_row.col(|ui| {
                                clipped_label(ui, row.relation.as_deref().unwrap_or("-"))
                            });
                            table_row
                                .col(|ui| clipped_label(ui, row.query.as_deref().unwrap_or("-")));
                        });
                    }
                });
        });
    }

    fn show_replication(&self, ui: &mut egui::Ui) {
        if self.replication.is_empty() {
            centered_status(ui, "No streaming replication clients");
            return;
        }

        table(ui, "monitor_replication_table", |table| {
            table
                .column(Column::initial(70.0).at_least(56.0))
                .column(Column::initial(140.0).at_least(96.0))
                .column(Column::initial(120.0).at_least(90.0))
                .column(Column::initial(92.0).at_least(72.0))
                .column(Column::initial(92.0).at_least(72.0))
                .column(Column::initial(108.0).at_least(80.0))
                .column(Column::remainder().at_least(220.0))
                .header(22.0, |mut header| {
                    header_text(&mut header, "PID");
                    header_text(&mut header, "Application");
                    header_text(&mut header, "Client");
                    header_text(&mut header, "State");
                    header_text(&mut header, "Sync");
                    header_text(&mut header, "Lag bytes");
                    header_text(&mut header, "LSN");
                })
                .body(|mut body| {
                    for row in &self.replication {
                        body.row(26.0, |mut table_row| {
                            table_row.col(|ui| {
                                ui.label(
                                    row.pid
                                        .map(|pid| pid.to_string())
                                        .unwrap_or_else(|| "-".into()),
                                );
                            });
                            table_row.col(|ui| {
                                clipped_label(ui, row.application_name.as_deref().unwrap_or("-"))
                            });
                            table_row.col(|ui| {
                                clipped_label(ui, row.client_addr.as_deref().unwrap_or("-"))
                            });
                            table_row.col(|ui| state_label(ui, row.state.as_deref()));
                            table_row.col(|ui| {
                                clipped_label(ui, row.sync_state.as_deref().unwrap_or("-"))
                            });
                            table_row.col(|ui| {
                                ui.label(
                                    row.replay_lag_bytes
                                        .map(format_count)
                                        .unwrap_or_else(|| "-".into()),
                                );
                            });
                            table_row.col(|ui| {
                                clipped_label(
                                    ui,
                                    &format!(
                                        "sent {} / replay {}",
                                        row.sent_lsn.as_deref().unwrap_or("-"),
                                        row.replay_lsn.as_deref().unwrap_or("-")
                                    ),
                                );
                            });
                        });
                    }
                });
        });
    }

    fn show_wal_io(&self, ui: &mut egui::Ui) {
        ui.columns(3, |columns| {
            if let Some(wal) = self.wal.data.as_ref() {
                metric_card(
                    &mut columns[0],
                    "WAL Bytes",
                    &format_numeric_bytes(&wal.wal_bytes),
                    &format!("{} records", format_count(wal.wal_records)),
                    Tone::Info,
                );
            } else {
                metric_card(
                    &mut columns[0],
                    "WAL",
                    if self.wal.available {
                        "No data"
                    } else {
                        "Unavailable"
                    },
                    self.wal
                        .unavailable_reason
                        .as_deref()
                        .unwrap_or("pg_stat_wal"),
                    Tone::Neutral,
                );
            }

            if let Some(io) = self.io.data.as_ref() {
                metric_card(
                    &mut columns[1],
                    "I/O Reads",
                    &format_count(io.reads),
                    &format!("{} writes", format_count(io.writes)),
                    Tone::Info,
                );
            } else {
                metric_card(
                    &mut columns[1],
                    "I/O",
                    if self.io.available {
                        "No data"
                    } else {
                        "Unavailable"
                    },
                    self.io
                        .unavailable_reason
                        .as_deref()
                        .unwrap_or("pg_stat_io"),
                    Tone::Neutral,
                );
            }

            if let Some(bgwriter) = &self.bgwriter {
                metric_card(
                    &mut columns[2],
                    "Checkpoints",
                    &format_count(bgwriter.checkpoints_timed + bgwriter.checkpoints_req),
                    &format!("{} requested", format_count(bgwriter.checkpoints_req)),
                    if bgwriter.checkpoints_req > bgwriter.checkpoints_timed {
                        Tone::Warning
                    } else {
                        Tone::Good
                    },
                );
            } else {
                metric_card(
                    &mut columns[2],
                    "Bgwriter",
                    "No data",
                    "pg_stat_bgwriter",
                    Tone::Neutral,
                );
            }
        });

        ui.add_space(10.0);
        if let Some(bgwriter) = &self.bgwriter {
            table(ui, "monitor_bgwriter_table", |table| {
                table
                    .column(Column::initial(240.0).at_least(160.0))
                    .column(Column::remainder().at_least(160.0))
                    .header(22.0, |mut header| {
                        header_text(&mut header, "Metric");
                        header_text(&mut header, "Value");
                    })
                    .body(|mut body| {
                        bgwriter_metric(&mut body, "Timed checkpoints", bgwriter.checkpoints_timed);
                        bgwriter_metric(
                            &mut body,
                            "Requested checkpoints",
                            bgwriter.checkpoints_req,
                        );
                        bgwriter_metric(
                            &mut body,
                            "Checkpoint buffers",
                            bgwriter.buffers_checkpoint,
                        );
                        bgwriter_metric(&mut body, "Clean buffers", bgwriter.buffers_clean);
                        bgwriter_metric(&mut body, "Backend buffers", bgwriter.buffers_backend);
                        bgwriter_metric(&mut body, "Allocated buffers", bgwriter.buffers_alloc);
                        bgwriter_metric(&mut body, "Backend fsync", bgwriter.buffers_backend_fsync);
                    });
            });
        }
    }

    fn show_maintenance(&self, ui: &mut egui::Ui) {
        if !self.maintenance.available {
            optional_banner(ui, &self.maintenance);
            return;
        }
        if self.maintenance.data.is_empty() {
            centered_status(ui, "No vacuum or index build in progress");
            return;
        }

        table(ui, "monitor_maintenance_table", |table| {
            table
                .column(Column::initial(120.0).at_least(90.0))
                .column(Column::initial(70.0).at_least(54.0))
                .column(Column::initial(180.0).at_least(120.0))
                .column(Column::initial(180.0).at_least(120.0))
                .column(Column::remainder().at_least(180.0))
                .column(Column::initial(100.0).at_least(80.0))
                .header(22.0, |mut header| {
                    header_text(&mut header, "Operation");
                    header_text(&mut header, "PID");
                    header_text(&mut header, "Relation");
                    header_text(&mut header, "Index");
                    header_text(&mut header, "Phase");
                    header_text(&mut header, "Progress");
                })
                .body(|mut body| {
                    for row in &self.maintenance.data {
                        body.row(26.0, |mut table_row| {
                            table_row.col(|ui| clipped_label(ui, &row.operation));
                            table_row.col(|ui| {
                                ui.label(row.pid.to_string());
                            });
                            table_row.col(|ui| {
                                clipped_label(ui, row.relation.as_deref().unwrap_or("-"))
                            });
                            table_row.col(|ui| {
                                clipped_label(ui, row.index_relation.as_deref().unwrap_or("-"))
                            });
                            table_row
                                .col(|ui| clipped_label(ui, row.phase.as_deref().unwrap_or("-")));
                            table_row.col(|ui| {
                                let progress = if row.progress_total > 0 {
                                    format_percent(
                                        row.progress_done as f64 / row.progress_total as f64,
                                    )
                                } else {
                                    "-".to_string()
                                };
                                ui.label(progress);
                            });
                        });
                    }
                });
        });
    }

    fn poll_promise(&mut self) {
        if let Some(promise) = &self.promise
            && let Some(result) = promise.ready()
        {
            match result {
                Ok(result) => self.apply_load_result(result.clone()),
                Err(error) => self.error = Some(error.clone()),
            }
            self.promise = None;
            self.loading_panel = None;
        }
    }

    fn apply_load_result(&mut self, result: MonitorLoadResult) {
        self.error = None;
        self.capabilities = Some(result.capabilities);
        self.snapshot = Some(result.snapshot.clone());
        self.last_refresh = Some(Instant::now());
        self.push_history(&result.snapshot);

        match result.payload {
            MonitorPayload::Overview => {}
            MonitorPayload::Activity(rows) => self.activity = rows,
            MonitorPayload::Queries(data) => self.statements = data,
            MonitorPayload::Storage { tables, indexes } => {
                self.tables = tables;
                self.indexes = indexes;
            }
            MonitorPayload::Locks(rows) => self.locks = rows,
            MonitorPayload::Replication(rows) => self.replication = rows,
            MonitorPayload::WalIo { wal, io, bgwriter } => {
                self.wal = wal;
                self.io = io;
                self.bgwriter = Some(bgwriter);
            }
            MonitorPayload::Maintenance(data) => self.maintenance = data,
        }

        if self.active_panel != result.panel {
            self.active_panel = result.panel;
        }
    }

    fn push_history(&mut self, snapshot: &MonitorSnapshot) {
        self.history.push_back(SnapshotPoint {
            active_connections: snapshot.active_connections as f64,
            waiting_locks: snapshot.waiting_lock_count as f64,
            cache_hit_ratio: snapshot.cache_hit_ratio.unwrap_or(0.0),
        });
        while self.history.len() > HISTORY_LIMIT {
            self.history.pop_front();
        }
    }

    fn clear_data_for_new_connection(&mut self) {
        self.error = None;
        self.capabilities = None;
        self.snapshot = None;
        self.history.clear();
        self.activity.clear();
        self.statements = OptionalMonitorData::default();
        self.tables.clear();
        self.indexes.clear();
        self.locks.clear();
        self.replication.clear();
        self.wal = OptionalMonitorData::default();
        self.io = OptionalMonitorData::default();
        self.bgwriter = None;
        self.maintenance = OptionalMonitorData::default();
        self.promise = None;
        self.loading_panel = None;
    }

    fn should_auto_refresh(&self) -> bool {
        if self.promise.is_some() {
            return false;
        }
        let Some(interval) = self.refresh_cadence.duration() else {
            return false;
        };
        self.last_refresh
            .map(|last_refresh| last_refresh.elapsed() >= interval)
            .unwrap_or(true)
    }

    fn start_load(
        &mut self,
        pools: crate::components::db_manager::PoolRegistry,
        dsn: Option<&str>,
    ) {
        if self.promise.is_some() {
            return;
        }
        self.refresh_requested = false;

        let Some(dsn) = dsn else {
            self.error = Some("No database selected".to_string());
            return;
        };

        let dsn = dsn.to_string();
        let panel = self.active_panel;
        let statement_options = self.statement_options;
        let table_sort = self.table_sort;
        let index_sort = self.index_sort;
        let row_limit = self.row_limit;
        let (sender, promise) = Promise::new();
        self.promise = Some(promise);
        self.loading_panel = Some(panel);

        crate::futures::spawn(async move {
            let result = tokio::time::timeout(MONITOR_QUERY_TIMEOUT, async move {
                let pool = pools
                    .get_or_create_pool(&dsn)
                    .await
                    .map_err(|error| format!("Connection failed: {error}"))?;
                let session = Session::from_pool(pool);
                let capabilities = session
                    .monitor_capabilities()
                    .await
                    .map_err(|error| error.to_string())?;
                let snapshot = session
                    .fetch_monitor_snapshot()
                    .await
                    .map_err(|error| error.to_string())?;
                let payload = load_panel_payload(
                    &session,
                    panel,
                    statement_options,
                    table_sort,
                    index_sort,
                    row_limit,
                )
                .await?;

                Ok::<_, String>(MonitorLoadResult {
                    panel,
                    capabilities,
                    snapshot,
                    payload,
                })
            })
            .await
            .map_err(|_| "Monitor query timed out".to_string())
            .and_then(std::convert::identity);

            sender.send(result);
        });
    }
}

async fn load_panel_payload(
    session: &Session,
    panel: MonitorPanel,
    statement_options: StatementOptions,
    table_sort: TableHealthSort,
    index_sort: IndexHealthSort,
    row_limit: i64,
) -> Result<MonitorPayload, String> {
    match panel {
        MonitorPanel::Overview => Ok(MonitorPayload::Overview),
        MonitorPanel::Activity => session
            .fetch_activity(row_limit)
            .await
            .map(MonitorPayload::Activity)
            .map_err(|error| error.to_string()),
        MonitorPanel::Queries => session
            .fetch_statement_stats(statement_options)
            .await
            .map(MonitorPayload::Queries)
            .map_err(|error| error.to_string()),
        MonitorPanel::Storage => {
            let tables = session
                .fetch_table_health(row_limit, table_sort)
                .await
                .map_err(|error| error.to_string())?;
            let indexes = session
                .fetch_index_health(row_limit, index_sort)
                .await
                .map_err(|error| error.to_string())?;
            Ok(MonitorPayload::Storage { tables, indexes })
        }
        MonitorPanel::Locks => session
            .fetch_locks(row_limit)
            .await
            .map(MonitorPayload::Locks)
            .map_err(|error| error.to_string()),
        MonitorPanel::Replication => session
            .fetch_replication()
            .await
            .map(MonitorPayload::Replication)
            .map_err(|error| error.to_string()),
        MonitorPanel::WalIo => {
            let wal = session
                .fetch_wal_stats()
                .await
                .map_err(|error| error.to_string())?;
            let io = session
                .fetch_io_stats()
                .await
                .map_err(|error| error.to_string())?;
            let bgwriter = session
                .fetch_background_writer()
                .await
                .map_err(|error| error.to_string())?;
            Ok(MonitorPayload::WalIo { wal, io, bgwriter })
        }
        MonitorPanel::Maintenance => session
            .fetch_maintenance_progress()
            .await
            .map(MonitorPayload::Maintenance)
            .map_err(|error| error.to_string()),
    }
}

fn screen_center(ctx: &Context) -> eframe::egui::Pos2 {
    ctx.content_rect().center()
}

fn table(ui: &mut egui::Ui, id: impl std::hash::Hash, add_contents: impl FnOnce(TableBuilder<'_>)) {
    egui::ScrollArea::both().show(ui, |ui| {
        let table = TableBuilder::new(ui)
            .id_salt(id)
            .striped(true)
            .resizable(true)
            .vscroll(true);
        add_contents(table);
    });
}

fn header_text(header: &mut egui_extras::TableRow<'_, '_>, text: &str) {
    header.col(|ui| {
        ui.strong(text);
    });
}

fn bgwriter_metric(body: &mut egui_extras::TableBody<'_>, label: &'static str, value: i64) {
    body.row(24.0, |mut row| {
        row.col(|ui| {
            ui.label(label);
        });
        row.col(|ui| {
            ui.label(format_count(value));
        });
    });
}

fn metric_card(ui: &mut egui::Ui, title: &str, value: &str, detail: &str, tone: Tone) {
    let color = tone_color(ui, tone);
    egui::Frame::group(ui.style())
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.set_min_height(86.0);
            ui.label(
                RichText::new(title)
                    .small()
                    .color(ui.visuals().weak_text_color()),
            );
            ui.add_space(4.0);
            ui.label(RichText::new(value).size(22.0).strong().color(color));
            ui.add_space(2.0);
            ui.label(
                RichText::new(detail)
                    .small()
                    .color(ui.visuals().weak_text_color()),
            );
        });
}

fn status_pill(ui: &mut egui::Ui, text: &str, tone: Tone) {
    let color = tone_color(ui, tone);
    let fill = color.gamma_multiply(if ui.visuals().dark_mode { 0.18 } else { 0.10 });
    egui::Frame::new()
        .fill(fill)
        .stroke(egui::Stroke::new(1.0, color.gamma_multiply(0.55)))
        .corner_radius(egui::CornerRadius::same(5))
        .inner_margin(egui::Margin::symmetric(6, 2))
        .show(ui, |ui| {
            ui.label(RichText::new(text).small().color(color));
        });
}

fn capability_label(ui: &mut egui::Ui, label: &str, enabled: bool) {
    ui.horizontal(|ui| {
        let tone = if enabled { Tone::Good } else { Tone::Neutral };
        status_pill(ui, if enabled { "on" } else { "off" }, tone);
        ui.label(
            RichText::new(label)
                .small()
                .color(ui.visuals().weak_text_color()),
        );
    });
}

fn state_label(ui: &mut egui::Ui, state: Option<&str>) {
    let state = state.unwrap_or("unknown");
    let tone = match state {
        "active" | "streaming" => Tone::Good,
        "idle in transaction" => Tone::Warning,
        "idle" => Tone::Neutral,
        _ => Tone::Info,
    };
    status_pill(ui, state, tone);
}

fn optional_banner<T>(ui: &mut egui::Ui, data: &OptionalMonitorData<T>) {
    let message = data
        .unavailable_reason
        .as_deref()
        .unwrap_or("This monitor data source is unavailable");
    egui::Frame::group(ui.style())
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(egui_phosphor::regular::INFO).color(tone_color(ui, Tone::Info)),
                );
                ui.label(message);
            });
        });
}

fn error_banner(ui: &mut egui::Ui, error: &str) {
    egui::Frame::group(ui.style())
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(egui_phosphor::regular::WARNING).color(tone_color(ui, Tone::Bad)),
                );
                ui.label(RichText::new(error).color(tone_color(ui, Tone::Bad)));
            });
        });
}

fn centered_status(ui: &mut egui::Ui, text: &str) {
    ui.centered_and_justified(|ui| {
        ui.label(RichText::new(text).color(ui.visuals().weak_text_color()));
    });
}

fn clipped_label(ui: &mut egui::Ui, text: &str) {
    let display = truncate_chars(text, 140);
    let response = ui.label(display.as_str());
    if display != text {
        response.on_hover_text(text);
    }
}

fn truncate_chars(text: &str, limit: usize) -> String {
    let first_line = text.lines().next().unwrap_or(text);
    if first_line.chars().count() <= limit {
        first_line.to_string()
    } else {
        format!("{}...", first_line.chars().take(limit).collect::<String>())
    }
}

#[derive(Debug, Clone, Copy)]
enum Tone {
    Neutral,
    Good,
    Warning,
    Bad,
    Info,
}

fn tone_color(ui: &egui::Ui, tone: Tone) -> Color32 {
    match tone {
        Tone::Neutral => ui.visuals().weak_text_color(),
        Tone::Good => Color32::from_rgb(58, 166, 112),
        Tone::Warning => Color32::from_rgb(214, 154, 54),
        Tone::Bad => Color32::from_rgb(214, 82, 82),
        Tone::Info => Color32::from_rgb(80, 148, 210),
    }
}

fn history_points(
    history: &VecDeque<SnapshotPoint>,
    value: impl Fn(&SnapshotPoint) -> f64,
) -> PlotPoints<'_> {
    history
        .iter()
        .enumerate()
        .map(|(index, point)| [index as f64, value(point)])
        .collect::<Vec<_>>()
        .into()
}

fn format_percent(value: f64) -> String {
    format!("{:.1}%", value * 100.0)
}

fn format_count(value: i64) -> String {
    let sign = if value < 0 { "-" } else { "" };
    let digits = value.abs().to_string();
    let mut out = String::new();
    for (index, ch) in digits.chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    format!("{sign}{}", out.chars().rev().collect::<String>())
}

fn format_bytes(value: i64) -> String {
    format_bytes_f64(value as f64)
}

fn format_numeric_bytes(value: &str) -> String {
    value
        .parse::<f64>()
        .map(format_bytes_f64)
        .unwrap_or_else(|_| value.to_string())
}

fn format_bytes_f64(value: f64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut size = value.abs();
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    let sign = if value < 0.0 { "-" } else { "" };
    if unit == 0 {
        format!("{sign}{size:.0} {}", UNITS[unit])
    } else {
        format!("{sign}{size:.1} {}", UNITS[unit])
    }
}

fn format_seconds(seconds: i64) -> String {
    if seconds < 60 {
        format!("{seconds}s")
    } else if seconds < 3600 {
        format!("{}m {}s", seconds / 60, seconds % 60)
    } else {
        format!("{}h {}m", seconds / 3600, (seconds % 3600) / 60)
    }
}

fn format_duration(duration: Duration) -> String {
    format_seconds(duration.as_secs() as i64)
}
