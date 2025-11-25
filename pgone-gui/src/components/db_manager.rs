use pgone_storage::blocking::StorageBlocking;
use sqlx::postgres::{PgPool, PgPoolOptions};
use crate::notify;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DbEngine {
    Postgresql,
    Mysql,
}

impl DbEngine {
    fn as_str(&self) -> &'static str {
        match self {
            DbEngine::Postgresql => "postgresql",
            DbEngine::Mysql => "mysql",
        }
    }
    
    fn all() -> &'static [DbEngine] {
        &[DbEngine::Postgresql, DbEngine::Mysql]
    }
}

#[derive(Debug, Clone)]
pub struct ParsedDsn {
    pub engine: String,
    pub host: String,
    pub port: String,
    pub database: String,
    pub user: String,
}

#[derive(Debug, Clone)]
pub struct DbFormData {
    pub engine: DbEngine,
    pub name: String,
    pub host: String,
    pub port: String,
    pub database: String,
    pub user: String,
    pub password: String,
    pub error: Option<String>,
    pub test_status: Option<bool>, // None = not tested, Some(true) = success, Some(false) = failed
}

impl Default for DbFormData {
    fn default() -> Self {
        Self {
            engine: DbEngine::Postgresql,
            name: String::new(),
            host: "localhost".to_string(),
            port: "5432".to_string(),
            database: String::new(),
            user: String::new(),
            password: String::new(),
            error: None,
            test_status: None,
        }
    }
}

impl DbFormData {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

pub struct DbManager {
    pub active_db_config_id: Option<String>,
    pub show_add_db: bool,
    pub add_db_form: DbFormData,
    pub show_manage_db: bool,
    pub show_edit_db: bool,
    pub edit_db_id: Option<String>,
    pub edit_db_form: DbFormData,
    pub storage: Option<StorageBlocking>,
    pub pools: std::collections::HashMap<u64, PgPool>,
}

impl Default for DbManager {
    fn default() -> Self {
        Self {
            active_db_config_id: None,
            show_add_db: false,
            add_db_form: DbFormData::default(),
            show_manage_db: false,
            show_edit_db: false,
            edit_db_id: None,
            edit_db_form: DbFormData::default(),
            storage: None,
            pools: Default::default(),
        }
    }
}

impl DbManager {
    /// Parse DSN to extract connection information
    pub fn parse_dsn(dsn: &str) -> Option<ParsedDsn> {
        // DSN format: postgresql://user:password@host:port/database
        let url = url::Url::parse(dsn).ok()?;
        let engine = url.scheme().to_string();
        let host = url.host_str()?.to_string();
        let port = url.port()
            .map(|p| p.to_string())
            .unwrap_or_else(|| {
                match engine.as_str() {
                    "postgresql" | "postgres" => "5432".to_string(),
                    "mysql" => "3306".to_string(),
                    _ => "5432".to_string(),
                }
            });
        let database = url.path().trim_start_matches('/').to_string();
        let user = url.username().to_string();
        
        Some(ParsedDsn {
            engine,
            host,
            port,
            database,
            user,
        })
    }

