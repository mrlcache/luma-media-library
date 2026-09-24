use media_core::{
    CatalogPage, ContinueWatchingItem, LibraryScanSummary, LibraryState, LibraryStatus,
    LibraryStore, LocalTitleDetail, ScanSummary,
};
use serde::Serialize;
use std::sync::atomic::Ordering;
use tauri::Manager;

mod hss_backdrop;
mod metadata;
mod opensubtitles;
mod tmdb;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DesktopBootstrap {
    version: &'static str,
    platform: &'static str,
    media_core_status: &'static str,
    native_window_frame: bool,
    window_shape: &'static str,
}

#[tauri::command]
fn desktop_bootstrap(frame_state: tauri::State<'_, NativeWindowFrameState>) -> DesktopBootstrap {
    DesktopBootstrap {
        version: env!("CARGO_PKG_VERSION"),
        platform: "desktop",
        media_core_status: "library-index-ready",
        native_window_frame: frame_state.0.load(Ordering::Relaxed),
        window_shape: "system",
    }
}

#[derive(Default)]
struct NativeWindowFrameState(std::sync::atomic::AtomicBool);

#[tauri::command]
fn get_library_status(state: tauri::State<'_, LibraryState>) -> Result<LibraryStatus, String> {
    let store = LibraryStore::open(&state.db_path)?;
    store.status(state.scanning.load(Ordering::Relaxed))
}

#[tauri::command]
fn get_catalog_page(
    offset: u32,
    count: u32,
    kind: Option<String>,
    query: Option<String>,
    sort: Option<String>,
    state: tauri::State<'_, LibraryState>,
) -> Result<CatalogPage, String> {
    let mut store = LibraryStore::open(&state.db_path)?;
    store.catalog_filtered_page(
        offset,
        count,
        kind.as_deref(),
        query.as_deref(),
        sort.as_deref(),
    )
}

