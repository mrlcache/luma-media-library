use std::{path::PathBuf, sync::{Mutex, atomic::{AtomicBool, Ordering}}};
use tauri::{Manager, menu::{Menu, MenuItem}, tray::TrayIconBuilder};

pub struct LowUsageState {
    enabled: AtomicBool,
    path: PathBuf,
    saving: Mutex<()>,
}

impl LowUsageState {
    pub fn enabled(&self) -> bool { self.enabled.load(Ordering::Acquire) }
}

pub fn background(app: &tauri::AppHandle) {
    if app.try_state::<LowUsageState>().is_some_and(|state| state.enabled()) {
        if let Some(window) = app.get_webview_window("main") { let _ = window.close(); }
    }
}

pub fn reopen(app: &tauri::AppHandle) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        let recreated = handle.get_webview_window("main").is_none();
        let result = match handle.get_webview_window("main") {
            Some(window) => Ok(window),
            None => handle.config().app.windows.iter().find(|config| config.label == "main")
                .ok_or_else(|| "Main window configuration is missing".to_owned())
                .and_then(|config| tauri::WebviewWindowBuilder::from_config(&handle, config)
                    .and_then(|builder| builder.build()).map_err(|error| error.to_string())),
        };
        match result {
            Ok(window) => {
                if let Some(artwork) = handle.try_state::<crate::artwork::ArtworkState>() { artwork.resume_desktop_cache(); }
                #[cfg(windows)]
                if let Ok(hwnd) = window.hwnd() {
                    let _ = crate::hss_backdrop::set_native_window_frame(hwnd.0 as usize);
                    // The previous window's material state must not suppress setup on the new HWND.
                    if recreated { let _ = crate::hss_backdrop::set_enabled(&handle.state::<crate::hss_backdrop::BackdropState>(), hwnd.0 as usize, false); }
                }
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
            Err(error) => eprintln!("Could not reopen Luma: {error}"),
        }
    });
}

fn ensure_tray(app: &tauri::AppHandle) -> Result<(), String> {
    if app.tray_by_id("luma-server").is_some() { return Ok(()); }
    let open = MenuItem::with_id(app, "open-luma", "Open Luma", true, None::<&str>).map_err(|e| e.to_string())?;
    let quit = MenuItem::with_id(app, "quit-luma", "Quit Luma", true, None::<&str>).map_err(|e| e.to_string())?;
    let menu = Menu::with_items(app, &[&open, &quit]).map_err(|e| e.to_string())?;
    let icon = app.default_window_icon().ok_or("Luma icon is unavailable")?.clone();
    TrayIconBuilder::with_id("luma-server").icon(icon).tooltip("Luma · Media server")
        .menu(&menu).show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open-luma" => reopen(app),
            "quit-luma" => app.exit(0),
            _ => {},
        }).build(app).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn setup(app: &tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let path = app.path().app_data_dir()?.join("low-usage-mode");
    let enabled = std::fs::read_to_string(&path).is_ok_and(|value| value.trim() == "1");
    // Never leave a headless server without a way to reopen or quit it.
    let enabled = enabled && ensure_tray(app).is_ok();
    app.manage(LowUsageState { enabled: AtomicBool::new(enabled), path, saving: Mutex::new(()) });
    Ok(())
}

#[tauri::command]
pub fn get_low_usage_mode(state: tauri::State<'_, LowUsageState>) -> bool { state.enabled() }

#[tauri::command]
pub fn set_low_usage_mode(app: tauri::AppHandle, state: tauri::State<'_, LowUsageState>, enabled: bool) -> Result<bool, String> {
    let _lock = state.saving.lock().map_err(|_| "The setting could not be saved")?;
    if enabled { ensure_tray(&app)?; }
    std::fs::write(&state.path, if enabled { "1" } else { "0" }).map_err(|e| format!("Could not save Low Usage Mode: {e}"))?;
    state.enabled.store(enabled, Ordering::Release);
    if !enabled { app.remove_tray_by_id("luma-server"); }
    Ok(enabled)
}
