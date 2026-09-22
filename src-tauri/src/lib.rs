use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DesktopBootstrap {
    version: &'static str,
    platform: &'static str,
    media_core_status: &'static str,
}

#[tauri::command]
fn desktop_bootstrap() -> DesktopBootstrap {
    DesktopBootstrap {
        version: env!("CARGO_PKG_VERSION"),
        platform: "desktop",
        media_core_status: "not-configured",
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .invoke_handler(tauri::generate_handler![desktop_bootstrap])
        .run(tauri::generate_context!())
        .expect("failed to run desktop application");
}
