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

pub struct DbManager {
    pub active_db_config_id: Option<String>,
    pub show_add_db: bool,
    pub add_db_engine: DbEngine,
    pub add_db_name: String,
    pub add_db_host: String,
    pub add_db_port: String,
    pub add_db_database: String,
    pub add_db_user: String,
    pub add_db_password: String,
    pub add_db_error: Option<String>,
    pub add_db_test_status: Option<bool>, // None = 未测试, Some(true) = 成功, Some(false) = 失败
    pub show_manage_db: bool,
    pub storage: Option<StorageBlocking>,
    pub rt: tokio::runtime::Runtime,
    pub pools: std::collections::HashMap<u64, PgPool>,
}

impl Default for DbManager {
    fn default() -> Self {
        Self {
            active_db_config_id: None,
            show_add_db: false,
            add_db_engine: DbEngine::Postgresql,
            add_db_name: String::new(),
            add_db_host: "localhost".to_string(),
            add_db_port: "5432".to_string(),
            add_db_database: String::new(),
            add_db_user: String::new(),
            add_db_password: String::new(),
            add_db_error: None,
            add_db_test_status: None,
            show_manage_db: false,
            storage: None,
            rt: tokio::runtime::Runtime::new().expect("tokio runtime"),
            pools: Default::default(),
        }
    }
}

impl DbManager {
    /// Reset add database form fields to default values
    pub fn reset_add_db_form(&mut self) {
        self.add_db_engine = DbEngine::Postgresql;
        self.add_db_name = String::new();
        self.add_db_host = "localhost".to_string();
        self.add_db_port = "5432".to_string();
        self.add_db_database = String::new();
        self.add_db_user = String::new();
        self.add_db_password = String::new();
        self.add_db_error = None;
        self.add_db_test_status = None;
    }

    pub fn ensure_storage(&mut self) {
        if self.storage.is_some() {
            return;
        }
        if let Ok(storage) = self
            .rt
            .block_on(async { StorageBlocking::open_local("pgone.db").await })
        {
            self.storage = Some(storage);
        }
    }

