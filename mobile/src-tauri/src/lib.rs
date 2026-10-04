use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf, time::Duration};
use tauri::{AppHandle, Manager, State};

mod local_downloads;
mod pairing;
mod player_device;
static LOCAL_METADATA_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Connection {
    url: String,
    token: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ConnectionStatus {
    url: Option<String>,
    paired: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct LocalCatalogItem {
    id: i64,
    title: String,
    extension: String,
    size_bytes: u64,
    modified_at: Option<i64>,
    kind: Option<String>,
    year: Option<u16>,
    overview: Option<String>,
    vote_average: Option<f32>,
    poster_url: Option<String>,
    backdrop_url: Option<String>,
    tmdb_id: Option<u64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LocalCatalogPage {
    items: Vec<LocalCatalogItem>,
    total: usize,
    offset: u32,
}

fn media_files(root: &PathBuf) -> Vec<(PathBuf, LocalCatalogItem)> {
    fn walk(dir: &PathBuf, output: &mut Vec<(PathBuf, LocalCatalogItem)>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, output);
                continue;
            }
            let Some(ext) = path.extension().and_then(|v| v.to_str()) else {
                continue;
            };
            if ![
                "mkv", "mp4", "m4v", "mov", "avi", "webm", "mpg", "mpeg", "ts",
            ]
            .iter()
            .any(|x| ext.eq_ignore_ascii_case(x))
            {
                continue;
            }
            let Ok(metadata) = fs::metadata(&path) else {
                continue;
            };
            let Some(stem) = path.file_stem().and_then(|v| v.to_str()) else {
                continue;
            };
            let mut hash = 2166136261u32;
            for byte in path.to_string_lossy().as_bytes() {
                hash = (hash ^ (*byte as u32)).wrapping_mul(16777619);
            }
            let id = 1_000_000_000_i64 + (hash as i64 % 1_000_000_000);
            let modified_at = metadata
                .modified()
                .ok()
                .and_then(|v| v.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|v| v.as_secs() as i64);
            output.push((
                path.clone(),
                LocalCatalogItem {
                    id,
                    title: stem.replace(['.', '_'], " "),
                    extension: ext.to_ascii_lowercase(),
                    size_bytes: metadata.len(),
                    modified_at,
                    kind: None,
                    year: None,
                    overview: None,
                    vote_average: None,
                    poster_url: None,
                    backdrop_url: None,
                    tmdb_id: None,
                },
            ));
        }
    }
    let mut output = Vec::new();
    walk(root, &mut output);
    output
}

fn ready_media_files(app: &AppHandle, root: &PathBuf) -> Vec<(PathBuf, LocalCatalogItem)> {
    let incomplete = app
        .state::<local_downloads::LocalDownloadsState>()
        .incomplete_files();
    let metadata: serde_json::Value = app
        .path()
        .app_data_dir()
        .ok()
        .and_then(|p| fs::read(p.join("local-metadata.json")).ok())
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    media_files(root)
        .into_iter()
        .filter(|(path, _)| !incomplete.contains(path))
        .map(|(path, mut item)| {
            if let Some(value) = metadata.get(item.id.to_string()) {
                item.title = value
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&item.title)
                    .to_owned();
                item.kind = value
                    .get("kind")
                    .and_then(|v| v.as_str())
                    .map(str::to_owned);
                item.year = value.get("year").and_then(|v| v.as_u64()).map(|v| v as u16);
                item.overview = value
                    .get("overview")
                    .and_then(|v| v.as_str())
                    .map(str::to_owned);
                item.vote_average = value
                    .get("voteAverage")
                    .and_then(|v| v.as_f64())
                    .map(|v| v as f32);
                item.poster_url = value
                    .get("posterUrl")
                    .and_then(|v| v.as_str())
                    .map(str::to_owned);
                item.backdrop_url = value
                    .get("backdropUrl")
                    .and_then(|v| v.as_str())
                    .map(str::to_owned);
                item.tmdb_id = value.get("id").and_then(|v| v.as_u64());
            }
            (path, item)
        })
        .collect()
}

