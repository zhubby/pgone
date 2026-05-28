use super::MonitorMetric;
use super::activity;
use super::bgwriter;
use super::indexes;
use super::locks;
use super::replication;
use super::statements;
use super::tables;
use crate::components::DbManager;
use eframe::egui::{Align2, Context, Id, Window};

/// Show monitor window
pub fn show_monitor_window(
    ctx: &Context,
    show_monitor: &mut Option<MonitorMetric>,
    db_manager: &mut DbManager,
) {
    let metric = match show_monitor {
        Some(m) => *m,
        None => return,
    };

    let mut open = true;
    let title = metric.title();

    // Get current database DSN
    let dsn = get_dsn(db_manager);
    let pools = db_manager.pools.clone();

    Window::new(title)
        .id(Id::new("monitor_window"))
        .open(&mut open)
        .default_pos(screen_center(ctx))
        .pivot(Align2::CENTER_CENTER)
        // .default_size([800.0, 600.0])
        // .min_size([600.0, 400.0])
        .show(ctx, |ui| match metric {
            MonitorMetric::Activity => {
                activity::show(ui, pools.clone(), dsn.as_deref());
            }
            MonitorMetric::Statements => {
                statements::show(ui, pools.clone(), dsn.as_deref());
            }
            MonitorMetric::Tables => {
                tables::show(ui, pools.clone(), dsn.as_deref());
            }
            MonitorMetric::Indexes => {
                indexes::show(ui, pools.clone(), dsn.as_deref());
            }
            MonitorMetric::Bgwriter => {
                bgwriter::show(ui, pools.clone(), dsn.as_deref());
            }
            MonitorMetric::Replication => {
                replication::show(ui, pools.clone(), dsn.as_deref());
            }
            MonitorMetric::Locks => {
                locks::show(ui, pools.clone(), dsn.as_deref());
            }
        });

    if !open {
        *show_monitor = None;
    }
}

/// Get current active database DSN
fn get_dsn(db_manager: &mut DbManager) -> Option<String> {
    db_manager.active_dsn()
}

fn screen_center(ctx: &Context) -> eframe::egui::Pos2 {
    ctx.content_rect().center()
}
