use crate::futures;
use pgone_storage::models::{DbConfig, FileIndex};
use pgone_storage::service::StorageService;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

#[derive(Default)]
struct StorageCache {
    db_configs: HashMap<String, DbConfig>,
    files: HashMap<String, FileIndex>,
    settings: HashMap<String, String>,
    ready: bool,
    last_error: Option<String>,
}

enum StorageCommand {
    Refresh,
    UpsertDbConfig(DbConfig),
    DeleteDbConfig(String),
    UpsertSetting {
        key: String,
        value: String,
    },
    CopyFile {
        source_path: String,
        target: FileUploadTarget,
    },
}

#[derive(Debug, Clone, Copy)]
pub enum FileUploadTarget {
    AddSslCert,
    AddSslKey,
    AddSslRootcert,
    EditSslCert,
    EditSslKey,
    EditSslRootcert,
}

#[derive(Debug, Clone)]
pub struct FileUploadResult {
    pub target: FileUploadTarget,
    pub file: FileIndex,
}

#[derive(Clone)]
pub struct GuiStorage {
    cache: Arc<Mutex<StorageCache>>,
    commands: mpsc::UnboundedSender<StorageCommand>,
    uploads: Arc<Mutex<Vec<Result<FileUploadResult, String>>>>,
}

impl GuiStorage {
    pub fn new(ctx: egui::Context) -> Self {
        let cache = Arc::new(Mutex::new(StorageCache::default()));
        let uploads = Arc::new(Mutex::new(Vec::new()));
        let (commands, mut receiver) = mpsc::unbounded_channel();

        let worker_cache = Arc::clone(&cache);
        let worker_uploads = Arc::clone(&uploads);
        futures::spawn(async move {
            let storage = match StorageService::open_default().await {
                Ok(storage) => {
                    set_ready(&worker_cache, true, None);
                    storage
                }
                Err(error) => {
                    set_ready(&worker_cache, false, Some(error.to_string()));
                    return;
                }
            };

            refresh_cache(&storage, &worker_cache).await;
            ctx.request_repaint();

            while let Some(command) = receiver.recv().await {
                match command {
                    StorageCommand::Refresh => {
                        refresh_cache(&storage, &worker_cache).await;
                    }
                    StorageCommand::UpsertDbConfig(cfg) => {
                        let result = storage.upsert_db_config(&cfg).await;
                        if let Err(error) = result {
                            set_error(&worker_cache, error.to_string());
                        }
                        refresh_cache(&storage, &worker_cache).await;
                    }
                    StorageCommand::DeleteDbConfig(id) => {
                        let result = storage.delete_db_config(&id).await;
                        if let Err(error) = result {
                            set_error(&worker_cache, error.to_string());
                        }
                        refresh_cache(&storage, &worker_cache).await;
                    }
                    StorageCommand::UpsertSetting { key, value } => {
                        if let Err(error) = storage.upsert_setting(&key, &value).await {
                            set_error(&worker_cache, error.to_string());
                        }
                        refresh_cache(&storage, &worker_cache).await;
                    }
                    StorageCommand::CopyFile {
                        source_path,
                        target,
                    } => {
                        let result = storage
                            .copy_file_to_index(&source_path)
                            .await
                            .map(|file| FileUploadResult { target, file })
                            .map_err(|error| error.to_string());
                        if let Ok(result) = &result {
                            cache_file(&worker_cache, result.file.clone());
                        }
                        if let Err(error) = &result {
                            set_error(&worker_cache, error.clone());
                        }
                        if let Ok(mut uploads) = worker_uploads.lock() {
                            uploads.push(result);
                        }
                        refresh_cache(&storage, &worker_cache).await;
                    }
                }

                ctx.request_repaint();
            }
        });

        let handle = Self {
            cache,
            commands,
            uploads,
        };
        handle.refresh();
        handle
    }

    pub fn refresh(&self) {
        let _ = self.commands.send(StorageCommand::Refresh);
    }

    pub fn is_ready(&self) -> bool {
        self.cache.lock().map(|cache| cache.ready).unwrap_or(false)
    }

    pub fn last_error(&self) -> Option<String> {
        self.cache
            .lock()
            .ok()
            .and_then(|cache| cache.last_error.clone())
    }

