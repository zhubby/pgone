#[derive(Debug, Clone, PartialEq, Eq, Hash, strum::Display, strum::EnumString)]
pub enum Tab {
    Chat,
    SqlEditor,
    Results,
    Sessions,
    DbConfig,
}
