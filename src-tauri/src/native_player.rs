//! Native video engines attached to the transparent Tauri window HWND.
//! The library is loaded at runtime so an unavailable engine cannot prevent app startup.

use libloading::Library;
use serde::Serialize;
use std::{ffi::{c_char, c_void, CString}, path::{Path, PathBuf}, sync::{mpsc, Mutex, OnceLock}};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{GetStockObject, BLACK_BRUSH};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
    GetWindowRect, IsIconic, PostMessageW, PostQuitMessage, RegisterClassW,
    SetWindowPos, ShowWindow, TranslateMessage, MSG, SW_HIDE, SWP_NOACTIVATE,
    SWP_SHOWWINDOW, WM_CLOSE, WM_DESTROY, WNDCLASSW, WS_EX_NOACTIVATE,
    WS_EX_TOOLWINDOW, WS_POPUP,
};

const MPV_FORMAT_FLAG: i32 = 3;
const MPV_FORMAT_DOUBLE: i32 = 5;

type MpvCreate = unsafe extern "C" fn() -> *mut c_void;
type MpvOption = unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char) -> i32;
type MpvInitialize = unsafe extern "C" fn(*mut c_void) -> i32;
type MpvCommand = unsafe extern "C" fn(*mut c_void, *const *const c_char) -> i32;
type MpvProperty = unsafe extern "C" fn(*mut c_void, *const c_char, i32, *mut c_void) -> i32;
type MpvDestroy = unsafe extern "C" fn(*mut c_void);

struct Mpv {
    _library: Library,
    handle: usize,
    command: MpvCommand,
    get_property: MpvProperty,
    set_property: MpvProperty,
    destroy: MpvDestroy,
}

// All native engine calls are serialized by NativePlayerState's mutex. The raw
// handles belong to the library kept alive in the same struct.
unsafe impl Send for Mpv {}

impl Mpv {
    fn open(path: &Path, surface: HWND) -> Result<Self, String> {
        let library = load_engine_library("libmpv-2.dll", &["mpv", "mpv\\bin"])?;
        unsafe {
            let create: MpvCreate = symbol(&library, b"mpv_create\0")?;
            let option: MpvOption = symbol(&library, b"mpv_set_option_string\0")?;
            let initialize: MpvInitialize = symbol(&library, b"mpv_initialize\0")?;
            let command: MpvCommand = symbol(&library, b"mpv_command\0")?;
            let get_property: MpvProperty = symbol(&library, b"mpv_get_property\0")?;
            let set_property: MpvProperty = symbol(&library, b"mpv_set_property\0")?;
            let destroy: MpvDestroy = symbol(&library, b"mpv_terminate_destroy\0")?;
            let handle = create();
            if handle.is_null() { return Err("libmpv could not create a player.".into()); }
            let player = Self { _library: library, handle: handle as usize, command, get_property, set_property, destroy };
            let result = (|| {
                // mpv's Windows --wid contract takes the HWND as an unsigned 32-bit value.
                player.option(option, "wid", &format!("{}", surface as usize as u32))?;
                player.option(option, "vo", "gpu-next")?;
                player.option(option, "hwdec", "auto-safe")?;
                player.option(option, "osc", "no")?;
                player.option(option, "input-default-bindings", "no")?;
                if initialize(handle) < 0 { return Err("libmpv could not initialize its video engine.".into()); }
                player.call(&["loadfile", &path.to_string_lossy(), "replace"])?;
                Ok(())
            })();
            result.map(|()| player)
        }
    }

    fn option(&self, setter: MpvOption, name: &str, value: &str) -> Result<(), String> {
        let name = CString::new(name).map_err(|_| "Invalid mpv option.".to_owned())?;
        let value = CString::new(value).map_err(|_| "Invalid mpv option value.".to_owned())?;
        if unsafe { setter(self.handle as *mut c_void, name.as_ptr(), value.as_ptr()) } < 0 {
            return Err(format!("libmpv rejected the {} option.", name.to_string_lossy()));
        }
        Ok(())
    }

