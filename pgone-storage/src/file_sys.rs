use crate::models::FileIndex;
use anyhow::{Context, Result};
#[cfg(feature = "backend-libsql")]
use libsql::{Connection, params};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
#[cfg(feature = "backend-turso")]
use turso::{Connection, params};
use uuid::Uuid;

const DATA_DIR: &str = "./data";

fn now_ts() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

/// 确保 ./data 目录存在，不存在则创建
pub async fn ensure_data_dir() -> Result<()> {
    let data_path = Path::new(DATA_DIR);
    if !data_path.exists() {
        tokio::fs::create_dir_all(data_path)
            .await
            .context("Failed to create data directory")?;
    }
    Ok(())
}

/// 获取当前日期的文件夹名称（YYYY-MM-DD 格式）
fn get_date_folder() -> String {
    use chrono::Local;
    Local::now().format("%Y-%m-%d").to_string()
}

/// 生成文件路径：./data/YYYY-MM-DD/{uuid}.{ext}
fn generate_file_path(uuid: &str, extension: &str) -> PathBuf {
    let date_folder = get_date_folder();
    let filename = if extension.is_empty() {
        uuid.to_string()
    } else {
        format!("{}.{}", uuid, extension)
    };
    PathBuf::from(DATA_DIR).join(date_folder).join(filename)
}

/// 获取文件扩展名
fn get_file_extension(path: &Path) -> String {
    path.extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_string()
}

/// 检测文件的 MIME 类型
fn detect_mime_type(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path).context("Failed to read file for MIME type detection")?;

    // 使用 infer 检测 MIME 类型
    if let Some(kind) = infer::get(&bytes) {
        Ok(kind.mime_type().to_string())
    } else {
        // 如果 infer 无法检测，尝试使用文件扩展名
        let ext = get_file_extension(path);
        if ext.is_empty() {
            Ok("application/octet-stream".to_string())
        } else {
            // 简单的扩展名到 MIME 类型映射
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

/// 复制文件到索引目录并记录到数据库
pub async fn copy_file_to_index(conn: &mut Connection, source_path: &str) -> Result<FileIndex> {
    // 确保数据目录存在
    ensure_data_dir().await?;

    let source = Path::new(source_path);
    if !source.exists() {
        anyhow::bail!("Source file does not exist: {}", source_path);
    }

    // 获取文件元数据
    let metadata = tokio::fs::metadata(source)
        .await
        .context("Failed to get file metadata")?;
    let file_size = metadata.len() as i64;

    // 检测 MIME 类型
    let file_type = detect_mime_type(source)?;

    // 获取文件扩展名
    let extension = get_file_extension(source);

    // 生成 UUID 和文件路径
    let id = Uuid::new_v4().to_string();
    let dest_path = generate_file_path(&id, &extension);

    // 确保目标目录存在
    if let Some(parent) = dest_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .context("Failed to create destination directory")?;
    }

    // 复制文件
    tokio::fs::copy(source, &dest_path)
        .await
        .context("Failed to copy file")?;

    // 获取相对路径（相对于 ./data）
    let current_path = dest_path
        .strip_prefix(DATA_DIR)
        .context("Failed to get relative path")?
        .to_string_lossy()
        .to_string();

    let now = now_ts();

    // 插入数据库记录
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

/// 删除文件索引和实际文件
pub async fn delete_file(conn: &mut Connection, id: &str) -> Result<()> {
    // 先获取文件信息
    let file_index = get_file(conn, id).await?;
    if let Some(file) = file_index {
        // 删除实际文件
        let file_path = Path::new(DATA_DIR).join(&file.current_path);
        if file_path.exists() {
            tokio::fs::remove_file(&file_path)
                .await
                .context("Failed to delete file")?;
        }

        // 删除数据库记录
        conn.execute("DELETE FROM file_index WHERE id=?1", params![id])
            .await
            .context("Failed to delete file index record")?;
    } else {
        anyhow::bail!("File index not found: {}", id);
    }

    Ok(())
}

/// 更新文件（替换文件内容）
pub async fn update_file(
    conn: &mut Connection,
    id: &str,
    new_source_path: &str,
) -> Result<FileIndex> {
    // 获取现有文件信息
    let mut file_index = get_file(conn, id).await?.context("File index not found")?;

    let new_source = Path::new(new_source_path);
    if !new_source.exists() {
        anyhow::bail!("New source file does not exist: {}", new_source_path);
    }

    // 获取新文件元数据
    let metadata = tokio::fs::metadata(new_source)
        .await
        .context("Failed to get new file metadata")?;
    let file_size = metadata.len() as i64;

    // 检测新文件的 MIME 类型
    let file_type = detect_mime_type(new_source)?;

    // 获取新文件扩展名
    let new_extension = get_file_extension(new_source);
    let old_extension = get_file_extension(Path::new(&file_index.current_path));

    // 如果扩展名不同，需要更新文件名
    let current_path = if new_extension != old_extension {
        // 删除旧文件
        let old_path = Path::new(DATA_DIR).join(&file_index.current_path);
        if old_path.exists() {
            tokio::fs::remove_file(&old_path)
                .await
                .context("Failed to remove old file")?;
        }

        // 生成新路径
        let new_path = generate_file_path(id, &new_extension);
        if let Some(parent) = new_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .context("Failed to create destination directory")?;
        }

        // 复制新文件
        tokio::fs::copy(new_source, &new_path)
            .await
            .context("Failed to copy new file")?;

        // 获取相对路径
        new_path
            .strip_prefix(DATA_DIR)
            .context("Failed to get relative path")?
            .to_string_lossy()
            .to_string()
    } else {
        // 扩展名相同，直接覆盖
        let dest_path = Path::new(DATA_DIR).join(&file_index.current_path);
        tokio::fs::copy(new_source, &dest_path)
            .await
            .context("Failed to copy new file")?;
        file_index.current_path
    };

    let updated_at = now_ts();

    // 更新数据库记录
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

/// 根据 ID 获取文件索引
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

/// 按路径查询文件索引
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

/// 按 MIME 类型查询文件索引
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

/// 按创建时间范围查询文件索引
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
