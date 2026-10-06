use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{path::{Path, PathBuf}, process::{Command, Stdio}, time::{Duration, Instant}};
use tauri::Manager;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddedSubtitle {
    pub index: u32,
    pub label: String,
    pub language: String,
    pub supported: bool,
}

fn tool(app: &tauri::AppHandle, name: &str) -> Result<PathBuf, String> {
    let file = if cfg!(windows) { format!("{name}.exe") } else { name.to_owned() };
    let bundled = app.path().resource_dir().map_err(|e| e.to_string())?
        .join("release-service/media-server/tools").join(&file);
    if bundled.is_file() { return Ok(bundled); }
    let development = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../media-server/tools").join(file);
    if development.is_file() { return Ok(development); }
    Err(format!("{name} is unavailable; embedded subtitles could not be prepared."))
}

fn key(path: &Path) -> Result<String, String> {
    let metadata = path.metadata().map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    hash.update(path.to_string_lossy().as_bytes());
    hash.update(metadata.len().to_le_bytes());
    hash.update(metadata.modified().map_err(|e| e.to_string())?.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos().to_le_bytes());
    Ok(format!("{:x}", hash.finalize()))
}

fn run(command: &mut Command, seconds: u64) -> Result<(), String> {
    command.stdin(Stdio::null()).stderr(Stdio::null());
    #[cfg(windows)] { use std::os::windows::process::CommandExt; command.creation_flags(0x08000000); }
    let mut child = command.spawn().map_err(|e| e.to_string())?;
    let deadline = Instant::now() + Duration::from_secs(seconds);
    loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            return if status.success() { Ok(()) } else { Err("The subtitle could not be read or converted.".into()) };
        }
        if Instant::now() >= deadline { let _ = child.kill(); let _ = child.wait(); return Err("Preparing this subtitle took too long.".into()); }
        std::thread::sleep(Duration::from_millis(20));
    }
}

pub fn parse_tracks(value: &serde_json::Value) -> Vec<EmbeddedSubtitle> {
    value["streams"].as_array().into_iter().flatten().filter_map(|stream| {
        let index = u32::try_from(stream["index"].as_u64()?).ok()?;
        let codec = stream["codec_name"].as_str().unwrap_or("unknown");
        let language = stream["tags"]["language"].as_str().unwrap_or("und").to_owned();
        let title = stream["tags"]["title"].as_str().filter(|s| !s.trim().is_empty()).unwrap_or(&language);
        let mut label = format!("{title} · {language}");
        if stream["disposition"]["forced"].as_i64() == Some(1) { label.push_str(" · Forced"); }
        if stream["disposition"]["hearing_impaired"].as_i64() == Some(1) { label.push_str(" · SDH"); }
        let supported = matches!(codec, "subrip" | "srt" | "ass" | "ssa" | "webvtt" | "mov_text" | "text" | "microdvd");
        if !supported { label.push_str(&format!(" · {codec}")); }
        Some(EmbeddedSubtitle { index, label, language, supported })
    }).collect()
}

pub fn embedded(app: &tauri::AppHandle, path: &Path, cache: &Path) -> Result<Vec<EmbeddedSubtitle>, String> {
    std::fs::create_dir_all(cache).map_err(|e| e.to_string())?;
    let destination = cache.join(format!("{}-tracks.json", key(path)?));
    let value = match std::fs::read(&destination).ok().and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok()) {
        Some(value) => value,
        None => {
            let executable = tool(app, "ffprobe")?;
            let temporary = cache.join(format!("{}-probe-{}.json", key(path)?, getrandom::u64().map_err(|e| e.to_string())?));
            let output = std::fs::File::create(&temporary).map_err(|e| e.to_string())?;
            let result = run(Command::new(executable).args(["-v", "error", "-select_streams", "s", "-show_streams", "-of", "json"]).arg(path).stdout(output), 15);
            let bytes = std::fs::read(&temporary);
            let _ = std::fs::remove_file(&temporary);
            result?;
            let bytes = bytes.map_err(|e| e.to_string())?;
            if bytes.len() > 8 * 1024 * 1024 { return Err("Subtitle metadata is too large.".into()); }
            let value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
            std::fs::write(&destination, bytes).map_err(|e| e.to_string())?;
            value
        }
    };
    Ok(parse_tracks(&value))
}

pub fn convert(app: &tauri::AppHandle, path: &Path, index: Option<u32>, cache: &Path) -> Result<PathBuf, String> {
    convert_with_tool(&tool(app, "ffmpeg")?, path, index, cache)
}