    /// Get database name by ID
    pub fn get_db_name(&mut self, id: &str) -> Option<String> {
        self.ensure_storage();
        if let Some(ref storage) = self.storage {
            // Use tokio::task::block_in_place for synchronous access from async context
            if let Ok(Some(cfg)) = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    storage.get_db_config(id).await
                })
            }) {
                return Some(cfg.id);
            }
        }
        None
    }

    /// Reset add database form fields to default values
    pub fn reset_add_db_form(&mut self) {
        self.add_db_form.reset();
    }

    /// Reset edit database form fields
    pub fn reset_edit_db_form(&mut self) {
        self.edit_db_id = None;
        self.edit_db_form.reset();
    }

    pub fn ensure_storage(&mut self) {
        if self.storage.is_some() {
            return;
        }
        if let Ok(storage) = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                StorageBlocking::open_local("pgone.db").await
            })
        }) {
            self.storage = Some(storage);
        }
    }

    #[allow(dead_code)]
    pub fn ui_db_config(&mut self, _app: &mut crate::AppFrame, ui: &mut egui::Ui) {
        self.ensure_storage();
        let mut to_switch: Option<String> = None;
        if let Some(storage) = &self.storage {
            let list = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    storage.list_db_configs(None).await
                })
            }).unwrap_or_default();
            for cfg in list {
                let icon = egui_phosphor::regular::DATABASE;
                let label = if Some(cfg.id.clone()) == self.active_db_config_id {
                    format!("{} {} (active)", icon, cfg.id)
                } else {
                    format!("{} {}", icon, cfg.id)
                };
                let resp: egui::Response = ui.selectable_label(false, label);
                if resp.double_clicked() {
                    to_switch = Some(cfg.id.clone());
                }
            }
        } else {
            ui.label("Storage not ready");
        }
        if let Some(target) = to_switch {
            let mut open = true;
            egui::Window::new("Switch Database Config")
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .show(ui.ctx(), |ui| {
                    ui.label(format!("Switch active DB config to '{}' ?", target));
                    ui.horizontal(|ui| {
                        if ui.button("Confirm").clicked() {
                            self.active_db_config_id = Some(target.clone());
                        }
                        if ui.button("Cancel").clicked() {}
                    });
                });
        }
    }

    pub fn ui_add_db_window(&mut self, ctx: &egui::Context) {
        if self.show_add_db {
            let mut open = true;
            egui::Window::new("New Database")
                .open(&mut open)
                .show(ctx, |ui| {
                    // 使用固定宽度的标签来对齐文本框
                    let label_width = 80.0;
                    
                    ui.horizontal(|ui| {
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.set_width(label_width);
                            ui.label("Type");
                        });
                        egui::ComboBox::from_id_salt("db_engine")
                            .selected_text(self.add_db_form.engine.as_str())
                            .show_ui(ui, |ui| {
                                for engine in DbEngine::all() {
                                    ui.selectable_value(
                                        &mut self.add_db_form.engine,
                                        *engine,
                                        engine.as_str(),
                                    );
                                }
                            });
                    });
                    ui.horizontal(|ui| {
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.set_width(label_width);
                            ui.label("Name");
                        });
                        ui.text_edit_singleline(&mut self.add_db_form.name);
                    });
                    ui.horizontal(|ui| {
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.set_width(label_width);
                            ui.label("Host");
                        });
                        ui.text_edit_singleline(&mut self.add_db_form.host);
                    });
                    ui.horizontal(|ui| {
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.set_width(label_width);
                            ui.label("Port");
                        });
                        ui.text_edit_singleline(&mut self.add_db_form.port);
                    });
                    ui.horizontal(|ui| {
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.set_width(label_width);
                            ui.label("Database");
                        });
                        ui.text_edit_singleline(&mut self.add_db_form.database);
                    });
                    ui.horizontal(|ui| {
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.set_width(label_width);
                            ui.label("User");
                        });
                        ui.text_edit_singleline(&mut self.add_db_form.user);
                    });
                    ui.horizontal(|ui| {
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.set_width(label_width);
                            ui.label("Password");
                        });
                        ui.add(
                            egui::TextEdit::singleline(&mut self.add_db_form.password).password(true),
                        );
                    });
                    
                    if let Some(err) = &self.add_db_form.error {
                        ui.colored_label(egui::Color32::RED, err);
                    }
                    
                    // 按钮布局：Test Connection 在左下角，Save 在右下角
                    ui.horizontal(|ui| {
                        // 左侧：测试连接按钮和状态标记
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            let test_button = egui::Button::new(
                                egui::RichText::new("Test Connection")
                                    .color(egui::Color32::RED)
                            );
                            if ui.add(test_button).clicked() {
                                self.test_connection();
                            }
                            
                            // 显示测试结果标记
                            if let Some(success) = self.add_db_form.test_status {
                                if success {
                                    ui.colored_label(egui::Color32::GREEN, "✓");
                                } else {
                                    ui.colored_label(egui::Color32::RED, "✗");
                                }
                            }
                        });
                        
                        // 右侧：Save 和 Cancel 按钮
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("Save").clicked() {
                                if let Err(e) = self.save_new_database() {
                                    self.add_db_form.error = Some(e.clone());
                                    notify::db_save_error(&e);
                                } else {
                                    let db_name = self.add_db_form.name.trim().to_string();
                                    self.show_add_db = false;
                                    self.add_db_form.error = None;
                                    self.add_db_form.test_status = None;
                                    self.reset_add_db_form();
                                    notify::db_save_success(&db_name);
                                }
                            }
                            if ui.button("Cancel").clicked() {
                                self.show_add_db = false;
                                self.reset_add_db_form();
                            }
                        });
                    });
                });
            if !open {
                self.show_add_db = false;
                self.reset_add_db_form();
            }
        }
    }

    pub fn ui_edit_db_window(&mut self, ctx: &egui::Context) {
        if self.show_edit_db {
            let mut open = true;
            egui::Window::new("Edit Database")
                .open(&mut open)
                .show(ctx, |ui| {
                    let label_width = 80.0;
                    
                    ui.horizontal(|ui| {
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.set_width(label_width);
                            ui.label("Type");
                        });
                        egui::ComboBox::from_id_salt("edit_db_engine")
                            .selected_text(self.edit_db_form.engine.as_str())
                            .show_ui(ui, |ui| {
                                for engine in DbEngine::all() {
                                    ui.selectable_value(
                                        &mut self.edit_db_form.engine,
                                        *engine,
                                        engine.as_str(),
                                    );
                                }
                            });
                    });
                    ui.horizontal(|ui| {
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.set_width(label_width);
                            ui.label("Name");
                        });
                        ui.text_edit_singleline(&mut self.edit_db_form.name);
                    });
                    ui.horizontal(|ui| {
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.set_width(label_width);
                            ui.label("Host");
                        });
                        ui.text_edit_singleline(&mut self.edit_db_form.host);
                    });
                    ui.horizontal(|ui| {
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.set_width(label_width);
                            ui.label("Port");
                        });
                        ui.text_edit_singleline(&mut self.edit_db_form.port);
                    });
                    ui.horizontal(|ui| {
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.set_width(label_width);
                            ui.label("Database");
                        });
                        ui.text_edit_singleline(&mut self.edit_db_form.database);
                    });
                    ui.horizontal(|ui| {
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.set_width(label_width);
                            ui.label("User");
                        });
                        ui.text_edit_singleline(&mut self.edit_db_form.user);
                    });
                    ui.horizontal(|ui| {
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.set_width(label_width);
                            ui.label("Password");
                        });
                        ui.add(
                            egui::TextEdit::singleline(&mut self.edit_db_form.password)
                                .password(true)
                                .hint_text("Leave empty to keep existing"),
                        );
                    });
                    
                    if let Some(err) = &self.edit_db_form.error {
                        ui.colored_label(egui::Color32::RED, err);
                    }
                    
                    ui.horizontal(|ui| {
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            let test_button = egui::Button::new(
                                egui::RichText::new("Test Connection")
                                    .color(egui::Color32::RED)
                            );
                            if ui.add(test_button).clicked() {
                                self.test_edit_connection();
                            }
                            
                            if let Some(success) = self.edit_db_form.test_status {
                                if success {
                                    ui.colored_label(egui::Color32::GREEN, "✓");
                                } else {
                                    ui.colored_label(egui::Color32::RED, "✗");
                                }
                            }
                        });
                        
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("Save").clicked() {
                                if let Err(e) = self.update_db_config() {
                                    self.edit_db_form.error = Some(e.clone());
                                    notify::db_save_error(&e);
                                } else {
                                    let db_name = self.edit_db_form.name.trim().to_string();
                                    self.show_edit_db = false;
                                    self.edit_db_form.error = None;
                                    self.edit_db_form.test_status = None;
                                    self.reset_edit_db_form();
                                    notify::db_save_success(&db_name);
                                }
                            }
                            if ui.button("Cancel").clicked() {
                                self.show_edit_db = false;
                                self.reset_edit_db_form();
                            }
                        });
                    });
                });
            if !open {
                self.show_edit_db = false;
                self.reset_edit_db_form();
            }
        }
    }

    pub fn ui_manage_db_window(&mut self, ctx: &egui::Context) {
        if self.show_manage_db {
            let mut open = true;
            egui::Window::new("Databases")
                .open(&mut open)
                .default_size(egui::vec2(600.0, 400.0))
                .show(ctx, |ui| {
                    self.ensure_storage();
                    if let Some(storage) = &self.storage {
                        let list = tokio::task::block_in_place(|| {
                            tokio::runtime::Handle::current().block_on(async {
                                storage.list_db_configs(None).await
                            })
                        }).unwrap_or_default();
                        
                        if list.is_empty() {
                            ui.label("No databases configured");
                        } else {
                            let mut to_select: Option<String> = None;
                            let mut to_edit: Option<String> = None;
                            let mut to_delete: Vec<String> = Vec::new();
                            let active_id = self.active_db_config_id.clone();
                            
                            egui::ScrollArea::vertical()
                                .auto_shrink([false; 2])
                                .show(ui, |ui| {
                                    for cfg in &list {
                                        ui.group(|ui| {
                                            // Parse DSN to show details
                                            let parsed = Self::parse_dsn(&cfg.dsn);
                                            
                                            // Database name and engine
                                            ui.horizontal(|ui| {
                                                ui.heading(format!("{} {}", egui_phosphor::regular::DATABASE, cfg.id));
                                                ui.label(format!("[{}]", cfg.engine));
                                                if Some(cfg.id.clone()) == active_id {
                                                    ui.colored_label(
                                                        egui::Color32::GREEN,
                                                        "(Active)"
                                                    );
                                                }
                                            });
                                            
                                            // Connection details
                                            if let Some(p) = parsed {
                                                ui.horizontal(|ui| {
                                                    ui.label("Host:");
                                                    ui.label(&p.host);
                                                    ui.add_space(20.0);
                                                    ui.label("Port:");
                                                    ui.label(&p.port);
                                                });
                                                ui.horizontal(|ui| {
                                                    ui.label("Database:");
                                                    ui.label(if p.database.is_empty() {
                                                        "<default>"
                                                    } else {
                                                        &p.database
                                                    });
                                                    ui.add_space(20.0);
                                                    ui.label("User:");
                                                    ui.label(&p.user);
                                                });
                                            }

                                            ui.separator();
                                            
                                            // Action buttons
                                            ui.horizontal(|ui| {
                                                if ui.button("Select").clicked() {
                                                    to_select = Some(cfg.id.clone());
                                                }
                                                if ui.button("Edit").clicked() {
                                                    to_edit = Some(cfg.id.clone());
                                                }
                                                if ui.button("Delete").clicked() {
                                                    to_delete.push(cfg.id.clone());
                                                }
                                            });
                                            
                                        });
                                    }
                                });
                            
                            // Execute actions outside the closure
                            if let Some(id) = to_select {
                                self.active_db_config_id = Some(id.clone());
                                notify::info(format!("Selected database: {}", id));
                            }
                            if let Some(id) = to_edit {
                                if let Err(e) = self.load_db_config(&id) {
                                    notify::error(format!("Failed to load: {}", e));
                                } else {
                                    self.show_edit_db = true;
                                }
                            }
                            for id in to_delete {
                                if let Some(ref storage) = self.storage {
                                    let _ = tokio::task::block_in_place(|| {
                                        tokio::runtime::Handle::current().block_on(async {
                                            storage.delete_db_config(&id).await
                                        })
                                    });
                                    // Clear active if deleted
                                    if self.active_db_config_id.as_ref() == Some(&id) {
                                        self.active_db_config_id = None;
                                    }
                                    notify::info(format!("Deleted database: {}", id));
                                }
                            }
                        }
                    } else {
                        ui.label("Storage not ready");
                    }
                });
            if !open {
                self.show_manage_db = false;
            }
        }
    }

    fn now_ts() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
    }

    pub fn test_connection(&mut self) {
        // 验证必填字段
        if self.add_db_form.host.trim().is_empty() {
            self.add_db_form.test_status = Some(false);
            self.add_db_form.error = Some("Host is required".into());
            notify::db_connection_error(
                &self.add_db_form.name.trim().to_string(),
                "Host is required"
            );
            return;
        }
        if self.add_db_form.user.trim().is_empty() {
            self.add_db_form.test_status = Some(false);
            self.add_db_form.error = Some("User is required".into());
            notify::db_connection_error(
                &self.add_db_form.name.trim().to_string(),
                "User is required"
            );
            return;
        }
        
        // 解析端口
        let port: u16 = match self.add_db_form.port.parse() {
            Ok(p) if p > 0 => p,
            _ => {
                self.add_db_form.test_status = Some(false);
                self.add_db_form.error = Some("Port must be a valid number > 0".into());
                notify::db_connection_error(
                    &self.add_db_form.name.trim().to_string(),
                    "Port must be a valid number > 0"
                );
                return;
            }
        };
        
        // 构建 DSN
        let dbname = if self.add_db_form.database.trim().is_empty() {
            String::new()
        } else {
            self.add_db_form.database.trim().to_string()
        };
        let dsn = format!(
            "{}://{}:{}@{}:{}{}",
            self.add_db_form.engine.as_str(),
            urlencoding::encode(self.add_db_form.user.trim()),
            urlencoding::encode(self.add_db_form.password.trim()),
            self.add_db_form.host.trim(),
            port,
            if dbname.is_empty() {
                String::new()
            } else {
                format!("/{}", dbname)
            }
        );
        
        // 获取数据库名称用于通知
        let db_name = if self.add_db_form.name.trim().is_empty() {
            format!("{}@{}:{}", self.add_db_form.user.trim(), self.add_db_form.host.trim(), port)
        } else {
            self.add_db_form.name.trim().to_string()
        };
        
        // 测试连接
        self.add_db_form.error = None;
        let result = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                PgPoolOptions::new()
                    .max_connections(1)
                    .connect(&dsn)
                    .await
            })
        });
        
        match result {
            Ok(pool) => {
                // 尝试执行一个简单查询来验证连接
                let query_result = tokio::task::block_in_place(|| {
                    tokio::runtime::Handle::current().block_on(async {
                        sqlx::query("SELECT 1").execute(&pool).await
                    })
                });
                match query_result {
                    Ok(_) => {
                        self.add_db_form.test_status = Some(true);
                        self.add_db_form.error = None;
                        notify::db_connection_success(&db_name);
                    }
                    Err(e) => {
                        self.add_db_form.test_status = Some(false);
                        let error_msg = format!("Connection test failed: {}", e);
                        self.add_db_form.error = Some(error_msg.clone());
                        notify::db_connection_error(&db_name, &error_msg);
                    }
                }
            }
            Err(e) => {
                self.add_db_form.test_status = Some(false);
                let error_msg = format!("Connection failed: {}", e);
                self.add_db_form.error = Some(error_msg.clone());
                notify::db_connection_error(&db_name, &error_msg);
            }
        }
    }

    pub fn save_new_database(&mut self) -> Result<(), String> {
        self.ensure_storage();
        let Some(storage) = self.storage.as_ref() else {
            return Err("storage not ready".into());
        };
        if self.add_db_form.name.trim().is_empty() {
            return Err("Name is required".into());
        }
        if self.add_db_form.host.trim().is_empty() {
            return Err("Host is required".into());
        }
        let port: u16 = self
            .add_db_form.port
            .parse()
            .map_err(|_| "Port must be a number")?;
        if port == 0 {
            return Err("Port must be > 0".into());
        }
        if self.add_db_form.user.trim().is_empty() {
            return Err("User is required".into());
        }
        let dbname = if self.add_db_form.database.trim().is_empty() {
            String::new()
        } else {
            self.add_db_form.database.trim().to_string()
        };
        let dsn = format!(
            "{}://{}:{}@{}:{}{}",
            self.add_db_form.engine.as_str(),
            urlencoding::encode(self.add_db_form.user.trim()),
            urlencoding::encode(self.add_db_form.password.trim()),
            self.add_db_form.host.trim(),
            port,
            if dbname.is_empty() {
                String::new()
            } else {
                format!("/{}", dbname)
            }
        );
        let now = Self::now_ts();
        let cfg = pgone_storage::models::DbConfig {
            id: self.add_db_form.name.trim().to_string(),
            engine: self.add_db_form.engine.as_str().to_string(),
            dsn,
            default_schemas: None,
            include_system: Some(false),
            created_at: now,
            updated_at: now,
        };
        let res = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                storage.upsert_db_config(&cfg).await
            })
        });
        match res {
            Ok(_) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }

    /// Load database config for editing
    pub fn load_db_config(&mut self, id: &str) -> Result<(), String> {
        self.ensure_storage();
        let Some(storage) = self.storage.as_ref() else {
            return Err("storage not ready".into());
        };
        let cfg = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                storage.get_db_config(id).await
            })
        }).map_err(|e| e.to_string())?;
        let Some(cfg) = cfg else {
            return Err("Database config not found".into());
        };
        
        // Parse DSN to fill form fields
        if let Some(parsed) = Self::parse_dsn(&cfg.dsn) {
            self.edit_db_id = Some(cfg.id.clone());
            self.edit_db_form.name = cfg.id.clone();
            self.edit_db_form.engine = match parsed.engine.as_str() {
                "postgresql" | "postgres" => DbEngine::Postgresql,
                "mysql" => DbEngine::Mysql,
                _ => DbEngine::Postgresql,
            };
            self.edit_db_form.host = parsed.host;
            self.edit_db_form.port = parsed.port;
            self.edit_db_form.database = parsed.database;
            self.edit_db_form.user = parsed.user;
            // Password is not stored, leave it empty
            self.edit_db_form.password = String::new();
        } else {
            return Err("Failed to parse DSN".into());
        }
        
        Ok(())
    }

    /// Update database config
    pub fn update_db_config(&mut self) -> Result<(), String> {
        self.ensure_storage();
        let Some(storage) = self.storage.as_ref() else {
            return Err("storage not ready".into());
        };
        let Some(ref id) = self.edit_db_id else {
            return Err("No database ID to update".into());
        };
        
        // Load existing config to preserve created_at
        let existing_cfg = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                storage.get_db_config(id).await
            })
        }).map_err(|e| e.to_string())?;
        let Some(existing_cfg) = existing_cfg else {
            return Err("Database config not found".into());
        };
        
        if self.edit_db_form.name.trim().is_empty() {
            return Err("Name is required".into());
        }
        if self.edit_db_form.host.trim().is_empty() {
            return Err("Host is required".into());
        }
        let port: u16 = self
            .edit_db_form.port
            .parse()
            .map_err(|_| "Port must be a number")?;
        if port == 0 {
            return Err("Port must be > 0".into());
        }
        if self.edit_db_form.user.trim().is_empty() {
            return Err("User is required".into());
        }
        
        // If password is empty, try to get it from existing DSN
        let password = if self.edit_db_form.password.trim().is_empty() {
            // Try to extract password from existing DSN
            if let Some(url) = url::Url::parse(&existing_cfg.dsn).ok() {
                url.password().unwrap_or("").to_string()
            } else {
                String::new()
            }
        } else {
            self.edit_db_form.password.trim().to_string()
        };
        
        let dbname = if self.edit_db_form.database.trim().is_empty() {
            String::new()
        } else {
            self.edit_db_form.database.trim().to_string()
        };
        let dsn = format!(
            "{}://{}:{}@{}:{}{}",
            self.edit_db_form.engine.as_str(),
            urlencoding::encode(self.edit_db_form.user.trim()),
            urlencoding::encode(&password),
            self.edit_db_form.host.trim(),
            port,
            if dbname.is_empty() {
                String::new()
            } else {
                format!("/{}", dbname)
            }
        );
        
        let cfg = pgone_storage::models::DbConfig {
            id: self.edit_db_form.name.trim().to_string(),
            engine: self.edit_db_form.engine.as_str().to_string(),
            dsn,
            default_schemas: existing_cfg.default_schemas.clone(),
            include_system: existing_cfg.include_system,
            created_at: existing_cfg.created_at,
            updated_at: Self::now_ts(),
        };
        
        let res = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                storage.upsert_db_config(&cfg).await
            })
        });
        match res {
            Ok(_) => {
                // Update active_db_config_id if it was the edited one
                if self.active_db_config_id.as_ref() == Some(id) {
                    self.active_db_config_id = Some(cfg.id.clone());
                }
                Ok(())
            },
            Err(e) => Err(e.to_string()),
        }
    }

    /// Test connection for edit form
    pub fn test_edit_connection(&mut self) {
        // Validation
        if self.edit_db_form.host.trim().is_empty() {
            self.edit_db_form.test_status = Some(false);
            self.edit_db_form.error = Some("Host is required".into());
            return;
        }
        if self.edit_db_form.user.trim().is_empty() {
            self.edit_db_form.test_status = Some(false);
            self.edit_db_form.error = Some("User is required".into());
            return;
        }
        
        let port: u16 = match self.edit_db_form.port.parse() {
            Ok(p) if p > 0 => p,
            _ => {
                self.edit_db_form.test_status = Some(false);
                self.edit_db_form.error = Some("Port must be a valid number > 0".into());
                return;
            }
        };
        
        // Get password from existing config if empty
        let password = if self.edit_db_form.password.trim().is_empty() {
            if let Some(ref id) = self.edit_db_id {
                if let Some(storage) = &self.storage {
                    if let Ok(Some(cfg)) = tokio::task::block_in_place(|| {
                        tokio::runtime::Handle::current().block_on(async {
                            storage.get_db_config(id).await
                        })
                    }) {
                        if let Some(url) = url::Url::parse(&cfg.dsn).ok() {
                            url.password().unwrap_or("").to_string()
                        } else {
                            String::new()
                        }
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                }
            } else {
                String::new()
            }
        } else {
            self.edit_db_form.password.trim().to_string()
        };
        
        let dbname = if self.edit_db_form.database.trim().is_empty() {
            String::new()
        } else {
            self.edit_db_form.database.trim().to_string()
        };
        let dsn = format!(
            "{}://{}:{}@{}:{}{}",
            self.edit_db_form.engine.as_str(),
            urlencoding::encode(self.edit_db_form.user.trim()),
            urlencoding::encode(&password),
            self.edit_db_form.host.trim(),
            port,
            if dbname.is_empty() {
                String::new()
            } else {
                format!("/{}", dbname)
            }
        );
        
        let db_name = if self.edit_db_form.name.trim().is_empty() {
            format!("{}@{}:{}", self.edit_db_form.user.trim(), self.edit_db_form.host.trim(), port)
        } else {
            self.edit_db_form.name.trim().to_string()
        };
        
        self.edit_db_form.error = None;
        let result = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                PgPoolOptions::new()
                    .max_connections(1)
                    .connect(&dsn)
                    .await
            })
        });
        
        match result {
            Ok(pool) => {
                let query_result = tokio::task::block_in_place(|| {
                    tokio::runtime::Handle::current().block_on(async {
                        sqlx::query("SELECT 1").execute(&pool).await
                    })
                });
                match query_result {
                    Ok(_) => {
                        self.edit_db_form.test_status = Some(true);
                        self.edit_db_form.error = None;
                        notify::db_connection_success(&db_name);
                    }
                    Err(e) => {
                        self.edit_db_form.test_status = Some(false);
                        let error_msg = format!("Connection test failed: {}", e);
                        self.edit_db_form.error = Some(error_msg.clone());
                        notify::db_connection_error(&db_name, &error_msg);
                    }
                }
            }
            Err(e) => {
                self.edit_db_form.test_status = Some(false);
                let error_msg = format!("Connection failed: {}", e);
                self.edit_db_form.error = Some(error_msg.clone());
                notify::db_connection_error(&db_name, &error_msg);
            }
        }
    }
}