    fn call(&self, args: &[&str]) -> Result<(), String> {
        let values: Vec<CString> = args.iter().map(|arg| CString::new(*arg).map_err(|_| "Invalid mpv command.".to_owned())).collect::<Result<_, _>>()?;
        let mut pointers: Vec<*const c_char> = values.iter().map(|value| value.as_ptr()).collect();
        pointers.push(std::ptr::null());
        if unsafe { (self.command)(self.handle as *mut c_void, pointers.as_ptr()) } < 0 {
            return Err("libmpv could not complete the playback command.".into());
        }
        Ok(())
    }

    fn get_double(&self, name: &str) -> f64 {
        let mut value = 0.0f64;
        let name = CString::new(name).expect("static property name");
        let result = unsafe { (self.get_property)(self.handle as *mut c_void, name.as_ptr(), MPV_FORMAT_DOUBLE, &mut value as *mut f64 as *mut c_void) };
        if result >= 0 && value.is_finite() { value } else { 0.0 }
    }

    fn get_flag(&self, name: &str) -> bool {
        let mut value = 0i32;
        let name = CString::new(name).expect("static property name");
        unsafe { (self.get_property)(self.handle as *mut c_void, name.as_ptr(), MPV_FORMAT_FLAG, &mut value as *mut i32 as *mut c_void) >= 0 && value != 0 }
    }

    fn set_double(&self, name: &str, mut value: f64) -> Result<(), String> {
        let name = CString::new(name).expect("static property name");
        if unsafe { (self.set_property)(self.handle as *mut c_void, name.as_ptr(), MPV_FORMAT_DOUBLE, &mut value as *mut f64 as *mut c_void) } < 0 {
            return Err("libmpv could not change the playback setting.".into());
        }
        Ok(())
    }

    fn set_flag(&self, name: &str, enabled: bool) -> Result<(), String> {
        let mut value = i32::from(enabled);
        let name = CString::new(name).expect("static property name");
        if unsafe { (self.set_property)(self.handle as *mut c_void, name.as_ptr(), MPV_FORMAT_FLAG, &mut value as *mut i32 as *mut c_void) } < 0 {
            return Err("libmpv could not change the playback setting.".into());
        }
        Ok(())
    }
}

impl Drop for Mpv {
    fn drop(&mut self) { unsafe { (self.destroy)(self.handle as *mut c_void) } }
}

type VlcNew = unsafe extern "C" fn(i32, *const *const c_char) -> *mut c_void;
type VlcRelease = unsafe extern "C" fn(*mut c_void);
type VlcMediaNew = unsafe extern "C" fn(*mut c_void, *const c_char) -> *mut c_void;
type VlcPlayerNew = unsafe extern "C" fn(*mut c_void) -> *mut c_void;
type VlcSetHwnd = unsafe extern "C" fn(*mut c_void, *mut c_void);
type VlcPlay = unsafe extern "C" fn(*mut c_void) -> i32;
type VlcPause = unsafe extern "C" fn(*mut c_void, i32);
type VlcStop = unsafe extern "C" fn(*mut c_void);
type VlcGetTime = unsafe extern "C" fn(*mut c_void) -> i64;
type VlcSetTime = unsafe extern "C" fn(*mut c_void, i64);
type VlcIsPlaying = unsafe extern "C" fn(*mut c_void) -> i32;
type VlcGetRate = unsafe extern "C" fn(*mut c_void) -> f32;
type VlcSetRate = unsafe extern "C" fn(*mut c_void, f32) -> i32;
type VlcGetVolume = unsafe extern "C" fn(*mut c_void) -> i32;
type VlcSetVolume = unsafe extern "C" fn(*mut c_void, i32) -> i32;
type VlcGetMute = unsafe extern "C" fn(*mut c_void) -> i32;
type VlcSetMute = unsafe extern "C" fn(*mut c_void, i32);

