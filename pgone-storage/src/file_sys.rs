use crate::{data_dir, data_file_path, models::FileIndex};
use anyhow::{Context, Result};
#[cfg(feature = "backend-libsql")]
use libsql::{Connection, params};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
#[cfg(feature = "backend-turso")]
use turso::{Connection, params};
use uuid::Uuid;

fn now_ts() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

/// Ensure the user's local data directory exists; create it if it does not
pub async fn ensure_data_dir() -> Result<()> {
    let data_path = data_dir();
    if !data_path.exists() {
        tokio::fs::create_dir_all(data_path)
            .await
            .context("Failed to create data directory")?;
    }
    Ok(())
}

/// Get the folder name for the current date (YYYY-MM-DD format)
fn get_date_folder() -> String {
    use chrono::Local;
    Local::now().format("%Y-%m-%d").to_string()
}

/// Generate file path: ~/.pgone/data/YYYY-MM-DD/{uuid}.{ext}
fn generate_file_path(uuid: &str, extension: &str) -> PathBuf {
    let date_folder = get_date_folder();
    let filename = if extension.is_empty() {
        uuid.to_string()
    } else {
        format!("{}.{}", uuid, extension)
    };
    data_file_path(Path::new(&date_folder).join(filename))
}

/// Get file extension
fn get_file_extension(path: &Path) -> String {
    path.extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_string()
}

/// Detect the MIME type of a file
fn detect_mime_type(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path).context("Failed to read file for MIME type detection")?;

    // Use infer to detect MIME type
    if let Some(kind) = infer::get(&bytes) {
        Ok(kind.mime_type().to_string())
    } else {
        // If infer cannot detect, try using file extension
        let ext = get_file_extension(path);
        if ext.is_empty() {
            Ok("application/octet-stream".to_string())
        } else {
            // Simple extension to MIME type mapping
            let mime = match ext.to_lowercase().as_str() {
                "txt" => "text/plain",
                "pdf" => "application/pdf",
                "jpg" | "jpeg" => "image/jpeg",
                "png" => "image/png",
                "gif" => "image/gif",
                "json" => "application/json",
                "xml" => "application/xml",
                "html" => "text/html",
                "css" => "text/css",
                "js" => "application/javascript",
                "zip" => "application/zip",
                _ => "application/octet-stream",
            };
            Ok(mime.to_string())
        }
    }
}

/// Copy file to the index directory and record it in the database
pub async fn copy_file_to_index(conn: &mut Connection, source_path: &str) -> Result<FileIndex> {
    // Ensure data directory exists
    ensure_data_dir().await?;

    let source = Path::new(source_path);
    if !source.exists() {
        anyhow::bail!("Source file does not exist: {}", source_path);
    }

    // Get file metadata
    let metadata = tokio::fs::metadata(source)
        .await
        .context("Failed to get file metadata")?;
    let file_size = metadata.len() as i64;

    // Detect MIME type
    let file_type = detect_mime_type(source)?;

    // Get file extension
    let extension = get_file_extension(source);

    // Generate UUID and file path
    let id = Uuid::new_v4().to_string();
    let dest_path = generate_file_path(&id, &extension);

    // Ensure destination directory exists
    if let Some(parent) = dest_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .context("Failed to create destination directory")?;
    }

    // Copy file
    tokio::fs::copy(source, &dest_path)
        .await
        .context("Failed to copy file")?;

    // Get relative path (relative to ~/.pgone/data)
    let current_path = dest_path
        .strip_prefix(data_dir())
        .context("Failed to get relative path")?
        .to_string_lossy()
        .to_string();

    let now = now_ts();

    // Insert database record
    conn.execute(
        "INSERT INTO file_index (id, current_path, original_path, file_size, file_type, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            id.as_str(),
            current_path.as_str(),
            source_path,
            file_size,
            file_type.as_str(),
            now,
            now
        ],
    )
    .await
    .context("Failed to insert file index record")?;

    Ok(FileIndex {
        id,
        current_path,
        original_path: source_path.to_string(),
        file_size,
        file_type,
        created_at: now,
        updated_at: now,
    })
}

