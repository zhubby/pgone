use crate::components::ChatCtx;
use crate::models::{Message, MessageContent, Role};
use chrono::Utc;
use std::path::PathBuf;

pub struct InputArea {
    pub input: String,
    pub pending_resources: Vec<PathBuf>,
}

impl Default for InputArea {
    fn default() -> Self {
        Self {
            input: String::new(),
            pending_resources: Vec::new(),
        }
    }
}

impl InputArea {
    pub fn ui(&mut self, ctxs: &mut ChatCtx, ui: &mut egui::Ui, should_send: &mut bool) {
        ui.separator();

        ui.horizontal(|ui| {
            ui.label(format!(
                "{} Message",
                egui_phosphor::regular::PAPER_PLANE_TILT
            ));
        });

        ui.separator();

        // File selection button
        ui.horizontal(|ui| {
            if ui
                .button(format!("{} Select Image", egui_phosphor::regular::IMAGE))
                .clicked()
            {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Image", &["png", "jpg", "jpeg", "gif", "webp", "bmp"])
                    .pick_file()
                {
                    self.pending_resources.push(path);
                }
            }

            if ui
                .button(format!("{} Select File", egui_phosphor::regular::FILE))
                .clicked()
            {
                if let Some(path) = rfd::FileDialog::new().pick_file() {
                    self.pending_resources.push(path);
                }
            }
        });

        // Resource list display
        if !self.pending_resources.is_empty() {
            ui.separator();
            ui.group(|ui| {
                ui.label("Resources to send:");
                let mut to_remove = Vec::new();
                for (idx, path) in self.pending_resources.iter().enumerate() {
                    ui.horizontal(|ui| {
                        let file_name = path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("Unknown file");

                        // If it is an image, try to show a thumbnail
                        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                            let is_image = matches!(
                                ext.to_lowercase().as_str(),
                                "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp"
                            );

                            if is_image {
                                if let Some(handle) = ctxs.preview.ensure_texture(ui.ctx(), path) {
                                    let thumb_size = egui::vec2(40.0, 40.0);
                                    let img = egui::widgets::Image::new(&handle)
                                        .fit_to_exact_size(thumb_size);
                                    ui.add(img);
                                }
                            } else {
                                ui.label(format!("{}", egui_phosphor::regular::FILE));
                            }
                        }

                        ui.label(file_name);

                        if ui
                            .small_button(format!("{}", egui_phosphor::regular::X))
                            .clicked()
                        {
                            to_remove.push(idx);
                        }
                    });
                }
                // Delete from back to front to avoid index issues
                for &idx in to_remove.iter().rev() {
                    self.pending_resources.remove(idx);
                }
            });
            ui.separator();
        }

        // Input field
        let editor = ui.add_sized(
            egui::Vec2::new(ui.available_width(), ui.available_height()),
            egui::TextEdit::multiline(&mut self.input).desired_rows(4),
        );

        if editor.has_focus() {
            let input = ui.input(|i| i.clone());
            let enter_pressed = input.key_pressed(egui::Key::Enter);
            let shift_pressed = input.modifiers.shift;

            // Enter sends, Shift+Enter inserts a newline
            if enter_pressed && !shift_pressed {
                // Prevent the default newline behavior
                ui.ctx().input_mut(|i| {
                    i.consume_key(egui::Modifiers::NONE, egui::Key::Enter);
                });
                // Remove any newline that may have been inserted
                if self.input.ends_with('\n') {
                    self.input.pop();
                }
                *should_send = true;
            }
            // Allow default newline behavior on Shift+Enter (let TextEdit handle the newline normally)
        }
    }

    pub fn send_resources(&mut self, ctxs: &mut ChatCtx) {
        // First collect all resource paths
        let resources: Vec<PathBuf> = self.pending_resources.drain(..).collect();

        // Send each resource
        for path in resources {
            // Check whether it is an image
            let is_image = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|ext| {
                    matches!(
                        ext.to_lowercase().as_str(),
                        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp"
                    )
                })
                .unwrap_or(false);

            if is_image {
                self.add_image_message(ctxs, path);
            } else {
                // Other files are sent as Markdown messages, including the file path
                if let Some(session) = ctxs.state.sessions.get_mut(ctxs.state.current_index) {
                    let message = Message {
                        role: Role::User,
                        timestamp: Utc::now(),
                        content: MessageContent::Markdown(format!("[File] {}", path.display())),
                    };
                    session.messages.push(message);
                    session.updated_at = Utc::now();

                    if let Err(e) = ctxs.storage.save_session(session) {
                        tracing::error!("Failed to save file message: {}", e);
                    }
                }
            }
        }
    }

    pub(crate) fn add_image_message(&mut self, ctxs: &mut ChatCtx, path: PathBuf) {
        let (w, h) = match image::open(&path) {
            Ok(img) => (img.width(), img.height()),
            Err(_) => (0, 0),
        };
        if let Some(session) = ctxs.state.sessions.get_mut(ctxs.state.current_index) {
            let message = Message {
                role: Role::User,
                timestamp: Utc::now(),
                content: MessageContent::Image {
                    path,
                    width: w,
                    height: h,
                },
            };
            session.messages.push(message);
            session.updated_at = Utc::now();

            if let Err(e) = ctxs.storage.save_session(session) {
                tracing::error!("Failed to save image message: {}", e);
            }
        }
    }
}
