//! On-device BitTorrent downloads for the Android build.
//!
//! The engine writes into app-private storage by default. A tiny manifest keeps
//! enough identity information to expose librqbit's fast-resumed transfers after
//! the process is restarted.

use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use librqbit::{
    api::TorrentIdOrHash, AddTorrent, AddTorrentOptions, AddTorrentResponse, Session,
    SessionOptions, SessionPersistenceConfig,
};
use serde::{Deserialize, Serialize};
use std::num::NonZeroU32;

const MANIFEST_FILE: &str = "local-downloads.json";
type ManagedTorrentHandle = Arc<librqbit::ManagedTorrent>;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SavedTransfer {
    info_hash: String,
    name: String,
    output_folder: String,
    #[serde(default)]
    queue_position: u32,
}

#[derive(Clone)]
struct Transfer {
    saved: SavedTransfer,
    handle: ManagedTorrentHandle,
}

/// One real librqbit session and its locally persisted transfer index.
pub struct LocalDownloads {
    session: Arc<Session>,
    app_data_dir: PathBuf,
    download_dir: PathBuf,
    transfers: Mutex<BTreeMap<String, Transfer>>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TorrentTransfer {
    pub info_hash: String,
    pub name: String,
    pub status: String,
    pub error: String,
    pub progress: f64,
    pub size_bytes: u64,
    pub downloaded_bytes: u64,
    pub uploaded_bytes: u64,
    pub download_rate: u64,
    pub upload_rate: u64,
    pub peers: u32,
    pub seeds: u32,
    pub queue_position: u32,
    pub eta_seconds: Option<u64>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TorrentSnapshot {
    pub engine: String,
    pub download_directory: String,
    pub transfers: Vec<TorrentTransfer>,
}

impl LocalDownloads {
    pub fn incomplete_files(&self) -> Vec<PathBuf> {
        let Ok(transfers) = self.transfers.lock() else {
            return Vec::new();
        };
        transfers
            .values()
            .flat_map(|transfer| {
                let stats = transfer.handle.stats();
                transfer
                    .handle
                    .with_metadata(|metadata| {
                        metadata
                            .file_infos
                            .iter()
                            .enumerate()
                            .filter(|(index, file)| {
                                stats.file_progress.get(*index).copied().unwrap_or(0) < file.len
                            })
                            .map(|(_, file)| {
                                transfer
                                    .handle
                                    .output_folder()
                                    .join(&file.relative_filename)
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default()
            })
            .collect()
    }
    /// Start a real on-device torrent session. Both paths should be resolved by
    /// the Tauri integration to app-private Android storage.
    pub async fn new(app_data_dir: PathBuf, download_dir: PathBuf) -> Result<Self, String> {
        fs::create_dir_all(&app_data_dir).map_err(display_error)?;
        fs::create_dir_all(&download_dir).map_err(display_error)?;
        let state_dir = app_data_dir.join("rqbit-session");
        fs::create_dir_all(&state_dir).map_err(display_error)?;
        let session = Session::new_with_opts(
            download_dir.clone(),
            SessionOptions {
                fastresume: true,
                persistence: Some(SessionPersistenceConfig::Json {
                    folder: Some(state_dir),
                }),
                ..SessionOptions::default()
            },
        )
        .await
        .map_err(display_error)?;

        let manifest_path = app_data_dir.join(MANIFEST_FILE);
        let saved: Vec<SavedTransfer> = match fs::read(&manifest_path) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|e| {
                format!(
                    "Could not read the local torrent index {}: {e}",
                    manifest_path.display()
                )
            })?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(error) => return Err(display_error(error)),
        };

        let mut transfers = BTreeMap::new();
        for item in saved {
            if let Ok(hash) = parse_hash(&item.info_hash) {
                if let Some(handle) = session.get(TorrentIdOrHash::Hash(hash)) {
                    transfers.insert(
                        item.info_hash.clone(),
                        Transfer {
                            saved: item,
                            handle,
                        },
                    );
                }
            }
        }

        Ok(Self {
            session,
            app_data_dir,
            download_dir,
            transfers: Mutex::new(transfers),
        })
    }

    pub async fn snapshot(&self) -> Result<TorrentSnapshot, String> {
        let transfers = self
            .transfers
            .lock()
            .map_err(|_| "Torrent state lock is poisoned")?;
        let mut rows = Vec::with_capacity(transfers.len());
        for (hash, transfer) in transfers.iter() {
            let stats = transfer.handle.stats();
            let live = stats.live.as_ref();
            let down_rate = live.map_or(0, |v| v.download_speed.as_bytes());
            let up_rate = live.map_or(0, |v| v.upload_speed.as_bytes());
            let progress = if stats.total_bytes == 0 {
                0.0
            } else {
                (stats.progress_bytes as f64 / stats.total_bytes as f64).clamp(0.0, 1.0)
            };
            let status = if stats.error.is_some() {
                "Paused"
            } else if stats.finished {
                "Seeding"
            } else {
                match stats.state {
                    librqbit::TorrentStatsState::Paused => "Paused",
                    librqbit::TorrentStatsState::Initializing { .. } => "Metadata",
                    librqbit::TorrentStatsState::Live if stats.progress_bytes > 0 => "Downloading",
                    librqbit::TorrentStatsState::Live => "Downloading",
                    librqbit::TorrentStatsState::Error => "Paused",
                }
            };
            let eta_seconds = (down_rate > 0)
                .then(|| stats.total_bytes.saturating_sub(stats.progress_bytes) / down_rate);
            let peers = live.map_or(0, |v| v.snapshot.peer_stats.live);
            rows.push(TorrentTransfer {
                info_hash: hash.clone(),
                name: transfer
                    .handle
                    .name()
                    .unwrap_or_else(|| transfer.saved.name.clone()),
                status: status.to_owned(),
                error: stats.error.unwrap_or_default(),
                progress,
                size_bytes: stats.total_bytes,
                downloaded_bytes: stats.progress_bytes,
                uploaded_bytes: stats.uploaded_bytes,
                download_rate: down_rate,
                upload_rate: up_rate,
                peers,
                seeds: 0,
                queue_position: transfer.saved.queue_position,
                eta_seconds,
            });
        }
        Ok(TorrentSnapshot {
            engine: "rqbit".to_owned(),
            download_directory: self.download_dir.to_string_lossy().into_owned(),
            transfers: rows,
        })
    }

    pub async fn add_magnet(
        &self,
        uri: String,
        destination: Option<PathBuf>,
    ) -> Result<String, String> {
        if !uri.trim_start().starts_with("magnet:?") {
            return Err("Expected a magnet link".to_owned());
        }
        self.add(AddTorrent::from_url(uri), destination).await
    }

    pub async fn add_file(
        &self,
        path: PathBuf,
        destination: Option<PathBuf>,
    ) -> Result<String, String> {
        let bytes =
            fs::read(&path).map_err(|e| format!("Could not read {}: {e}", path.display()))?;
        self.add(AddTorrent::from_bytes(bytes), destination).await
    }

    pub async fn add_data(
        &self,
        encoded: String,
        destination: Option<PathBuf>,
    ) -> Result<String, String> {
        let bytes = STANDARD
            .decode(encoded.trim())
            .map_err(|e| format!("Torrent data is not valid base64: {e}"))?;
        if bytes.len() > 5_000_000 {
            return Err("Torrent metadata is too large".into());
        }
        self.add(AddTorrent::from_bytes(bytes), destination).await
    }

    async fn add(
        &self,
        source: AddTorrent<'_>,
        destination: Option<PathBuf>,
    ) -> Result<String, String> {
        let output_folder = match destination {
            Some(path) => {
                if !path.is_absolute()
                    || !path.starts_with(&self.download_dir)
                    || path
                        .components()
                        .any(|c| matches!(c, std::path::Component::ParentDir))
                {
                    return Err("Choose a folder inside the phone download directory".into());
                }
                fs::create_dir_all(&path).map_err(display_error)?;
                path
            }
            None => self.download_dir.clone(),
        };
        let options = AddTorrentOptions {
            overwrite: false,
            output_folder: Some(output_folder.to_string_lossy().into_owned()),
            ..AddTorrentOptions::default()
        };
        let response = self
            .session
            .add_torrent(source, Some(options))
            .await
            .map_err(display_error)?;
        let handle = match response {
            AddTorrentResponse::Added(_, handle)
            | AddTorrentResponse::AlreadyManaged(_, handle) => handle,
            AddTorrentResponse::ListOnly(_) => {
                return Err("Torrent metadata was listed without starting a download".to_owned())
            }
        };
        let info_hash = handle.info_hash().as_string();
        let name = handle.name().unwrap_or_else(|| info_hash.clone());
        let saved = SavedTransfer {
            info_hash: info_hash.clone(),
            name,
            output_folder: output_folder.to_string_lossy().into_owned(),
            queue_position: self
                .transfers
                .lock()
                .map_err(|_| "Torrent state lock is poisoned")?
                .len() as u32,
        };
        self.transfers
            .lock()
            .map_err(|_| "Torrent state lock is poisoned")?
            .insert(info_hash.clone(), Transfer { saved, handle });
        self.persist_manifest()?;
        Ok(info_hash)
    }

    pub async fn set_paused(&self, info_hash: String, paused: bool) -> Result<(), String> {
        let handle = self.find_handle(&info_hash)?;
        if paused {
            self.session.pause(&handle).await
        } else {
            self.session.unpause(&handle).await
        }
        .map_err(display_error)
    }

    pub async fn remove(&self, info_hash: String) -> Result<(), String> {
        let hash = parse_hash(&info_hash)?;
        self.session
            .delete(TorrentIdOrHash::Hash(hash), false)
            .await
            .map_err(display_error)?;
        self.transfers
            .lock()
            .map_err(|_| "Torrent state lock is poisoned")?
            .remove(&info_hash);
        self.persist_manifest()
    }

    /// Reorder the visible download queue. All transfers run concurrently in
    /// librqbit, but the order is retained across restarts for UI consistency.
    pub async fn move_queue(&self, info_hash: String, direction: i32) -> Result<(), String> {
        let mut rows = self
            .transfers
            .lock()
            .map_err(|_| "Torrent state lock is poisoned")?;
        if !rows.contains_key(&info_hash) {
            return Err(format!("No local torrent has info hash {info_hash}"));
        }
        let mut ordered: Vec<_> = rows.keys().cloned().collect();
        ordered.sort_by_key(|hash| {
            rows.get(hash)
                .map(|v| v.saved.queue_position)
                .unwrap_or(u32::MAX)
        });
        let index = ordered.iter().position(|hash| hash == &info_hash).unwrap();
        let target = if direction < 0 {
            index.saturating_sub(1)
        } else {
            (index + 1).min(ordered.len().saturating_sub(1))
        };
        ordered.swap(index, target);
        for (position, hash) in ordered.into_iter().enumerate() {
            if let Some(transfer) = rows.get_mut(&hash) {
                transfer.saved.queue_position = position as u32;
            }
        }
        drop(rows);
        self.persist_manifest()
    }

    pub async fn set_limits(&self, download: u64, upload: u64) -> Result<(), String> {
        let download = to_rate_limit(download)?;
        let upload = to_rate_limit(upload)?;
        self.session.ratelimits.set_download_bps(download);
        self.session.ratelimits.set_upload_bps(upload);
        Ok(())
    }

    fn find_handle(&self, info_hash: &str) -> Result<ManagedTorrentHandle, String> {
        self.transfers
            .lock()
            .map_err(|_| "Torrent state lock is poisoned")?
            .get(info_hash)
            .map(|transfer| transfer.handle.clone())
            .ok_or_else(|| format!("No local torrent has info hash {info_hash}"))
    }

    fn persist_manifest(&self) -> Result<(), String> {
        let rows = self
            .transfers
            .lock()
            .map_err(|_| "Torrent state lock is poisoned")?;
        let saved: Vec<_> = rows
            .values()
            .map(|transfer| transfer.saved.clone())
            .collect();
        let bytes = serde_json::to_vec_pretty(&saved).map_err(display_error)?;
        let target = self.app_data_dir.join(MANIFEST_FILE);
        let temporary = self.app_data_dir.join(format!("{MANIFEST_FILE}.tmp"));
        fs::write(&temporary, bytes).map_err(display_error)?;
        fs::rename(&temporary, &target).map_err(display_error)
    }
}

fn to_rate_limit(value: u64) -> Result<Option<NonZeroU32>, String> {
    if value == 0 {
        return Ok(None);
    }
    let value = u32::try_from(value)
        .map_err(|_| "Rate limit exceeds the engine's 32-bit bytes-per-second limit")?;
    NonZeroU32::new(value)
        .map(Some)
        .ok_or_else(|| "Rate limit must be positive".to_owned())
}

fn parse_hash(value: &str) -> Result<librqbit::dht::Id20, String> {
    value
        .parse()
        .map_err(|e| format!("Invalid torrent info hash: {e}"))
}

fn display_error(error: impl std::fmt::Display) -> String {
    error.to_string()
}
