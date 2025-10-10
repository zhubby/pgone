use intellid_storage::blocking::StorageBlocking;
use sqlx::postgres::PgPool;

pub struct DbManager {
    pub active_db_config_id: Option<String>,
    pub show_add_db: bool,
    pub add_db_engine: String,
    pub add_db_name: String,
    pub add_db_host: String,
    pub add_db_port: String,
    pub add_db_database: String,
    pub add_db_user: String,
    pub add_db_password: String,
    pub add_db_error: Option<String>,
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
            add_db_engine: "postgres".to_string(),
            add_db_name: String::new(),
            add_db_host: "localhost".to_string(),
            add_db_port: "5432".to_string(),
            add_db_database: String::new(),
            add_db_user: String::new(),
            add_db_password: String::new(),
            add_db_error: None,
            show_manage_db: false,
            storage: None,
            rt: tokio::runtime::Runtime::new().expect("tokio runtime"),
            pools: Default::default(),
        }
    }
}

impl DbManager {
    pub fn ensure_storage(&mut self) {
        if self.storage.is_some() {
            return;
        }
        if let Ok(storage) = self
            .rt
            .block_on(async { StorageBlocking::open_local("intellid.db").await })
        {
            self.storage = Some(storage);
        }
    }

    pub fn ui_db_config(&mut self, app: &mut crate::IntelliGuiApp, ui: &mut egui::Ui) {
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
                    ui.horizontal(|ui| {
                        ui.label("Type");
                        ui.text_edit_singleline(&mut self.add_db_engine);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Name");
                        ui.text_edit_singleline(&mut self.add_db_name);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Host");
                        ui.text_edit_singleline(&mut self.add_db_host);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Port");
                        ui.text_edit_singleline(&mut self.add_db_port);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Database");
                        ui.text_edit_singleline(&mut self.add_db_database);
                    });
                    ui.horizontal(|ui| {
                        ui.label("User");
                        ui.text_edit_singleline(&mut self.add_db_user);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Password");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.add_db_password).password(true),
                        );
                    });
                    if let Some(err) = &self.add_db_error {
                        ui.colored_label(egui::Color32::RED, err);
                    }
                    if ui.button("Save").clicked() {
                        if let Err(e) = self.save_new_database() {
                            self.add_db_error = Some(e);
                        } else {
                            self.show_add_db = false;
                            self.add_db_error = None;
                        }
                    }
                });
            if !open {
                self.show_add_db = false;
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
                                ui.label(format!("{}", cfg.id));
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

    pub fn save_new_database(&mut self) -> Result<(), String> {
        self.ensure_storage();
        let Some(storage) = self.storage.as_ref() else {
            return Err("storage not ready".into());
        };
        if self.add_db_name.trim().is_empty() {
            return Err("Name is required".into());
        }
        if self.add_db_engine.trim().is_empty() {
            return Err("Type is required".into());
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
            self.add_db_engine.trim(),
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
        let cfg = intellid_storage::models::DbConfig {
            id: self.add_db_name.trim().to_string(),
            engine: self.add_db_engine.trim().to_string(),
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