#[tauri::command]
fn get_local_title_detail(
    media_id: i64,
    state: tauri::State<'_, LibraryState>,
    app: tauri::AppHandle,
) -> Result<Option<LocalTitleDetail>, String> {
    let store = LibraryStore::open(&state.db_path)?;
    let Some(detail) = store.catalog_detail(media_id)? else {
        return Ok(None);
    };
    let scope = app.asset_protocol_scope();
    for file in &detail.files {
        scope
            .allow_file(std::path::Path::new(&file.path))
            .map_err(|error| format!("Could not authorize an episode preview: {error}"))?;
    }
    Ok(Some(detail))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SubtitleSource {
    label: String,
    path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ResolvedMediaFile {
    path: String,
    subtitles: Vec<SubtitleSource>,
    resume_position_seconds: f64,
}

#[tauri::command]
fn resolve_media_file(
    media_id: i64,
    state: tauri::State<'_, LibraryState>,
    app: tauri::AppHandle,
) -> Result<ResolvedMediaFile, String> {
    let store = LibraryStore::open(&state.db_path)?;
    let path = store.resolve_media_path(media_id)?.ok_or_else(|| {
        "This media file is no longer in the library. Refresh the library and try again.".to_owned()
    })?;
    let scope = app.asset_protocol_scope();
    scope
        .allow_file(&path)
        .map_err(|error| format!("Could not authorize playback for this media file: {error}"))?;

    let subtitle_dir = state
        .db_path
        .parent()
        .unwrap_or(&state.db_path)
        .join("subtitle-cache");
    let mut subtitles = Vec::new();
    for (index, subtitle) in store.subtitle_files(media_id)?.into_iter().enumerate() {
        let Some(file_name) = subtitle.file_name() else {
            continue;
        };
        let label = file_name.to_string_lossy().to_string();
        let playback_path = if subtitle
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("srt"))
        {
            let contents = std::fs::read(&subtitle)
                .map_err(|error| format!("Could not read a local subtitle: {error}"))?;
            if contents.len() > 8 * 1024 * 1024 {
                continue;
            }
            let text = String::from_utf8_lossy(&contents);
            let converted = srt_to_webvtt(text.trim_start_matches('\u{feff}'));
            std::fs::create_dir_all(&subtitle_dir)
                .map_err(|error| format!("Could not prepare the subtitle cache: {error}"))?;
            let converted_path = subtitle_dir.join(format!("local-{media_id}-{index}.vtt"));
            std::fs::write(&converted_path, converted)
                .map_err(|error| format!("Could not prepare a local subtitle: {error}"))?;
            converted_path
        } else {
            subtitle
        };
        scope
            .allow_file(&playback_path)
            .map_err(|error| format!("Could not authorize a subtitle for playback: {error}"))?;
        subtitles.push(SubtitleSource {
            label,
            path: playback_path.to_string_lossy().to_string(),
        });
    }

    Ok(ResolvedMediaFile {
        path: path.to_string_lossy().to_string(),
        subtitles,
        resume_position_seconds: store.playback_position(media_id)?.unwrap_or(0.0),
    })
}

fn srt_to_webvtt(contents: &str) -> String {
    let mut converted = String::from("WEBVTT\n\n");
    for line in contents.lines() {
        let line = line.trim_end_matches('\r');
        if line.contains("-->") {
            converted.push_str(&line.replace(',', "."));
        } else {
            converted.push_str(line);
        }
        converted.push('\n');
    }
    converted
}

#[tauri::command]
fn save_playback_progress(
    media_id: i64,
    position_seconds: f64,
    duration_seconds: f64,
    state: tauri::State<'_, LibraryState>,
) -> Result<(), String> {
    LibraryStore::open(&state.db_path)?.save_playback_progress(
        media_id,
        position_seconds,
        duration_seconds,
    )
}

#[tauri::command]
fn open_media_in_system_player(
    media_id: i64,
    state: tauri::State<'_, LibraryState>,
) -> Result<(), String> {
    let store = LibraryStore::open(&state.db_path)?;
    let path = store
        .resolve_media_path(media_id)?
        .ok_or_else(|| "This media file is no longer in the library.".to_owned())?;

    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        let operation: Vec<u16> = "open".encode_utf16().chain(std::iter::once(0)).collect();
        let file: Vec<u16> = path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let result = unsafe {
            windows_sys::Win32::UI::Shell::ShellExecuteW(
                std::ptr::null_mut(),
                operation.as_ptr(),
                file.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL,
            )
        };
        if (result as isize) <= 32 {
            return Err(
                "Windows could not open this file in the configured desktop player.".to_owned(),
            );
        }
    }

    #[cfg(target_os = "macos")]
    std::process::Command::new("open")
        .arg(&path)
        .spawn()
        .map_err(|_| "Could not open the file in the configured desktop player.".to_owned())?;

    #[cfg(all(unix, not(target_os = "macos")))]
    std::process::Command::new("xdg-open")
        .arg(&path)
        .spawn()
        .map_err(|_| "Could not open the file in the configured desktop player.".to_owned())?;

    Ok(())
}

#[tauri::command]
fn get_continue_watching(
    count: u32,
    state: tauri::State<'_, LibraryState>,
) -> Result<Vec<ContinueWatchingItem>, String> {
    LibraryStore::open(&state.db_path)?.continue_watching(count)
}

#[tauri::command]
fn set_opensubtitles_api_key(
    api_key: String,
    state: tauri::State<'_, opensubtitles::OpenSubtitlesState>,
) -> Result<(), String> {
    state.set_api_key(&api_key)
}

#[tauri::command]
async fn login_opensubtitles(
    username: String,
    password: String,
    state: tauri::State<'_, opensubtitles::OpenSubtitlesState>,
) -> Result<(), String> {
    state.login(&username, &password).await
}