struct Vlc {
    _library: Library,
    instance: usize,
    player: usize,
    release_instance: VlcRelease,
    release_player: VlcRelease,
    stop: VlcStop,
    play: VlcPlay,
    pause: VlcPause,
    get_time: VlcGetTime,
    get_length: VlcGetTime,
    set_time: VlcSetTime,
    is_playing: VlcIsPlaying,
    get_rate: VlcGetRate,
    set_rate: VlcSetRate,
    get_volume: VlcGetVolume,
    set_volume: VlcSetVolume,
    get_mute: VlcGetMute,
    set_mute: VlcSetMute,
}

unsafe impl Send for Vlc {}

impl Vlc {
    fn open(path: &Path, surface: HWND) -> Result<Self, String> {
        let library = load_engine_library("libvlc.dll", &["VideoLAN\\VLC"])?;
        unsafe {
            let new: VlcNew = symbol(&library, b"libvlc_new\0")?;
            let release_instance: VlcRelease = symbol(&library, b"libvlc_release\0")?;
            let media_new: VlcMediaNew = symbol(&library, b"libvlc_media_new_path\0")?;
            let media_release: VlcRelease = symbol(&library, b"libvlc_media_release\0")?;
            let player_new: VlcPlayerNew = symbol(&library, b"libvlc_media_player_new_from_media\0")?;
            let release_player: VlcRelease = symbol(&library, b"libvlc_media_player_release\0")?;
            let set_hwnd: VlcSetHwnd = symbol(&library, b"libvlc_media_player_set_hwnd\0")?;
            let stop: VlcStop = symbol(&library, b"libvlc_media_player_stop\0")?;
            let play: VlcPlay = symbol(&library, b"libvlc_media_player_play\0")?;
            let pause: VlcPause = symbol(&library, b"libvlc_media_player_set_pause\0")?;
            let get_time: VlcGetTime = symbol(&library, b"libvlc_media_player_get_time\0")?;
            let get_length: VlcGetTime = symbol(&library, b"libvlc_media_player_get_length\0")?;
            let set_time: VlcSetTime = symbol(&library, b"libvlc_media_player_set_time\0")?;
            let is_playing: VlcIsPlaying = symbol(&library, b"libvlc_media_player_is_playing\0")?;
            let get_rate: VlcGetRate = symbol(&library, b"libvlc_media_player_get_rate\0")?;
            let set_rate: VlcSetRate = symbol(&library, b"libvlc_media_player_set_rate\0")?;
            let get_volume: VlcGetVolume = symbol(&library, b"libvlc_audio_get_volume\0")?;
            let set_volume: VlcSetVolume = symbol(&library, b"libvlc_audio_set_volume\0")?;
            let get_mute: VlcGetMute = symbol(&library, b"libvlc_audio_get_mute\0")?;
            let set_mute: VlcSetMute = symbol(&library, b"libvlc_audio_set_mute\0")?;
            let instance = new(0, std::ptr::null());
            if instance.is_null() { return Err("libVLC could not initialize. Check the VLC installation.".into()); }
            // std::fs::canonicalize returns a verbatim Windows path (\\?\C:\...).
            // libVLC treats that prefix as an invalid file://?/C: URL, even though
            // the same file opens when passed as an ordinary drive or UNC path.
            let path = CString::new(vlc_media_path(path)).map_err(|_| "Invalid media path.".to_owned())?;
            let media = media_new(instance, path.as_ptr());
            if media.is_null() { release_instance(instance); return Err("libVLC could not open this media file.".into()); }
            let player = player_new(media);
            media_release(media);
            if player.is_null() { release_instance(instance); return Err("libVLC could not create a player.".into()); }
            set_hwnd(player, surface);
            let engine = Self { _library: library, instance: instance as usize, player: player as usize, release_instance, release_player, stop, play, pause, get_time, get_length, set_time, is_playing, get_rate, set_rate, get_volume, set_volume, get_mute, set_mute };
            if play(player) < 0 { return Err("libVLC could not start playback.".into()); }
            Ok(engine)
        }
    }
}

