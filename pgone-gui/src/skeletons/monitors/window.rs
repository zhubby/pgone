use crate::components::DbManager;
use crate::futures;
use eframe::egui::{Align2, Context, Window};
use super::MonitorMetric;
use super::activity;
use super::statements;
use super::tables;
use super::indexes;
use super::bgwriter;
use super::replication;
use super::locks;

/// 显示监控窗口
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
    
    // 获取当前数据库的DSN
    let dsn = get_dsn(db_manager);
    
    Window::new(title)
        .open(&mut open)
        .default_pos(screen_center(ctx))
        .pivot(Align2::CENTER_CENTER)
        // .default_size([800.0, 600.0])
        // .min_size([600.0, 400.0])
        .show(ctx, |ui| {
            match metric {
                MonitorMetric::Activity => {
                    activity::show(ui, dsn.as_deref());
                }
                MonitorMetric::Statements => {
                    statements::show(ui, dsn.as_deref());
                }
                MonitorMetric::Tables => {
                    tables::show(ui, dsn.as_deref());
                }
                MonitorMetric::Indexes => {
                    indexes::show(ui, dsn.as_deref());
                }
                MonitorMetric::Bgwriter => {
                    bgwriter::show(ui, dsn.as_deref());
                }
                MonitorMetric::Replication => {
                    replication::show(ui, dsn.as_deref());
                }
                MonitorMetric::Locks => {
                    locks::show(ui, dsn.as_deref());
                }
            }
        });

    if !open {
        *show_monitor = None;
    }
}

/// 获取当前活动数据库的DSN
fn get_dsn(db_manager: &mut DbManager) -> Option<String> {
    db_manager.ensure_storage();
    db_manager
        .active_db_config_id
        .as_ref()
        .and_then(|id| {
            db_manager
                .storage
                .as_ref()
                .and_then(|storage| {
                    futures::block_on_async(async {
                        storage.get_db_config(id).await
                    })
                    .ok()
                    .flatten()
                    .map(|cfg| cfg.dsn)
                })
        })
}

fn screen_center(ctx: &Context) -> eframe::egui::Pos2 {
    ctx.screen_rect().center()
}

