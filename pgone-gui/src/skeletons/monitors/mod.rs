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
pub enum MonitorMetric {
    #[strum(serialize = "Activity")]
    Activity,
    #[strum(serialize = "Statements")]
    Statements,
    #[strum(serialize = "Tables")]
    Tables,
    #[strum(serialize = "Indexes")]
    Indexes,
    #[strum(serialize = "Bgwriter")]
    Bgwriter,
    #[strum(serialize = "Replication")]
    Replication,
    #[strum(serialize = "Locks")]
    Locks,
}

impl MonitorMetric {
    /// Get display name for the monitor metric
    pub fn title(&self) -> &'static str {
        match self {
            MonitorMetric::Activity => "Activity (pg_stat_activity)",
            MonitorMetric::Statements => "Statements (pg_stat_statements)",
            MonitorMetric::Tables => "Tables (pg_stat_user_tables)",
            MonitorMetric::Indexes => "Indexes (pg_stat_user_indexes)",
            MonitorMetric::Bgwriter => "Bgwriter (pg_stat_bgwriter)",
            MonitorMetric::Replication => "Replication (pg_stat_replication)",
            MonitorMetric::Locks => "Locks (pg_locks)",
        }
    }
}
