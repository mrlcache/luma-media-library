use media_core::{CatalogPage, LibraryState, LibraryStatus, LibraryStore, ScanSummary};
use serde::Serialize;
use std::sync::atomic::Ordering;
use tauri::Manager;

mod hss_backdrop;

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
    state: tauri::State<'_, LibraryState>,
) -> Result<CatalogPage, String> {
    let mut store = LibraryStore::open(&state.db_path)?;
    store.catalog_page(offset, count)
}

#[tauri::command]
async fn scan_library(
    root_path: String,
    state: tauri::State<'_, LibraryState>,
) -> Result<ScanSummary, String> {
    if state
        .scanning
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
        .is_err()
    {
        return Err("A library scan is already in progress.".to_owned());
    }

    let db_path = state.db_path.clone();
    let scanning = state.scanning.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _scan_guard = ScanGuard(scanning);
        let mut store = LibraryStore::open(&db_path)?;
        store.scan_root(&root_path)
    })
    .await
    .map_err(|error| format!("The library scan could not finish: {error}"))?
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
            scan_library,
            set_hss_acrylic_enabled
        ])
        .run(tauri::generate_context!())
        .expect("failed to run desktop application");
}