impl Drop for Vlc {
    fn drop(&mut self) {
        unsafe {
            (self.stop)(self.player as *mut c_void);
            (self.release_player)(self.player as *mut c_void);
            (self.release_instance)(self.instance as *mut c_void);
        }
    }
}

enum Engine { Mpv(Mpv), Vlc(Vlc) }

impl Engine {
    fn snapshot(&self) -> PlaybackSnapshot {
        match self {
            Self::Mpv(engine) => PlaybackSnapshot {
                engine: "mpv", playing: engine.get_double("duration") > 0.0
                    && !engine.get_flag("idle-active") && !engine.get_flag("pause"),
                position_seconds: engine.get_double("time-pos"), duration_seconds: engine.get_double("duration"),
                volume: engine.get_double("volume"), muted: engine.get_flag("mute"), rate: engine.get_double("speed"),
            },
            Self::Vlc(engine) => unsafe {
                let player = engine.player as *mut c_void;
                PlaybackSnapshot {
                    engine: "vlc", playing: (engine.is_playing)(player) != 0,
                    position_seconds: ((engine.get_time)(player).max(0) as f64) / 1000.0,
                    duration_seconds: ((engine.get_length)(player).max(0) as f64) / 1000.0,
                    volume: (engine.get_volume)(player).max(0) as f64,
                    muted: (engine.get_mute)(player) != 0,
                    rate: (engine.get_rate)(player) as f64,
                }
            },
        }
    }

    fn action(&self, action: &str, value: Option<f64>) -> Result<(), String> {
        let number = value.filter(|v| v.is_finite()).unwrap_or(0.0);
        match self {
            Self::Mpv(engine) => match action {
                "play" => engine.set_flag("pause", false),
                "pause" => engine.set_flag("pause", true),
                "seek" => engine.set_double("time-pos", number.max(0.0)),
                "volume" => engine.set_double("volume", number.clamp(0.0, 100.0)),
                "mute" => engine.set_flag("mute", number != 0.0),
                "rate" => engine.set_double("speed", number.clamp(0.5, 3.0)),
                _ => Err("Unknown playback action.".into()),
            },
            Self::Vlc(engine) => unsafe {
                let player = engine.player as *mut c_void;
                match action {
                    "play" => if (engine.play)(player) < 0 { Err("VLC could not play.".into()) } else { Ok(()) },
                    "pause" => { (engine.pause)(player, 1); Ok(()) },
                    "seek" => { (engine.set_time)(player, (number.max(0.0) * 1000.0) as i64); Ok(()) },
                    "volume" => if (engine.set_volume)(player, number.clamp(0.0, 100.0) as i32) < 0 { Err("VLC could not change volume.".into()) } else { Ok(()) },
                    "mute" => { (engine.set_mute)(player, i32::from(number != 0.0)); Ok(()) },
                    "rate" => if (engine.set_rate)(player, number.clamp(0.5, 3.0) as f32) < 0 { Err("VLC could not change speed.".into()) } else { Ok(()) },
                    _ => Err("Unknown playback action.".into()),
                }
            },
        }
    }
}

#[derive(Clone, Serialize)]
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

struct NativeSession { engine: Engine, surface: VideoSurface, parent: usize }

const VIDEO_CLASS: &[u16] = &['M' as u16, 'e' as u16, 'd' as u16, 'i' as u16,
    'a' as u16, 'V' as u16, 'i' as u16, 'd' as u16, 'e' as u16, 'o' as u16, 0];

unsafe extern "system" fn video_window_proc(hwnd: HWND, message: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    match message {
        WM_CLOSE => { DestroyWindow(hwnd); 0 }
        WM_DESTROY => { PostQuitMessage(0); 0 }
        _ => DefWindowProcW(hwnd, message, w, l),
    }
}

