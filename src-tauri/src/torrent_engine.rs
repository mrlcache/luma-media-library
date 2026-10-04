//! Thin, serialized Rust adapter for the direct libtorrent C++ bridge.
//! A missing DLL is reported only on the Torrents page, never during app startup.

use libloading::Library;
use serde::Serialize;
use tauri::{Emitter, Manager};
use std::{ffi::{c_char, c_void, CString}, path::{Path, PathBuf}, sync::{Arc, Mutex}, time::Duration};

type Create = unsafe extern "C" fn(*const c_char, *const c_char, *mut c_char, i32) -> *mut c_void;
type Destroy = unsafe extern "C" fn(*mut c_void);
type Add = unsafe extern "C" fn(*mut c_void, *const c_char, *mut c_char, i32, *mut c_char, i32) -> i32;
type List = unsafe extern "C" fn(*mut c_void, *mut RawTransfer, i32, *mut c_char, i32) -> i32;
type Files = unsafe extern "C" fn(*mut c_void, *const c_char, *mut RawFile, i32, *mut c_char, i32) -> i32;
type Paused = unsafe extern "C" fn(*mut c_void, *const c_char, i32, *mut c_char, i32) -> i32;
type MoveQueue = unsafe extern "C" fn(*mut c_void, *const c_char, i32, *mut c_char, i32) -> i32;
type Limits = unsafe extern "C" fn(*mut c_void, i32, i32, *mut c_char, i32) -> i32;
type Remove = unsafe extern "C" fn(*mut c_void, *const c_char, *mut c_char, i32) -> i32;
type Save = unsafe extern "C" fn(*mut c_void, *mut c_char, i32) -> i32;

#[repr(C)]
#[derive(Clone, Copy)]
struct RawTransfer {
    info_hash: [c_char; 65],
    name: [c_char; 512],
    error: [c_char; 512],
    state: i32,
    paused: i32,
    auto_managed: i32,
    queue_position: i32,
    peers: i32,
    seeds: i32,
    progress: f64,
    size_bytes: i64,
    downloaded_bytes: i64,
    uploaded_bytes: i64,
    download_rate: i32,
    upload_rate: i32,
}