#[tauri::command]
fn mobile_catalog_page(
    app: AppHandle,
    offset: Option<u32>,
    count: Option<u32>,
    kind: Option<String>,
    query: Option<String>,
    sort: Option<String>,
) -> Result<LocalCatalogPage, String> {
    let root = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("Downloads");
    let mut files = ready_media_files(&app, &root);
    if let Some(term) = query.filter(|v| !v.trim().is_empty()) {
        let term = term.to_lowercase();
        files.retain(|(_, item)| item.title.to_lowercase().contains(&term));
    }
    if let Some(kind) = kind {
        files.retain(|(_, item)| item.kind.as_deref() == Some(&kind));
    }
    let mut seen = std::collections::HashSet::new();
    files.retain(|(_, item)| {
        item.kind.as_deref() != Some("series")
            || item.tmdb_id.is_none()
            || seen.insert(item.tmdb_id)
    });
    match sort.as_deref() {
        Some("Title") | Some("title") => {
            files.sort_by(|a, b| a.1.title.to_lowercase().cmp(&b.1.title.to_lowercase()))
        }
        _ => files.sort_by(|a, b| b.1.modified_at.cmp(&a.1.modified_at)),
    }
    let total = files.len();
    let offset = offset.unwrap_or(0);
    let count = count.unwrap_or(48).min(200) as usize;
    let items = files
        .into_iter()
        .skip(offset as usize)
        .take(count)
        .map(|(_, item)| item)
        .collect();
    Ok(LocalCatalogPage {
        items,
        total,
        offset,
    })
}

#[tauri::command]
fn mobile_library_status(app: AppHandle) -> Result<serde_json::Value, String> {
    let root = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("Downloads");
    let count = ready_media_files(&app, &root).len();
    Ok(
        serde_json::json!({"folders":[root.to_string_lossy()],"rootCount":1,"fileCount":count,"matchedCount":0,"unmatchedCount":count,"pendingCount":0,"lastScanAt":null,"isScanning":false}),
    )
}

#[tauri::command]
fn mobile_resolve_media_file(app: AppHandle, media_id: i64) -> Result<serde_json::Value, String> {
    let root = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("Downloads");
    let (path, _) = ready_media_files(&app, &root)
        .into_iter()
        .find(|(_, item)| item.id == media_id)
        .ok_or("Downloaded media file was not found")?;
    app.asset_protocol_scope()
        .allow_file(&path)
        .map_err(|e| format!("Could not authorize playback for this file: {e}"))?;
    let resume = read_local_progress(&app)?
        .into_iter()
        .find(|v| v.id == media_id)
        .map(|v| v.position_seconds)
        .unwrap_or(0.0);
    Ok(
        serde_json::json!({"path":path.to_string_lossy(),"subtitles":[],"resumePositionSeconds":resume}),
    )
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LocalProgress {
    id: i64,
    position_seconds: f64,
    duration_seconds: f64,
    updated_at: f64,
}

fn progress_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("mobile-playback.json"))
}
fn read_local_progress(app: &AppHandle) -> Result<Vec<LocalProgress>, String> {
    let path = progress_path(app)?;
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map_err(|e| format!("Local playback history is invalid: {e}")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(format!("Could not read local playback history: {error}")),
    }
}
fn save_local_progress(
    app: &AppHandle,
    id: i64,
    position: Option<f64>,
    duration: Option<f64>,
) -> Result<(), String> {
    let mut rows = read_local_progress(app)?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64();
    if let Some(row) = rows.iter_mut().find(|row| row.id == id) {
        if let Some(value) = position {
            row.position_seconds = value.max(0.0);
        }
        if let Some(value) = duration {
            row.duration_seconds = value.max(0.0);
        }
        row.updated_at = now;
    } else {
        rows.push(LocalProgress {
            id,
            position_seconds: position.unwrap_or(0.0).max(0.0),
            duration_seconds: duration.unwrap_or(0.0).max(0.0),
            updated_at: now,
        });
    }
    let path = progress_path(app)?;
    let temp = path.with_extension("tmp");
    fs::write(&temp, serde_json::to_vec(&rows).map_err(|e| e.to_string())?)
        .map_err(|e| format!("Could not save playback history: {e}"))?;
    fs::rename(temp, path).map_err(|e| format!("Could not save playback history: {e}"))
}

