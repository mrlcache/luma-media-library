use serde::Serialize;

#[derive(Default)]
pub struct NativePlayerState;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackSnapshot {
    engine: &'static str,
    playing: bool,
    position_seconds: f64,
    duration_seconds: f64,
    volume: f64,
    muted: bool,
    rate: f64,
}

#[tauri::command]
pub fn start_native_player(_window: tauri::WebviewWindow, _media_id: i64, _engine: String,
    _library: tauri::State<'_, media_core::LibraryState>, _player: tauri::State<'_, NativePlayerState>) -> Result<PlaybackSnapshot, String> {
    Err("Native mpv and VLC playback is currently available on Windows.".into())
}

#[tauri::command]
pub fn native_player_status(_player: tauri::State<'_, NativePlayerState>) -> Result<PlaybackSnapshot, String> {
    Err("Native playback is not available on this platform.".into())
}

#[tauri::command]
pub fn native_player_action(_player: tauri::State<'_, NativePlayerState>, _action: String, _value: Option<f64>) -> Result<PlaybackSnapshot, String> {
    Err("Native playback is not available on this platform.".into())
}

#[tauri::command]
pub fn resize_native_player(_window: tauri::WebviewWindow, _player: tauri::State<'_, NativePlayerState>) -> Result<(), String> { Ok(()) }

#[tauri::command]
pub fn stop_native_player(_window: tauri::WebviewWindow, _player: tauri::State<'_, NativePlayerState>) -> Result<(), String> { Ok(()) }