#[repr(C)]
struct RawFile { path: [c_char; 4096], size_bytes: i64, completed_bytes: i64 }

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferSnapshot {
    pub info_hash: String,
    pub name: String,
    pub status: &'static str,
    pub error: String,
    pub progress: f64,
    pub size_bytes: i64,
    pub downloaded_bytes: i64,
    pub uploaded_bytes: i64,
    pub download_rate: i32,
    pub upload_rate: i32,
    pub peers: i32,
    pub seeds: i32,
    pub queue_position: i32,
    pub eta_seconds: Option<i64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TorrentSnapshot {
    pub engine: &'static str,
    pub download_directory: String,
    pub transfers: Vec<TransferSnapshot>,
}

struct TorrentEngine {
    _library: Library,
    handle: usize,
    destroy: Destroy,
    add_magnet: Add,
    add_file: Add,
    list: List,
    files: Files,
    set_paused: Paused,
    move_queue: MoveQueue,
    set_limits: Limits,
    remove: Remove,
    save: Save,
}

// All calls to this native session are made under TorrentState's mutex.
unsafe impl Send for TorrentEngine {}

impl Drop for TorrentEngine {
    fn drop(&mut self) {
        let mut error = [0 as c_char; 512];
        unsafe {
            (self.save)(self.handle as *mut c_void, error.as_mut_ptr(), error.len() as i32);
            (self.destroy)(self.handle as *mut c_void);
        }
    }
}

impl TorrentEngine {
    fn open(state_directory: &Path, download_directory: &Path, resource_directory: &Path) -> Result<Self, String> {
        let library = load_bridge(resource_directory)?;
        unsafe {
            let create: Create = symbol(&library, b"mt_create\0")?;
            let destroy: Destroy = symbol(&library, b"mt_destroy\0")?;
            let add_magnet: Add = symbol(&library, b"mt_add_magnet\0")?;
            let add_file: Add = symbol(&library, b"mt_add_torrent_file\0")?;
            let list: List = symbol(&library, b"mt_list\0")?;
            let files: Files = symbol(&library, b"mt_files\0")?;
            let set_paused: Paused = symbol(&library, b"mt_set_paused\0")?;
            let move_queue: MoveQueue = symbol(&library, b"mt_move_queue\0")?;
            let set_limits: Limits = symbol(&library, b"mt_set_limits\0")?;
            let remove: Remove = symbol(&library, b"mt_remove\0")?;
            let save: Save = symbol(&library, b"mt_save\0")?;
            let state = c_path(state_directory)?;
            let download = c_path(download_directory)?;
            let mut error = [0 as c_char; 512];
            let handle = create(state.as_ptr(), download.as_ptr(), error.as_mut_ptr(), error.len() as i32);
            if handle.is_null() { return Err(error_text(&error)); }
            Ok(Self { _library: library, handle: handle as usize, destroy, add_magnet, add_file,
                list, files, set_paused, move_queue, set_limits, remove, save })
        }
    }

    fn snapshot(&self, download_directory: &Path) -> Result<TorrentSnapshot, String> {
        let mut error = [0 as c_char; 512];
        let count = unsafe { (self.list)(self.handle as *mut c_void, std::ptr::null_mut(), 0,
            error.as_mut_ptr(), error.len() as i32) };
        check(count, &error)?;
        if count > 10_000 { return Err("Too many torrents to display safely.".into()); }
        let mut rows = vec![unsafe { std::mem::zeroed::<RawTransfer>() }; count as usize];
        if count > 0 {
            let returned = unsafe { (self.list)(self.handle as *mut c_void, rows.as_mut_ptr(), count,
                error.as_mut_ptr(), error.len() as i32) };
            check(returned, &error)?;
            rows.truncate(returned.min(count) as usize);
        }
        let transfers = rows.into_iter().map(|row| {
            let size = row.size_bytes.max(0);
            let done = row.downloaded_bytes.max(0);
            let remaining = size.saturating_sub(done);
            let status = if row.paused != 0 {
                if row.auto_managed != 0 { "Queued" } else { "Paused" }
            } else {
                match row.state { 2 => "Metadata", 3 => "Downloading", 4 | 5 => "Seeding", _ => "Checking" }
            };
            TransferSnapshot {
                info_hash: char_array(&row.info_hash), name: char_array(&row.name),
                status, error: char_array(&row.error), progress: row.progress.clamp(0.0, 1.0),
                size_bytes: size, downloaded_bytes: done, uploaded_bytes: row.uploaded_bytes.max(0),
                download_rate: row.download_rate.max(0), upload_rate: row.upload_rate.max(0),
                peers: row.peers.max(0), seeds: row.seeds.max(0), queue_position: row.queue_position,
                eta_seconds: if remaining > 0 && row.download_rate > 0 {
                    Some(remaining / i64::from(row.download_rate))
                } else { None },
            }
        }).collect();
        Ok(TorrentSnapshot { engine: "libtorrent", download_directory: download_directory.to_string_lossy().into_owned(), transfers })
    }

    fn completed_files(&self, hash: &str) -> Result<Vec<PathBuf>, String> {
        let hash = CString::new(hash).map_err(|_| "Invalid torrent ID.".to_owned())?;
        let mut error = [0 as c_char; 512];
        let count = unsafe { (self.files)(self.handle as *mut c_void, hash.as_ptr(), std::ptr::null_mut(), 0, error.as_mut_ptr(), 512) };
        check(count, &error)?;
        if count > 100_000 { return Err("Too many torrent files to import.".into()); }
        let mut rows: Vec<RawFile> = (0..count).map(|_| unsafe { std::mem::zeroed() }).collect();
        if count > 0 {
            let returned = unsafe { (self.files)(self.handle as *mut c_void, hash.as_ptr(), rows.as_mut_ptr(), count, error.as_mut_ptr(), 512) };
            check(returned, &error)?;
            rows.truncate(returned.min(count) as usize);
        }
        let mut paths = Vec::new();
        for row in rows {
            if row.completed_bytes < row.size_bytes { continue; }
            let path = PathBuf::from(char_array(&row.path));
            let metadata = std::fs::metadata(&path).map_err(|error| format!("Completed file is not ready: {error}"))?;
            if metadata.len() < row.size_bytes.max(0) as u64 { return Err("Completed file is still being flushed to disk.".into()); }
            paths.push(path);
        }
        Ok(paths)
    }

    fn add(&self, value: &str, file: bool) -> Result<String, String> {
        let value = CString::new(value).map_err(|_| "Invalid torrent link or path.".to_owned())?;
        let mut info_hash = [0 as c_char; 65];
        let mut error = [0 as c_char; 512];
        let method = if file { self.add_file } else { self.add_magnet };
        let result = unsafe { method(self.handle as *mut c_void, value.as_ptr(), info_hash.as_mut_ptr(),
            info_hash.len() as i32, error.as_mut_ptr(), error.len() as i32) };
        check(result, &error)?;
        Ok(char_array(&info_hash))
    }

    fn paused(&self, info_hash: &str, paused: bool) -> Result<(), String> {
        let hash = CString::new(info_hash).map_err(|_| "Invalid torrent ID.".to_owned())?;
        let mut error = [0 as c_char; 512];
        let result = unsafe { (self.set_paused)(self.handle as *mut c_void, hash.as_ptr(), i32::from(paused),
            error.as_mut_ptr(), error.len() as i32) };
        check(result, &error)
    }

    fn move_queue(&self, info_hash: &str, direction: i32) -> Result<(), String> {
        let hash = CString::new(info_hash).map_err(|_| "Invalid torrent ID.".to_owned())?;
        let mut error = [0 as c_char; 512];
        let result = unsafe { (self.move_queue)(self.handle as *mut c_void, hash.as_ptr(), direction,
            error.as_mut_ptr(), error.len() as i32) };
        check(result, &error)
    }

    fn limits(&self, download: i32, upload: i32) -> Result<(), String> {
        let mut error = [0 as c_char; 512];
        let result = unsafe { (self.set_limits)(self.handle as *mut c_void, download, upload,
            error.as_mut_ptr(), error.len() as i32) };
        check(result, &error)
    }

    fn remove(&self, info_hash: &str) -> Result<(), String> {
        let hash = CString::new(info_hash).map_err(|_| "Invalid torrent ID.".to_owned())?;
        let mut error = [0 as c_char; 512];
        let result = unsafe { (self.remove)(self.handle as *mut c_void, hash.as_ptr(),
            error.as_mut_ptr(), error.len() as i32) };
        check(result, &error)
    }
}

fn char_array(bytes: &[c_char]) -> String {
    let end = bytes.iter().position(|byte| *byte == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end].iter().map(|byte| *byte as u8).collect::<Vec<_>>()).into_owned()
}

