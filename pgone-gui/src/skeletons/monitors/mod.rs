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
    #[strum(serialize = "连接状态")]
    Activity,
    #[strum(serialize = "查询性能")]
    Statements,
    #[strum(serialize = "表读写统计")]
    Tables,
    #[strum(serialize = "索引使用情况")]
    Indexes,
    #[strum(serialize = "资源使用率")]
    Bgwriter,
    #[strum(serialize = "复制状态")]
    Replication,
    #[strum(serialize = "锁状态")]
    Locks,
}

impl MonitorMetric {
    /// 获取监控指标的显示名称
    pub fn title(&self) -> &'static str {
        match self {
            MonitorMetric::Activity => "连接状态 (pg_stat_activity)",
            MonitorMetric::Statements => "查询性能 (pg_stat_statements)",
            MonitorMetric::Tables => "表读写统计 (pg_stat_user_tables)",
            MonitorMetric::Indexes => "索引使用情况 (pg_stat_user_indexes)",
            MonitorMetric::Bgwriter => "资源使用率 (pg_stat_bgwriter)",
            MonitorMetric::Replication => "复制状态 (pg_stat_replication)",
            MonitorMetric::Locks => "锁状态 (pg_locks)",
        }
    }
}

