use crate::models::*;
use anyhow::Result;
#[cfg(feature = "backend-libsql")]
use libsql::{Connection, params};
use std::time::{SystemTime, UNIX_EPOCH};
#[cfg(feature = "backend-turso")]
use turso::{Connection, params};

fn now_ts() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

pub async fn upsert_db_config(conn: &mut Connection, cfg: &DbConfig) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO db_configs (id, engine, dsn, default_schemas, include_system, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, COALESCE((SELECT created_at FROM db_configs WHERE id=?1), ?6), ?7)",
        params![
            cfg.id.as_str(),
            cfg.engine.as_str(),
            cfg.dsn.as_str(),
            cfg.default_schemas.as_deref(),
            cfg.include_system.map(|b| if b {1i64} else {0i64}),
            cfg.created_at,
            cfg.updated_at,
        ],
    ).await?;
    Ok(())
}

pub async fn get_db_config(conn: &mut Connection, id: &str) -> Result<Option<DbConfig>> {
    let mut rows = conn.query("SELECT id, engine, dsn, default_schemas, include_system, created_at, updated_at FROM db_configs WHERE id=?1", params![id]).await?;
    if let Some(row) = rows.next().await? {
        Ok(Some(DbConfig {
            id: row.get::<String>(0)?,
            engine: row.get::<String>(1)?,
            dsn: row.get::<String>(2)?,
            default_schemas: row.get::<Option<String>>(3)?,
            include_system: row.get::<Option<i64>>(4)?.map(|v| v != 0),
            created_at: row.get::<i64>(5)?,
            updated_at: row.get::<i64>(6)?,
        }))
    } else {
        Ok(None)
    }
}

pub async fn list_db_configs(conn: &mut Connection, limit: Option<i64>) -> Result<Vec<DbConfig>> {
    let sql = match limit {
        Some(_) => {
            "SELECT id, engine, dsn, default_schemas, include_system, created_at, updated_at FROM db_configs ORDER BY updated_at DESC LIMIT ?1"
        }
        None => {
            "SELECT id, engine, dsn, default_schemas, include_system, created_at, updated_at FROM db_configs ORDER BY updated_at DESC"
        }
    };
    let mut rows = if let Some(l) = limit {
        conn.query(sql, params![l]).await?
    } else {
        conn.query(sql, params![]).await?
    };
    let mut out = Vec::new();
    while let Some(row) = rows.next().await? {
        out.push(DbConfig {
            id: row.get::<String>(0)?,
            engine: row.get::<String>(1)?,
            dsn: row.get::<String>(2)?,
            default_schemas: row.get::<Option<String>>(3)?,
            include_system: row.get::<Option<i64>>(4)?.map(|v| v != 0),
            created_at: row.get::<i64>(5)?,
            updated_at: row.get::<i64>(6)?,
        });
    }
    Ok(out)
}

pub async fn delete_db_config(conn: &mut Connection, id: &str) -> Result<()> {
    // program-side cascade: delete sessions and messages
    let mut srows = conn
        .query("SELECT id FROM sessions WHERE config_id=?1", params![id])
        .await?;
    let mut sess_ids: Vec<String> = Vec::new();
    while let Some(r) = srows.next().await? {
        sess_ids.push(r.get::<String>(0)?);
    }
    for sid in sess_ids {
        delete_session(conn, &sid).await?;
    }
    conn.execute("DELETE FROM db_configs WHERE id=?1", params![id])
        .await?;
    Ok(())
}

pub async fn create_session(conn: &mut Connection, s: &Session) -> Result<()> {
    conn.execute(
        "INSERT INTO sessions (id, title, config_id, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![s.id.as_str(), s.title.as_str(), s.config_id.as_deref(), s.created_at, s.updated_at],
    ).await?;
    Ok(())
}

pub async fn update_session_title(conn: &mut Connection, id: &str, title: &str) -> Result<()> {
    conn.execute(
        "UPDATE sessions SET title=?2, updated_at=?3 WHERE id=?1",
        params![id, title, now_ts()],
    )
    .await?;
    Ok(())
}

pub async fn list_sessions(conn: &mut Connection, limit: i64) -> Result<Vec<Session>> {
    let mut rows = conn.query("SELECT id, title, config_id, created_at, updated_at FROM sessions ORDER BY updated_at DESC LIMIT ?1", params![limit]).await?;
    let mut out = Vec::new();
    while let Some(r) = rows.next().await? {
        out.push(Session {
            id: r.get::<String>(0)?,
            title: r.get::<String>(1)?,
            config_id: r.get::<Option<String>>(2)?,
            created_at: r.get::<i64>(3)?,
            updated_at: r.get::<i64>(4)?,
        });
    }
    Ok(out)
}