fn error_text(error: &[c_char]) -> String {
    let message = char_array(error);
    if message.is_empty() { "The libtorrent bridge failed without an error message.".into() } else { message }
}

fn check(result: i32, error: &[c_char]) -> Result<(), String> {
    if result < 0 { Err(error_text(error)) } else { Ok(()) }
}

fn c_path(path: &Path) -> Result<CString, String> {
    CString::new(path.to_string_lossy().as_bytes()).map_err(|_| "Invalid torrent directory.".into())
}

unsafe fn symbol<T: Copy>(library: &Library, name: &[u8]) -> Result<T, String> {
    library.get::<T>(name).map(|symbol| *symbol).map_err(|error| format!("The torrent bridge is missing {}: {error}",
        String::from_utf8_lossy(name).trim_end_matches('\0')))
}

fn load_bridge(resource_directory: &Path) -> Result<Library, String> {
    let mut candidates = Vec::new();
    candidates.push(resource_directory.join("torrent-engine").join("media_libtorrent_bridge.dll"));
    candidates.push(resource_directory.join("media_libtorrent_bridge.dll"));
    if let Ok(exe) = std::env::current_exe() {
        if let Some(directory) = exe.parent() {
            candidates.push(directory.join("media_libtorrent_bridge.dll"));
            candidates.push(directory.join("torrent-engine").join("media_libtorrent_bridge.dll"));
        }
    }
    if cfg!(debug_assertions) {
        candidates.push(Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..").join("native").join("libtorrent-bridge").join("build")
            .join("Release").join("media_libtorrent_bridge.dll"));
    }
    let path = candidates.into_iter().find(|candidate| candidate.is_file())
        .ok_or_else(|| "The libtorrent engine is not built or installed yet.".to_owned())?;
    let library = unsafe { Library::new(&path) }
        .map_err(|error| format!("Could not load the libtorrent engine at {}: {error}", path.display()))?;
    Ok(library)
}