#[tauri::command]
async fn search_opensubtitles(
    query: String,
    language: String,
    year: Option<u16>,
    kind: String,
    state: tauri::State<'_, opensubtitles::OpenSubtitlesState>,
) -> Result<Vec<opensubtitles::SubtitleSearchResult>, String> {
    state.search(&query, &language, year, &kind).await
}

#[tauri::command]
async fn download_opensubtitle(
    media_id: i64,
    file_id: i64,
    state: tauri::State<'_, opensubtitles::OpenSubtitlesState>,
    app: tauri::AppHandle,
) -> Result<String, String> {
    state.download(media_id, file_id, &app).await
}

#[tauri::command]
async fn test_tmdb_connection(state: tauri::State<'_, tmdb::TmdbState>) -> Result<(), String> {
    state.test_connection().await
}

#[tauri::command]
async fn search_tmdb(
    query: String,
    state: tauri::State<'_, tmdb::TmdbState>,
) -> Result<Vec<tmdb::TmdbSearchResult>, String> {
    state.search(&query).await
}

#[tauri::command]
async fn get_title_trailer(
    media_id: i64,
    library: tauri::State<'_, LibraryState>,
    tmdb: tauri::State<'_, tmdb::TmdbState>,
) -> Result<Option<tmdb::TmdbTrailer>, String> {
    let path = library.db_path.clone();
    let target = tauri::async_runtime::spawn_blocking(move ||
        LibraryStore::open(&path)?.tmdb_target(media_id)
    ).await.map_err(|_| "Could not read the local title metadata.".to_owned())??;
    let Some((id, kind)) = target else { return Ok(None) };
    tmdb.trailer_for_kind(id, &kind).await
}

#[tauri::command]
async fn get_title_logo(
    media_id: i64,
    library: tauri::State<'_, LibraryState>,
    tmdb: tauri::State<'_, tmdb::TmdbState>,
) -> Result<Option<String>, String> {
    let path = library.db_path.clone();
    let target = tauri::async_runtime::spawn_blocking(move ||
        LibraryStore::open(&path)?.tmdb_target(media_id)
    ).await.map_err(|_| "Could not read the local title metadata.".to_owned())??;
    let Some((id, kind)) = target else { return Ok(None) };
    tmdb.logo_for_kind(id, &kind).await
}

#[tauri::command]
async fn scan_library(
    root_path: String,
    state: tauri::State<'_, LibraryState>,
    tmdb: tauri::State<'_, tmdb::TmdbState>,
) -> Result<ScanSummary, String> {
    if state
        .scanning
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
        .is_err()
    {
        return Err("A library scan is already in progress.".to_owned());
    }

    let _scan_guard = ScanGuard(state.scanning.clone());
    let db_path = state.db_path.clone();
    let scan_path = db_path.clone();
    let mut summary = tauri::async_runtime::spawn_blocking(move || {
        let mut store = LibraryStore::open(&scan_path)?;
        store.scan_root(&root_path)
    })
    .await
    .map_err(|error| format!("The library scan could not finish: {error}"))??;
    let report = metadata::enrich_library(&db_path, tmdb.inner()).await;
    summary.matched_count = report.matched_count;
    summary.unmatched_count = report.unmatched_count;
    summary.pending_count = report.pending_count;
    summary.metadata_error = report.error;
    Ok(summary)
}

#[tauri::command]
async fn rescan_library(
    state: tauri::State<'_, LibraryState>,
    tmdb: tauri::State<'_, tmdb::TmdbState>,
) -> Result<LibraryScanSummary, String> {
    if state
        .scanning
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
        .is_err()
    {
        return Err("A library scan is already in progress.".to_owned());
    }

    let _scan_guard = ScanGuard(state.scanning.clone());
    let db_path = state.db_path.clone();
    let scan_path = db_path.clone();
    let mut summary = tauri::async_runtime::spawn_blocking(move || {
        let mut store = LibraryStore::open(&scan_path)?;
        let summary = store.rescan_roots()?;
        store.retry_unmatched_metadata()?;
        Ok::<_, String>(summary)
    })
    .await
    .map_err(|error| format!("The library refresh could not finish: {error}"))??;
    let report = metadata::enrich_library(&db_path, tmdb.inner()).await;
    summary.matched_count = report.matched_count;
    summary.unmatched_count = report.unmatched_count;
    summary.pending_count = report.pending_count;
    summary.metadata_error = report.error;
    Ok(summary)
}

