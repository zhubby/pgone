pub mod dock;
pub mod formatters;
pub mod menu_bar;
pub mod monitors;
pub mod panels;
pub mod status_bar;
pub mod windows;

#[derive(Debug, Clone, PartialEq, Eq, Hash, strum::Display, strum::EnumString)]
pub enum Tab {
    Chat,
    SqlEditor,
    Results,
    Sessions,
    DbConfig,
}