pub async fn delete_session(conn: &mut Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM messages WHERE session_id=?1", params![id])
        .await?;
    conn.execute("DELETE FROM sessions WHERE id=?1", params![id])
        .await?;
    Ok(())
}

pub async fn clear_session_messages(conn: &mut Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM messages WHERE session_id=?1", params![id])
        .await?;
    Ok(())
}

pub async fn append_markdown(
    conn: &mut Connection,
    session_id: &str,
    role: Role,
    md: &str,
) -> Result<String> {
    let id = uuid();
    conn.execute(
        "INSERT INTO messages (id, session_id, role, timestamp, kind, content_markdown) VALUES (?1, ?2, ?3, ?4, 'Markdown', ?5)",
        params![id.as_str(), session_id, format_role(&role), now_ts(), md],
    ).await?;
    Ok(id)
}

pub async fn append_image(
    conn: &mut Connection,
    session_id: &str,
    role: Role,
    path: &str,
    w: i64,
    h: i64,
) -> Result<String> {
    let id = uuid();
    conn.execute(
        "INSERT INTO messages (id, session_id, role, timestamp, kind, image_path, image_w, image_h) VALUES (?1, ?2, ?3, ?4, 'Image', ?5, ?6, ?7)",
        params![id.as_str(), session_id, format_role(&role), now_ts(), path, w, h],
    ).await?;
    Ok(id)
}

pub async fn append_video(
    conn: &mut Connection,
    session_id: &str,
    role: Role,
    path: &str,
    dur_ms: Option<i64>,
) -> Result<String> {
    let id = uuid();
    conn.execute(
        "INSERT INTO messages (id, session_id, role, timestamp, kind, video_path, video_duration_ms) VALUES (?1, ?2, ?3, ?4, 'Video', ?5, ?6)",
        params![id.as_str(), session_id, format_role(&role), now_ts(), path, dur_ms],
    ).await?;
    Ok(id)
}

pub async fn list_messages(
    conn: &mut Connection,
    session_id: &str,
    limit: i64,
) -> Result<Vec<Message>> {
    let mut rows = conn.query(
        "SELECT id, session_id, role, timestamp, kind, content_markdown, image_path, image_w, image_h, video_path, video_duration_ms
         FROM messages WHERE session_id=?1 ORDER BY timestamp ASC LIMIT ?2",
        params![session_id, limit]
    ).await?;
    let mut out = Vec::new();
    while let Some(r) = rows.next().await? {
        out.push(Message {
            id: r.get::<String>(0)?,
            session_id: r.get::<String>(1)?,
            role: parse_role(r.get::<String>(2)?),
            timestamp: r.get::<i64>(3)?,
            kind: parse_kind(r.get::<String>(4)?),
            content_markdown: r.get::<Option<String>>(5)?,
            image_path: r.get::<Option<String>>(6)?,
            image_w: r.get::<Option<i64>>(7)?,
            image_h: r.get::<Option<i64>>(8)?,
            video_path: r.get::<Option<String>>(9)?,
            video_duration_ms: r.get::<Option<i64>>(10)?,
        });
    }
    Ok(out)
}

/// 查询 messages 表中指定 session_id 的记录，按创建时间从近到远排序，返回前 10 条
/// 
/// # 参数
/// - `conn`: 数据库连接
/// - `session_id`: session 的 id（必需）
/// 
/// # 返回
/// 返回 Message 数组，按 timestamp 降序排列，最新的记录在前，最多 10 条
pub async fn query_messages_by_session(
    conn: &mut Connection,
    session_id: &str,
) -> Result<Vec<Message>> {
    let mut rows = conn.query(
        "SELECT id, session_id, role, timestamp, kind, content_markdown, image_path, image_w, image_h, video_path, video_duration_ms
         FROM messages WHERE session_id=?1 ORDER BY timestamp DESC LIMIT 10",
        params![session_id]
    ).await?;

    let mut out = Vec::new();
    while let Some(r) = rows.next().await? {
        out.push(Message {
            id: r.get::<String>(0)?,
            session_id: r.get::<String>(1)?,
            role: parse_role(r.get::<String>(2)?),
            timestamp: r.get::<i64>(3)?,
            kind: parse_kind(r.get::<String>(4)?),
            content_markdown: r.get::<Option<String>>(5)?,
            image_path: r.get::<Option<String>>(6)?,
            image_w: r.get::<Option<i64>>(7)?,
            image_h: r.get::<Option<i64>>(8)?,
            video_path: r.get::<Option<String>>(9)?,
            video_duration_ms: r.get::<Option<i64>>(10)?,
        });
    }
    Ok(out)
}

