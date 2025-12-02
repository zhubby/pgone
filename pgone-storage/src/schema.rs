use anyhow::Result;
#[cfg(feature = "backend-libsql")]
use libsql::Connection;
#[cfg(feature = "backend-turso")]
use turso::Connection;

pub async fn migrate(conn: &mut Connection) -> Result<()> {
    // no foreign keys; program ensures associations
    conn.execute(
        "CREATE TABLE IF NOT EXISTS db_configs (
            id TEXT PRIMARY KEY,
            engine TEXT NOT NULL,
            dsn TEXT NOT NULL,
            default_schemas TEXT,
            include_system INTEGER,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        )",
        (),
    )
    .await?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS sessions (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            config_id TEXT,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        )",
        (),
    )
    .await?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_sessions_config_id ON sessions(config_id)",
        (),
    )
    .await?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_sessions_updated_at ON sessions(updated_at)",
        (),
    )
    .await?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS messages (
            id TEXT PRIMARY KEY,
            session_id TEXT NOT NULL,
            role TEXT NOT NULL,
            timestamp INTEGER NOT NULL,
            kind TEXT NOT NULL,
            content_markdown TEXT,
            image_path TEXT,
            image_w INTEGER,
            image_h INTEGER,
            video_path TEXT,
            video_duration_ms INTEGER
        )",
        (),
    )
    .await?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_messages_session_time ON messages(session_id, timestamp)",
        (),
    )
    .await?;

    // auth tables
    conn.execute(
        "CREATE TABLE IF NOT EXISTS auth_users (
            id TEXT PRIMARY KEY,
            login TEXT NOT NULL,
            name TEXT,
            avatar_url TEXT,
            email TEXT,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        )",
        (),
    )
    .await?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS auth_tokens (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            provider TEXT NOT NULL,
            access_token TEXT NOT NULL,
            scope TEXT,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        )",
        (),
    )
    .await?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_auth_tokens_updated_at ON auth_tokens(updated_at)",
        (),
    )
    .await?;

    // settings table (key-value store)
    conn.execute(
        "CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL,
            updated_at INTEGER NOT NULL
        )",
        (),
    )
    .await?;

    // file_index table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS file_index (
            id TEXT PRIMARY KEY,
            current_path TEXT NOT NULL,
            original_path TEXT NOT NULL,
            file_size INTEGER NOT NULL,
            file_type TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        )",
        (),
    )
    .await?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_file_index_original_path ON file_index(original_path)",
        (),
    )
    .await?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_file_index_file_type ON file_index(file_type)",
        (),
    )
    .await?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_file_index_created_at ON file_index(created_at)",
        (),
    )
    .await?;

    Ok(())
}
