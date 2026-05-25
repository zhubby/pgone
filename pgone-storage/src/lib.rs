use anyhow::Result;
#[cfg(feature = "backend-libsql")]
use libsql::{Builder, Connection, Database};
use std::path::{Path, PathBuf};
#[cfg(feature = "backend-turso")]
use turso::{Builder, Connection, Database};

pub mod blocking;
pub mod file_sys;
pub mod models;
pub mod query;
pub mod schema;
pub mod storage;

pub const APP_DIR_NAME: &str = ".pgone";
pub const DATABASE_FILE_NAME: &str = "pgone.db";
pub const DATA_DIR_NAME: &str = "data";
pub const VECTOR_DATABASE_FILE_NAME: &str = "vector.db";
#[deprecated(note = "use database_path() for the user-local storage path")]
pub const DATABASE_PATH: &str = "pgone.db";

fn home_dir() -> Result<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
        .ok_or_else(|| anyhow::anyhow!("Unable to determine user home directory"))
}

#[must_use]
pub fn app_dir() -> PathBuf {
    home_dir()
        .map(|home| home.join(APP_DIR_NAME))
        .unwrap_or_else(|_| PathBuf::from(APP_DIR_NAME))
}

#[must_use]
pub fn database_path() -> PathBuf {
    app_dir().join(DATABASE_FILE_NAME)
}

#[must_use]
pub fn data_dir() -> PathBuf {
    app_dir().join(DATA_DIR_NAME)
}

#[must_use]
pub fn vector_database_path() -> PathBuf {
    app_dir().join(VECTOR_DATABASE_FILE_NAME)
}

#[must_use]
pub fn data_file_path(relative_path: impl AsRef<Path>) -> PathBuf {
    data_dir().join(relative_path)
}

pub async fn ensure_app_dir() -> Result<()> {
    tokio::fs::create_dir_all(app_dir()).await?;
    Ok(())
}

async fn migrate_legacy_local_storage() -> Result<()> {
    let database_path = database_path();
    let legacy_database_path = PathBuf::from(DATABASE_FILE_NAME);
    if !database_path.exists()
        && legacy_database_path.exists()
        && legacy_database_path != database_path
    {
        tokio::fs::copy(&legacy_database_path, &database_path).await?;
    }

    let data_dir = data_dir();
    let legacy_data_dir = PathBuf::from(DATA_DIR_NAME);
    if !data_dir.exists() && legacy_data_dir.is_dir() && legacy_data_dir != data_dir {
        copy_dir_all(&legacy_data_dir, &data_dir)?;
    }

    Ok(())
}

fn copy_dir_all(source: &Path, destination: &Path) -> Result<()> {
    std::fs::create_dir_all(destination)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let destination_path = destination.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_all(&entry.path(), &destination_path)?;
        } else if file_type.is_file() {
            std::fs::copy(entry.path(), destination_path)?;
        }
    }
    Ok(())
}

pub struct Storage {
    db: Database,
}

impl Storage {
    pub async fn open_default() -> Result<Self> {
        ensure_app_dir().await?;
        migrate_legacy_local_storage().await?;
        let path = database_path();
        Self::open_local_path(path).await
    }

    pub async fn open_local(path: &str) -> Result<Self> {
        Self::open_local_path(path).await
    }

    pub async fn open_local_path(path: impl AsRef<Path>) -> Result<Self> {
        if let Some(parent) = path.as_ref().parent()
            && !parent.as_os_str().is_empty()
        {
            tokio::fs::create_dir_all(parent).await?;
        }
        let path = path.as_ref().to_string_lossy().to_string();
        let db = Builder::new_local(path).build().await?;
        let s = Self { db };
        s.migrate().await?;
        Ok(s)
    }

    pub async fn conn(&self) -> Result<Connection> {
        Ok(self.db.connect()?)
    }

    async fn migrate(&self) -> Result<()> {
        let mut conn = self.conn().await?;
        schema::migrate(&mut conn).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_paths_live_under_pgone_app_dir() {
        assert_eq!(
            app_dir().file_name().and_then(|name| name.to_str()),
            Some(APP_DIR_NAME)
        );
        assert_eq!(
            database_path().file_name().and_then(|name| name.to_str()),
            Some(DATABASE_FILE_NAME)
        );
        assert_eq!(
            data_dir().file_name().and_then(|name| name.to_str()),
            Some(DATA_DIR_NAME)
        );
        assert_eq!(
            vector_database_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some(VECTOR_DATABASE_FILE_NAME)
        );
    }

    #[test]
    fn data_file_path_resolves_inside_data_dir() {
        assert_eq!(
            data_file_path("2026-05-25/file.txt"),
            data_dir().join("2026-05-25/file.txt")
        );
    }
}