// =====================
// Auth storage helpers
// =====================

pub async fn upsert_auth_user(conn: &mut Connection, u: &AuthUser) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO auth_users (id, login, name, avatar_url, email, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, COALESCE((SELECT created_at FROM auth_users WHERE id=?1), ?6), ?7)",
        params![
            u.id.as_str(),
            u.login.as_str(),
            u.name.as_deref(),
            u.avatar_url.as_deref(),
            u.email.as_deref(),
            u.created_at,
            u.updated_at,
        ],
    ).await?;
    Ok(())
}

pub async fn insert_auth_token(conn: &mut Connection, t: &AuthToken) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO auth_tokens (id, user_id, provider, access_token, scope, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, COALESCE((SELECT created_at FROM auth_tokens WHERE id=?1), ?6), ?7)",
        params![
            t.id.as_str(),
            t.user_id.as_str(),
            t.provider.as_str(),
            t.access_token.as_str(),
            t.scope.as_deref(),
            t.created_at,
            t.updated_at,
        ],
    ).await?;
    Ok(())
}

pub async fn get_current_user(conn: &mut Connection) -> Result<Option<AuthUser>> {
    let mut rows = conn.query(
        "SELECT u.id, u.login, u.name, u.avatar_url, u.email, u.created_at, u.updated_at
         FROM auth_users u
         JOIN auth_tokens t ON t.user_id = u.id
         ORDER BY t.updated_at DESC
         LIMIT 1",
        params![],
    ).await?;
    if let Some(r) = rows.next().await? {
        Ok(Some(AuthUser {
            id: r.get::<String>(0)?,
            login: r.get::<String>(1)?,
            name: r.get::<Option<String>>(2)?,
            avatar_url: r.get::<Option<String>>(3)?,
            email: r.get::<Option<String>>(4)?,
            created_at: r.get::<i64>(5)?,
            updated_at: r.get::<i64>(6)?,
        }))
    } else {
        Ok(None)
    }
}

fn uuid() -> String {
    use std::time::SystemTime;
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("id-{}", t)
}
fn format_role(r: &Role) -> &'static str {
    match r {
        Role::User => "User",
        Role::Assistant => "Assistant",
        Role::System => "System",
    }
}
fn parse_role(s: String) -> Role {
    match s.as_str() {
        "User" => Role::User,
        "Assistant" => Role::Assistant,
        _ => Role::System,
    }
}
fn parse_kind(s: String) -> MessageKind {
    match s.as_str() {
        "Markdown" => MessageKind::Markdown,
        "Image" => MessageKind::Image,
        _ => MessageKind::Video,
    }
}

// =====================
// Settings storage helpers (key-value)
// =====================

use std::collections::HashMap;

pub async fn upsert_setting(conn: &mut Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)",
        params![key, value, now_ts()],
    )
    .await?;
    Ok(())
}

pub async fn get_setting(conn: &mut Connection, key: &str) -> Result<Option<String>> {
    let mut rows = conn
        .query("SELECT value FROM settings WHERE key=?1", params![key])
        .await?;
    if let Some(row) = rows.next().await? {
        Ok(Some(row.get::<String>(0)?))
    } else {
        Ok(None)
    }
}

pub async fn get_all_settings(conn: &mut Connection) -> Result<HashMap<String, String>> {
    let mut rows = conn
        .query("SELECT key, value FROM settings", params![])
        .await?;
    let mut settings = HashMap::new();
    while let Some(row) = rows.next().await? {
        let key: String = row.get(0)?;
        let value: String = row.get(1)?;
        settings.insert(key, value);
    }
    Ok(settings)
}

pub async fn delete_setting(conn: &mut Connection, key: &str) -> Result<()> {
    conn.execute("DELETE FROM settings WHERE key=?1", params![key])
        .await?;
    Ok(())
}

pub async fn clear_settings(conn: &mut Connection) -> Result<()> {
    conn.execute("DELETE FROM settings", params![]).await?;
    Ok(())
}