/// Delete file index and the actual file
pub async fn delete_file(conn: &mut Connection, id: &str) -> Result<()> {
    // Get file info first
    let file_index = get_file(conn, id).await?;
    if let Some(file) = file_index {
        // Delete the actual file
        let file_path = data_file_path(&file.current_path);
        if file_path.exists() {
            tokio::fs::remove_file(&file_path)
                .await
                .context("Failed to delete file")?;
        }

        // Delete database record
        conn.execute("DELETE FROM file_index WHERE id=?1", params![id])
            .await
            .context("Failed to delete file index record")?;
    } else {
        anyhow::bail!("File index not found: {}", id);
    }

    Ok(())
}

/// Update file (replace file content)
pub async fn update_file(
    conn: &mut Connection,
    id: &str,
    new_source_path: &str,
) -> Result<FileIndex> {
    // Get existing file info
    let mut file_index = get_file(conn, id).await?.context("File index not found")?;

    let new_source = Path::new(new_source_path);
    if !new_source.exists() {
        anyhow::bail!("New source file does not exist: {}", new_source_path);
    }

    // Get new file metadata
    let metadata = tokio::fs::metadata(new_source)
        .await
        .context("Failed to get new file metadata")?;
    let file_size = metadata.len() as i64;

    // Detect MIME type of the new file
    let file_type = detect_mime_type(new_source)?;

    // Get new file extension
    let new_extension = get_file_extension(new_source);
    let old_extension = get_file_extension(Path::new(&file_index.current_path));

    // If extension differs, update the file name
    let current_path = if new_extension != old_extension {
        // Delete old file
        let old_path = data_file_path(&file_index.current_path);
        if old_path.exists() {
            tokio::fs::remove_file(&old_path)
                .await
                .context("Failed to remove old file")?;
        }

        // Generate new path
        let new_path = generate_file_path(id, &new_extension);
        if let Some(parent) = new_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .context("Failed to create destination directory")?;
        }

        // Copy new file
        tokio::fs::copy(new_source, &new_path)
            .await
            .context("Failed to copy new file")?;

        // Get relative path
        new_path
            .strip_prefix(data_dir())
            .context("Failed to get relative path")?
            .to_string_lossy()
            .to_string()
    } else {
        // Same extension, overwrite directly
        let dest_path = data_file_path(&file_index.current_path);
        tokio::fs::copy(new_source, &dest_path)
            .await
            .context("Failed to copy new file")?;
        file_index.current_path
    };

    let updated_at = now_ts();

    // Update database record
    conn.execute(
        "UPDATE file_index SET current_path=?2, original_path=?3, file_size=?4, file_type=?5, updated_at=?6
         WHERE id=?1",
        params![
            id,
            current_path.as_str(),
            new_source_path,
            file_size,
            file_type.as_str(),
            updated_at
        ],
    )
    .await
    .context("Failed to update file index record")?;

    file_index.current_path = current_path;
    file_index.original_path = new_source_path.to_string();
    file_index.file_size = file_size;
    file_index.file_type = file_type;
    file_index.updated_at = updated_at;

    Ok(file_index)
}

/// Get file index by ID
pub async fn get_file(conn: &mut Connection, id: &str) -> Result<Option<FileIndex>> {
    let mut rows = conn
        .query(
            "SELECT id, current_path, original_path, file_size, file_type, created_at, updated_at
             FROM file_index WHERE id=?1",
            params![id],
        )
        .await?;

    if let Some(row) = rows.next().await? {
        Ok(Some(FileIndex {
            id: row.get::<String>(0)?,
            current_path: row.get::<String>(1)?,
            original_path: row.get::<String>(2)?,
            file_size: row.get::<i64>(3)?,
            file_type: row.get::<String>(4)?,
            created_at: row.get::<i64>(5)?,
            updated_at: row.get::<i64>(6)?,
        }))
    } else {
        Ok(None)
    }
}