fn local_title_detail(app: AppHandle, media_id: i64) -> Result<serde_json::Value, String> {
    let root = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("Downloads");
    let (path, item) = ready_media_files(&app, &root)
        .into_iter()
        .find(|(_, item)| item.id == media_id)
        .ok_or("Downloaded media file was not found")?;
    app.asset_protocol_scope()
        .allow_file(&path)
        .map_err(|e| format!("Could not authorize this file: {e}"))?;
    let mut files:Vec<serde_json::Value>=ready_media_files(&app,&root).into_iter().filter(|(_,candidate)|candidate.id==media_id || (item.kind.as_deref()==Some("series") && item.tmdb_id.is_some() && candidate.tmdb_id==item.tmdb_id)).map(|(path,record)|{
        let name=path.file_name().and_then(|v|v.to_str()).unwrap_or("video");
        let (season,episode)=episode_numbers(name);
        serde_json::json!({"mediaId":record.id,"fileName":name,"path":path.to_string_lossy(),"season":season,"episode":episode})
    }).collect();
    files.sort_by_key(|file| {
        (
            file.get("season").and_then(|v| v.as_u64()).unwrap_or(0),
            file.get("episode").and_then(|v| v.as_u64()).unwrap_or(0),
        )
    });
    Ok(serde_json::json!({"media":item,"files":files,"watchedBefore":null,"tmdbId":item.tmdb_id}))
}

fn episode_numbers(name: &str) -> (Option<u32>, Option<u32>) {
    let name = name.to_ascii_uppercase();
    for (index, character) in name.char_indices() {
        if character != 'S' {
            continue;
        }
        let digits: String = name[index + 1..]
            .chars()
            .take_while(char::is_ascii_digit)
            .collect();
        if digits.is_empty() || digits.len() > 2 {
            continue;
        }
        let Some(rest) = name[index + 1 + digits.len()..].strip_prefix('E') else {
            continue;
        };
        let episode: String = rest.chars().take_while(char::is_ascii_digit).collect();
        if episode.is_empty() || episode.len() > 3 {
            continue;
        }
        return (digits.parse().ok(), episode.parse().ok());
    }
    (None, None)
}

fn connection_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&dir)
        .map_err(|e| format!("Cannot create mobile settings directory: {e}"))?;
    Ok(dir.join("desktop-connection.json"))
}

fn load_connection(app: &AppHandle) -> Result<Option<Connection>, String> {
    let path = connection_path(app)?;
    if !path.exists() {
        return Ok(None);
    }
    let bytes = fs::read(path).map_err(|e| format!("Cannot read saved desktop connection: {e}"))?;
    let connection: Connection = serde_json::from_slice(&bytes)
        .map_err(|_| "Saved desktop connection is invalid; pair the device again".to_string())?;
    Ok(Some(connection))
}

#[tauri::command]
fn get_mobile_connection(app: AppHandle) -> Result<ConnectionStatus, String> {
    let connection = load_connection(&app)?;
    Ok(ConnectionStatus {
        url: connection.as_ref().map(|c| c.url.clone()),
        paired: connection
            .as_ref()
            .map(|c| !c.token.is_empty())
            .unwrap_or(false),
    })
}

