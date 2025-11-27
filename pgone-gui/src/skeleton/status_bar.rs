use crate::components::DbManager;
use crate::futures;
use eframe::egui::{Context, TopBottomPanel};

pub fn show_status_bar(ctx: &Context, db: &mut DbManager) {
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
        });
    });
}