fn register_video_window() -> Result<(), String> {
    static REGISTERED: OnceLock<bool> = OnceLock::new();
    if *REGISTERED.get_or_init(|| unsafe {
        let class = WNDCLASSW {
            style: 0, lpfnWndProc: Some(video_window_proc), cbClsExtra: 0, cbWndExtra: 0,
            hInstance: GetModuleHandleW(std::ptr::null()), hIcon: std::ptr::null_mut(),
            hCursor: std::ptr::null_mut(), hbrBackground: GetStockObject(BLACK_BRUSH),
            lpszMenuName: std::ptr::null(), lpszClassName: VIDEO_CLASS.as_ptr(),
        };
        RegisterClassW(&class) != 0
    }) { Ok(()) } else { Err("Could not create the native video surface.".into()) }
}

struct VideoSurface { hwnd: usize }

impl VideoSurface {
    fn new(parent: HWND) -> Result<Self, String> {
        register_video_window()?;
        let mut rect = RECT { left: 0, top: 0, right: 0, bottom: 0 };
        if unsafe { GetWindowRect(parent, &mut rect) } == 0 {
            return Err("Could not position the native video surface.".into());
        }
        let (tx, rx) = mpsc::sync_channel(1);
        std::thread::Builder::new().name("media-video-window".into()).spawn(move || unsafe {
            let hwnd = CreateWindowExW(WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                VIDEO_CLASS.as_ptr(), std::ptr::null(), WS_POPUP,
                rect.left, rect.top, rect.right - rect.left, rect.bottom - rect.top,
                std::ptr::null_mut(), std::ptr::null_mut(),
                GetModuleHandleW(std::ptr::null()), std::ptr::null());
            let created = !hwnd.is_null();
            let _ = tx.send(if created { Ok(hwnd as usize) } else { Err("Could not open the native video window.".to_owned()) });
            if created {
                let mut message: MSG = std::mem::zeroed();
                while GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) > 0 {
                    TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
            }
        }).map_err(|error| format!("Could not start the native video surface: {error}"))?;
        let hwnd = rx.recv().map_err(|_| "The native video surface stopped unexpectedly.".to_owned())??;
        let surface = Self { hwnd };
        surface.sync(parent);
        Ok(surface)
    }

    fn hwnd(&self) -> HWND { self.hwnd as HWND }

    fn sync(&self, parent: HWND) {
        unsafe {
            if IsIconic(parent) != 0 { ShowWindow(self.hwnd(), SW_HIDE); return; }
            let mut rect = RECT { left: 0, top: 0, right: 0, bottom: 0 };
            if GetWindowRect(parent, &mut rect) != 0 {
                // Separate top-level windows avoid WebView2's native-child airspace:
                // the transparent UI remains above the video and owns the controls.
                SetWindowPos(self.hwnd(), parent, rect.left, rect.top,
                    rect.right - rect.left, rect.bottom - rect.top,
                    SWP_NOACTIVATE | SWP_SHOWWINDOW);
            }
        }
    }
}

impl Drop for VideoSurface {
    fn drop(&mut self) { unsafe { let _ = PostMessageW(self.hwnd(), WM_CLOSE, 0, 0); } }
}

fn vlc_media_path(path: &Path) -> String {
    let path = path.to_string_lossy();
    if let Some(unc) = path.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{unc}")
    } else if let Some(drive) = path.strip_prefix(r"\\?\") {
        drive.to_owned()
    } else {
        path.into_owned()
    }
}

#[derive(Default)]
pub struct NativePlayerState(Mutex<Option<NativeSession>>);

fn engine_paths(name: &str, folders: &[&str]) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() { paths.push(dir.join(name)); paths.push(dir.join("player-engines").join(name)); }
    }
    for variable in ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"] {
        if let Some(root) = std::env::var_os(variable).map(PathBuf::from) {
            for folder in folders { paths.push(root.join(folder).join(name)); }
        }
    }
    paths
}

fn load_engine_library(name: &str, folders: &[&str]) -> Result<Library, String> {
    let path = engine_paths(name, folders).into_iter().find(|path| path.is_file())
        .ok_or_else(|| format!("{name} was not found. Install the engine or put its DLL in the app's player-engines folder."))?;
    // Search dependencies next to the selected DLL without changing the process-wide DLL path.
    let library = unsafe { libloading::os::windows::Library::load_with_flags(&path, 0x00000100 | 0x00001000) }
        .map_err(|error| format!("Could not load {}: {error}", path.display()))?;
    Ok(library.into())
}