fn convert_with_tool(ffmpeg: &Path, path: &Path, index: Option<u32>, cache: &Path) -> Result<PathBuf, String> {
    std::fs::create_dir_all(cache).map_err(|e| e.to_string())?;
    let destination = cache.join(format!("{}-{}.vtt", key(path)?, index.map(|n| n.to_string()).unwrap_or_else(|| "external".into())));
    if destination.is_file() { return Ok(destination); }
    let temporary = cache.join(format!("{}-{}.vtt", key(path)?, getrandom::u64().map_err(|e| e.to_string())?));
    let mut command = Command::new(ffmpeg);
    command.args(["-v", "error", "-nostdin", "-y", "-i"]).arg(path)
        .args(["-map", &index.map(|n| format!("0:{n}")).unwrap_or_else(|| "0:s:0".into()), "-c:s", "webvtt", "-f", "webvtt"])
        .arg(&temporary).stdout(Stdio::null());
    if let Err(error) = run(&mut command, 60) { let _ = std::fs::remove_file(&temporary); return Err(error); }
    let length = temporary.metadata().map_err(|e| e.to_string())?.len();
    if length > 8 * 1024 * 1024 { let _ = std::fs::remove_file(&temporary); return Err("The subtitle is too large.".into()); }
    if let Err(error) = std::fs::rename(&temporary, &destination) {
        let _ = std::fs::remove_file(&temporary);
        if !destination.is_file() { return Err(error.to_string()); }
    }
    Ok(destination)
}

#[cfg(test)] mod tests {
    #[test] fn extracts_the_selected_embedded_track_as_webvtt() {
        use super::*;
        let tools = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../media-server/tools");
        let executable = tools.join(if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" });
        assert!(executable.is_file(), "FFmpeg is required for this integration test");
        let directory = std::env::temp_dir().join(format!("luma-subtitle-test-{}", getrandom::u64().unwrap()));
        std::fs::create_dir_all(&directory).unwrap();
        let first = directory.join("en.srt"); let second = directory.join("pt.srt");
        std::fs::write(&first, "1\n00:00:00,100 --> 00:00:01,500\nHello\n").unwrap();
        std::fs::write(&second, "1\n00:00:00,100 --> 00:00:01,500\nOlá\n").unwrap();
        let movie = directory.join("fixture.mkv");
        run(Command::new(&executable).args(["-v","error","-y","-f","lavfi","-i","color=size=16x16:rate=1:duration=2","-i"])
            .arg(&first).arg("-i").arg(&second).args(["-map","0:v","-map","1:s","-map","2:s","-c:v","mpeg4","-c:s","srt"])
            .arg(&movie).stdout(Stdio::null()), 15).unwrap();
        let converted = convert_with_tool(&executable, &movie, Some(2), &directory).unwrap();
        let text = std::fs::read_to_string(&converted).unwrap();
        assert!(text.starts_with("WEBVTT")); assert!(text.contains("Olá")); assert!(!text.contains("Hello"));
        assert!(text.contains("00:00.100 --> 00:01.500"));
        assert_eq!(convert_with_tool(&executable, &movie, Some(2), &directory).unwrap(), converted);
        // Only known fixture files; remove without recursive directory deletion.
        for entry in std::fs::read_dir(&directory).unwrap().flatten() { std::fs::remove_file(entry.path()).unwrap(); }
        std::fs::remove_dir(directory).unwrap();
    }
    #[test] fn all_tracks_are_listed_including_image_and_forced_tracks() {
        let value = serde_json::json!({"streams":[
            {"index":2,"codec_name":"subrip","tags":{"language":"eng","title":"English"},"disposition":{"forced":1}},
            {"index":4,"codec_name":"ass","tags":{"language":"por"},"disposition":{"hearing_impaired":1}},
            {"index":8,"codec_name":"hdmv_pgs_subtitle","tags":{"language":"jpn"}}
        ]});
        let tracks = super::parse_tracks(&value);
        assert_eq!(tracks.len(), 3);
        assert_eq!(tracks[1].index, 4);
        assert!(tracks[0].label.contains("Forced"));
        assert!(tracks[1].label.contains("SDH"));
        assert!(tracks[1].supported);
        assert!(!tracks[2].supported);
    }
}
/// Reject oversized sidecars before allocating their entire contents.
pub fn read_bounded_subtitle(path: &std::path::Path) -> Result<Vec<u8>, String> {
    use std::io::Read;
    const LIMIT: u64 = 8 * 1024 * 1024;
    let file = std::fs::File::open(path).map_err(|error| format!("Could not read a local subtitle: {error}"))?;
    if file.metadata().map_err(|error| error.to_string())?.len() > LIMIT { return Err("The subtitle is too large.".into()); }
    let mut bytes = Vec::new();
    file.take(LIMIT + 1).read_to_end(&mut bytes).map_err(|error| error.to_string())?;
    if bytes.len() as u64 > LIMIT { return Err("The subtitle is too large.".into()); }
    Ok(bytes)
}

#[cfg(test)]
mod bounded_read_tests {
    #[test]
    fn oversized_sidecars_are_rejected_before_reading_their_contents() {
        let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let directory = std::env::temp_dir().join(format!("luma-subtitle-limit-{}-{nonce}",std::process::id()));
        std::fs::create_dir(&directory).unwrap();
        let path = directory.join("large.srt");
        let file = std::fs::File::create(&path).unwrap();
        file.set_len(8 * 1024 * 1024 + 1).unwrap();
        assert_eq!(super::read_bounded_subtitle(&path).unwrap_err(), "The subtitle is too large.");
        drop(file);
        std::fs::write(&path, b"subtitle").unwrap();
        assert_eq!(super::read_bounded_subtitle(&path).unwrap(), b"subtitle");
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }
}
