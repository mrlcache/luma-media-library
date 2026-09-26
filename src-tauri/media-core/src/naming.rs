use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedMediaName {
    pub query: String,
    pub normalized: String,
    pub kind: String,
    pub year: Option<u16>,
}

pub fn parse_media_name(file_title: &str, relative_path: &str) -> Option<ParsedMediaName> {
    let filename = file_title.trim();
    if filename.is_empty() {
        return None;
    }

    let relative = Path::new(relative_path);
    let directories = relative
        .parent()
        .into_iter()
        .flat_map(Path::components)
        .filter_map(|component| component.as_os_str().to_str())
        .collect::<Vec<_>>();
    let season_directory = directories
        .last()
        .is_some_and(|name| is_season_directory(name));
    let episode_start = episode_marker_start(filename);
    let is_series = episode_start.is_some() || season_directory;

    let source = if is_series && season_directory && episode_start.is_none() {
        directories
            .get(directories.len().saturating_sub(2))
            .copied()
            .unwrap_or("")
    } else if let Some(start) = episode_start {
        let prefix = filename[..start].trim_matches(|character: char| !character.is_alphanumeric());
        if prefix.is_empty() {
            if season_directory {
                directories
                    .get(directories.len().saturating_sub(2))
                    .copied()
                    .unwrap_or("")
            } else {
                directories.last().copied().unwrap_or("")
            }
        } else {
            prefix
        }
    } else {
        filename
    };

    let cleaned = remove_bracketed_release_groups(source);
    let (title, year) = truncate_release_noise(&cleaned);
    let query = title.split_whitespace().collect::<Vec<_>>().join(" ");
    if query.chars().count() < 2 || query.chars().count() > 120 {
        return None;
    }
    let normalized = normalize_title(&query);
    if normalized.is_empty() {
        return None;
    }

    Some(ParsedMediaName {
        query,
        normalized,
        kind: if is_series { "series" } else { "movie" }.to_owned(),
        year,
    })
}

pub fn infer_media_kind(file_title: &str, relative_path: &str) -> &'static str {
    let relative = Path::new(relative_path);
    let season_directory = relative
        .parent()
        .into_iter()
        .flat_map(Path::components)
        .filter_map(|component| component.as_os_str().to_str())
        .last()
        .is_some_and(is_season_directory);
    if season_directory || episode_marker_start(file_title).is_some() {
        "series"
    } else {
        "movie"
    }
}

pub fn episode_marker_start(value: &str) -> Option<usize> {
    let upper = value.to_ascii_uppercase();
    for (start, token) in token_spans(&upper) {
        let bytes = token.as_bytes();
        let is_season_episode = bytes.len() >= 4
            && bytes[0] == b'S'
            && bytes[1..]
                .iter()
                .position(|byte| *byte == b'E')
                .map(|offset| offset + 1)
                .is_some_and(|e| {
                    (2..=3).contains(&e)
                        && (1..=2).contains(&(bytes.len().saturating_sub(e + 1)))
                        && bytes[1..e].iter().all(u8::is_ascii_digit)
                        && bytes[e + 1..].iter().all(u8::is_ascii_digit)
                });
        let is_x_episode = bytes.len() >= 3
            && bytes
                .iter()
                .position(|byte| *byte == b'X')
                .is_some_and(|x| {
                    (1..=2).contains(&x)
                        && (1..=2).contains(&(bytes.len().saturating_sub(x + 1)))
                        && bytes[..x].iter().all(u8::is_ascii_digit)
                        && bytes[x + 1..].iter().all(u8::is_ascii_digit)
                });
        if is_season_episode || is_x_episode {
            return Some(start);
        }
    }
    None
}

pub fn episode_position(file_title: &str, relative_path: &str) -> (Option<u16>, Option<u16>) {
    let upper = file_title.to_ascii_uppercase();
    for (_, token) in token_spans(&upper) {
        let bytes = token.as_bytes();
        if bytes.first() == Some(&b'S') {
            if let Some(e) = bytes.iter().position(|byte| *byte == b'E') {
                if let (Some(season), Some(episode)) = (
                    parse_ascii_number(&bytes[1..e]),
                    parse_ascii_number(&bytes[e + 1..]),
                ) {
                    return (Some(season), Some(episode));
                }
            }
        }
        if let Some(x) = bytes.iter().position(|byte| *byte == b'X') {
            if let (Some(season), Some(episode)) = (
                parse_ascii_number(&bytes[..x]),
                parse_ascii_number(&bytes[x + 1..]),
            ) {
                return (Some(season), Some(episode));
            }
        }
    }

    let season = Path::new(relative_path)
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .filter_map(|part| {
            let normalized = part.to_ascii_lowercase();
            let digits = normalized
                .strip_prefix("season")
                .or_else(|| normalized.strip_prefix('s'))?
                .trim_start_matches(|character: char| !character.is_ascii_digit());
            parse_ascii_number(digits.as_bytes())
        })
        .last();
    (season, None)
}

