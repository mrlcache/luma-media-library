use std::collections::BTreeMap;
use std::path::Path;

use media_core::{LibraryStore, MediaMetadata, MetadataCandidate, MetadataLookup};

use crate::tmdb::{TmdbSearchResult, TmdbState};

const LOOKUP_CONCURRENCY: usize = 3;
const CANDIDATE_BATCH_SIZE: u32 = 256;

pub struct EnrichmentReport {
    pub matched_count: u64,
    pub unmatched_count: u64,
    pub pending_count: u64,
    pub error: Option<String>,
}

#[derive(Clone)]
struct ParsedTitle {
    query: String,
    normalized: String,
    kind: String,
    year: Option<u16>,
}

pub async fn enrich_library(db_path: &Path, tmdb: &TmdbState) -> EnrichmentReport {
    let mut request_error = None;
    loop {
        let candidates_path = db_path.to_owned();
        let candidates = match tauri::async_runtime::spawn_blocking(move || {
            LibraryStore::open(&candidates_path)?.metadata_candidates(CANDIDATE_BATCH_SIZE)
        })
        .await
        {
            Ok(Ok(candidates)) => candidates,
            Ok(Err(error)) => {
                request_error = Some(error);
                break;
            }
            Err(error) => {
                request_error = Some(format!("Could not prepare metadata lookup: {error}"));
                break;
            }
        };
        if candidates.is_empty() {
            break;
        }

        let (lookups, batch_error) = lookup_batch(candidates, tmdb).await;
        if !lookups.is_empty() {
            let write_path = db_path.to_owned();
            let write_result = tauri::async_runtime::spawn_blocking(move || {
                let mut store = LibraryStore::open(&write_path)?;
                store.save_metadata_lookups(&lookups)
            })
            .await;
            match write_result {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    request_error = Some(error);
                    break;
                }
                Err(error) => {
                    request_error =
                        Some(format!("The metadata results could not be saved: {error}"));
                    break;
                }
            }
        }
        if batch_error.is_some() {
            request_error = batch_error;
            break;
        }
    }

    report_from_database(db_path, request_error).await
}

async fn lookup_batch(
    candidates: Vec<MetadataCandidate>,
    tmdb: &TmdbState,
) -> (Vec<MetadataLookup>, Option<String>) {
    let mut grouped = BTreeMap::<(String, String, Option<u16>), Vec<(i64, ParsedTitle)>>::new();
    let mut lookups = Vec::new();
    for candidate in candidates {
        if let Some(parsed) = parse_title(&candidate) {
            let key = (parsed.kind.clone(), parsed.normalized.clone(), parsed.year);
            grouped
                .entry(key)
                .or_default()
                .push((candidate.media_id, parsed));
        } else {
            lookups.push(MetadataLookup {
                media_ids: vec![candidate.media_id],
                metadata: None,
            });
        }
    }

    let queries = grouped
        .into_values()
        .map(|members| {
            let query = members[0].1.clone();
            let media_ids = members.iter().map(|(id, _)| *id).collect::<Vec<_>>();
            (query, media_ids)
        })
        .collect::<Vec<_>>();
    let mut request_error = None;
    for batch in queries.chunks(LOOKUP_CONCURRENCY) {
        let mut jobs = Vec::with_capacity(batch.len());
        for (parsed, media_ids) in batch {
            let query = parsed.query.clone();
            let kind = parsed.kind.clone();
            let expected_title = parsed.normalized.clone();
            let expected_year = parsed.year;
            let ids = media_ids.clone();
            let tmdb = tmdb.clone();
            jobs.push(tauri::async_runtime::spawn(async move {
                let result = tmdb.search_for_kind(&query, &kind).await;
                (ids, expected_title, expected_year, result)
            }));
        }

        let mut batch_failed = false;
        for job in jobs {
            match job.await {
                Ok((media_ids, expected_title, expected_year, Ok(results))) => {
                    let metadata = exact_match(&results, &expected_title, expected_year);
                    lookups.push(MetadataLookup {
                        media_ids,
                        metadata,
                    });
                }
                Ok((_, _, _, Err(error))) => {
                    batch_failed = true;
                    request_error.get_or_insert(error);
                }
                Err(error) => {
                    batch_failed = true;
                    request_error.get_or_insert_with(|| {
                        format!("The metadata lookup could not finish: {error}")
                    });
                }
            }
        }
        if batch_failed {
            break;
        }
    }
    (lookups, request_error)
}