#[tauri::command]
fn set_mobile_connection(app: AppHandle, url: String, token: String) -> Result<(), String> {
    let url = url.trim().trim_end_matches('/').to_string();
    if url.is_empty() {
        return Err("Desktop address and pairing token are required".into());
    }
    let token = if token.trim().is_empty() {
        load_connection(&app)?
            .filter(|connection| connection.url == url)
            .map(|connection| connection.token)
            .ok_or_else(|| "Pairing token is required for this desktop address".to_string())?
    } else {
        token.trim().to_string()
    };
    let parsed =
        reqwest::Url::parse(&url).map_err(|_| "Desktop address must be a valid URL".to_string())?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
        return Err("Desktop address must use HTTP or HTTPS and include a host".into());
    }
    let path = connection_path(&app)?;
    let serialized = serde_json::to_vec(&Connection { url, token }).map_err(|e| e.to_string())?;
    let temp = path.with_extension("tmp");
    fs::write(&temp, serialized).map_err(|e| format!("Cannot save desktop connection: {e}"))?;
    fs::rename(&temp, &path).map_err(|e| format!("Cannot save desktop connection: {e}"))
}

#[tauri::command]
fn clear_mobile_connection(app: AppHandle) -> Result<(), String> {
    let path = connection_path(&app)?;
    if path.exists() {
        fs::remove_file(path).map_err(|e| format!("Cannot clear desktop connection: {e}"))?;
    }
    Ok(())
}

#[tauri::command]
async fn mobile_remote_command(
    app: AppHandle,
    command: String,
    args: Option<serde_json::Value>,
) -> Result<serde_json::Value, String> {
    let connection = load_connection(&app)?
        .ok_or_else(|| "Connect this phone to Luma Desktop first".to_string())?;
    if connection.token.is_empty() {
        return Err("Desktop pairing is incomplete; pair the device again".into());
    }
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(45))
        .build()
        .map_err(|e| format!("Cannot create desktop connection: {e}"))?;
    let endpoint = format!("{}/api/v1/command", connection.url);
    let response = client.post(endpoint).bearer_auth(connection.token)
        .json(&serde_json::json!({"command": command, "args": args.unwrap_or_else(|| serde_json::json!({}))}))
        .send().await.map_err(|e| format!("Cannot reach Luma Desktop: {e}"))?;
    let status = response.status();
    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Desktop returned an invalid response: {e}"))?;
    if !status.is_success() {
        return Err(body
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("Desktop request failed")
            .to_string());
    }
    Ok(body)
}