#[derive(Clone)]
pub struct TorrentState {
    engine: Arc<Mutex<Option<TorrentEngine>>>,
    state_directory: PathBuf,
    download_directory: PathBuf,
    resource_directory: PathBuf,
}

impl TorrentState {
    pub fn new(app_data_directory: &Path, resource_directory: &Path) -> Self {
        let download_directory = std::env::var_os("USERPROFILE")
            .map(PathBuf::from).unwrap_or_else(|| app_data_directory.to_path_buf())
            .join("Downloads").join("Media Library");
        Self { engine: Arc::new(Mutex::new(None)),
            state_directory: app_data_directory.join("torrent-state"), download_directory,
            resource_directory: resource_directory.to_path_buf() }
    }

    pub fn start_save_worker(&self) {
        let weak = Arc::downgrade(&self.engine);
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_secs(60));
            let Some(shared) = weak.upgrade() else { break; };
            if let Ok(slot) = shared.lock() {
                if let Some(engine) = slot.as_ref() {
                    let mut error = [0 as c_char; 512];
                    let result = unsafe { (engine.save)(engine.handle as *mut c_void, error.as_mut_ptr(), error.len() as i32) };
                    if result < 0 { eprintln!("Could not save torrent state: {}", error_text(&error)); }
                }
            };
        });
    }

    pub fn start_library_worker(&self, app: tauri::AppHandle) {
        let weak = Arc::downgrade(&self.engine);
        let download_directory = self.download_directory.clone();
        std::thread::spawn(move || {
            let mut imported = std::collections::HashSet::new();
            loop {
                std::thread::sleep(Duration::from_secs(2));
                let Some(shared) = weak.upgrade() else { break; };
                let completed = {
                    let Ok(slot) = shared.lock() else { continue; };
                    let Some(engine) = slot.as_ref() else { continue; };
                    let Ok(snapshot) = engine.snapshot(&download_directory) else { continue; };
                    snapshot.transfers.into_iter().filter(|item| item.progress >= 1.0 && item.size_bytes > 0 && item.error.is_empty() && item.status != "Checking" && !imported.contains(&item.info_hash))
                        .filter_map(|item| match engine.completed_files(&item.info_hash) {
                            Ok(paths) => Some((item.info_hash, paths)),
                            Err(error) => { eprintln!("Torrent library import will retry: {error}"); None }
                        }).collect::<Vec<_>>()
                };
                if completed.is_empty() { continue; }
                let library = app.state::<media_core::LibraryState>();
                if library.scanning.compare_exchange(false, true, std::sync::atomic::Ordering::AcqRel, std::sync::atomic::Ordering::Relaxed).is_err() { continue; }
                let guard = crate::ScanGuard(library.scanning.clone());
                let mut changed = false;
                for (hash, paths) in completed {
                    match media_core::LibraryStore::open(&library.db_path).and_then(|mut store| store.import_completed_files(&download_directory, &paths)) {
                        Ok(count) => { imported.insert(hash); changed |= count > 0; }
                        Err(error) => eprintln!("Torrent library import will retry: {error}"),
                    }
                }
                if changed {
                    let _ = app.emit("library-changed", ());
                    tauri::async_runtime::block_on(crate::metadata::enrich_library(&library.db_path, app.state::<crate::tmdb::TmdbState>().inner()));
                    let _ = app.emit("library-changed", ());
                }
                drop(guard);
            }
        });
    }

    fn with_engine<T>(&self, operation: impl FnOnce(&TorrentEngine, &Path) -> Result<T, String>) -> Result<T, String> {
        let mut slot = self.engine.lock().map_err(|_| "The torrent engine is unavailable.".to_owned())?;
        if slot.is_none() { *slot = Some(TorrentEngine::open(&self.state_directory, &self.download_directory, &self.resource_directory)?); }
        operation(slot.as_ref().expect("engine was initialized"), &self.download_directory)
    }
}

