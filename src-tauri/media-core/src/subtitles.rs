use std::path::{Path, PathBuf};

pub fn decode_subtitle_text(bytes: &[u8]) -> String {
    if bytes.starts_with(&[0xff, 0xfe]) || bytes.starts_with(&[0xfe, 0xff]) {
        let little = bytes[0] == 0xff;
        let units = bytes[2..].chunks_exact(2).map(|p| if little { u16::from_le_bytes([p[0], p[1]]) } else { u16::from_be_bytes([p[0], p[1]]) }).collect::<Vec<_>>();
        return String::from_utf16_lossy(&units);
    }
    if let Ok(text) = std::str::from_utf8(bytes) { return text.trim_start_matches('\u{feff}').to_owned(); }
    // Older external SRT files commonly use Windows-1252, including Portuguese accents.
    const CP1252: [char;32] = ['€','\u{81}','‚','ƒ','„','…','†','‡','ˆ','‰','Š','‹','Œ','\u{8d}','Ž','\u{8f}','\u{90}','‘','’','“','”','•','–','—','˜','™','š','›','œ','\u{9d}','ž','Ÿ'];
    bytes.iter().map(|b| if (0x80..=0x9f).contains(b) { CP1252[(*b-0x80) as usize] } else { char::from(*b) }).collect()
}

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn finds_release_folders_without_mixing_episodes() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        for name in ["Show.S01E01.mkv", "Show.S01E02.mkv", "Show.S01E01.pt-br.srt", "Show.S01E02.en.srt", "Subs/Show.S01E01/English.srt", "Subs/Show.S01E02/English.srt", "Subs/Show.S01E01.ass", "Subs/English.srt"] {
            let path = root.join(name); std::fs::create_dir_all(path.parent().unwrap()).unwrap(); std::fs::write(path, b"test").unwrap();
        }
        for name in ["Other.S01E01.en.srt", "Subs/Other.S01E01/English.srt"] {
            let path = root.join(name); std::fs::create_dir_all(path.parent().unwrap()).unwrap(); std::fs::write(path, b"test").unwrap();
        }
        let found = discover_subtitle_files(&root, &root.join("Show.S01E01.mkv")).unwrap();
        assert_eq!(found.len(), 3);
        assert!(found.iter().all(|p| !p.to_string_lossy().contains("S01E02")));
    }
    #[test] fn finds_generic_movie_languages_and_vobsub_once() {
        let temp = tempfile::tempdir().unwrap(); let root = temp.path().canonicalize().unwrap();
        for name in ["Film.mkv", "Subs/English.srt", "Subs/Portuguese.ass", "Film.idx", "Film.sub"] {
            let path = root.join(name); std::fs::create_dir_all(path.parent().unwrap()).unwrap(); std::fs::write(path, b"test").unwrap();
        }
        let found = discover_subtitle_files(&root, &root.join("Film.mkv")).unwrap();
        assert_eq!(found.len(), 3);
        assert!(!found.iter().any(|p| p.extension().unwrap() == "sub"));
    }
    #[test] fn accents_survive_utf8_utf16_and_legacy_encoding() {
        assert_eq!(decode_subtitle_text("ação".as_bytes()), "ação");
        let bytes:Vec<u8> = [0xff,0xfe].into_iter().chain("ação".encode_utf16().flat_map(u16::to_le_bytes)).collect();
        assert_eq!(decode_subtitle_text(&bytes), "ação");
        assert_eq!(decode_subtitle_text(&[b'a',0xe7,0xe3,b'o']), "ação");
    }
}

/// Match sidecars and release subtitle folders without mixing adjacent episodes.
pub fn discover_subtitle_files(root: &Path, media_path: &Path) -> Result<Vec<PathBuf>, String> {
    let directory = media_path.parent().ok_or("The media file has no parent folder.")?;
    let stem = media_path.file_stem().and_then(|s| s.to_str()).unwrap_or_default().to_lowercase();
    let episode = crate::naming::episode_position(&stem, "");
    let episode_title = |value: &str| crate::naming::episode_marker_start(value).map(|index| crate::naming::normalize_title(&value[..index])).unwrap_or_default();
    let title = episode_title(&stem);
    let same_episode = |value: &str| episode.1.is_some() && crate::naming::episode_position(value, "") == episode && (episode_title(value).is_empty() || episode_title(value) == title);
    let nearby: Vec<_> = std::fs::read_dir(directory).map_err(|e| e.to_string())?.flatten().map(|e| e.path()).collect();
    let video_count = nearby.iter().filter(|p| p.extension().and_then(|s| s.to_str()).is_some_and(|s| matches!(s.to_ascii_lowercase().as_str(), "mkv" | "mp4" | "avi" | "m4v" | "mov" | "webm" | "ts"))).count();
    let mut pending = vec![(directory.to_path_buf(), 0, false)];
    let mut subtitles = Vec::new();
    while let Some((folder, depth, scoped)) = pending.pop() {
        for path in std::fs::read_dir(&folder).map_err(|e| e.to_string())?.flatten().map(|e| e.path()) {
            let Ok(canonical) = path.canonicalize() else { continue; };
            if !canonical.starts_with(root) { continue; }
            let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or_default().to_lowercase();
            if canonical.is_dir() && depth < 3 {
                let folder_name = path.file_name().and_then(|s| s.to_str()).unwrap_or_default().to_lowercase();
                let named = matches!(folder_name.as_str(), "subs" | "subtitles" | "subtitle" | "legendas");
                let matches_media = folder_name == stem || same_episode(&folder_name);
                if named || matches_media || depth > 0 {
                    pending.push((canonical, depth + 1, scoped || matches_media));
                }
                continue;
            }
            let extension = path.extension().and_then(|s| s.to_str()).unwrap_or_default().to_ascii_lowercase();
            if !canonical.is_file() || !matches!(extension.as_str(), "srt" | "vtt" | "ass" | "ssa" | "sub" | "idx") { continue; }
            // VobSub pairs are represented by the .idx file, not twice.
            if extension == "sub" && path.with_extension("idx").is_file() { continue; }
            let exact = name == stem || name.strip_prefix(&stem).is_some_and(|s| s.starts_with(['.', ' ', '_', '-']));
            let candidate_episode = crate::naming::episode_position(&name, "");
            let matches_episode = same_episode(&name);
            let generic_scoped = depth > 0 && (scoped || (episode.1.is_none() && video_count == 1)) && candidate_episode.1.is_none();
            let loose_language = episode.1.is_none() && video_count == 1 && matches!(name.as_str(), "english" | "portuguese" | "portuguese (brazil)" | "en" | "eng" | "pt" | "pt-br" | "por" | "spanish" | "es" | "spa" | "french" | "fr" | "fra");
            if exact || matches_episode || generic_scoped || loose_language { subtitles.push(canonical); }
        }
    }
    subtitles.sort(); subtitles.dedup();
    Ok(subtitles)
}