unsafe fn symbol<T: Copy>(library: &Library, name: &[u8]) -> Result<T, String> {
    library.get::<T>(name).map(|symbol| *symbol)
        .map_err(|error| format!("The installed player library is missing {}: {error}", String::from_utf8_lossy(name).trim_end_matches('\0')))
}

fn player_window_handle(window: &tauri::WebviewWindow) -> Result<HWND, String> {
    window.hwnd().map(|handle| handle.0).map_err(|error| format!("Could not access the app window: {error}"))
}

#[tauri::command]
pub fn start_native_player(window: tauri::WebviewWindow, media_id: i64, engine: String,
    library: tauri::State<'_, media_core::LibraryState>, player: tauri::State<'_, NativePlayerState>) -> Result<PlaybackSnapshot, String> {
    if engine != "mpv" && engine != "vlc" { return Err("Choose mpv or VLC.".into()); }
    let mut store = media_core::LibraryStore::open(&library.db_path)?;
    let path = store.resolve_media_path(media_id)?.ok_or_else(|| "This media file is no longer in the library.".to_owned())?;
    let mut slot = player.0.lock().map_err(|_| "The player is unavailable.".to_owned())?;
    if let Some(previous) = slot.take() { drop(previous); }
    let parent = player_window_handle(&window)?;
    let surface = VideoSurface::new(parent)?;
    let opened = match engine.as_str() {
        "mpv" => Mpv::open(&path, surface.hwnd()).map(Engine::Mpv),
        _ => Vlc::open(&path, surface.hwnd()).map(Engine::Vlc),
    };
    let engine = opened?;
    let snapshot = engine.snapshot();
    *slot = Some(NativeSession { engine, surface, parent: parent as usize });
    store.record_playback_activity(media_id)?;
    Ok(snapshot)
}

#[tauri::command]
pub fn native_player_status(player: tauri::State<'_, NativePlayerState>) -> Result<PlaybackSnapshot, String> {
    let slot = player.0.lock().map_err(|_| "The player is unavailable.".to_owned())?;
    slot.as_ref().map(|session| {
        session.surface.sync(session.parent as HWND);
        session.engine.snapshot()
    }).ok_or_else(|| "No media is playing.".into())
}

#[tauri::command]
pub fn native_player_action(player: tauri::State<'_, NativePlayerState>, action: String, value: Option<f64>) -> Result<PlaybackSnapshot, String> {
    let slot = player.0.lock().map_err(|_| "The player is unavailable.".to_owned())?;
    let session = slot.as_ref().ok_or_else(|| "No media is playing.".to_owned())?;
    session.engine.action(&action, value)?;
    Ok(session.engine.snapshot())
}

#[tauri::command]
pub fn resize_native_player(window: tauri::WebviewWindow, player: tauri::State<'_, NativePlayerState>) -> Result<(), String> {
    let slot = player.0.lock().map_err(|_| "The player is unavailable.".to_owned())?;
    if let Some(session) = slot.as_ref() { session.surface.sync(player_window_handle(&window)?); }
    Ok(())
}

#[tauri::command]
pub fn stop_native_player(_window: tauri::WebviewWindow, player: tauri::State<'_, NativePlayerState>) -> Result<(), String> {
    let mut slot = player.0.lock().map_err(|_| "The player is unavailable.".to_owned())?;
    if let Some(session) = slot.take() { drop(session); }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::vlc_media_path;
    use std::path::Path;

    #[test]
    fn vlc_accepts_canonical_windows_drive_and_unc_paths() {
        assert_eq!(vlc_media_path(Path::new(r"\\?\C:\Videos\movie.mkv")), r"C:\Videos\movie.mkv");
        assert_eq!(vlc_media_path(Path::new(r"\\?\UNC\server\share\movie.mkv")), r"\\server\share\movie.mkv");
    }
}
