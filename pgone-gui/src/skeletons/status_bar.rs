use crate::components::DbManager;
use crate::futures;
use crate::models::Settings;
use eframe::egui::{Context, TopBottomPanel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use once_cell::sync::Lazy;

// 缓存 System 实例以提高性能
static SYSTEM: Lazy<Arc<Mutex<sysinfo::System>>> = Lazy::new(|| {
    Arc::new(Mutex::new(sysinfo::System::new_all()))
});

// 记录上次刷新时间，用于控制刷新频率（每秒一次）
static LAST_REFRESH: Lazy<Arc<Mutex<Option<Instant>>>> = Lazy::new(|| {
    Arc::new(Mutex::new(None))
});

// 刷新间隔：1秒
const REFRESH_INTERVAL: Duration = Duration::from_secs(1);

pub fn show_status_bar(ctx: &Context, db: &mut DbManager, settings: &Settings) {
    TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
        ui.horizontal(|ui| {
            // Database selection button and display
            ui.horizontal(|ui| {
                if ui.button("Select Database").clicked() {
                    db.show_manage_db = true;
                }
                ui.separator();
                let active_id = db.active_db_config_id.clone();
                let db_name = if let Some(ref id) = active_id {
                    db.get_db_name(id).unwrap_or_else(|| id.clone())
                } else {
                    "<no db>".to_string()
                };
                ui.label(format!("Selected Database Config: {}", db_name));
                ui.separator();

                if active_id.is_some() {
                    db.ensure_storage();
                    if let Some(ref storage) = db.storage {
                        if let Ok(Some(cfg)) = futures::block_on_async(async {
                            storage.get_db_config(&active_id.as_ref().unwrap()).await
                        }) {
                            // Parse DSN to get connection details
                            if let Some(parsed) = crate::components::DbManager::parse_dsn(&cfg.dsn)
                            {
                                ui.horizontal(|ui| {
                                    ui.label(egui_phosphor::regular::DATABASE);
                                    ui.label(egui::RichText::new(&cfg.id).strong());
                                });
                                ui.horizontal(|ui| {
                                    ui.label("Engine:");
                                    ui.label(&cfg.engine);
                                });
                                ui.horizontal(|ui| {
                                    ui.label("Host:");
                                    ui.label(&parsed.host);
                                });
                                ui.horizontal(|ui| {
                                    ui.label("Database:");
                                    ui.label(if parsed.database.is_empty() {
                                        "<default>"
                                    } else {
                                        &parsed.database
                                    });
                                });
                            }
                        }
                    }
                }
            });
            
            // 如果启用了监控，显示系统监控信息
            if settings.enable_monitor {
                // 检查是否需要刷新（每秒刷新一次）
                let should_refresh = {
                    let mut last_refresh = LAST_REFRESH.lock().unwrap();
                    let now = Instant::now();
                    let should = last_refresh
                        .map(|last| now.duration_since(last) >= REFRESH_INTERVAL)
                        .unwrap_or(true);
                    
                    if should {
                        *last_refresh = Some(now);
                        // 请求在下次刷新间隔后重绘，确保持续更新
                        ctx.request_repaint_after(REFRESH_INTERVAL);
                    }
                    should
                };
                
                ui.separator();
                if let Ok(mut system) = SYSTEM.lock() {
                    // 只在需要时刷新系统信息
                    if should_refresh {
                        system.refresh_all();
                    }
                    
                    let pid = std::process::id();
                    if let Some(process) = system.process(sysinfo::Pid::from(pid as usize)) {
                        let process_name = process.name().to_string_lossy().to_string();
                        let cpu_usage = process.cpu_usage();
                        let memory_kb = process.memory() / 1024;
                        let memory_mb = memory_kb as f64 / 1024.0;
                        
                        // 格式化内存显示
                        let memory_str = if memory_mb >= 1.0 {
                            format!("{:.2} MB", memory_mb)
                        } else {
                            format!("{} KB", memory_kb)
                        };
                        
                        // 获取网络信息 - sysinfo 0.37 中网络信息获取方式可能不同
                        // 暂时显示占位信息，后续可以根据实际 API 调整
                        let network_info = "网络: 监控中".to_string();
                        
                        ui.horizontal(|ui| {
                            ui.label(format!("进程: {}", process_name));
                            ui.separator();
                            ui.label(format!("CPU: {:.1}%", cpu_usage));
                            ui.separator();
                            ui.label(format!("内存: {}", memory_str));
                            ui.separator();
                            ui.label(&network_info);
                        });
                    }
                }
            }
        });
    });
}

