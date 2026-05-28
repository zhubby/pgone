use crate::components::DbManager;
use crate::components::db_manager::PoolRegistry;
use crate::models::Settings;
use eframe::egui::{Context, Panel, ThemePreference, Ui};
use once_cell::sync::Lazy;
use regex::Regex;
use sqlx::Row;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate};

// Cache System instance for performance, only refresh current process info on demand.
static SYSTEM: Lazy<Arc<Mutex<sysinfo::System>>> =
    Lazy::new(|| Arc::new(Mutex::new(sysinfo::System::new())));

// Track last refresh time to control refresh frequency (once per second)
static LAST_REFRESH: Lazy<Arc<Mutex<Option<Instant>>>> = Lazy::new(|| Arc::new(Mutex::new(None)));

// Cache database version info (by connection ID)
static DB_VERSION_CACHE: Lazy<Arc<Mutex<HashMap<String, (String, Instant)>>>> =
    Lazy::new(|| Arc::new(Mutex::new(HashMap::new())));
static DB_VERSION_PENDING: Lazy<Arc<Mutex<HashSet<String>>>> =
    Lazy::new(|| Arc::new(Mutex::new(HashSet::new())));

// Refresh interval: 1 second
const REFRESH_INTERVAL: Duration = Duration::from_secs(1);
// Database version cache TTL: 30 seconds
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

                            // Get and display database version info
                            let db_version =
                                get_db_version(ctx, db.pools.clone(), &cfg.dsn, &cfg.id);
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

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                show_build_info(ui);

                if settings.enable_monitor {
                    ui.separator();
                    show_monitoring_info(ui, ctx);
                }
            });
        });
    });

    requested_theme
}

fn show_build_info(ui: &mut Ui) {
    let version = env!("CARGO_PKG_VERSION");
    let branch = option_env!("VERGEN_GIT_BRANCH").unwrap_or("unknown");
    let sha = option_env!("VERGEN_GIT_SHA")
        .map(short_sha)
        .unwrap_or("unknown");
    let dirty = option_env!("VERGEN_GIT_DIRTY").unwrap_or("unknown");
    let dirty_label = match dirty {
        "true" => "dirty",
        "false" => "clean",
        value => value,
    };
    let dirty_icon = match dirty {
        "true" => egui_phosphor::regular::WARNING_CIRCLE,
        "false" => egui_phosphor::regular::CHECK_CIRCLE,
        _ => egui_phosphor::regular::CIRCLE_DASHED,
    };
    let dirty_color = match dirty {
        "true" => ui.visuals().warn_fg_color,
        "false" => ui.visuals().weak_text_color(),
        _ => ui.visuals().text_color(),
    };

    let response = ui
        .horizontal(|ui| {
            ui.label(egui_phosphor::regular::PACKAGE);
            ui.label(format!("v{version}"));
            ui.separator();
            ui.label(egui_phosphor::regular::GIT_BRANCH);
            ui.label(branch);
            ui.separator();
            ui.label(egui_phosphor::regular::GIT_COMMIT);
            ui.label(sha);
            ui.separator();
            ui.colored_label(dirty_color, dirty_icon);
            ui.label(dirty_label);
        })
        .response;

    let full_sha = option_env!("VERGEN_GIT_SHA").unwrap_or("unknown");
    let build_timestamp = option_env!("VERGEN_BUILD_TIMESTAMP").unwrap_or("unknown");
    response.on_hover_text(format!(
        "Release: v{version}\nBranch: {branch}\nCommit: {full_sha}\nStatus: {dirty_label}\nBuild: {build_timestamp}"
    ));
}

fn short_sha(sha: &str) -> &str {
    sha.get(..7).unwrap_or(sha)
}

fn show_monitoring_info(ui: &mut Ui, ctx: &Context) {
    // Check if refresh is needed (refresh once per second)
    let should_refresh = {
        let mut last_refresh = LAST_REFRESH.lock().unwrap();
        let now = Instant::now();
        let should = last_refresh
            .map(|last| now.duration_since(last) >= REFRESH_INTERVAL)
            .unwrap_or(true);

        if should {
            *last_refresh = Some(now);
            // Request repaint after next refresh interval to ensure continuous updates
            ctx.request_repaint_after(REFRESH_INTERVAL);
        }
        should
    };

    if let Ok(mut system) = SYSTEM.lock() {
        let pid = sysinfo::Pid::from(std::process::id() as usize);

        // Only refresh current process CPU/memory when needed, avoid UI thread scanning entire system.
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

            // Format memory display
            let memory_str = if memory_mb >= 1.0 {
                format!("{:.2} MB", memory_mb)
            } else {
                format!("{} KB", memory_kb)
            };

            // Get network info - network info retrieval may differ in sysinfo 0.37
            // Temporarily display placeholder info, can be adjusted based on actual API later
            let network_info = "Network: Monitoring".to_string();

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
        }
    }
}

// Static regex for extracting version info
static VERSION_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"PostgreSQL\s+(\d+\.\d+(?:\.\d+)?)").unwrap());

static ARCH_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\b(64-bit|32-bit)\b").unwrap());

/// Extract key version info from PostgreSQL version() output
/// e.g.: "PostgreSQL 10.12 (Debian 10.12-1.pgdg90+1) on x86_64-pc-linux-gnu, compiled by gcc (Debian 6.3.0-18+deb9u1) 6.3.0 20170516, 64-bit"
/// Extracted as: "PostgreSQL 10.12 (64-bit)"
fn extract_version_info(full_version: &str) -> String {
    // Extract version number
    let version_num = VERSION_RE
        .captures(full_version)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str())
        .unwrap_or("");

    // Extract architecture info
    let arch = ARCH_RE.find(full_version).map(|m| m.as_str()).unwrap_or("");

    // Combine results
    if version_num.is_empty() && arch.is_empty() {
        // If unable to parse, return first 50 characters of original string
        full_version.chars().take(50).collect::<String>()
    } else if arch.is_empty() {
        format!("PostgreSQL {}", version_num)
    } else {
        format!("PostgreSQL {} ({})", version_num, arch)
    }
}

/// Get database version info, using cache to avoid frequent queries
fn get_db_version(ctx: &Context, pools: PoolRegistry, dsn: &str, db_id: &str) -> Option<String> {
    let mut cache = DB_VERSION_CACHE.lock().unwrap();
    let now = Instant::now();

    // Check cache
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
                let pool = pools
                    .get_or_create_pool(&dsn)
                    .await
                    .map_err(sqlx::Error::Protocol)?;
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