fn parse_ascii_number(bytes: &[u8]) -> Option<u16> {
    if bytes.is_empty() || !bytes.iter().all(u8::is_ascii_digit) {
        return None;
    }
    std::str::from_utf8(bytes).ok()?.parse().ok()
}

pub fn normalize_title(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn remove_bracketed_release_groups(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    let mut characters = value.chars().peekable();
    while let Some(character) = characters.next() {
        match character {
            '[' | '{' => {
                let closing = if character == '[' { ']' } else { '}' };
                let mut content = String::new();
                for next in characters.by_ref() {
                    if next == closing {
                        break;
                    }
                    content.push(next);
                }
                if content.trim().parse::<u16>().is_ok_and(is_release_year) {
                    result.push(' ');
                    result.push_str(content.trim());
                    result.push(' ');
                } else {
                    result.push(' ');
                }
            }
            '(' => {
                let mut content = String::new();
                for next in characters.by_ref() {
                    if next == ')' {
                        break;
                    }
                    content.push(next);
                }
                if content.trim().parse::<u16>().is_ok_and(is_release_year) {
                    result.push(' ');
                    result.push_str(content.trim());
                    result.push(' ');
                } else {
                    result.push(' ');
                }
            }
            _ => result.push(character),
        }
    }
    result
}

fn is_release_year(year: u16) -> bool {
    (1900..=2099).contains(&year)
}

fn truncate_release_noise(value: &str) -> (String, Option<u16>) {
    const NOISE: &[&str] = &[
        "480p", "720p", "1080p", "2160p", "4320p", "bluray", "brrip", "bdrip", "webrip", "webdl",
        "web", "hdtv", "dvdrip", "x264", "x265", "h264", "h265", "hevc", "av1", "hdr", "hdr10",
        "dv", "remux", "proper", "repack", "aac", "ac3", "dts", "atmos",
    ];
    let mut words = Vec::new();
    let mut year = None;
    for word in value.split(|character: char| !character.is_alphanumeric()) {
        if word.is_empty() {
            continue;
        }
        if let Ok(parsed_year) = word.parse::<u16>() {
            if is_release_year(parsed_year) {
                year = Some(parsed_year);
                break;
            }
        }
        let normalized = word.to_ascii_lowercase();
        if NOISE.iter().any(|noise| *noise == normalized) {
            break;
        }
        words.push(word);
    }
    (words.join(" "), year)
}

fn token_spans(value: &str) -> Vec<(usize, &str)> {
    let mut spans = Vec::new();
    let mut start = None;
    for (index, character) in value.char_indices() {
        if character.is_alphanumeric() {
            start.get_or_insert(index);
        } else if let Some(token_start) = start.take() {
            spans.push((token_start, &value[token_start..index]));
        }
    }
    if let Some(token_start) = start {
        spans.push((token_start, &value[token_start..]));
    }
    spans
}

fn is_season_directory(value: &str) -> bool {
    let normalized = value.to_ascii_lowercase();
    let rest = normalized
        .strip_prefix("season")
        .or_else(|| normalized.strip_prefix("series"));
    rest.is_some_and(|rest| rest.chars().any(|character| character.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::{episode_position, infer_media_kind, parse_media_name};

    #[test]
    fn extracts_episode_position_from_filename_or_season_folder() {
        assert_eq!(
            episode_position(
                "Reacher.S02E04.Winter.Break.mkv",
                "Reacher/Season 2/Reacher.S02E04.mkv"
            ),
            (Some(2), Some(4))
        );
        assert_eq!(
            episode_position("Show.1x09.Title.mkv", "Show/Season 1/Show.1x09.mkv"),
            (Some(1), Some(9))
        );
        assert_eq!(
            episode_position("Episode 4.mkv", "Show/Season 3/Episode 4.mkv"),
            (Some(3), None)
        );
    }

    #[test]
    fn parses_movies_and_groups_episode_names() {
        let movie = parse_media_name(
            "The.Matrix.1999.1080p.BluRay.x264-GROUP",
            "The.Matrix.1999.1080p.BluRay.x264-GROUP.mkv",
        )
        .expect("movie title");
        assert_eq!(movie.query, "The Matrix");
        assert_eq!(movie.kind, "movie");
        assert_eq!(movie.year, Some(1999));

        let episode = parse_media_name(
            "Station Eleven S01E02 Ayo Edebiri",
            "Station Eleven/Season 1/Station Eleven S01E02.mkv",
        )
        .expect("series title");
        assert_eq!(episode.query, "Station Eleven");
        assert_eq!(episode.kind, "series");
        assert_eq!(
            infer_media_kind("S01E02", "Station Eleven/Season 1/S01E02.mkv"),
            "series"
        );

        let episode_without_season_folder = parse_media_name(
            "MobLand (2025) - S01E03 1080p WEB-DL",
            "MobLand (2025) - S01E03 1080p WEB-DL.mkv",
        )
        .expect("two-digit season episode marker");
        assert_eq!(episode_without_season_folder.query, "MobLand");
        assert_eq!(episode_without_season_folder.kind, "series");
        assert_eq!(episode_without_season_folder.year, Some(2025));
    }
}