    pub fn list_db_configs(&self) -> Vec<DbConfig> {
        let mut configs = self
            .cache
            .lock()
            .map(|cache| cache.db_configs.values().cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        configs.sort_by(|a, b| a.id.cmp(&b.id));
        configs
    }

    pub fn get_db_config(&self, id: &str) -> Option<DbConfig> {
        self.cache
            .lock()
            .ok()
            .and_then(|cache| cache.db_configs.get(id).cloned())
    }

    pub fn get_default_db_config(&self) -> Option<DbConfig> {
        self.cache.lock().ok().and_then(|cache| {
            cache
                .db_configs
                .values()
                .find(|cfg| cfg.default_config.unwrap_or(false))
                .cloned()
        })
    }

    pub fn upsert_db_config(&self, cfg: DbConfig) {
        if let Ok(mut cache) = self.cache.lock() {
            cache.db_configs.insert(cfg.id.clone(), cfg.clone());
        }
        let _ = self.commands.send(StorageCommand::UpsertDbConfig(cfg));
    }

    pub fn delete_db_config(&self, id: &str) {
        if let Ok(mut cache) = self.cache.lock() {
            cache.db_configs.remove(id);
        }
        let _ = self
            .commands
            .send(StorageCommand::DeleteDbConfig(id.to_string()));
    }

    pub fn get_file(&self, id: &str) -> Option<FileIndex> {
        self.cache
            .lock()
            .ok()
            .and_then(|cache| cache.files.get(id).cloned())
    }

    pub fn list_files(&self) -> Vec<FileIndex> {
        self.cache
            .lock()
            .map(|cache| cache.files.values().cloned().collect())
            .unwrap_or_default()
    }

    pub fn copy_file_to_index(&self, source_path: String, target: FileUploadTarget) {
        let _ = self.commands.send(StorageCommand::CopyFile {
            source_path,
            target,
        });
    }

    pub fn take_file_upload_results(&self) -> Vec<Result<FileUploadResult, String>> {
        self.uploads
            .lock()
            .map(|mut uploads| uploads.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn get_all_settings(&self) -> HashMap<String, String> {
        self.cache
            .lock()
            .map(|cache| cache.settings.clone())
            .unwrap_or_default()
    }

    pub fn upsert_setting(&self, key: String, value: String) {
        if let Ok(mut cache) = self.cache.lock() {
            cache.settings.insert(key.clone(), value.clone());
        }
        let _ = self
            .commands
            .send(StorageCommand::UpsertSetting { key, value });
    }
}

async fn refresh_cache(storage: &StorageService, cache: &Arc<Mutex<StorageCache>>) {
    match storage.list_db_configs(None).await {
        Ok(configs) => {
            if let Ok(mut cache) = cache.lock() {
                cache.db_configs = configs
                    .into_iter()
                    .map(|config| (config.id.clone(), config))
                    .collect();
                cache.ready = true;
                cache.last_error = None;
            }
        }
        Err(error) => set_error(cache, error.to_string()),
    }

    match storage.list_files().await {
        Ok(files) => {
            if let Ok(mut cache) = cache.lock() {
                cache.files = files
                    .into_iter()
                    .map(|file| (file.id.clone(), file))
                    .collect();
            }
        }
        Err(error) => set_error(cache, error.to_string()),
    }

    match storage.get_all_settings().await {
        Ok(settings) => {
            if let Ok(mut cache) = cache.lock() {
                cache.settings = settings;
            }
        }
        Err(error) => set_error(cache, error.to_string()),
    }
}

fn cache_file(cache: &Arc<Mutex<StorageCache>>, file: FileIndex) {
    if let Ok(mut cache) = cache.lock() {
        cache.files.insert(file.id.clone(), file);
    }
}

fn set_ready(cache: &Arc<Mutex<StorageCache>>, ready: bool, error: Option<String>) {
    if let Ok(mut cache) = cache.lock() {
        cache.ready = ready;
        cache.last_error = error;
    }
}

fn set_error(cache: &Arc<Mutex<StorageCache>>, error: String) {
    if let Ok(mut cache) = cache.lock() {
        cache.last_error = Some(error);
    }
}