/// Query file index by path
pub async fn query_files_by_path(
    conn: &mut Connection,
    path: &str,
    search_original: bool,
) -> Result<Vec<FileIndex>> {
    let sql = if search_original {
        "SELECT id, current_path, original_path, file_size, file_type, created_at, updated_at
         FROM file_index WHERE original_path=?1"
    } else {
        "SELECT id, current_path, original_path, file_size, file_type, created_at, updated_at
         FROM file_index WHERE current_path=?1"
    };

    let mut rows = conn.query(sql, params![path]).await?;
    let mut results = Vec::new();

    while let Some(row) = rows.next().await? {
        results.push(FileIndex {
            id: row.get::<String>(0)?,
            current_path: row.get::<String>(1)?,
            original_path: row.get::<String>(2)?,
            file_size: row.get::<i64>(3)?,
            file_type: row.get::<String>(4)?,
            created_at: row.get::<i64>(5)?,
            updated_at: row.get::<i64>(6)?,
        });
    }

    Ok(results)
}

/// Query file index by MIME type
pub async fn query_files_by_type(conn: &mut Connection, mime_type: &str) -> Result<Vec<FileIndex>> {
    let mut rows = conn
        .query(
            "SELECT id, current_path, original_path, file_size, file_type, created_at, updated_at
             FROM file_index WHERE file_type=?1 ORDER BY created_at DESC",
            params![mime_type],
        )
        .await?;

    let mut results = Vec::new();
    while let Some(row) = rows.next().await? {
        results.push(FileIndex {
            id: row.get::<String>(0)?,
            current_path: row.get::<String>(1)?,
            original_path: row.get::<String>(2)?,
            file_size: row.get::<i64>(3)?,
            file_type: row.get::<String>(4)?,
            created_at: row.get::<i64>(5)?,
            updated_at: row.get::<i64>(6)?,
        });
    }

    Ok(results)
}

/// Query file index by creation date range
pub async fn query_files_by_date_range(
    conn: &mut Connection,
    start: Option<i64>,
    end: Option<i64>,
) -> Result<Vec<FileIndex>> {
    let (sql, params_vec) = match (start, end) {
        (Some(s), Some(e)) => (
            "SELECT id, current_path, original_path, file_size, file_type, created_at, updated_at
             FROM file_index WHERE created_at >= ?1 AND created_at <= ?2 ORDER BY created_at DESC",
            vec![s, e],
        ),
        (Some(s), None) => (
            "SELECT id, current_path, original_path, file_size, file_type, created_at, updated_at
             FROM file_index WHERE created_at >= ?1 ORDER BY created_at DESC",
            vec![s],
        ),
        (None, Some(e)) => (
            "SELECT id, current_path, original_path, file_size, file_type, created_at, updated_at
             FROM file_index WHERE created_at <= ?1 ORDER BY created_at DESC",
            vec![e],
        ),
        (None, None) => (
            "SELECT id, current_path, original_path, file_size, file_type, created_at, updated_at
             FROM file_index ORDER BY created_at DESC",
            vec![],
        ),
    };

    let mut rows = match params_vec.len() {
        2 => {
            conn.query(sql, params![params_vec[0], params_vec[1]])
                .await?
        }
        1 => conn.query(sql, params![params_vec[0]]).await?,
        _ => conn.query(sql, params![]).await?,
    };

    let mut results = Vec::new();
    while let Some(row) = rows.next().await? {
        results.push(FileIndex {
            id: row.get::<String>(0)?,
            current_path: row.get::<String>(1)?,
            original_path: row.get::<String>(2)?,
            file_size: row.get::<i64>(3)?,
            file_type: row.get::<String>(4)?,
            created_at: row.get::<i64>(5)?,
            updated_at: row.get::<i64>(6)?,
        });
    }

    Ok(results)
}
