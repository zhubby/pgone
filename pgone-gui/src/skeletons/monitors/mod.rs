use strum::{Display, EnumString};

pub mod activity;
pub mod statements;
pub mod tables;
pub mod indexes;
pub mod bgwriter;
pub mod replication;
pub mod locks;
pub mod window;

/// 监控指标类型枚举
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
    /// 获取监控指标的显示名称
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

