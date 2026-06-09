use strum::{Display, EnumString};

pub mod activity;
pub mod bgwriter;
pub mod indexes;
pub mod locks;
pub mod replication;
pub mod statements;
pub mod tables;
pub mod window;

/// Monitor metric type enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Display, EnumString)]
pub enum MonitorPanel {
    #[strum(serialize = "Overview")]
    Overview,
    #[strum(serialize = "Activity")]
    Activity,
    #[strum(serialize = "Queries")]
    Queries,
    #[strum(serialize = "Storage")]
    Storage,
    #[strum(serialize = "Locks")]
    Locks,
    #[strum(serialize = "Replication")]
    Replication,
    #[strum(serialize = "WAL & I/O")]
    WalIo,
    #[strum(serialize = "Maintenance")]
    Maintenance,
}

impl MonitorPanel {
    pub const ALL: [Self; 8] = [
        Self::Overview,
        Self::Activity,
        Self::Queries,
        Self::Storage,
        Self::Locks,
        Self::Replication,
        Self::WalIo,
        Self::Maintenance,
    ];

    /// Get display name for the monitor metric
    pub fn title(&self) -> &'static str {
        match self {
            MonitorPanel::Overview => "Overview",
            MonitorPanel::Activity => "Activity",
            MonitorPanel::Queries => "Queries",
            MonitorPanel::Storage => "Storage",
            MonitorPanel::Locks => "Locks",
            MonitorPanel::Replication => "Replication",
            MonitorPanel::WalIo => "WAL & I/O",
            MonitorPanel::Maintenance => "Maintenance",
        }
    }

    pub fn subtitle(&self) -> &'static str {
        match self {
            MonitorPanel::Overview => "database health summary",
            MonitorPanel::Activity => "sessions and wait events",
            MonitorPanel::Queries => "pg_stat_statements",
            MonitorPanel::Storage => "tables and indexes",
            MonitorPanel::Locks => "lock waits and blockers",
            MonitorPanel::Replication => "streaming replication",
            MonitorPanel::WalIo => "WAL, I/O, bgwriter",
            MonitorPanel::Maintenance => "vacuum and index progress",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            MonitorPanel::Overview => egui_phosphor::regular::GAUGE,
            MonitorPanel::Activity => egui_phosphor::regular::ACTIVITY,
            MonitorPanel::Queries => egui_phosphor::regular::FILE_SQL,
            MonitorPanel::Storage => egui_phosphor::regular::DATABASE,
            MonitorPanel::Locks => egui_phosphor::regular::LOCK,
            MonitorPanel::Replication => egui_phosphor::regular::ARROWS_CLOCKWISE,
            MonitorPanel::WalIo => egui_phosphor::regular::HARD_DRIVES,
            MonitorPanel::Maintenance => egui_phosphor::regular::WRENCH,
        }
    }
}

pub use window::MonitorWorkbench;
