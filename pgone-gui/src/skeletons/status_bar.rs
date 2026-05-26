use crate::components::DbManager;
use crate::models::Settings;
use eframe::egui::{Context, Panel, ThemePreference, Ui};
use once_cell::sync::Lazy;
use regex::Regex;
use sqlx::Row;
use sqlx::postgres::PgPoolOptions;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate};

// 缓存 System 实例以提高性能，仅按需刷新当前进程信息。
static SYSTEM: Lazy<Arc<Mutex<sysinfo::System>>> =
    Lazy::new(|| Arc::new(Mutex::new(sysinfo::System::new())));

// 记录上次刷新时间，用于控制刷新频率（每秒一次）
static LAST_REFRESH: Lazy<Arc<Mutex<Option<Instant>>>> = Lazy::new(|| Arc::new(Mutex::new(None)));

// 缓存数据库版本信息（按连接ID）
static DB_VERSION_CACHE: Lazy<Arc<Mutex<HashMap<String, (String, Instant)>>>> =
    Lazy::new(|| Arc::new(Mutex::new(HashMap::new())));
static DB_VERSION_PENDING: Lazy<Arc<Mutex<HashSet<String>>>> =
    Lazy::new(|| Arc::new(Mutex::new(HashSet::new())));

// 刷新间隔：1秒
const REFRESH_INTERVAL: Duration = Duration::from_secs(1);
// 数据库版本缓存有效期：30秒
const VERSION_CACHE_TTL: Duration = Duration::from_secs(30);

pub fn show_status_bar(
    root_ui: &mut Ui,
    ctx: &Context,
    db: &mut DbManager,
    settings: &Settings,
) -> Option<ThemePreference> {
    let mut requested_theme = None;

    Panel::bottom("status_bar").show_inside(root_ui, |ui| {
        ui.horizontal(|ui| {
            let mut preference = ui.ctx().options(|opt| opt.theme_preference);
            if ui
                .add(egui_theme_switch::ThemeSwitch::new(&mut preference))
                .changed()
            {
                requested_theme = Some(preference);
                ctx.request_repaint();
            }

            ui.horizontal(|ui| {
                if db.active_db_config_id.is_some() {
                    ui.separator();
                    if let Some(cfg) = db.active_db_config() {
                        // Parse DSN to get connection details
                        if let Some(parsed) = crate::components::DbManager::parse_dsn(&cfg.dsn) {
                            ui.horizontal(|ui| {
                                ui.label(egui_phosphor::regular::DATABASE);
                                ui.label(egui::RichText::new(&cfg.id).strong());
                            });
                            ui.horizontal(|ui| {
                                ui.label(egui_phosphor::regular::GEAR);
                                ui.label("Engine:");
                                ui.label(&cfg.engine);
                            });
                            ui.horizontal(|ui| {
                                ui.label(egui_phosphor::regular::GLOBE);
                                ui.label("Host:");
                                ui.label(&parsed.host);
                            });
                            ui.horizontal(|ui| {
                                ui.label(egui_phosphor::regular::DATABASE);
                                ui.label("Database:");
                                ui.label(if parsed.database.is_empty() {
                                    "<default>"
                                } else {
                                    &parsed.database
                                });
                            });

                            // 获取并显示数据库版本信息
                            let db_version = get_db_version(ctx, &cfg.dsn, &cfg.id);
                            if let Some(version) = db_version {
                                let short_version = extract_version_info(&version);
                                ui.horizontal(|ui| {
                                    ui.label(egui_phosphor::regular::TAG);
                                    ui.label("Version:");
                                    ui.label(egui::RichText::new(short_version));
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

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Ok(mut system) = SYSTEM.lock() {
                        let pid = sysinfo::Pid::from(std::process::id() as usize);

                        // 只在需要时刷新当前进程的 CPU/内存，避免 UI 线程扫描全系统。
                        if should_refresh {
                            system.refresh_processes_specifics(
                                ProcessesToUpdate::Some(&[pid]),
                                true,
                                ProcessRefreshKind::nothing()
                                    .with_cpu()
                                    .with_memory()
                                    .without_tasks(),
                            );
                        }

                        if let Some(process) = system.process(pid) {
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

                            ui.label(egui_phosphor::regular::DESKTOP);
                            ui.label(format!("Process: {}", process_name));
                            ui.separator();
                            ui.label(egui_phosphor::regular::CHART_PIE);
                            ui.label(format!("CPU: {:.1}%", cpu_usage));
                            ui.separator();
                            ui.label(egui_phosphor::regular::HARD_DRIVE);
                            ui.label(format!("Memory: {}", memory_str));
                            ui.separator();
                            ui.label(egui_phosphor::regular::NETWORK);
                            ui.label(&network_info);
                            ui.separator();
                        }
                    }
                });
            }
        });
    });

    requested_theme
}

// 静态正则表达式，用于提取版本信息
static VERSION_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"PostgreSQL\s+(\d+\.\d+(?:\.\d+)?)").unwrap());

static ARCH_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\b(64-bit|32-bit)\b").unwrap());

/// 从 PostgreSQL version() 输出中提取关键版本信息
/// 例如: "PostgreSQL 10.12 (Debian 10.12-1.pgdg90+1) on x86_64-pc-linux-gnu, compiled by gcc (Debian 6.3.0-18+deb9u1) 6.3.0 20170516, 64-bit"
/// 提取为: "PostgreSQL 10.12 (64-bit)"
fn extract_version_info(full_version: &str) -> String {
    // 提取版本号
    let version_num = VERSION_RE
        .captures(full_version)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str())
        .unwrap_or("");

    // 提取架构信息
    let arch = ARCH_RE.find(full_version).map(|m| m.as_str()).unwrap_or("");

    // 组合结果
    if version_num.is_empty() && arch.is_empty() {
        // 如果无法解析，返回原始字符串的前50个字符
        full_version.chars().take(50).collect::<String>()
    } else if arch.is_empty() {
        format!("PostgreSQL {}", version_num)
    } else {
        format!("PostgreSQL {} ({})", version_num, arch)
    }
}

/// 获取数据库版本信息，使用缓存机制避免频繁查询
fn get_db_version(ctx: &Context, dsn: &str, db_id: &str) -> Option<String> {
    let mut cache = DB_VERSION_CACHE.lock().unwrap();
    let now = Instant::now();

    // 检查缓存
    if let Some((version, cached_time)) = cache.get(db_id) {
        if now.duration_since(*cached_time) < VERSION_CACHE_TTL {
            return Some(version.clone());
        }
    }

    let stale_value = cache.get(db_id).map(|(version, _)| version.clone());
    drop(cache);

    let should_spawn = DB_VERSION_PENDING
        .lock()
        .map(|mut pending| pending.insert(db_id.to_string()))
        .unwrap_or(false);

    if should_spawn {
        let dsn = dsn.to_string();
        let db_id = db_id.to_string();
        let ctx = ctx.clone();
        crate::futures::spawn(async move {
            let version_result = async {
                let pool = PgPoolOptions::new()
                    .max_connections(1)
                    .connect(&dsn)
                    .await?;
                let row = sqlx::query("SELECT version() as version")
                    .fetch_one(&pool)
                    .await?;
                Ok::<String, sqlx::Error>(row.get("version"))
            }
            .await;

            if let Ok(version) = version_result
                && let Ok(mut cache) = DB_VERSION_CACHE.lock()
            {
                cache.insert(db_id.clone(), (version, Instant::now()));
            }
            if let Ok(mut pending) = DB_VERSION_PENDING.lock() {
                pending.remove(&db_id);
            }
            ctx.request_repaint();
        });
    }

    stale_value
}