async fn report_from_database(db_path: &Path, error: Option<String>) -> EnrichmentReport {
    let report_path = db_path.to_owned();
    let counts = tauri::async_runtime::spawn_blocking(move || {
        LibraryStore::open(&report_path)?.metadata_counts()
    })
    .await;
    match counts {
        Ok(Ok((matched_count, unmatched_count, pending_count))) => EnrichmentReport {
            matched_count,
            unmatched_count,
            pending_count,
            error,
        },
        Ok(Err(count_error)) => EnrichmentReport {
            matched_count: 0,
            unmatched_count: 0,
            pending_count: 0,
            error: Some(error.unwrap_or(count_error)),
        },
        Err(join_error) => EnrichmentReport {
            matched_count: 0,
            unmatched_count: 0,
            pending_count: 0,
            error: Some(
                error.unwrap_or_else(|| format!("Could not read metadata progress: {join_error}")),
            ),
        },
    }
}

fn parse_title(candidate: &MetadataCandidate) -> Option<ParsedTitle> {
    let filename = candidate.file_title.trim();
    if filename.is_empty() {
        return None;
    }

    let relative = Path::new(&candidate.relative_path);
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
            directories
                .last()
                .copied()
                .filter(|name| !is_season_directory(name))
                .unwrap_or("")
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

    Some(ParsedTitle {
        query,
        normalized,
        kind: if is_series { "series" } else { "movie" }.to_owned(),
        year,
    })
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
            if (1900..=2099).contains(&parsed_year) {
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

fn episode_marker_start(value: &str) -> Option<usize> {
    let upper = value.to_ascii_uppercase();
    for (start, token) in token_spans(&upper) {
        let bytes = token.as_bytes();
        let is_season_episode = bytes.len() >= 4
            && bytes[0] == b'S'
            && bytes[1..]
                .iter()
                .position(|byte| *byte == b'E')
                .is_some_and(|e| {
                    (1..=2).contains(&e)
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

fn normalize_title(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn exact_match(
    results: &[TmdbSearchResult],
    expected_title: &str,
    expected_year: Option<u16>,
) -> Option<MediaMetadata> {
    let mut matches = results.iter().filter(|result| {
        normalize_title(&result.title) == expected_title
            && expected_year.is_none_or(|year| result.year == Some(year))
    });
    let result = matches.next()?;
    if matches.next().is_some() {
        return None;
    }
    Some(MediaMetadata {
        tmdb_id: result.id,
        kind: result.kind.clone(),
        title: result.title.clone(),
        year: result.year,
        overview: result.overview.clone(),
        vote_average: result.vote_average,
        poster_url: result.poster_url.clone(),
        backdrop_url: result.backdrop_url.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::{episode_marker_start, exact_match, normalize_title, parse_title};
    use crate::tmdb::TmdbSearchResult;
    use media_core::MetadataCandidate;

    #[test]
    fn parses_release_names_and_episode_folders_conservatively() {
        let movie = parse_title(&MetadataCandidate {
            media_id: 1,
            file_title: "The.Matrix.1999.1080p.BluRay.x264-GROUP".to_owned(),
            relative_path: "The.Matrix.1999.1080p.BluRay.x264-GROUP.mkv".to_owned(),
        })
        .expect("movie title");
        assert_eq!(movie.query, "The Matrix");
        assert_eq!(movie.kind, "movie");
        assert_eq!(movie.year, Some(1999));

        let movie_with_parenthesized_year = parse_title(&MetadataCandidate {
            media_id: 3,
            file_title: "Dune Part Two (2024) [1080p]".to_owned(),
            relative_path: "Dune Part Two (2024) [1080p].mkv".to_owned(),
        })
        .expect("movie title and year");
        assert_eq!(movie_with_parenthesized_year.query, "Dune Part Two");
        assert_eq!(movie_with_parenthesized_year.year, Some(2024));

        let episode = parse_title(&MetadataCandidate {
            media_id: 2,
            file_title: "Station Eleven S01E02 Ayo Edebiri.mkv".to_owned(),
            relative_path: "Station Eleven/Season 1/Station Eleven S01E02.mkv".to_owned(),
        })
        .expect("series title");
        assert_eq!(episode.query, "Station Eleven");
        assert_eq!(episode.kind, "series");
    }

    #[test]
    fn only_accepts_unique_exact_title_and_year_matches() {
        let result = TmdbSearchResult {
            id: 603,
            title: "The Matrix".to_owned(),
            kind: "movie".to_owned(),
            year: Some(1999),
            overview: String::new(),
            vote_average: Some(8.2),
            poster_url: Some("https://image.tmdb.org/t/p/w342/poster.jpg".to_owned()),
            backdrop_url: None,
        };
        assert!(exact_match(
            &[result.clone()],
            &normalize_title("The Matrix"),
            Some(1999)
        )
        .is_some());
        assert!(exact_match(&[result.clone()], &normalize_title("Matrix"), Some(1999)).is_none());
        assert!(exact_match(&[result], &normalize_title("The Matrix"), Some(2000)).is_none());
    }

    #[test]
    fn recognizes_episode_markers_without_matching_quality_numbers() {
        assert_eq!(episode_marker_start("Show.Name.S01E02.mkv"), Some(10));
        assert!(episode_marker_start("Movie.1080p.x264").is_none());
    }
}