    #[allow(dead_code)]
    pub fn ui_db_config(&mut self, _app: &mut crate::AppFrame, ui: &mut egui::Ui) {
        self.ensure_storage();
        let mut to_switch: Option<String> = None;
        if let Some(storage) = &self.storage {
            let list = self
                .rt
                .block_on(async { storage.list_db_configs(None).await })
                .unwrap_or_default();
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
                            .selected_text(self.add_db_engine.as_str())
                            .show_ui(ui, |ui| {
                                for engine in DbEngine::all() {
                                    ui.selectable_value(
                                        &mut self.add_db_engine,
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
                        ui.text_edit_singleline(&mut self.add_db_name);
                    });
                    ui.horizontal(|ui| {
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.set_width(label_width);
                            ui.label("Host");
                        });
                        ui.text_edit_singleline(&mut self.add_db_host);
                    });
                    ui.horizontal(|ui| {
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.set_width(label_width);
                            ui.label("Port");
                        });
                        ui.text_edit_singleline(&mut self.add_db_port);
                    });
                    ui.horizontal(|ui| {
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.set_width(label_width);
                            ui.label("Database");
                        });
                        ui.text_edit_singleline(&mut self.add_db_database);
                    });
                    ui.horizontal(|ui| {
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.set_width(label_width);
                            ui.label("User");
                        });
                        ui.text_edit_singleline(&mut self.add_db_user);
                    });
                    ui.horizontal(|ui| {
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.set_width(label_width);
                            ui.label("Password");
                        });
                        ui.add(
                            egui::TextEdit::singleline(&mut self.add_db_password).password(true),
                        );
                    });
                    
                    if let Some(err) = &self.add_db_error {
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
                            if let Some(success) = self.add_db_test_status {
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
                                    self.add_db_error = Some(e.clone());
                                    notify::db_save_error(&e);
                                } else {
                                    let db_name = self.add_db_name.trim().to_string();
                                    self.show_add_db = false;
                                    self.add_db_error = None;
                                    self.add_db_test_status = None;
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

    pub fn ui_manage_db_window(&mut self, ctx: &egui::Context) {
        if self.show_manage_db {
            let mut open = true;
            egui::Window::new("Databases")
                .open(&mut open)
                .show(ctx, |ui| {
                    self.ensure_storage();
                    if let Some(storage) = &self.storage {
                        let list = self
                            .rt
                            .block_on(async { storage.list_db_configs(None).await })
                            .unwrap_or_default();
                        for cfg in list {
                            ui.horizontal(|ui| {
                                ui.label(cfg.id.to_string());
                                ui.small(format!("[{}]", cfg.engine));
                                if ui.small_button("Delete").clicked() {
                                    let _ = self.rt.block_on(async {
                                        storage.delete_db_config(&cfg.id).await
                                    });
                                }
                            });
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
        if self.add_db_host.trim().is_empty() {
            self.add_db_test_status = Some(false);
            self.add_db_error = Some("Host is required".into());
            notify::db_connection_error(
                &self.add_db_name.trim().to_string(),
                "Host is required"
            );
            return;
        }
        if self.add_db_user.trim().is_empty() {
            self.add_db_test_status = Some(false);
            self.add_db_error = Some("User is required".into());
            notify::db_connection_error(
                &self.add_db_name.trim().to_string(),
                "User is required"
            );
            return;
        }
        
        // 解析端口
        let port: u16 = match self.add_db_port.parse() {
            Ok(p) if p > 0 => p,
            _ => {
                self.add_db_test_status = Some(false);
                self.add_db_error = Some("Port must be a valid number > 0".into());
                notify::db_connection_error(
                    &self.add_db_name.trim().to_string(),
                    "Port must be a valid number > 0"
                );
                return;
            }
        };
        
        // 构建 DSN
        let dbname = if self.add_db_database.trim().is_empty() {
            String::new()
        } else {
            self.add_db_database.trim().to_string()
        };
        let dsn = format!(
            "{}://{}:{}@{}:{}{}",
            self.add_db_engine.as_str(),
            urlencoding::encode(self.add_db_user.trim()),
            urlencoding::encode(self.add_db_password.trim()),
            self.add_db_host.trim(),
            port,
            if dbname.is_empty() {
                String::new()
            } else {
                format!("/{}", dbname)
            }
        );
        
        // 获取数据库名称用于通知
        let db_name = if self.add_db_name.trim().is_empty() {
            format!("{}@{}:{}", self.add_db_user.trim(), self.add_db_host.trim(), port)
        } else {
            self.add_db_name.trim().to_string()
        };
        
        // 测试连接
        self.add_db_error = None;
        let result = self.rt.block_on(async {
            PgPoolOptions::new()
                .max_connections(1)
                .connect(&dsn)
                .await
        });
        
        match result {
            Ok(pool) => {
                // 尝试执行一个简单查询来验证连接
                let query_result = self.rt.block_on(async {
                    sqlx::query("SELECT 1").execute(&pool).await
                });
                match query_result {
                    Ok(_) => {
                        self.add_db_test_status = Some(true);
                        self.add_db_error = None;
                        notify::db_connection_success(&db_name);
                    }
                    Err(e) => {
                        self.add_db_test_status = Some(false);
                        let error_msg = format!("Connection test failed: {}", e);
                        self.add_db_error = Some(error_msg.clone());
                        notify::db_connection_error(&db_name, &error_msg);
                    }
                }
            }
            Err(e) => {
                self.add_db_test_status = Some(false);
                let error_msg = format!("Connection failed: {}", e);
                self.add_db_error = Some(error_msg.clone());
                notify::db_connection_error(&db_name, &error_msg);
            }
        }
    }

    pub fn save_new_database(&mut self) -> Result<(), String> {
        self.ensure_storage();
        let Some(storage) = self.storage.as_ref() else {
            return Err("storage not ready".into());
        };
        if self.add_db_name.trim().is_empty() {
            return Err("Name is required".into());
        }
        if self.add_db_host.trim().is_empty() {
            return Err("Host is required".into());
        }
        let port: u16 = self
            .add_db_port
            .parse()
            .map_err(|_| "Port must be a number")?;
        if port == 0 {
            return Err("Port must be > 0".into());
        }
        if self.add_db_user.trim().is_empty() {
            return Err("User is required".into());
        }
        let dbname = if self.add_db_database.trim().is_empty() {
            String::new()
        } else {
            self.add_db_database.trim().to_string()
        };
        let dsn = format!(
            "{}://{}:{}@{}:{}{}",
            self.add_db_engine.as_str(),
            urlencoding::encode(self.add_db_user.trim()),
            urlencoding::encode(self.add_db_password.trim()),
            self.add_db_host.trim(),
            port,
            if dbname.is_empty() {
                String::new()
            } else {
                format!("/{}", dbname)
            }
        );
        let now = Self::now_ts();
        let cfg = pgone_storage::models::DbConfig {
            id: self.add_db_name.trim().to_string(),
            engine: self.add_db_engine.as_str().to_string(),
            dsn,
            default_schemas: None,
            include_system: Some(false),
            created_at: now,
            updated_at: now,
        };
        let res = self
            .rt
            .block_on(async { storage.upsert_db_config(&cfg).await });
        match res {
            Ok(_) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
}
