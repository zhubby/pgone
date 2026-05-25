use crate::futures;
use crate::notify;
use pgone_storage::blocking::StorageBlocking;
use sqlx::postgres::{PgPool, PgPoolOptions};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DbEngine {
    Postgresql,
    TimescaleDB,
    PgVector,
}

impl DbEngine {
    fn as_str(&self) -> &'static str {
        match self {
            DbEngine::Postgresql => "postgresql",
            DbEngine::TimescaleDB => "timescaledb",
            DbEngine::PgVector => "pgvector",
        }
    }

    fn all() -> &'static [DbEngine] {
        &[
            DbEngine::Postgresql,
            DbEngine::TimescaleDB,
            DbEngine::PgVector,
        ]
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
    // SSL configuration
    pub ssl_enabled: bool,
    pub ssl_mode: String,
    pub ssl_cert_file_id: Option<String>,
    pub ssl_key_file_id: Option<String>,
    pub ssl_rootcert_file_id: Option<String>,
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
            ssl_enabled: false,
            ssl_mode: "prefer".to_string(),
            ssl_cert_file_id: None,
            ssl_key_file_id: None,
            ssl_rootcert_file_id: None,
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
    pub delete_confirm_id: Option<String>,
    pub show_delete_confirm: bool,
}

#[derive(Debug, Clone)]
pub enum FilePickerTarget {
    AddSslCert,
    AddSslKey,
    AddSslRootcert,
    EditSslCert,
    EditSslKey,
    EditSslRootcert,
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
            delete_confirm_id: None,
            show_delete_confirm: false,
        }
    }
}

impl DbManager {
    pub fn shutdown(&mut self) {
        let pools = std::mem::take(&mut self.pools);
        if pools.is_empty() {
            return;
        }

        tracing::info!("正在关闭 {} 个 GUI 数据库连接池", pools.len());
        futures::block_on_async(async move {
            let close_futures = pools
                .into_values()
                .map(|pool| async move {
                    pool.close().await;
                })
                .collect::<Vec<_>>();

            match tokio::time::timeout(
                std::time::Duration::from_secs(5),
                ::futures::future::join_all(close_futures),
            )
            .await
            {
                Ok(_) => tracing::info!("GUI 数据库连接池已关闭"),
                Err(_) => tracing::warn!("关闭 GUI 数据库连接池超时"),
            }
        });
    }

    /// Build DSN with SSL parameters if enabled
    fn build_dsn(
        engine: &str,
        user: &str,
        password: &str,
        host: &str,
        port: u16,
        database: &str,
        ssl_enabled: bool,
        ssl_mode: &str,
        ssl_cert_file_id: Option<&String>,
        ssl_key_file_id: Option<&String>,
        ssl_rootcert_file_id: Option<&String>,
        storage: Option<&StorageBlocking>,
    ) -> Result<String, String> {
        let dbname = if database.trim().is_empty() {
            String::new()
        } else {
            database.trim().to_string()
        };

        let mut dsn = format!(
            "{}://{}:{}@{}:{}{}",
            engine,
            urlencoding::encode(user.trim()),
            urlencoding::encode(password.trim()),
            host.trim(),
            port,
            if dbname.is_empty() {
                String::new()
            } else {
                format!("/{}", dbname)
            }
        );

        // Add SSL parameters if enabled
        if ssl_enabled {
            let mut params = vec![format!("sslmode={}", urlencoding::encode(ssl_mode))];

            // Get file paths from file IDs
            if let Some(storage) = storage {
                if let Some(cert_id) = ssl_cert_file_id {
                    if let Ok(Some(file)) =
                        futures::block_on_async(async { storage.get_file(cert_id).await })
                    {
                        let cert_path = pgone_storage::data_file_path(&file.current_path)
                            .to_string_lossy()
                            .to_string();
                        params.push(format!("sslcert={}", urlencoding::encode(&cert_path)));
                    }
                }

                if let Some(key_id) = ssl_key_file_id {
                    if let Ok(Some(file)) =
                        futures::block_on_async(async { storage.get_file(key_id).await })
                    {
                        let key_path = pgone_storage::data_file_path(&file.current_path)
                            .to_string_lossy()
                            .to_string();
                        params.push(format!("sslkey={}", urlencoding::encode(&key_path)));
                    }
                }

                if let Some(rootcert_id) = ssl_rootcert_file_id {
                    if let Ok(Some(file)) =
                        futures::block_on_async(async { storage.get_file(rootcert_id).await })
                    {
                        let rootcert_path = pgone_storage::data_file_path(&file.current_path)
                            .to_string_lossy()
                            .to_string();
                        params.push(format!(
                            "sslrootcert={}",
                            urlencoding::encode(&rootcert_path)
                        ));
                    }
                }
            }

            dsn.push('?');
            dsn.push_str(&params.join("&"));
        }

        Ok(dsn)
    }