#[tauri::command]
async fn set_hss_acrylic_enabled(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, hss_backdrop::BackdropState>,
    enabled: bool,
) -> Result<(), String> {
    #[cfg(windows)]
    {
        let main_hwnd = window
            .hwnd()
            .map_err(|error| format!("Could not access the app window: {error}"))?
            .0 as usize;
        let backdrop_state = state.inner().clone();
        let (result_sender, result_receiver) = std::sync::mpsc::sync_channel(1);
        window
            .run_on_main_thread(move || {
                let _ = result_sender.send(hss_backdrop::set_enabled(
                    &backdrop_state,
                    main_hwnd,
                    enabled,
                ));
            })
            .map_err(|error| format!("Could not update the native HSS backdrop: {error}"))?;
        result_receiver
            .recv_timeout(std::time::Duration::from_secs(2))
            .map_err(|error| format!("The native HSS backdrop did not respond: {error}"))??;
    }

    #[cfg(not(windows))]
    {
        let _ = (window, state, enabled);
    }

    Ok(())
}

struct ScanGuard(std::sync::Arc<std::sync::atomic::AtomicBool>);

impl Drop for ScanGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&app_data_dir)?;
            let db_path = app_data_dir.join("media-library.sqlite3");
            LibraryStore::open(&db_path).map_err(std::io::Error::other)?;
            app.manage(LibraryState {
                db_path,
                scanning: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            });
            app.manage(
                tmdb::TmdbState::new(app_data_dir.join("tmdb_read_access_token"))
                    .map_err(std::io::Error::other)?,
            );
            app.manage(
                opensubtitles::OpenSubtitlesState::new(app_data_dir.clone())
                    .map_err(std::io::Error::other)?,
            );
            app.manage(hss_backdrop::BackdropState::default());

            let frame_state = NativeWindowFrameState::default();
            #[cfg(windows)]
            if let Some(window) = app.get_webview_window("main") {
                match window.hwnd() {
                    Ok(hwnd) => match hss_backdrop::set_native_window_frame(hwnd.0 as usize) {
                        Ok(()) => frame_state.0.store(true, Ordering::Relaxed),
                        Err(error) => eprintln!("Native window frame unavailable: {error}"),
                    },
                    Err(error) => eprintln!("Could not access the main window frame: {error}"),
                }
            }
            app.manage(frame_state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            desktop_bootstrap,
            get_library_status,
            get_catalog_page,
            get_local_title_detail,
            resolve_media_file,
            save_playback_progress,
            open_media_in_system_player,
            get_continue_watching,
            set_opensubtitles_api_key,
            login_opensubtitles,
            search_opensubtitles,
            download_opensubtitle,
            test_tmdb_connection,
            search_tmdb,
            get_title_trailer,
            get_title_logo,
            scan_library,
            rescan_library,
            set_hss_acrylic_enabled
        ])
        .run(tauri::generate_context!())
        .expect("failed to run desktop application");
}

#[cfg(test)]
mod player_tests {
    use super::srt_to_webvtt;

    #[test]
    fn converts_srt_timestamps_without_changing_dialogue_punctuation() {
        let converted = srt_to_webvtt("1\r\n00:00:01,250 --> 00:00:02,500\r\nHello, friend.\r\n");
        assert_eq!(
            converted,
            "WEBVTT\n\n1\n00:00:01.250 --> 00:00:02.500\nHello, friend.\n"
        );
    }
}