async fn run<T: Send + 'static>(state: tauri::State<'_, TorrentState>,
    operation: impl FnOnce(&TorrentEngine, &Path) -> Result<T, String> + Send + 'static) -> Result<T, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.with_engine(operation)).await
        .map_err(|error| format!("Torrent worker stopped: {error}"))?
}

#[tauri::command]
pub async fn torrent_snapshot(state: tauri::State<'_, TorrentState>) -> Result<TorrentSnapshot, String> {
    run(state, |engine, path| engine.snapshot(path)).await
}

#[tauri::command]
pub async fn torrent_add_magnet(state: tauri::State<'_, TorrentState>, uri: String) -> Result<String, String> {
    run(state, move |engine, _| engine.add(&uri, false)).await
}

#[tauri::command]
pub async fn torrent_add_file(state: tauri::State<'_, TorrentState>, path: String) -> Result<String, String> {
    run(state, move |engine, _| engine.add(&path, true)).await
}

#[tauri::command]
pub async fn torrent_add_data(state: tauri::State<'_, TorrentState>, encoded: String) -> Result<String, String> {
    use base64::Engine;
    use std::io::Write;
    if encoded.len() > 6_666_668 { return Err("Torrent metadata is too large.".into()); }
    let bytes = base64::engine::general_purpose::STANDARD.decode(encoded)
        .map_err(|_| "Invalid torrent metadata.".to_owned())?;
    if bytes.is_empty() || bytes.len() > 5_000_000 || bytes[0] != b'd' { return Err("Invalid torrent metadata.".into()); }
    let directory = state.state_directory.clone();
    run(state, move |engine, _| {
        std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
        let unique = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|error| error.to_string())?.as_nanos();
        let path = directory.join(format!("incoming-{unique}.torrent"));
        let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(&path).map_err(|error| error.to_string())?;
        let result = file.write_all(&bytes).map_err(|error| error.to_string());
        drop(file);
        let result = result.and_then(|_| engine.add(&path.to_string_lossy(), true));
        let _ = std::fs::remove_file(&path);
        result
    }).await
}

#[tauri::command]
pub async fn torrent_set_paused(state: tauri::State<'_, TorrentState>, info_hash: String, paused: bool) -> Result<(), String> {
    run(state, move |engine, _| engine.paused(&info_hash, paused)).await
}

#[tauri::command]
pub async fn torrent_move_queue(state: tauri::State<'_, TorrentState>, info_hash: String, direction: i32) -> Result<(), String> {
    run(state, move |engine, _| engine.move_queue(&info_hash, direction)).await
}

#[tauri::command]
pub async fn torrent_set_limits(state: tauri::State<'_, TorrentState>, download: i32, upload: i32) -> Result<(), String> {
    run(state, move |engine, _| engine.limits(download, upload)).await
}

#[tauri::command]
pub async fn torrent_remove(state: tauri::State<'_, TorrentState>, info_hash: String) -> Result<(), String> {
    run(state, move |engine, _| engine.remove(&info_hash)).await
}

#[cfg(test)]
mod tests {
    use super::char_array;

    #[test]
    fn reads_null_terminated_native_text() {
        assert_eq!(char_array(&[65, 66, 0, 67]), "AB");
    }
}
