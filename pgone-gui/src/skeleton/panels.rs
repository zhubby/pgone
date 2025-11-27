use crate::components::{ChatCtx, ChatPanel, DbManager, DbTree, PreviewManager, ResultsTable, SqlCtx};
use crate::models::PersistedState;
use crate::storage::SessionStorage;
use eframe::egui::{CentralPanel, Context, Frame, SidePanel};

pub fn show_left_panel(
    ctx: &Context,
    visible: bool,
    width: f32,
    db_tree: &mut DbTree,
    db: &mut DbManager,
    results_table: &mut ResultsTable,
) {
    SidePanel::left("left_panel")
        .resizable(true)
        .default_width(width)
        .min_width(100.0)
        .max_width(500.0)
        .show_animated(ctx, visible, |ui| {
            db_tree.ui(ui, db, results_table);
        });
}

pub fn show_right_panel(
    ctx: &Context,
    visible: bool,
    width: f32,
    chat: &mut ChatPanel,
    state: &mut PersistedState,
    preview: &mut PreviewManager,
    storage: &mut SessionStorage,
) {
    SidePanel::right("right_panel")
        .resizable(true)
        .default_width(width)
        .min_width(100.0)
        .max_width(500.0)
        .show_animated(ctx, visible, |ui| {
            let settings = state.settings.clone();
            let mut chat_ctx = ChatCtx {
                state,
                preview,
                send_shortcut: settings.send_shortcut,
                openai_api_key: settings.openai_api_key.clone(),
                openai_model: settings.openai_model.clone(),
                storage,
            };
            chat.ui(&mut chat_ctx, ui);
        });
}

pub fn show_center_panel(
    ctx: &Context,
    db: &mut DbManager,
    results_table: &mut ResultsTable,
    state: &PersistedState,
) {
    CentralPanel::default()
        .frame(Frame::central_panel(&ctx.style()).inner_margin(0.))
        .show(ctx, |ui| {
            // Ensure storage is initialized before creating SqlCtx
            db.ensure_storage();
            let mut sql_ctx = SqlCtx {
                state: state.clone(),
                db: crate::components::DbManager {
                    active_db_config_id: db.active_db_config_id.clone(),
                    pools: db.pools.clone(),
                    storage: None, // Will be initialized in run_sql via ensure_storage()
                    ..Default::default()
                },
            };
            // Initialize storage in SqlCtx by ensuring it's available
            if db.storage.is_some() {
                sql_ctx.db.ensure_storage();
            }
            results_table.ui(ui, Some(&mut sql_ctx));
            // Update pools back if they were modified
            db.pools = sql_ctx.db.pools;
        });
}

