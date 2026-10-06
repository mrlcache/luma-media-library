use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct NativePlaybackPreferences {
    _autoplay_next_episode: bool,
    _audio_language: String,
    _subtitle_language: String,
    _subtitle_font: String,
    _subtitle_size: u16,
    _subtitle_position: u8,
}

impl Default for NativePlaybackPreferences {
    fn default() -> Self { Self { _autoplay_next_episode: true, _audio_language: "system".into(), _subtitle_language: "auto".into(), _subtitle_font: "Manrope".into(), _subtitle_size: 100, _subtitle_position: 0 } }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackTrack { _id: i64, _label: String, _language: String, _selected: bool }

#[derive(Default)]
pub struct NativePlayerState;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackSnapshot {
    engine: &'static str,
    playing: bool,
    ended: bool,
    position_seconds: f64,
    duration_seconds: f64,
    volume: f64,
    muted: bool,
    rate: f64,
    audio_tracks: Vec<PlaybackTrack>,
    subtitle_tracks: Vec<PlaybackTrack>,
}

#[tauri::command]
pub fn start_native_player(_window: tauri::WebviewWindow, _media_id: i64, _engine: String, _preferences: Option<NativePlaybackPreferences>,
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
pub fn native_player_load_subtitle(_path: String, _player: tauri::State<'_, NativePlayerState>) -> Result<PlaybackSnapshot, String> {
    Err("Native playback is not available on this platform.".into())
}

#[tauri::command]
pub fn resize_native_player(_window: tauri::WebviewWindow, _player: tauri::State<'_, NativePlayerState>) -> Result<(), String> { Ok(()) }

#[tauri::command]
pub fn stop_native_player(_window: tauri::WebviewWindow, _player: tauri::State<'_, NativePlayerState>) -> Result<(), String> { Ok(()) }

impl NativePlayerState { pub fn release_desktop_session(&self) {} }