#[tauri::command]
async fn mobile_local_command(
    app: AppHandle,
    command: String,
    args: Option<serde_json::Value>,
    downloads: State<'_, local_downloads::LocalDownloadsState>,
) -> Result<serde_json::Value, String> {
    let args = args.unwrap_or_else(|| serde_json::json!({}));
    let string_arg = |key: &str| {
        args.get(key)
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .ok_or_else(|| format!("Missing {key}"))
    };
    let optional_path = || {
        args.get("destination")
            .and_then(|v| v.as_str())
            .map(PathBuf::from)
    };
    match command.as_str() {
        "save_local_metadata" => {
            use tauri::Emitter;
            let _guard = LOCAL_METADATA_LOCK
                .lock()
                .map_err(|_| "Local metadata is unavailable")?;
            let id = args
                .get("mediaId")
                .and_then(|v| v.as_i64())
                .filter(|v| *v >= 1_000_000_000)
                .ok_or("Invalid local media id")?;
            let metadata = args
                .get("metadata")
                .filter(|v| v.is_object())
                .ok_or("Missing metadata")?;
            let path = app
                .path()
                .app_data_dir()
                .map_err(|e| e.to_string())?
                .join("local-metadata.json");
            let mut data: serde_json::Value = fs::read(&path)
                .ok()
                .and_then(|bytes| serde_json::from_slice(&bytes).ok())
                .unwrap_or_else(|| serde_json::json!({}));
            data[id.to_string()] = metadata.clone();
            let temp = path.with_extension("tmp");
            fs::write(&temp, data.to_string()).map_err(|e| e.to_string())?;
            fs::rename(temp, path).map_err(|e| e.to_string())?;
            let _ = app.emit("library-changed", ());
            Ok(serde_json::Value::Null)
        }
        "torrent_snapshot" => {
            let (app_data_dir, download_dir) = mobile_download_paths(&app)?;
            let snapshot = if local_downloads::has_saved_transfers(&app_data_dir) {
                ensure_local_downloads(&app, &downloads)
                    .await?
                    .snapshot()
                    .await?
            } else {
                local_downloads::empty_snapshot(&download_dir)
            };
            serde_json::to_value(snapshot).map_err(|e| e.to_string())
        }
        "torrent_add_magnet" => {
            let hash = ensure_local_downloads(&app, &downloads)
                .await?
                .add_magnet(string_arg("uri")?, optional_path())
                .await?;
            Ok(serde_json::Value::String(hash))
        }
        "torrent_add_file" => {
            let hash = ensure_local_downloads(&app, &downloads)
                .await?
                .add_file(PathBuf::from(string_arg("path")?), optional_path())
                .await?;
            Ok(serde_json::Value::String(hash))
        }
        "torrent_add_data" => {
            let hash = ensure_local_downloads(&app, &downloads)
                .await?
                .add_data(string_arg("encoded")?, optional_path())
                .await?;
            Ok(serde_json::Value::String(hash))
        }
        "torrent_set_paused" => {
            let paused = args
                .get("paused")
                .and_then(|v| v.as_bool())
                .ok_or("Missing paused")?;
            ensure_local_downloads(&app, &downloads)
                .await?
                .set_paused(string_arg("infoHash")?, paused)
                .await?;
            Ok(serde_json::Value::Null)
        }
        "torrent_remove" => {
            ensure_local_downloads(&app, &downloads)
                .await?
                .remove(string_arg("infoHash")?)
                .await?;
            Ok(serde_json::Value::Null)
        }
        "torrent_move_queue" => {
            let direction = args
                .get("direction")
                .and_then(|v| v.as_i64())
                .ok_or("Missing direction")? as i32;
            ensure_local_downloads(&app, &downloads)
                .await?
                .move_queue(string_arg("infoHash")?, direction)
                .await?;
            Ok(serde_json::Value::Null)
        }
        "torrent_set_limits" => {
            let download = args
                .get("download")
                .and_then(|v| v.as_u64())
                .ok_or("Missing download")?;
            let upload = args
                .get("upload")
                .and_then(|v| v.as_u64())
                .ok_or("Missing upload")?;
            ensure_local_downloads(&app, &downloads)
                .await?
                .set_limits(download, upload)
                .await?;
            Ok(serde_json::Value::Null)
        }
        "get_catalog_page" => serde_json::to_value(mobile_catalog_page(
            app,
            args.get("offset")
                .and_then(|v| v.as_u64())
                .map(|v| v as u32),
            args.get("count").and_then(|v| v.as_u64()).map(|v| v as u32),
            args.get("kind")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            args.get("query")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            args.get("sort")
                .and_then(|v| v.as_str())
                .map(str::to_string),
        )?)
        .map_err(|e| e.to_string()),
        "get_library_status" => mobile_library_status(app),
        "resolve_media_file" => {
            let id = args
                .get("mediaId")
                .and_then(|v| v.as_i64())
                .ok_or("Missing mediaId")?;
            mobile_resolve_media_file(app, id)
        }
        "get_local_title_detail" => {
            let id = args
                .get("mediaId")
                .and_then(|v| v.as_i64())
                .ok_or("Missing mediaId")?;
            local_title_detail(app, id).map(serde_json::Value::from)
        }
        "save_playback_progress" => {
            let id = args
                .get("mediaId")
                .and_then(|v| v.as_i64())
                .ok_or("Missing mediaId")?;
            let position = args
                .get("positionSeconds")
                .and_then(|v| v.as_f64())
                .ok_or("Missing positionSeconds")?;
            let duration = args
                .get("durationSeconds")
                .and_then(|v| v.as_f64())
                .ok_or("Missing durationSeconds")?;
            save_local_progress(&app, id, Some(position), Some(duration))?;
            Ok(serde_json::Value::Null)
        }
        "record_playback_activity" => {
            let id = args
                .get("mediaId")
                .and_then(|v| v.as_i64())
                .ok_or("Missing mediaId")?;
            save_local_progress(&app, id, None, None)?;
            Ok(serde_json::Value::Null)
        }
        "get_continue_watching" | "get_playback_history" => {
            let history = read_local_progress(&app)?;
            let root = app
                .path()
                .app_data_dir()
                .map_err(|e| e.to_string())?
                .join("Downloads");
            let catalog = ready_media_files(&app, &root);
            let limit = args.get("count").and_then(|v| v.as_u64()).unwrap_or(12) as usize;
            let mut items: Vec<serde_json::Value> = history.into_iter().filter_map(|entry| {
                let (_, media) = catalog.iter().find(|(_, media)| media.id == entry.id)?;
                if command == "get_continue_watching" && (entry.position_seconds < 1.0 || entry.position_seconds / entry.duration_seconds.max(1.0) > 0.94) { return None; }
                Some(if command == "get_continue_watching" {
                    serde_json::json!({"id":media.id,"title":media.title,"kind":media.kind,"year":media.year,"overview":media.overview,"voteAverage":media.vote_average,"posterUrl":media.poster_url,"backdropUrl":media.backdrop_url,"positionSeconds":entry.position_seconds,"durationSeconds":entry.duration_seconds,"updatedAt":entry.updated_at})
                } else {
                    serde_json::json!({"id":media.id,"title":media.title,"kind":media.kind,"year":media.year,"overview":media.overview,"voteAverage":media.vote_average,"posterUrl":media.poster_url,"backdropUrl":media.backdrop_url,"updatedAt":entry.updated_at})
                })
            }).collect();
            items.sort_by(|a, b| {
                b.get("updatedAt")
                    .and_then(|v| v.as_f64())
                    .partial_cmp(&a.get("updatedAt").and_then(|v| v.as_f64()))
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            items.truncate(limit);
            Ok(serde_json::Value::Array(items))
        }
        _ => Err(format!("Unsupported on-device command: {command}")),
    }
}

fn mobile_download_paths(app: &AppHandle) -> Result<(PathBuf, PathBuf), String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("Cannot access private app storage for downloads: {error}"))?;
    let download_dir = app_data_dir.join("Downloads");
    Ok((app_data_dir, download_dir))
}

async fn ensure_local_downloads<'a>(
    app: &AppHandle,
    state: &'a local_downloads::LocalDownloadsState,
) -> Result<&'a local_downloads::LocalDownloads, String> {
    let (app_data_dir, download_dir) = mobile_download_paths(app)?;
    state.get_or_init(app_data_dir, download_dir).await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(player_device::init())
        .setup(|app| {
            app.manage(local_downloads::LocalDownloadsState::default());
            app.manage(pairing::PairingState::default());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_mobile_connection,
            set_mobile_connection,
            clear_mobile_connection,
            mobile_remote_command,
            mobile_local_command,
            mobile_catalog_page,
            mobile_library_status,
            mobile_resolve_media_file,
            player_device::mobile_player_levels,
            pairing::discover_luma_computers,
            pairing::request_mobile_pair,
            pairing::check_mobile_pair,
            pairing::cancel_mobile_pair
        ])
        .run(tauri::generate_context!())
        .expect("Could not start Luma mobile");
}
