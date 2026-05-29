use egui_notify::{Anchor, Toasts};
use std::sync::{Mutex, OnceLock};

/// Global notification manager
static TOASTS: OnceLock<Mutex<Toasts>> = OnceLock::new();

/// Initialize notification system
fn get_toasts() -> &'static Mutex<Toasts> {
    TOASTS.get_or_init(|| Mutex::new(Toasts::default().with_anchor(Anchor::BottomRight)))
}

/// Show notifications (must be called every frame)
pub fn show(ctx: &egui::Context) {
    if let Ok(mut toasts) = get_toasts().lock() {
        toasts.show(ctx);
    }
}

/// Truncate message to maximum length (100 characters)
fn truncate_text(text: &str) -> String {
    const MAX_LENGTH: usize = 100;
    if text.len() <= MAX_LENGTH {
        text.to_string()
    } else {
        format!("{}...", &text[..MAX_LENGTH])
    }
}

/// Extract text content from WidgetText and truncate if needed
fn truncate_message(message: impl Into<egui::WidgetText>) -> egui::WidgetText {
    let widget_text = message.into();

    // Convert WidgetText to string using Debug format, then clean it up
    let wstr = format!("{}", widget_text.text());

    // Truncate if needed
    let truncated = truncate_text(&wstr);

    truncated.into()
}

/// Show success notification (green)
#[allow(dead_code)]
pub fn success(message: impl Into<egui::WidgetText>) {
    if let Ok(mut toasts) = get_toasts().lock() {
        toasts.success(truncate_message(message));
    }
}

/// Show error notification (red)
#[allow(dead_code)]
pub fn error(message: impl Into<egui::WidgetText>) {
    if let Ok(mut toasts) = get_toasts().lock() {
        toasts.error(truncate_message(message));
    }
}

/// Show warning notification (yellow)
#[allow(dead_code)]
pub fn warning(message: impl Into<egui::WidgetText>) {
    if let Ok(mut toasts) = get_toasts().lock() {
        toasts.warning(truncate_message(message));
    }
}

/// Show info notification (blue)
#[allow(dead_code)]
pub fn info(message: impl Into<egui::WidgetText>) {
    if let Ok(mut toasts) = get_toasts().lock() {
        toasts.info(truncate_message(message));
    }
}

/// Database connection success notification
#[allow(dead_code)]
pub fn db_connection_success(db_name: &str) {
    success(format!("Database connection successful: {}", db_name));
}

/// Database connection error notification
#[allow(dead_code)]
pub fn db_connection_error(db_name: &str, err: &str) {
    error(format!("Database connection failed: {} - {}", db_name, err));
}

/// Database save success notification
#[allow(dead_code)]
pub fn db_save_success(db_name: &str) {
    success(format!("Database configuration saved: {}", db_name));
}

/// Database save error notification
#[allow(dead_code)]
pub fn db_save_error(err: &str) {
    error(format!("Failed to save database configuration: {}", err));
}

/// Database delete success notification
#[allow(dead_code)]
pub fn db_delete_success(db_name: &str) {
    info(format!("Database configuration deleted: {}", db_name));
}

/// SQL execution success notification
#[allow(dead_code)]
pub fn sql_execute_success(rows: u64) {
    success(format!("SQL executed successfully, affected {} rows", rows));
}

/// SQL execution error notification
#[allow(dead_code)]
pub fn sql_execute_error(err: &str) {
    error(format!("SQL execution failed: {}", err));
}

/// File save success notification
#[allow(dead_code)]
pub fn file_save_success(file_path: &str) {
    success(format!("File saved: {}", file_path));
}

/// File save error notification
#[allow(dead_code)]
pub fn file_save_error(file_path: &str, err: &str) {
    error(format!("Failed to save file: {} - {}", file_path, err));
}

/// File load success notification
#[allow(dead_code)]
pub fn file_load_success(file_path: &str) {
    info(format!("File loaded: {}", file_path));
}

/// File load error notification
#[allow(dead_code)]
pub fn file_load_error(file_path: &str, err: &str) {
    error(format!("Failed to load file: {} - {}", file_path, err));
}

/// Operation cancelled notification
#[allow(dead_code)]
pub fn operation_cancelled() {
    info("Operation cancelled".to_string());
}

/// Operation completed notification
#[allow(dead_code)]
pub fn operation_completed(message: &str) {
    success(format!("Operation completed: {}", message));
}

/// Network request success notification
#[allow(dead_code)]
pub fn network_success(message: &str) {
    success(format!("Network request successful: {}", message));
}

/// Network request error notification
#[allow(dead_code)]
pub fn network_error(message: &str) {
    error(format!("Network request failed: {}", message));
}

/// Copy success notification
#[allow(dead_code)]
pub fn copy_success(content: &str) {
    info(format!("Copied to clipboard: {}", content));
}

/// Copy error notification
#[allow(dead_code)]
pub fn copy_error(err: &str) {
    error(format!("Failed to copy: {}", err));
}