    /// Parse DSN to extract connection information
    pub fn parse_dsn(dsn: &str) -> Option<ParsedDsn> {
        // DSN format: postgresql://user:password@host:port/database
        let url = url::Url::parse(dsn).ok()?;
        let engine = url.scheme().to_string();
        let host = url.host_str()?.to_string();
        let port = url
            .port()
            .map(|p| p.to_string())
            .unwrap_or_else(|| match engine.as_str() {
                "postgresql" | "postgres" => "5432".to_string(),
                "mysql" => "3306".to_string(),
                _ => "5432".to_string(),
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
            // Use block_on_async for synchronous access from async context
            if let Ok(Some(cfg)) =
                futures::block_on_async(async { storage.get_db_config(id).await })
            {
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
        if let Ok(storage) =
            futures::block_on_async(async { StorageBlocking::open_default().await })
        {
            self.storage = Some(storage);
        }
    }

    #[allow(dead_code)]
    pub fn ui_db_config(&mut self, _app: &mut crate::AppFrame, ui: &mut egui::Ui) {
        self.ensure_storage();
        let mut to_switch: Option<String> = None;
        if let Some(storage) = &self.storage {
            let list = futures::block_on_async(async { storage.list_db_configs(None).await })
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
            let center = ui.ctx().content_rect().center();
            egui::Window::new("Switch Database Config")
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .default_pos(center)
                .pivot(egui::Align2::CENTER_CENTER)
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
            let center = ctx.content_rect().center();
            egui::Window::new("New Database")
                .open(&mut open)
                .default_pos(center)
                .pivot(egui::Align2::CENTER_CENTER)
                .collapsible(false)
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
                            egui::TextEdit::singleline(&mut self.add_db_form.password)
                                .password(true),
                        );
                    });

                    // SSL Configuration Section
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.set_width(label_width);
                            ui.label("SSL");
                        });
                        ui.checkbox(&mut self.add_db_form.ssl_enabled, "启用SSL");
                    });

                    if self.add_db_form.ssl_enabled {
                        ui.horizontal(|ui| {
                            ui.with_layout(
                                egui::Layout::left_to_right(egui::Align::Center),
                                |ui| {
                                    ui.set_width(label_width);
                                    ui.label("SSL Mode");
                                },
                            );
                            egui::ComboBox::from_id_salt("add_ssl_mode")
                                .selected_text(&self.add_db_form.ssl_mode)
                                .show_ui(ui, |ui| {
                                    for mode in [
                                        "disable",
                                        "allow",
                                        "prefer",
                                        "require",
                                        "verify-ca",
                                        "verify-full",
                                    ] {
                                        ui.selectable_value(
                                            &mut self.add_db_form.ssl_mode,
                                            mode.to_string(),
                                            mode,
                                        );
                                    }
                                });
                        });

                        // SSL Certificate files
                        ui.horizontal(|ui| {
                            ui.with_layout(
                                egui::Layout::left_to_right(egui::Align::Center),
                                |ui| {
                                    ui.set_width(label_width);
                                    ui.label("客户端证书");
                                },
                            );
                            if ui.button("选择文件").clicked() {
                                self.select_and_upload_file(FilePickerTarget::AddSslCert);
                            }
                            if let Some(ref file_id) = self.add_db_form.ssl_cert_file_id {
                                if let Some(file_name) = self.get_file_name(file_id) {
                                    ui.label(format!("已选择: {}", file_name));
                                }
                            }
                        });

                        ui.horizontal(|ui| {
                            ui.with_layout(
                                egui::Layout::left_to_right(egui::Align::Center),
                                |ui| {
                                    ui.set_width(label_width);
                                    ui.label("客户端密钥");
                                },
                            );
                            if ui.button("选择文件").clicked() {
                                self.select_and_upload_file(FilePickerTarget::AddSslKey);
                            }
                            if let Some(ref file_id) = self.add_db_form.ssl_key_file_id {
                                if let Some(file_name) = self.get_file_name(file_id) {
                                    ui.label(format!("已选择: {}", file_name));
                                }
                            }
                        });

                        ui.horizontal(|ui| {
                            ui.with_layout(
                                egui::Layout::left_to_right(egui::Align::Center),
                                |ui| {
                                    ui.set_width(label_width);
                                    ui.label("根证书");
                                },
                            );
                            if ui.button("选择文件").clicked() {
                                self.select_and_upload_file(FilePickerTarget::AddSslRootcert);
                            }
                            if let Some(ref file_id) = self.add_db_form.ssl_rootcert_file_id {
                                if let Some(file_name) = self.get_file_name(file_id) {
                                    ui.label(format!("已选择: {}", file_name));
                                }
                            }
                        });
                    }

                    if let Some(err) = &self.add_db_form.error {
                        ui.colored_label(egui::Color32::RED, err);
                    }

                    // 按钮布局：Test Connection 在左下角，Save 在右下角
                    ui.horizontal(|ui| {
                        // 左侧：测试连接按钮和状态标记
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            let test_button = egui::Button::new(
                                egui::RichText::new("Test Connection").color(egui::Color32::RED),
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
            let center = ctx.content_rect().center();
            egui::Window::new("Edit Database")
                .open(&mut open)
                .default_pos(center)
                .pivot(egui::Align2::CENTER_CENTER)
                .collapsible(false)
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

                    // SSL Configuration Section
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.set_width(label_width);
                            ui.label("SSL");
                        });
                        ui.checkbox(&mut self.edit_db_form.ssl_enabled, "启用SSL");
                    });

                    if self.edit_db_form.ssl_enabled {
                        ui.horizontal(|ui| {
                            ui.with_layout(
                                egui::Layout::left_to_right(egui::Align::Center),
                                |ui| {
                                    ui.set_width(label_width);
                                    ui.label("SSL Mode");
                                },
                            );
                            egui::ComboBox::from_id_salt("edit_ssl_mode")
                                .selected_text(&self.edit_db_form.ssl_mode)
                                .show_ui(ui, |ui| {
                                    for mode in [
                                        "disable",
                                        "allow",
                                        "prefer",
                                        "require",
                                        "verify-ca",
                                        "verify-full",
                                    ] {
                                        ui.selectable_value(
                                            &mut self.edit_db_form.ssl_mode,
                                            mode.to_string(),
                                            mode,
                                        );
                                    }
                                });
                        });

                        // SSL Certificate files
                        ui.horizontal(|ui| {
                            ui.with_layout(
                                egui::Layout::left_to_right(egui::Align::Center),
                                |ui| {
                                    ui.set_width(label_width);
                                    ui.label("客户端证书");
                                },
                            );
                            if ui.button("选择文件").clicked() {
                                self.select_and_upload_file(FilePickerTarget::EditSslCert);
                            }
                            if let Some(ref file_id) = self.edit_db_form.ssl_cert_file_id {
                                if let Some(file_name) = self.get_file_name(file_id) {
                                    ui.label(format!("已选择: {}", file_name));
                                }
                            }
                        });

                        ui.horizontal(|ui| {
                            ui.with_layout(
                                egui::Layout::left_to_right(egui::Align::Center),
                                |ui| {
                                    ui.set_width(label_width);
                                    ui.label("客户端密钥");
                                },
                            );
                            if ui.button("选择文件").clicked() {
                                self.select_and_upload_file(FilePickerTarget::EditSslKey);
                            }
                            if let Some(ref file_id) = self.edit_db_form.ssl_key_file_id {
                                if let Some(file_name) = self.get_file_name(file_id) {
                                    ui.label(format!("已选择: {}", file_name));
                                }
                            }
                        });

                        ui.horizontal(|ui| {
                            ui.with_layout(
                                egui::Layout::left_to_right(egui::Align::Center),
                                |ui| {
                                    ui.set_width(label_width);
                                    ui.label("根证书");
                                },
                            );
                            if ui.button("选择文件").clicked() {
                                self.select_and_upload_file(FilePickerTarget::EditSslRootcert);
                            }
                            if let Some(ref file_id) = self.edit_db_form.ssl_rootcert_file_id {
                                if let Some(file_name) = self.get_file_name(file_id) {
                                    ui.label(format!("已选择: {}", file_name));
                                }
                            }
                        });
                    }

                    if let Some(err) = &self.edit_db_form.error {
                        ui.colored_label(egui::Color32::RED, err);
                    }

                    ui.horizontal(|ui| {
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            let test_button = egui::Button::new(
                                egui::RichText::new("Test Connection").color(egui::Color32::RED),
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
            let center = ctx.content_rect().center();
            egui::Window::new("Databases")
                .open(&mut open)
                .default_size(egui::vec2(600.0, 400.0))
                .default_pos(center)
                .pivot(egui::Align2::CENTER_CENTER)
                .collapsible(false)
                .show(ctx, |ui| {
                    self.ensure_storage();
                    if let Some(storage) = &self.storage {
                        let list =
                            futures::block_on_async(async { storage.list_db_configs(None).await })
                                .unwrap_or_default();

                        if list.is_empty() {
                            ui.label("No databases configured");
                        } else {
                            let mut to_select: Option<String> = None;
                            let mut to_edit: Option<String> = None;
                            let mut to_set_default: Option<String> = None;
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
                                                ui.heading(format!(
                                                    "{} {}",
                                                    egui_phosphor::regular::DATABASE,
                                                    cfg.id
                                                ));
                                                ui.label(format!("[{}]", cfg.engine));
                                                if Some(cfg.id.clone()) == active_id {
                                                    ui.colored_label(
                                                        egui::Color32::GREEN,
                                                        "(Active)",
                                                    );
                                                }
                                                if cfg.default_config == Some(true) {
                                                    ui.colored_label(egui::Color32::BLUE, "(默认)");
                                                }
                                            });

                                            // Connection details
                                            if let Some(p) = parsed {
                                                ui.columns(2, |columns| {
                                                    // 左列
                                                    columns[0].horizontal(|ui| {
                                                        ui.label(format!(
                                                            "{} Host:",
                                                            egui_phosphor::regular::GLOBE
                                                        ));
                                                        ui.label(&p.host);
                                                    });
                                                    columns[0].horizontal(|ui| {
                                                        ui.label(format!(
                                                            "{} Database:",
                                                            egui_phosphor::regular::DATABASE
                                                        ));
                                                        ui.label(if p.database.is_empty() {
                                                            "<default>"
                                                        } else {
                                                            &p.database
                                                        });
                                                    });

                                                    // 右列
                                                    columns[1].horizontal(|ui| {
                                                        ui.label(format!(
                                                            "{} Port:",
                                                            egui_phosphor::regular::PLUG
                                                        ));
                                                        ui.label(&p.port);
                                                    });
                                                    columns[1].horizontal(|ui| {
                                                        ui.label(format!(
                                                            "{} User:",
                                                            egui_phosphor::regular::USER
                                                        ));
                                                        ui.label(&p.user);
                                                    });
                                                });
                                            }

                                            ui.separator();

                                            // Action buttons
                                            ui.horizontal(|ui| {
                                                if ui.button("选择").clicked() {
                                                    to_select = Some(cfg.id.clone());
                                                }
                                                if ui.button("编辑").clicked() {
                                                    to_edit = Some(cfg.id.clone());
                                                }
                                                if cfg.default_config != Some(true) {
                                                    if ui.button("设为默认").clicked() {
                                                        to_set_default = Some(cfg.id.clone());
                                                    }
                                                }
                                                if ui
                                                    .button(
                                                        egui::RichText::new("删除")
                                                            .color(egui::Color32::RED),
                                                    )
                                                    .clicked()
                                                {
                                                    self.delete_confirm_id = Some(cfg.id.clone());
                                                    self.show_delete_confirm = true;
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
                            if let Some(id) = to_set_default {
                                if let Err(e) = self.set_default_db_config(&id) {
                                    notify::error(format!("设置默认配置失败: {}", e));
                                } else {
                                    notify::info(format!("已将 '{}' 设为默认配置", id));
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

        // Show delete confirmation dialog
        if self.show_delete_confirm {
            let mut open = true;
            let id_to_delete = self.delete_confirm_id.clone();
            let center = ctx.content_rect().center();

            egui::Window::new("确认删除")
                .open(&mut open)
                .default_pos(center)
                .pivot(egui::Align2::CENTER_CENTER)
                .collapsible(false)
                .show(ctx, |ui| {
                    if let Some(ref id) = id_to_delete {
                        ui.label(format!("确定要删除数据库配置 '{}' 吗？", id));
                        ui.label("此操作不可撤销。");
                        ui.add_space(10.0);

                        ui.horizontal(|ui| {
                            if ui.button("取消").clicked() {
                                self.show_delete_confirm = false;
                                self.delete_confirm_id = None;
                            }
                            if ui
                                .button(egui::RichText::new("确认删除").color(egui::Color32::RED))
                                .clicked()
                            {
                                if let Some(ref storage) = self.storage {
                                    let id_clone = id.clone();
                                    let _ = futures::block_on_async(async {
                                        storage.delete_db_config(&id_clone).await
                                    });
                                    // Clear active if deleted
                                    if self.active_db_config_id.as_ref() == Some(id) {
                                        self.active_db_config_id = None;
                                    }
                                    notify::info(format!("Deleted database: {}", id));
                                }
                                self.show_delete_confirm = false;
                                self.delete_confirm_id = None;
                            }
                        });
                    }
                });

            if !open {
                self.show_delete_confirm = false;
                self.delete_confirm_id = None;
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
                "Host is required",
            );
            return;
        }
        if self.add_db_form.user.trim().is_empty() {
            self.add_db_form.test_status = Some(false);
            self.add_db_form.error = Some("User is required".into());
            notify::db_connection_error(
                &self.add_db_form.name.trim().to_string(),
                "User is required",
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
                    "Port must be a valid number > 0",
                );
                return;
            }
        };

        // 构建 DSN
        let dsn = match Self::build_dsn(
            self.add_db_form.engine.as_str(),
            &self.add_db_form.user,
            &self.add_db_form.password,
            &self.add_db_form.host,
            port,
            &self.add_db_form.database,
            self.add_db_form.ssl_enabled,
            &self.add_db_form.ssl_mode,
            self.add_db_form.ssl_cert_file_id.as_ref(),
            self.add_db_form.ssl_key_file_id.as_ref(),
            self.add_db_form.ssl_rootcert_file_id.as_ref(),
            self.storage.as_ref(),
        ) {
            Ok(dsn) => dsn,
            Err(e) => {
                self.add_db_form.test_status = Some(false);
                self.add_db_form.error = Some(e.clone());
                notify::db_connection_error(&self.add_db_form.name.trim().to_string(), &e);
                return;
            }
        };

        // 获取数据库名称用于通知
        let db_name = if self.add_db_form.name.trim().is_empty() {
            format!(
                "{}@{}:{}",
                self.add_db_form.user.trim(),
                self.add_db_form.host.trim(),
                port
            )
        } else {
            self.add_db_form.name.trim().to_string()
        };

        // 测试连接
        self.add_db_form.error = None;
        let result = futures::block_on_async(async {
            PgPoolOptions::new().max_connections(1).connect(&dsn).await
        });

        match result {
            Ok(pool) => {
                // 尝试执行一个简单查询来验证连接
                let query_result =
                    futures::block_on_async(async { sqlx::query("SELECT 1").execute(&pool).await });
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

    /// Quickly verify database connection with timeout
    /// Returns Ok(()) if connection is successful, Err(error_message) if failed
    pub fn verify_connection_quickly(dsn: &str) -> Result<(), String> {
        use std::time::Duration;
        use tokio::time::timeout;

        // Set timeout to 5 seconds
        let timeout_duration = Duration::from_secs(5);

        futures::block_on_async(async {
            match timeout(timeout_duration, async {
                // Try to connect
                let pool = PgPoolOptions::new()
                    .max_connections(1)
                    .connect(dsn)
                    .await
                    .map_err(|e| format!("Connection failed: {}", e))?;

                // Try to execute a simple query
                sqlx::query("SELECT 1")
                    .execute(&pool)
                    .await
                    .map_err(|e| format!("Query execution failed: {}", e))?;

                Ok::<(), String>(())
            })
            .await
            {
                Ok(Ok(())) => Ok(()),
                Ok(Err(e)) => Err(e),
                Err(_) => Err("Connection timeout: database is not reachable".to_string()),
            }
        })
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
            .add_db_form
            .port
            .parse()
            .map_err(|_| "Port must be a number")?;
        if port == 0 {
            return Err("Port must be > 0".into());
        }
        if self.add_db_form.user.trim().is_empty() {
            return Err("User is required".into());
        }
        let dsn = Self::build_dsn(
            self.add_db_form.engine.as_str(),
            &self.add_db_form.user,
            &self.add_db_form.password,
            &self.add_db_form.host,
            port,
            &self.add_db_form.database,
            self.add_db_form.ssl_enabled,
            &self.add_db_form.ssl_mode,
            self.add_db_form.ssl_cert_file_id.as_ref(),
            self.add_db_form.ssl_key_file_id.as_ref(),
            self.add_db_form.ssl_rootcert_file_id.as_ref(),
            self.storage.as_ref(),
        )
        .map_err(|e| format!("Failed to build DSN: {}", e))?;

        let now = Self::now_ts();
        let cfg = pgone_storage::models::DbConfig {
            id: self.add_db_form.name.trim().to_string(),
            engine: self.add_db_form.engine.as_str().to_string(),
            dsn,
            default_schemas: None,
            include_system: Some(false),
            default_config: Some(false),
            created_at: now,
            updated_at: now,
        };
        let res = futures::block_on_async(async { storage.upsert_db_config(&cfg).await });
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
        let cfg = futures::block_on_async(async { storage.get_db_config(id).await })
            .map_err(|e| e.to_string())?;
        let Some(cfg) = cfg else {
            return Err("Database config not found".into());
        };

        // Parse DSN to fill form fields
        if let Some(parsed) = Self::parse_dsn(&cfg.dsn) {
            self.edit_db_id = Some(cfg.id.clone());
            self.edit_db_form.name = cfg.id.clone();
            self.edit_db_form.engine = match parsed.engine.as_str() {
                "postgresql" | "postgres" => DbEngine::Postgresql,
                "timescaledb" => DbEngine::TimescaleDB,
                "pgvector" => DbEngine::PgVector,
                _ => DbEngine::Postgresql,
            };
            self.edit_db_form.host = parsed.host;
            self.edit_db_form.port = parsed.port;
            self.edit_db_form.database = parsed.database;
            self.edit_db_form.user = parsed.user;
            // Password is not stored, leave it empty
            self.edit_db_form.password = String::new();

            // Parse SSL parameters from DSN
            if let Ok(url) = url::Url::parse(&cfg.dsn) {
                let query_pairs: std::collections::HashMap<String, String> = url
                    .query_pairs()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect();

                // Check if SSL is enabled
                if let Some(ssl_mode) = query_pairs.get("sslmode") {
                    self.edit_db_form.ssl_enabled = true;
                    self.edit_db_form.ssl_mode = ssl_mode.clone();

                    // Try to find file IDs from paths
                    self.ensure_storage();
                    if let Some(storage) = &self.storage {
                        // Find SSL cert file ID
                        if let Some(cert_path) = query_pairs.get("sslcert") {
                            if let Some(file_id) = Self::find_file_id_by_path(storage, cert_path) {
                                self.edit_db_form.ssl_cert_file_id = Some(file_id);
                            }
                        }

                        // Find SSL key file ID
                        if let Some(key_path) = query_pairs.get("sslkey") {
                            if let Some(file_id) = Self::find_file_id_by_path(storage, key_path) {
                                self.edit_db_form.ssl_key_file_id = Some(file_id);
                            }
                        }

                        // Find SSL rootcert file ID
                        if let Some(rootcert_path) = query_pairs.get("sslrootcert") {
                            if let Some(file_id) =
                                Self::find_file_id_by_path(storage, rootcert_path)
                            {
                                self.edit_db_form.ssl_rootcert_file_id = Some(file_id);
                            }
                        }
                    }
                } else {
                    self.edit_db_form.ssl_enabled = false;
                }
            }
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
        let existing_cfg = futures::block_on_async(async { storage.get_db_config(id).await })
            .map_err(|e| e.to_string())?;
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
            .edit_db_form
            .port
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

        let dsn = Self::build_dsn(
            self.edit_db_form.engine.as_str(),
            &self.edit_db_form.user,
            &password,
            &self.edit_db_form.host,
            port,
            &self.edit_db_form.database,
            self.edit_db_form.ssl_enabled,
            &self.edit_db_form.ssl_mode,
            self.edit_db_form.ssl_cert_file_id.as_ref(),
            self.edit_db_form.ssl_key_file_id.as_ref(),
            self.edit_db_form.ssl_rootcert_file_id.as_ref(),
            self.storage.as_ref(),
        )
        .map_err(|e| format!("Failed to build DSN: {}", e))?;

        let cfg = pgone_storage::models::DbConfig {
            id: self.edit_db_form.name.trim().to_string(),
            engine: self.edit_db_form.engine.as_str().to_string(),
            dsn,
            default_schemas: existing_cfg.default_schemas.clone(),
            include_system: existing_cfg.include_system,
            default_config: existing_cfg.default_config,
            created_at: existing_cfg.created_at,
            updated_at: Self::now_ts(),
        };

        let res = futures::block_on_async(async { storage.upsert_db_config(&cfg).await });
        match res {
            Ok(_) => {
                // Update active_db_config_id if it was the edited one
                if self.active_db_config_id.as_ref() == Some(id) {
                    self.active_db_config_id = Some(cfg.id.clone());
                }
                Ok(())
            }
            Err(e) => Err(e.to_string()),
        }
    }

    /// Open file picker dialog and upload selected file
    pub fn select_and_upload_file(&mut self, target: FilePickerTarget) {
        self.ensure_storage();

        // Open file dialog
        if let Some(path) = rfd::FileDialog::new()
            .add_filter(
                "Certificate Files",
                &["crt", "cer", "pem", "key", "p12", "pfx"],
            )
            .add_filter("All Files", &["*"])
            .pick_file()
        {
            // Upload file to storage
            if let Some(storage) = &self.storage {
                let file_path = path.to_string_lossy().to_string();
                match futures::block_on_async(async {
                    storage.copy_file_to_index(&file_path).await
                }) {
                    Ok(file_index) => {
                        // Set the file ID based on target
                        match target {
                            FilePickerTarget::AddSslCert => {
                                self.add_db_form.ssl_cert_file_id = Some(file_index.id.clone());
                            }
                            FilePickerTarget::AddSslKey => {
                                self.add_db_form.ssl_key_file_id = Some(file_index.id.clone());
                            }
                            FilePickerTarget::AddSslRootcert => {
                                self.add_db_form.ssl_rootcert_file_id = Some(file_index.id.clone());
                            }
                            FilePickerTarget::EditSslCert => {
                                self.edit_db_form.ssl_cert_file_id = Some(file_index.id.clone());
                            }
                            FilePickerTarget::EditSslKey => {
                                self.edit_db_form.ssl_key_file_id = Some(file_index.id.clone());
                            }
                            FilePickerTarget::EditSslRootcert => {
                                self.edit_db_form.ssl_rootcert_file_id =
                                    Some(file_index.id.clone());
                            }
                        }
                        notify::info(format!("文件已上传: {}", file_index.original_path));
                    }
                    Err(e) => {
                        notify::error(format!("上传文件失败: {}", e));
                    }
                }
            }
        }
    }

    /// Get file name by ID
    fn get_file_name(&self, file_id: &str) -> Option<String> {
        if let Some(storage) = &self.storage {
            if let Ok(Some(file)) =
                futures::block_on_async(async { storage.get_file(file_id).await })
            {
                return Some(
                    std::path::Path::new(&file.original_path)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(&file.original_path)
                        .to_string(),
                );
            }
        }
        None
    }

    /// Find file ID by path (supports current data-dir paths, ./data/xxx, and xxx formats)
    fn find_file_id_by_path(storage: &StorageBlocking, path: &str) -> Option<String> {
        let normalized_path = std::path::Path::new(path)
            .strip_prefix(pgone_storage::data_dir())
            .ok()
            .and_then(|path| path.to_str())
            .or_else(|| path.strip_prefix("./data/"))
            .unwrap_or(path);

        // List all files and search by path
        if let Ok(files) = futures::block_on_async(async { storage.list_files().await }) {
            // Try to find by current_path
            if let Some(file) = files.iter().find(|f| f.current_path == normalized_path) {
                return Some(file.id.clone());
            }

            // Try to find by original_path
            if let Some(file) = files
                .iter()
                .find(|f| f.original_path == path || f.original_path == normalized_path)
            {
                return Some(file.id.clone());
            }
        }

        None
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
                    if let Ok(Some(cfg)) =
                        futures::block_on_async(async { storage.get_db_config(id).await })
                    {
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

        let dsn = match Self::build_dsn(
            self.edit_db_form.engine.as_str(),
            &self.edit_db_form.user,
            &password,
            &self.edit_db_form.host,
            port,
            &self.edit_db_form.database,
            self.edit_db_form.ssl_enabled,
            &self.edit_db_form.ssl_mode,
            self.edit_db_form.ssl_cert_file_id.as_ref(),
            self.edit_db_form.ssl_key_file_id.as_ref(),
            self.edit_db_form.ssl_rootcert_file_id.as_ref(),
            self.storage.as_ref(),
        ) {
            Ok(dsn) => dsn,
            Err(e) => {
                self.edit_db_form.test_status = Some(false);
                self.edit_db_form.error = Some(e.clone());
                notify::db_connection_error(&self.edit_db_form.name.trim().to_string(), &e);
                return;
            }
        };

        let db_name = if self.edit_db_form.name.trim().is_empty() {
            format!(
                "{}@{}:{}",
                self.edit_db_form.user.trim(),
                self.edit_db_form.host.trim(),
                port
            )
        } else {
            self.edit_db_form.name.trim().to_string()
        };

        self.edit_db_form.error = None;
        let result = futures::block_on_async(async {
            PgPoolOptions::new().max_connections(1).connect(&dsn).await
        });

        match result {
            Ok(pool) => {
                let query_result =
                    futures::block_on_async(async { sqlx::query("SELECT 1").execute(&pool).await });
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

    /// Set a database config as default
    /// This will set the specified config's default_config to true
    /// and set all other configs' default_config to false
    pub fn set_default_db_config(&mut self, id: &str) -> Result<(), String> {
        self.ensure_storage();
        let Some(storage) = self.storage.as_ref() else {
            return Err("storage not ready".into());
        };

        // Get all configs
        let all_configs = futures::block_on_async(async { storage.list_db_configs(None).await })
            .map_err(|e| format!("Failed to list configs: {}", e))?;

        let now = Self::now_ts();

        // First, set all configs' default_config to false
        for cfg in &all_configs {
            let mut updated_cfg = cfg.clone();
            updated_cfg.default_config = Some(false);
            updated_cfg.updated_at = now;
            let _ = futures::block_on_async(async { storage.upsert_db_config(&updated_cfg).await });
        }

        // Then, set the specified config's default_config to true
        if let Some(mut target_cfg) = all_configs.iter().find(|c| c.id == id).cloned() {
            target_cfg.default_config = Some(true);
            target_cfg.updated_at = now;
            let res =
                futures::block_on_async(async { storage.upsert_db_config(&target_cfg).await });
            match res {
                Ok(_) => Ok(()),
                Err(e) => Err(format!("Failed to set default config: {}", e)),
            }
        } else {
            Err(format!("Database config '{}' not found", id))
        }
    }
}
