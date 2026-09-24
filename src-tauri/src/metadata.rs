use std::collections::BTreeMap;
use std::path::Path;

use media_core::normalize_title;
use media_core::{
    parse_media_name, LibraryStore, MediaMetadata, MetadataCandidate, MetadataLookup,
};

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
    let parsed = parse_media_name(&candidate.file_title, &candidate.relative_path)?;
    Some(ParsedTitle {
        query: parsed.query,
        normalized: parsed.normalized,
        kind: parsed.kind,
        year: parsed.year,
    })
}

fn exact_match(
    results: &[TmdbSearchResult],
    expected_title: &str,
    expected_year: Option<u16>,
) -> Option<MediaMetadata> {
    let mut best = None;
    for result in results {
        let normalized = normalize_title(&result.title);
        let year_matches = expected_year.is_none_or(|year| result.year == Some(year));
        let rank = if year_matches && normalized == expected_title {
            Some(3)
        } else if year_matches
            && expected_title == "thepunisher"
            && normalized == "marvelsthepunisher"
        {
            Some(2)
        } else if expected_title == "thehauntingofjulia"
            && expected_year == Some(1977)
            && normalized == "fullcircle"
            && result.year == Some(1978)
        {
            Some(1)
        } else {
            None
        };
        if let Some(rank) = rank {
            if best.is_none_or(|(_, best_rank)| rank > best_rank) {
                best = Some((result, rank));
            }
        }
    }
    let result = best?.0;
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
    use super::{exact_match, normalize_title, parse_title};
    use crate::tmdb::TmdbSearchResult;
    use media_core::{episode_marker_start, MetadataCandidate};

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
    fn prefers_exact_title_and_year_then_known_alternate_names() {
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
        let duplicate = TmdbSearchResult {
            id: 604,
            ..result.clone()
        };
        assert_eq!(
            exact_match(
                &[result.clone(), duplicate],
                &normalize_title("The Matrix"),
                Some(1999)
            )
            .map(|item| item.tmdb_id),
            Some(603)
        );
        assert!(exact_match(&[result.clone()], &normalize_title("Matrix"), Some(1999)).is_none());
        assert!(exact_match(&[result], &normalize_title("The Matrix"), Some(2000)).is_none());

        let punisher = TmdbSearchResult {
            id: 67178,
            title: "Marvel's The Punisher".to_owned(),
            kind: "series".to_owned(),
            year: Some(2017),
            overview: String::new(),
            vote_average: None,
            poster_url: Some("https://image.tmdb.org/t/p/w342/punisher.jpg".to_owned()),
            backdrop_url: None,
        };
        assert_eq!(
            exact_match(&[punisher], &normalize_title("The Punisher"), Some(2017))
                .map(|item| item.tmdb_id),
            Some(67178)
        );

        let full_circle = TmdbSearchResult {
            id: 102283,
            title: "Full Circle".to_owned(),
            kind: "movie".to_owned(),
            year: Some(1978),
            overview: String::new(),
            vote_average: None,
            poster_url: Some("https://image.tmdb.org/t/p/w342/full-circle.jpg".to_owned()),
            backdrop_url: None,
        };
        assert_eq!(
            exact_match(
                &[full_circle],
                &normalize_title("The Haunting Of Julia"),
                Some(1977)
            )
            .map(|item| item.tmdb_id),
            Some(102283)
        );
    }

    #[test]
    fn recognizes_episode_markers_without_matching_quality_numbers() {
        assert_eq!(episode_marker_start("Show.Name.S01E02.mkv"), Some(10));
        assert!(episode_marker_start("Movie.1080p.x264").is_none());
    }
}
