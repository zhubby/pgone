use crate::error::{Result, SqlError};
use crate::session::Session;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use sqlx::postgres::PgRow;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MonitorCapabilities {
    pub pg_stat_statements: bool,
    pub pg_stat_io: bool,
    pub pg_stat_wal: bool,
    pub pg_stat_checkpointer: bool,
    pub progress_vacuum: bool,
    pub progress_create_index: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OptionalMonitorData<T> {
    pub available: bool,
    pub unavailable_reason: Option<String>,
    pub data: T,
}

impl<T> OptionalMonitorData<T> {
    #[must_use]
    pub fn available(data: T) -> Self {
        Self {
            available: true,
            unavailable_reason: None,
            data,
        }
    }
}

impl<T: Default> OptionalMonitorData<T> {
    #[must_use]
    pub fn unavailable(reason: impl Into<String>) -> Self {
        Self {
            available: false,
            unavailable_reason: Some(reason.into()),
            data: T::default(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MonitorSnapshot {
    pub database_name: String,
    pub server_version: String,
    pub max_connections: i64,
    pub total_connections: i64,
    pub active_connections: i64,
    pub idle_connections: i64,
    pub waiting_connections: i64,
    pub xact_commit: i64,
    pub xact_rollback: i64,
    pub cache_hit_ratio: Option<f64>,
    pub blks_read: i64,
    pub blks_hit: i64,
    pub deadlocks: i64,
    pub temp_bytes: i64,
    pub lock_count: i64,
    pub waiting_lock_count: i64,
    pub replication_clients: i64,
    pub maintenance_jobs: i64,
    pub wal_bytes: Option<String>,
    pub io_reads: Option<i64>,
    pub io_writes: Option<i64>,
    pub stats_reset: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ActivityRow {
    pub pid: i32,
    pub user_name: Option<String>,
    pub application_name: Option<String>,
    pub client_addr: Option<String>,
    pub backend_type: Option<String>,
    pub state: Option<String>,
    pub wait_event_type: Option<String>,
    pub wait_event: Option<String>,
    pub query_start: Option<String>,
    pub state_change: Option<String>,
    pub query_age_seconds: Option<i64>,
    pub query: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StatementRow {
    pub query: String,
    pub calls: i64,
    pub rows: i64,
    pub total_exec_time_ms: f64,
    pub mean_exec_time_ms: f64,
    pub shared_blks_hit: i64,
    pub shared_blks_read: i64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum StatementSort {
    #[default]
    TotalExecTime,
    MeanExecTime,
    Calls,
    Rows,
    SharedReads,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct StatementOptions {
    pub limit: i64,
    pub sort: StatementSort,
}

impl Default for StatementOptions {
    fn default() -> Self {
        Self {
            limit: 25,
            sort: StatementSort::TotalExecTime,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TableHealthRow {
    pub schema_name: String,
    pub table_name: String,
    pub size_bytes: i64,
    pub size_pretty: String,
    pub live_tuples: i64,
    pub dead_tuples: i64,
    pub dead_tuple_ratio: f64,
    pub seq_scan: i64,
    pub seq_tup_read: i64,
    pub idx_scan: i64,
    pub idx_tup_fetch: i64,
    pub inserts: i64,
    pub updates: i64,
    pub deletes: i64,
    pub last_vacuum: Option<String>,
    pub last_autovacuum: Option<String>,
    pub last_analyze: Option<String>,
    pub last_autoanalyze: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TableHealthSort {
    #[default]
    Size,
    DeadTuples,
    DeadTupleRatio,
    SeqScan,
    Writes,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IndexHealthRow {
    pub schema_name: String,
    pub table_name: String,
    pub index_name: String,
    pub size_bytes: i64,
    pub size_pretty: String,
    pub idx_scan: i64,
    pub idx_tup_read: i64,
    pub idx_tup_fetch: i64,
    pub unique: bool,
    pub valid: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum IndexHealthSort {
    #[default]
    LeastScanned,
    MostScanned,
    Size,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LockRow {
    pub lock_type: String,
    pub mode: String,
    pub granted: bool,
    pub pid: Option<i32>,
    pub blocking_pids: Vec<i32>,
    pub user_name: Option<String>,
    pub state: Option<String>,
    pub wait_event_type: Option<String>,
    pub wait_event: Option<String>,
    pub relation: Option<String>,
    pub transaction_id: Option<String>,
    pub query_age_seconds: Option<i64>,
    pub query: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReplicationRow {
    pub pid: Option<i32>,
    pub user_name: Option<String>,
    pub application_name: Option<String>,
    pub client_addr: Option<String>,
    pub state: Option<String>,
    pub sync_state: Option<String>,
    pub sent_lsn: Option<String>,
    pub write_lsn: Option<String>,
    pub flush_lsn: Option<String>,
    pub replay_lsn: Option<String>,
    pub replay_lag_bytes: Option<i64>,
    pub backend_start: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BackgroundWriterStats {
    pub checkpoints_timed: i64,
    pub checkpoints_req: i64,
    pub checkpoint_write_time_ms: f64,
    pub checkpoint_sync_time_ms: f64,
    pub buffers_checkpoint: i64,
    pub buffers_clean: i64,
    pub maxwritten_clean: i64,
    pub buffers_backend: i64,
    pub buffers_backend_fsync: i64,
    pub buffers_alloc: i64,
    pub stats_reset: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WalStats {
    pub wal_records: i64,
    pub wal_fpi: i64,
    pub wal_bytes: String,
    pub wal_buffers_full: i64,
    pub wal_write: i64,
    pub wal_sync: i64,
    pub wal_write_time_ms: f64,
    pub wal_sync_time_ms: f64,
    pub stats_reset: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IoStats {
    pub reads: i64,
    pub writes: i64,
    pub writebacks: i64,
    pub extends: i64,
    pub hits: i64,
    pub evictions: i64,
    pub fsyncs: i64,
    pub read_time_ms: f64,
    pub write_time_ms: f64,
    pub fsync_time_ms: f64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MaintenanceProgressRow {
    pub operation: String,
    pub pid: i32,
    pub database_name: Option<String>,
    pub relation: Option<String>,
    pub index_relation: Option<String>,
    pub phase: Option<String>,
    pub progress_done: i64,
    pub progress_total: i64,
}

impl Session {
    pub async fn monitor_capabilities(&self) -> Result<MonitorCapabilities> {
        let row = sqlx::query(
            r#"
            SELECT
                EXISTS (
                    SELECT 1
                    FROM pg_catalog.pg_extension
                    WHERE extname = 'pg_stat_statements'
                ) AS pg_stat_statements,
                pg_catalog.to_regclass('pg_catalog.pg_stat_io') IS NOT NULL AS pg_stat_io,
                pg_catalog.to_regclass('pg_catalog.pg_stat_wal') IS NOT NULL AS pg_stat_wal,
                pg_catalog.to_regclass('pg_catalog.pg_stat_checkpointer') IS NOT NULL AS pg_stat_checkpointer,
                pg_catalog.to_regclass('pg_catalog.pg_stat_progress_vacuum') IS NOT NULL AS progress_vacuum,
                pg_catalog.to_regclass('pg_catalog.pg_stat_progress_create_index') IS NOT NULL AS progress_create_index
            "#,
        )
        .fetch_one(self.pool())
        .await
        .map_err(monitor_error)?;

        Ok(MonitorCapabilities {
            pg_stat_statements: row.get("pg_stat_statements"),
            pg_stat_io: row.get("pg_stat_io"),
            pg_stat_wal: row.get("pg_stat_wal"),
            pg_stat_checkpointer: row.get("pg_stat_checkpointer"),
            progress_vacuum: row.get("progress_vacuum"),
            progress_create_index: row.get("progress_create_index"),
        })
    }

    pub async fn fetch_monitor_snapshot(&self) -> Result<MonitorSnapshot> {
        let capabilities = self.monitor_capabilities().await?;

        let activity = sqlx::query(
            r#"
            SELECT
                pg_catalog.current_database() AS database_name,
                pg_catalog.current_setting('server_version') AS server_version,
                (SELECT setting::bigint FROM pg_catalog.pg_settings WHERE name = 'max_connections') AS max_connections,
                COUNT(*) AS total_connections,
                COUNT(*) FILTER (WHERE state = 'active') AS active_connections,
                COUNT(*) FILTER (WHERE state = 'idle') AS idle_connections,
                COUNT(*) FILTER (WHERE wait_event_type IS NOT NULL) AS waiting_connections
            FROM pg_catalog.pg_stat_activity
            WHERE datname = pg_catalog.current_database()
            "#,
        )
        .fetch_one(self.pool())
        .await
        .map_err(monitor_error)?;

        let database = sqlx::query(
            r#"
            SELECT
                xact_commit,
                xact_rollback,
                blks_read,
                blks_hit,
                deadlocks,
                temp_bytes,
                CASE WHEN stats_reset IS NULL THEN NULL ELSE to_char(stats_reset, 'YYYY-MM-DD HH24:MI:SS') END AS stats_reset
            FROM pg_catalog.pg_stat_database
            WHERE datname = pg_catalog.current_database()
            "#,
        )
        .fetch_one(self.pool())
        .await
        .map_err(monitor_error)?;

        let locks = sqlx::query(
            r#"
            SELECT
                COUNT(*) AS lock_count,
                COUNT(*) FILTER (WHERE NOT granted) AS waiting_lock_count
            FROM pg_catalog.pg_locks
            "#,
        )
        .fetch_one(self.pool())
        .await
        .map_err(monitor_error)?;

        let replication_clients: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM pg_catalog.pg_stat_replication")
                .fetch_one(self.pool())
                .await
                .map_err(monitor_error)?;

        let maintenance_jobs = fetch_maintenance_count(self, &capabilities).await?;
        let wal_bytes = if capabilities.pg_stat_wal {
            Some(fetch_wal_bytes(self).await?)
        } else {
            None
        };
        let (io_reads, io_writes) = if capabilities.pg_stat_io {
            fetch_io_totals(self).await?
        } else {
            (None, None)
        };

        let blks_read = i64_value(&database, "blks_read");
        let blks_hit = i64_value(&database, "blks_hit");
        let cache_hit_ratio = cache_hit_ratio(blks_read, blks_hit);

        Ok(MonitorSnapshot {
            database_name: activity.get("database_name"),
            server_version: activity.get("server_version"),
            max_connections: i64_value(&activity, "max_connections"),
            total_connections: i64_value(&activity, "total_connections"),
            active_connections: i64_value(&activity, "active_connections"),
            idle_connections: i64_value(&activity, "idle_connections"),
            waiting_connections: i64_value(&activity, "waiting_connections"),
            xact_commit: i64_value(&database, "xact_commit"),
            xact_rollback: i64_value(&database, "xact_rollback"),
            cache_hit_ratio,
            blks_read,
            blks_hit,
            deadlocks: i64_value(&database, "deadlocks"),
            temp_bytes: i64_value(&database, "temp_bytes"),
            lock_count: i64_value(&locks, "lock_count"),
            waiting_lock_count: i64_value(&locks, "waiting_lock_count"),
            replication_clients,
            maintenance_jobs,
            wal_bytes,
            io_reads,
            io_writes,
            stats_reset: database.get("stats_reset"),
        })
    }

    pub async fn fetch_activity(&self, limit: i64) -> Result<Vec<ActivityRow>> {
        let rows = sqlx::query(
            r#"
            SELECT
                pid,
                usename AS user_name,
                NULLIF(application_name, '') AS application_name,
                client_addr::text AS client_addr,
                backend_type,
                state,
                wait_event_type,
                wait_event,
                CASE WHEN query_start IS NULL THEN NULL ELSE to_char(query_start, 'YYYY-MM-DD HH24:MI:SS') END AS query_start,
                CASE WHEN state_change IS NULL THEN NULL ELSE to_char(state_change, 'YYYY-MM-DD HH24:MI:SS') END AS state_change,
                CASE WHEN query_start IS NULL THEN NULL ELSE EXTRACT(EPOCH FROM clock_timestamp() - query_start)::bigint END AS query_age_seconds,
                LEFT(COALESCE(query, ''), 500) AS query
            FROM pg_catalog.pg_stat_activity
            WHERE datname = pg_catalog.current_database()
            ORDER BY
                CASE WHEN state = 'active' THEN 0 WHEN wait_event_type IS NOT NULL THEN 1 ELSE 2 END,
                query_start NULLS LAST
            LIMIT $1
            "#,
        )
        .bind(limit.max(1))
        .fetch_all(self.pool())
        .await
        .map_err(monitor_error)?;

        Ok(rows
            .into_iter()
            .map(|row| ActivityRow {
                pid: row.get("pid"),
                user_name: row.get("user_name"),
                application_name: row.get("application_name"),
                client_addr: row.get("client_addr"),
                backend_type: row.get("backend_type"),
                state: row.get("state"),
                wait_event_type: row.get("wait_event_type"),
                wait_event: row.get("wait_event"),
                query_start: row.get("query_start"),
                state_change: row.get("state_change"),
                query_age_seconds: row.get("query_age_seconds"),
                query: row.get("query"),
            })
            .collect())
    }

    pub async fn fetch_statement_stats(
        &self,
        options: StatementOptions,
    ) -> Result<OptionalMonitorData<Vec<StatementRow>>> {
        let capabilities = self.monitor_capabilities().await?;
        if !capabilities.pg_stat_statements {
            return Ok(OptionalMonitorData::unavailable(
                "pg_stat_statements extension is not enabled",
            ));
        }

        let order_by = match options.sort {
            StatementSort::TotalExecTime => "total_exec_time DESC",
            StatementSort::MeanExecTime => "mean_exec_time DESC",
            StatementSort::Calls => "calls DESC",
            StatementSort::Rows => "rows DESC",
            StatementSort::SharedReads => "shared_blks_read DESC",
        };
        let sql = format!(
            r#"
            SELECT
                LEFT(query, 700) AS query,
                COALESCE(calls, 0)::bigint AS calls,
                COALESCE(rows, 0)::bigint AS rows,
                COALESCE(total_exec_time, 0)::float8 AS total_exec_time,
                COALESCE(mean_exec_time, 0)::float8 AS mean_exec_time,
                COALESCE(shared_blks_hit, 0)::bigint AS shared_blks_hit,
                COALESCE(shared_blks_read, 0)::bigint AS shared_blks_read
            FROM pg_stat_statements
            ORDER BY {order_by}
            LIMIT $1
            "#
        );

        let rows = sqlx::query(&sql)
            .bind(options.limit.clamp(1, 200))
            .fetch_all(self.pool())
            .await
            .map_err(monitor_error)?;

        let data = rows
            .into_iter()
            .map(|row| StatementRow {
                query: row.get("query"),
                calls: i64_value(&row, "calls"),
                rows: i64_value(&row, "rows"),
                total_exec_time_ms: f64_value(&row, "total_exec_time"),
                mean_exec_time_ms: f64_value(&row, "mean_exec_time"),
                shared_blks_hit: i64_value(&row, "shared_blks_hit"),
                shared_blks_read: i64_value(&row, "shared_blks_read"),
            })
            .collect();

        Ok(OptionalMonitorData::available(data))
    }

    pub async fn fetch_table_health(
        &self,
        limit: i64,
        sort: TableHealthSort,
    ) -> Result<Vec<TableHealthRow>> {
        let order_by = match sort {
            TableHealthSort::Size => "size_bytes DESC",
            TableHealthSort::DeadTuples => "n_dead_tup DESC",
            TableHealthSort::DeadTupleRatio => "dead_tuple_ratio DESC",
            TableHealthSort::SeqScan => "seq_scan DESC",
            TableHealthSort::Writes => {
                "(COALESCE(n_tup_ins, 0) + COALESCE(n_tup_upd, 0) + COALESCE(n_tup_del, 0)) DESC"
            }
        };
        let sql = format!(
            r#"
            SELECT
                schemaname AS schema_name,
                relname AS table_name,
                pg_catalog.pg_total_relation_size(relid)::bigint AS size_bytes,
                pg_catalog.pg_size_pretty(pg_catalog.pg_total_relation_size(relid)) AS size_pretty,
                COALESCE(n_live_tup, 0)::bigint AS n_live_tup,
                COALESCE(n_dead_tup, 0)::bigint AS n_dead_tup,
                CASE
                    WHEN COALESCE(n_live_tup, 0) + COALESCE(n_dead_tup, 0) > 0
                    THEN COALESCE(n_dead_tup, 0)::float8 / (COALESCE(n_live_tup, 0) + COALESCE(n_dead_tup, 0))::float8
                    ELSE 0::float8
                END AS dead_tuple_ratio,
                COALESCE(seq_scan, 0)::bigint AS seq_scan,
                COALESCE(seq_tup_read, 0)::bigint AS seq_tup_read,
                COALESCE(idx_scan, 0)::bigint AS idx_scan,
                COALESCE(idx_tup_fetch, 0)::bigint AS idx_tup_fetch,
                COALESCE(n_tup_ins, 0)::bigint AS n_tup_ins,
                COALESCE(n_tup_upd, 0)::bigint AS n_tup_upd,
                COALESCE(n_tup_del, 0)::bigint AS n_tup_del,
                CASE WHEN last_vacuum IS NULL THEN NULL ELSE to_char(last_vacuum, 'YYYY-MM-DD HH24:MI:SS') END AS last_vacuum,
                CASE WHEN last_autovacuum IS NULL THEN NULL ELSE to_char(last_autovacuum, 'YYYY-MM-DD HH24:MI:SS') END AS last_autovacuum,
                CASE WHEN last_analyze IS NULL THEN NULL ELSE to_char(last_analyze, 'YYYY-MM-DD HH24:MI:SS') END AS last_analyze,
                CASE WHEN last_autoanalyze IS NULL THEN NULL ELSE to_char(last_autoanalyze, 'YYYY-MM-DD HH24:MI:SS') END AS last_autoanalyze
            FROM pg_catalog.pg_stat_user_tables
            ORDER BY {order_by}
            LIMIT $1
            "#
        );

        let rows = sqlx::query(&sql)
            .bind(limit.clamp(1, 200))
            .fetch_all(self.pool())
            .await
            .map_err(monitor_error)?;

        Ok(rows
            .into_iter()
            .map(|row| TableHealthRow {
                schema_name: row.get("schema_name"),
                table_name: row.get("table_name"),
                size_bytes: i64_value(&row, "size_bytes"),
                size_pretty: row.get("size_pretty"),
                live_tuples: i64_value(&row, "n_live_tup"),
                dead_tuples: i64_value(&row, "n_dead_tup"),
                dead_tuple_ratio: f64_value(&row, "dead_tuple_ratio"),
                seq_scan: i64_value(&row, "seq_scan"),
                seq_tup_read: i64_value(&row, "seq_tup_read"),
                idx_scan: i64_value(&row, "idx_scan"),
                idx_tup_fetch: i64_value(&row, "idx_tup_fetch"),
                inserts: i64_value(&row, "n_tup_ins"),
                updates: i64_value(&row, "n_tup_upd"),
                deletes: i64_value(&row, "n_tup_del"),
                last_vacuum: row.get("last_vacuum"),
                last_autovacuum: row.get("last_autovacuum"),
                last_analyze: row.get("last_analyze"),
                last_autoanalyze: row.get("last_autoanalyze"),
            })
            .collect())
    }

    pub async fn fetch_index_health(
        &self,
        limit: i64,
        sort: IndexHealthSort,
    ) -> Result<Vec<IndexHealthRow>> {
        let order_by = match sort {
            IndexHealthSort::LeastScanned => "COALESCE(s.idx_scan, 0) ASC, size_bytes DESC",
            IndexHealthSort::MostScanned => "COALESCE(s.idx_scan, 0) DESC",
            IndexHealthSort::Size => "size_bytes DESC",
        };
        let sql = format!(
            r#"
            SELECT
                s.schemaname AS schema_name,
                s.relname AS table_name,
                s.indexrelname AS index_name,
                pg_catalog.pg_relation_size(s.indexrelid)::bigint AS size_bytes,
                pg_catalog.pg_size_pretty(pg_catalog.pg_relation_size(s.indexrelid)) AS size_pretty,
                COALESCE(s.idx_scan, 0)::bigint AS idx_scan,
                COALESCE(s.idx_tup_read, 0)::bigint AS idx_tup_read,
                COALESCE(s.idx_tup_fetch, 0)::bigint AS idx_tup_fetch,
                i.indisunique AS unique,
                i.indisvalid AS valid
            FROM pg_catalog.pg_stat_user_indexes s
            JOIN pg_catalog.pg_index i ON i.indexrelid = s.indexrelid
            ORDER BY {order_by}
            LIMIT $1
            "#
        );

        let rows = sqlx::query(&sql)
            .bind(limit.clamp(1, 200))
            .fetch_all(self.pool())
            .await
            .map_err(monitor_error)?;

        Ok(rows
            .into_iter()
            .map(|row| IndexHealthRow {
                schema_name: row.get("schema_name"),
                table_name: row.get("table_name"),
                index_name: row.get("index_name"),
                size_bytes: i64_value(&row, "size_bytes"),
                size_pretty: row.get("size_pretty"),
                idx_scan: i64_value(&row, "idx_scan"),
                idx_tup_read: i64_value(&row, "idx_tup_read"),
                idx_tup_fetch: i64_value(&row, "idx_tup_fetch"),
                unique: row.get("unique"),
                valid: row.get("valid"),
            })
            .collect())
    }

    pub async fn fetch_locks(&self, limit: i64) -> Result<Vec<LockRow>> {
        let rows = sqlx::query(
            r#"
            SELECT
                l.locktype AS lock_type,
                l.mode,
                l.granted,
                l.pid,
                CASE WHEN l.pid IS NULL THEN ARRAY[]::integer[] ELSE pg_catalog.pg_blocking_pids(l.pid) END AS blocking_pids,
                a.usename AS user_name,
                a.state,
                a.wait_event_type,
                a.wait_event,
                CASE WHEN l.relation IS NULL THEN NULL ELSE l.relation::regclass::text END AS relation,
                l.transactionid::text AS transaction_id,
                CASE WHEN a.query_start IS NULL THEN NULL ELSE EXTRACT(EPOCH FROM clock_timestamp() - a.query_start)::bigint END AS query_age_seconds,
                LEFT(a.query, 500) AS query
            FROM pg_catalog.pg_locks l
            LEFT JOIN pg_catalog.pg_stat_activity a ON a.pid = l.pid
            ORDER BY
                l.granted ASC,
                query_age_seconds DESC NULLS LAST,
                l.locktype,
                l.mode
            LIMIT $1
            "#,
        )
        .bind(limit.clamp(1, 500))
        .fetch_all(self.pool())
        .await
        .map_err(monitor_error)?;

        Ok(rows
            .into_iter()
            .map(|row| LockRow {
                lock_type: row.get("lock_type"),
                mode: row.get("mode"),
                granted: row.get("granted"),
                pid: row.get("pid"),
                blocking_pids: row.get("blocking_pids"),
                user_name: row.get("user_name"),
                state: row.get("state"),
                wait_event_type: row.get("wait_event_type"),
                wait_event: row.get("wait_event"),
                relation: row.get("relation"),
                transaction_id: row.get("transaction_id"),
                query_age_seconds: row.get("query_age_seconds"),
                query: row.get("query"),
            })
            .collect())
    }

    pub async fn fetch_replication(&self) -> Result<Vec<ReplicationRow>> {
        let rows = sqlx::query(
            r#"
            SELECT
                pid,
                usename AS user_name,
                NULLIF(application_name, '') AS application_name,
                client_addr::text AS client_addr,
                state,
                sync_state,
                sent_lsn::text AS sent_lsn,
                write_lsn::text AS write_lsn,
                flush_lsn::text AS flush_lsn,
                replay_lsn::text AS replay_lsn,
                CASE
                    WHEN sent_lsn IS NULL OR replay_lsn IS NULL THEN NULL
                    ELSE pg_catalog.pg_wal_lsn_diff(sent_lsn, replay_lsn)::bigint
                END AS replay_lag_bytes,
                CASE WHEN backend_start IS NULL THEN NULL ELSE to_char(backend_start, 'YYYY-MM-DD HH24:MI:SS') END AS backend_start
            FROM pg_catalog.pg_stat_replication
            ORDER BY application_name, pid
            "#,
        )
        .fetch_all(self.pool())
        .await
        .map_err(monitor_error)?;

        Ok(rows
            .into_iter()
            .map(|row| ReplicationRow {
                pid: row.get("pid"),
                user_name: row.get("user_name"),
                application_name: row.get("application_name"),
                client_addr: row.get("client_addr"),
                state: row.get("state"),
                sync_state: row.get("sync_state"),
                sent_lsn: row.get("sent_lsn"),
                write_lsn: row.get("write_lsn"),
                flush_lsn: row.get("flush_lsn"),
                replay_lsn: row.get("replay_lsn"),
                replay_lag_bytes: row.get("replay_lag_bytes"),
                backend_start: row.get("backend_start"),
            })
            .collect())
    }

    pub async fn fetch_background_writer(&self) -> Result<BackgroundWriterStats> {
        let row = sqlx::query(
            r#"
            SELECT
                checkpoints_timed,
                checkpoints_req,
                checkpoint_write_time,
                checkpoint_sync_time,
                buffers_checkpoint,
                buffers_clean,
                maxwritten_clean,
                buffers_backend,
                buffers_backend_fsync,
                buffers_alloc,
                CASE WHEN stats_reset IS NULL THEN NULL ELSE to_char(stats_reset, 'YYYY-MM-DD HH24:MI:SS') END AS stats_reset
            FROM pg_catalog.pg_stat_bgwriter
            "#,
        )
        .fetch_one(self.pool())
        .await
        .map_err(monitor_error)?;

        Ok(BackgroundWriterStats {
            checkpoints_timed: i64_value(&row, "checkpoints_timed"),
            checkpoints_req: i64_value(&row, "checkpoints_req"),
            checkpoint_write_time_ms: f64_value(&row, "checkpoint_write_time"),
            checkpoint_sync_time_ms: f64_value(&row, "checkpoint_sync_time"),
            buffers_checkpoint: i64_value(&row, "buffers_checkpoint"),
            buffers_clean: i64_value(&row, "buffers_clean"),
            maxwritten_clean: i64_value(&row, "maxwritten_clean"),
            buffers_backend: i64_value(&row, "buffers_backend"),
            buffers_backend_fsync: i64_value(&row, "buffers_backend_fsync"),
            buffers_alloc: i64_value(&row, "buffers_alloc"),
            stats_reset: row.get("stats_reset"),
        })
    }

    pub async fn fetch_wal_stats(&self) -> Result<OptionalMonitorData<Option<WalStats>>> {
        let capabilities = self.monitor_capabilities().await?;
        if !capabilities.pg_stat_wal {
            return Ok(OptionalMonitorData::unavailable(
                "pg_stat_wal is not available on this PostgreSQL server",
            ));
        }

        let row = sqlx::query(
            r#"
            SELECT
                wal_records,
                wal_fpi,
                wal_bytes::text AS wal_bytes,
                wal_buffers_full,
                wal_write,
                wal_sync,
                wal_write_time,
                wal_sync_time,
                CASE WHEN stats_reset IS NULL THEN NULL ELSE to_char(stats_reset, 'YYYY-MM-DD HH24:MI:SS') END AS stats_reset
            FROM pg_catalog.pg_stat_wal
            "#,
        )
        .fetch_one(self.pool())
        .await
        .map_err(monitor_error)?;

        Ok(OptionalMonitorData::available(Some(WalStats {
            wal_records: i64_value(&row, "wal_records"),
            wal_fpi: i64_value(&row, "wal_fpi"),
            wal_bytes: row
                .try_get::<Option<String>, _>("wal_bytes")
                .ok()
                .flatten()
                .unwrap_or_else(|| "0".to_string()),
            wal_buffers_full: i64_value(&row, "wal_buffers_full"),
            wal_write: i64_value(&row, "wal_write"),
            wal_sync: i64_value(&row, "wal_sync"),
            wal_write_time_ms: f64_value(&row, "wal_write_time"),
            wal_sync_time_ms: f64_value(&row, "wal_sync_time"),
            stats_reset: row.get("stats_reset"),
        })))
    }

    pub async fn fetch_io_stats(&self) -> Result<OptionalMonitorData<Option<IoStats>>> {
        let capabilities = self.monitor_capabilities().await?;
        if !capabilities.pg_stat_io {
            return Ok(OptionalMonitorData::unavailable(
                "pg_stat_io is not available on this PostgreSQL server",
            ));
        }

        let row = sqlx::query(
            r#"
            SELECT
                COALESCE(SUM(reads), 0)::bigint AS reads,
                COALESCE(SUM(writes), 0)::bigint AS writes,
                COALESCE(SUM(writebacks), 0)::bigint AS writebacks,
                COALESCE(SUM(extends), 0)::bigint AS extends,
                COALESCE(SUM(hits), 0)::bigint AS hits,
                COALESCE(SUM(evictions), 0)::bigint AS evictions,
                COALESCE(SUM(fsyncs), 0)::bigint AS fsyncs,
                COALESCE(SUM(read_time), 0)::float8 AS read_time,
                COALESCE(SUM(write_time), 0)::float8 AS write_time,
                COALESCE(SUM(fsync_time), 0)::float8 AS fsync_time
            FROM pg_catalog.pg_stat_io
            "#,
        )
        .fetch_one(self.pool())
        .await
        .map_err(monitor_error)?;

        Ok(OptionalMonitorData::available(Some(IoStats {
            reads: i64_value(&row, "reads"),
            writes: i64_value(&row, "writes"),
            writebacks: i64_value(&row, "writebacks"),
            extends: i64_value(&row, "extends"),
            hits: i64_value(&row, "hits"),
            evictions: i64_value(&row, "evictions"),
            fsyncs: i64_value(&row, "fsyncs"),
            read_time_ms: f64_value(&row, "read_time"),
            write_time_ms: f64_value(&row, "write_time"),
            fsync_time_ms: f64_value(&row, "fsync_time"),
        })))
    }

    pub async fn fetch_maintenance_progress(
        &self,
    ) -> Result<OptionalMonitorData<Vec<MaintenanceProgressRow>>> {
        let capabilities = self.monitor_capabilities().await?;
        if !capabilities.progress_vacuum && !capabilities.progress_create_index {
            return Ok(OptionalMonitorData::unavailable(
                "progress monitoring views are not available on this PostgreSQL server",
            ));
        }

        let mut rows = Vec::new();
        if capabilities.progress_vacuum {
            let vacuum_rows = sqlx::query(
                r#"
                SELECT
                    'vacuum' AS operation,
                    pid,
                    datname AS database_name,
                    relid::regclass::text AS relation,
                    NULL::text AS index_relation,
                    phase,
                    heap_blks_scanned::bigint AS progress_done,
                    heap_blks_total::bigint AS progress_total
                FROM pg_catalog.pg_stat_progress_vacuum
                "#,
            )
            .fetch_all(self.pool())
            .await
            .map_err(monitor_error)?;
            rows.extend(vacuum_rows);
        }

        if capabilities.progress_create_index {
            let index_rows = sqlx::query(
                r#"
                SELECT
                    command AS operation,
                    pid,
                    datname AS database_name,
                    relid::regclass::text AS relation,
                    index_relid::regclass::text AS index_relation,
                    phase,
                    blocks_done::bigint AS progress_done,
                    blocks_total::bigint AS progress_total
                FROM pg_catalog.pg_stat_progress_create_index
                "#,
            )
            .fetch_all(self.pool())
            .await
            .map_err(monitor_error)?;
            rows.extend(index_rows);
        }

        Ok(OptionalMonitorData::available(
            rows.into_iter()
                .map(|row| MaintenanceProgressRow {
                    operation: row.get("operation"),
                    pid: row.get("pid"),
                    database_name: row.get("database_name"),
                    relation: row.get("relation"),
                    index_relation: row.get("index_relation"),
                    phase: row.get("phase"),
                    progress_done: i64_value(&row, "progress_done"),
                    progress_total: i64_value(&row, "progress_total"),
                })
                .collect(),
        ))
    }
}

async fn fetch_maintenance_count(
    session: &Session,
    capabilities: &MonitorCapabilities,
) -> Result<i64> {
    let mut count = 0;
    if capabilities.progress_vacuum {
        count +=
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM pg_catalog.pg_stat_progress_vacuum")
                .fetch_one(session.pool())
                .await
                .map_err(monitor_error)?;
    }
    if capabilities.progress_create_index {
        count += sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM pg_catalog.pg_stat_progress_create_index",
        )
        .fetch_one(session.pool())
        .await
        .map_err(monitor_error)?;
    }
    Ok(count)
}

async fn fetch_wal_bytes(session: &Session) -> Result<String> {
    sqlx::query_scalar("SELECT COALESCE(wal_bytes, 0)::text FROM pg_catalog.pg_stat_wal")
        .fetch_one(session.pool())
        .await
        .map_err(monitor_error)
}

async fn fetch_io_totals(session: &Session) -> Result<(Option<i64>, Option<i64>)> {
    let row = sqlx::query(
        r#"
        SELECT
            COALESCE(SUM(reads), 0)::bigint AS reads,
            COALESCE(SUM(writes), 0)::bigint AS writes
        FROM pg_catalog.pg_stat_io
        "#,
    )
    .fetch_one(session.pool())
    .await
    .map_err(monitor_error)?;

    Ok((
        Some(i64_value(&row, "reads")),
        Some(i64_value(&row, "writes")),
    ))
}

fn cache_hit_ratio(reads: i64, hits: i64) -> Option<f64> {
    let total = reads + hits;
    (total > 0).then_some(hits as f64 / total as f64)
}

fn i64_value(row: &PgRow, column: &str) -> i64 {
    row.try_get::<Option<i64>, _>(column)
        .ok()
        .flatten()
        .unwrap_or_default()
}

fn f64_value(row: &PgRow, column: &str) -> f64 {
    row.try_get::<Option<f64>, _>(column)
        .ok()
        .flatten()
        .unwrap_or_default()
}

fn monitor_error(error: sqlx::Error) -> SqlError {
    let message = error.to_string();
    if message.contains("permission denied") || message.contains("must be superuser") {
        SqlError::PermissionDenied(message)
    } else {
        SqlError::Execution(message)
    }
}
