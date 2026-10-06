//! Bounded local media discovery and the persistent SQLite index.
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::Serialize;
use walkdir::WalkDir;

use crate::naming::{episode_position, infer_media_kind, normalize_title, parse_media_name};

const BATCH_SIZE: usize = 256;
const MAX_SCAN_DEPTH: usize = 64;
const MAX_FILES_PER_ROOT: usize = 1_000_000;
const GROUPED_CATALOG_CTE: &str = "WITH current_items AS (
    SELECT items.id, files.display_name, files.extension, files.size_bytes,
           files.modified_at, metadata.kind AS metadata_kind, metadata.release_year,
           metadata.title AS metadata_title, metadata.overview,
           metadata.vote_average, metadata.poster_url, metadata.backdrop_url,
           metadata.tmdb_id, items.local_title, items.local_kind,
           items.local_year, items.local_key
    FROM media_files AS files
    JOIN library_roots AS roots ON roots.id = files.root_id
    JOIN media_items AS items
      ON items.root_id = files.root_id AND items.relative_path = files.relative_path
    LEFT JOIN media_metadata AS metadata ON metadata.media_id = items.id
    WHERE files.generation = roots.current_generation
      AND NOT EXISTS (SELECT 1 FROM media_extras WHERE media_id = items.id)
), grouped AS (
    SELECT MIN(id) AS id,
           COALESCE(MAX(NULLIF(metadata_title, '')), MIN(local_title), MIN(display_name)) AS title,
           MIN(extension) AS extension,
           SUM(size_bytes) AS size_bytes,
           MAX(modified_at) AS modified_at,
           MAX(COALESCE(metadata_kind, local_kind)) AS kind,
           MAX(COALESCE(release_year, local_year)) AS release_year,
           MAX(overview) AS overview,
           MAX(vote_average) AS vote_average,
           MAX(poster_url) AS poster_url,
           MAX(backdrop_url) AS backdrop_url,
           CASE WHEN tmdb_id IS NOT NULL
                THEN COALESCE(metadata_kind, '') || ':' || tmdb_id
                WHEN local_kind = 'series'
                THEN 'series:' || local_key || ':' || COALESCE(local_year, '')
                ELSE 'file:' || id END AS group_key
    FROM current_items
    GROUP BY CASE WHEN tmdb_id IS NOT NULL
         THEN COALESCE(metadata_kind, '') || ':' || tmdb_id
         WHEN local_kind = 'series'
         THEN 'series:' || local_key || ':' || COALESCE(local_year, '')
         ELSE 'file:' || id END
)";

const VIDEO_EXTENSIONS: &[&str] = &[
    "avi", "flv", "m2ts", "m4v", "mkv", "mov", "mp4", "mpeg", "mpg", "mts", "ts", "webm", "wmv",
];

#[derive(Clone)]
pub struct LibraryState {
    pub db_path: PathBuf,
    pub scanning: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryStatus {
    pub folders: Vec<String>,
    pub root_count: u64,
    pub file_count: u64,
    pub matched_count: u64,
    pub unmatched_count: u64,
    pub pending_count: u64,
    pub last_scan_at: Option<i64>,
    pub is_scanning: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanSummary {
    pub root_name: String,
    pub file_count: u64,
    pub matched_count: u64,
    pub unmatched_count: u64,
    pub pending_count: u64,
    pub metadata_error: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryScanSummary {
    pub root_count: u64,
    pub file_count: u64,
    pub matched_count: u64,
    pub unmatched_count: u64,
    pub pending_count: u64,
    pub metadata_error: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogMedia {
    pub id: i64,
    pub playback_uuid: String,
    pub title: String,
    pub extension: String,
    pub size_bytes: u64,
    pub modified_at: Option<i64>,
    pub kind: Option<String>,
    pub year: Option<u16>,
    pub overview: Option<String>,
    pub vote_average: Option<f32>,
    pub poster_url: Option<String>,
    pub backdrop_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct MetadataCandidate {
    pub media_id: i64,
    pub file_title: String,
    pub relative_path: String,
}

#[derive(Debug, Clone)]
pub struct MediaMetadata {
    pub tmdb_id: u64,
    pub kind: String,
    pub title: String,
    pub year: Option<u16>,
    pub overview: String,
    pub vote_average: Option<f32>,
    pub poster_url: Option<String>,
    pub backdrop_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct MetadataLookup {
    pub media_ids: Vec<i64>,
    pub metadata: Option<MediaMetadata>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogPage {
    pub items: Vec<CatalogMedia>,
    pub total: u64,
    pub offset: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalEpisodeFile {
    pub media_id: i64,
    pub playback_uuid: String,
    pub file_name: String,
    pub path: String,
    pub season: Option<u16>,
    pub episode: Option<u16>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalTitleDetail {
    pub media: CatalogMedia,
    pub files: Vec<LocalEpisodeFile>,
    pub extras: Vec<LocalEpisodeFile>,
    pub watched_before: Option<EpisodePosition>,
    pub tmdb_id: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EpisodePosition {
    pub season: u16,
    pub episode: u16,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinueWatchingItem {
    pub id: i64,
    pub playback_id: i64,
    pub playback_uuid: String,
    pub title: String,
    pub kind: Option<String>,
    pub year: Option<u16>,
    pub overview: Option<String>,
    pub vote_average: Option<f32>,
    pub poster_url: Option<String>,
    pub backdrop_url: Option<String>,
    pub position_seconds: f64,
    pub duration_seconds: f64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackHistoryItem {
    pub id: i64,
    pub title: String,
    pub kind: Option<String>,
    pub year: Option<u16>,
    pub overview: Option<String>,
    pub vote_average: Option<f32>,
    pub poster_url: Option<String>,
    pub backdrop_url: Option<String>,
    pub updated_at: i64,
}

pub struct LibraryStore {
    connection: Connection,
}

fn move_file(source: &Path, destination: &Path) -> Result<(), std::io::Error> {
    match std::fs::rename(source, destination) {
        Ok(()) => Ok(()),
        Err(error) if matches!(error.raw_os_error(), Some(17 | 18)) => {
            std::fs::copy(source, destination)?;
            if let Err(remove_error) = std::fs::remove_file(source) {
                let _ = std::fs::remove_file(destination);
                return Err(remove_error);
            }
            Ok(())
        }
        Err(error) => Err(error),
    }
}

fn rollback_moves(moved: &[(PathBuf, PathBuf)]) {
    for (source, destination) in moved.iter().rev() {
        if let Some(parent) = source.parent() { let _ = std::fs::create_dir_all(parent); }
        if std::fs::rename(destination, source).is_err() {
            if std::fs::copy(destination, source).is_ok() { let _ = std::fs::remove_file(destination); }
        }
    }
}

impl LibraryStore {
    /// One entry per TMDb title, across the complete active library (not a catalog page).
    pub fn recommendation_profile(&self) -> Result<Vec<(u64, String, String, Option<i64>)>, String> {
        let mut statement = self.connection.prepare(
            "SELECT metadata.tmdb_id, metadata.kind, MAX(COALESCE(NULLIF(metadata.title, ''), items.local_title, files.display_name)), MAX(activity.updated_at)
             FROM media_metadata AS metadata
             JOIN media_items AS items ON items.id = metadata.media_id
             JOIN media_files AS files ON files.root_id = items.root_id AND files.relative_path = items.relative_path
             JOIN library_roots AS roots ON roots.id = files.root_id AND files.generation = roots.current_generation
             LEFT JOIN playback_activity AS activity ON activity.media_id = items.id
             WHERE metadata.tmdb_id > 0 AND metadata.kind IN ('movie', 'series')
               AND NOT EXISTS (SELECT 1 FROM media_extras WHERE media_id = items.id)
             GROUP BY metadata.kind, metadata.tmdb_id
             ORDER BY MAX(activity.updated_at) DESC, MAX(files.modified_at) DESC"
        ).map_err(|error| format!("Could not read recommendation profile: {error}"))?;
        let rows = statement.query_map([], |row| Ok((row.get::<_, i64>(0)? as u64, row.get(1)?, row.get(2)?, row.get(3)?)))
            .map_err(|error| format!("Could not read recommendation profile: {error}"))?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|error| error.to_string())
    }
    pub fn open(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("Could not create the local data directory: {error}"))?;
        }

        let mut connection = Connection::open(path)
            .map_err(|error| format!("Could not open the local library database: {error}"))?;
        connection
            .busy_timeout(std::time::Duration::from_secs(5))
            .map_err(|error| format!("Could not configure the local library database: {error}"))?;
        connection
            .execute_batch(
                "PRAGMA foreign_keys = ON;
                 PRAGMA journal_mode = WAL;
                 CREATE TABLE IF NOT EXISTS library_roots (
                    id INTEGER PRIMARY KEY,
                    canonical_path TEXT NOT NULL UNIQUE,
                    current_generation INTEGER NOT NULL DEFAULT 0,
                    last_scanned_at INTEGER
                 );
                 CREATE TABLE IF NOT EXISTS media_files (
                    id INTEGER PRIMARY KEY,
                    root_id INTEGER NOT NULL REFERENCES library_roots(id) ON DELETE CASCADE,
                    relative_path TEXT NOT NULL,
                    display_name TEXT NOT NULL,
                    extension TEXT NOT NULL,
                    size_bytes INTEGER NOT NULL,
                    modified_at INTEGER,
                    generation INTEGER NOT NULL,
                    UNIQUE(root_id, relative_path, generation)
                 );
                 CREATE INDEX IF NOT EXISTS media_files_root_generation
                    ON media_files(root_id, generation);
                 CREATE TABLE IF NOT EXISTS media_items (
                    id INTEGER PRIMARY KEY,
                    root_id INTEGER NOT NULL REFERENCES library_roots(id) ON DELETE CASCADE,
                    relative_path TEXT NOT NULL,
                    UNIQUE(root_id, relative_path)
                 );
                 CREATE TABLE IF NOT EXISTS media_metadata (
                    media_id INTEGER PRIMARY KEY REFERENCES media_items(id) ON DELETE CASCADE,
                    tmdb_id INTEGER,
                    kind TEXT,
                    title TEXT,
                    release_year INTEGER,
                    overview TEXT,
                    vote_average REAL,
                    poster_url TEXT,
                    backdrop_url TEXT,
                    looked_up_at INTEGER NOT NULL
                 );
                 CREATE INDEX IF NOT EXISTS media_metadata_tmdb
                    ON media_metadata(kind, tmdb_id);",
            )
            .map_err(|error| format!("Could not initialize the local library database: {error}"))?;

        let mut schema_version: i64 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(|error| format!("Could not read the local library schema: {error}"))?;
        if schema_version < 1 {
            let migration = connection
                .transaction()
                .map_err(|error| format!("Could not migrate the local library: {error}"))?;
            migration
                .execute(
                    "INSERT OR IGNORE INTO media_items(root_id, relative_path)
                     SELECT DISTINCT root_id, relative_path FROM media_files",
                    [],
                )
                .map_err(|error| {
                    format!("Could not preserve existing media identities: {error}")
                })?;
            migration
                .execute_batch("PRAGMA user_version = 1")
                .map_err(|error| format!("Could not update the local library schema: {error}"))?;
            migration.commit().map_err(|error| {
                format!("Could not finish the local library migration: {error}")
            })?;
            schema_version = 1;
        }
        if schema_version < 2 {
            connection
                .execute_batch("PRAGMA user_version = 2")
                .map_err(|error| format!("Could not update the local library schema: {error}"))?;
            schema_version = 2;
        }
        if schema_version < 3 {
            let migration = connection
                .transaction()
                .map_err(|error| format!("Could not migrate the local library: {error}"))?;
            migration
                .execute_batch(
                    "ALTER TABLE media_items ADD COLUMN local_title TEXT;
                     ALTER TABLE media_items ADD COLUMN local_kind TEXT;
                     ALTER TABLE media_items ADD COLUMN local_year INTEGER;
                     ALTER TABLE media_items ADD COLUMN local_key TEXT;",
                )
                .map_err(|error| format!("Could not add local media labels: {error}"))?;
            backfill_local_labels(&migration)?;
            migration
                .execute(
                    "UPDATE media_metadata SET looked_up_at = 0 WHERE tmdb_id IS NULL",
                    [],
                )
                .map_err(|error| format!("Could not queue unmatched media for lookup: {error}"))?;
            migration
                .execute_batch("PRAGMA user_version = 3")
                .map_err(|error| format!("Could not update the local library schema: {error}"))?;
            migration.commit().map_err(|error| {
                format!("Could not finish the local library migration: {error}")
            })?;
            schema_version = 3;
        }
        if schema_version < 4 {
            let migration = connection
                .transaction()
                .map_err(|error| format!("Could not migrate the local library: {error}"))?;
            backfill_local_labels(&migration)?;
            migration
                .execute(
                    "UPDATE media_metadata SET looked_up_at = 0 WHERE tmdb_id IS NULL",
                    [],
                )
                .map_err(|error| format!("Could not queue unmatched media for lookup: {error}"))?;
            migration
                .execute_batch("PRAGMA user_version = 4")
                .map_err(|error| format!("Could not update the local library schema: {error}"))?;
            migration.commit().map_err(|error| {
                format!("Could not finish the local library migration: {error}")
            })?;
        }

        let schema_version: i64 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(|error| format!("Could not read the local library schema: {error}"))?;
        if schema_version < 5 {
            connection
                .execute_batch(
                    "CREATE TABLE IF NOT EXISTS playback_progress (
                        media_id INTEGER PRIMARY KEY REFERENCES media_items(id) ON DELETE CASCADE,
                        position_seconds REAL NOT NULL,
                        duration_seconds REAL NOT NULL,
                        updated_at INTEGER NOT NULL
                     );
                     CREATE INDEX IF NOT EXISTS playback_progress_updated
                        ON playback_progress(updated_at DESC);
                     PRAGMA user_version = 5;",
                )
                .map_err(|error| format!("Could not prepare playback history: {error}"))?;
        }

        let schema_version: i64 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(|error| format!("Could not read the local library schema: {error}"))?;
        if schema_version < 6 {
            connection
                .execute_batch(
                    "CREATE TABLE IF NOT EXISTS playback_activity (
                        media_id INTEGER PRIMARY KEY REFERENCES media_items(id) ON DELETE CASCADE,
                        updated_at INTEGER NOT NULL
                     );
                     CREATE INDEX IF NOT EXISTS playback_activity_updated
                        ON playback_activity(updated_at DESC);
                     PRAGMA user_version = 6;",
                )
                .map_err(|error| format!("Could not prepare the playback history: {error}"))?;
        }
        connection
            .execute(
                "INSERT OR IGNORE INTO playback_activity(media_id, updated_at)
                 SELECT media_id, updated_at FROM playback_progress",
                [],
            )
            .map_err(|error| format!("Could not preserve existing playback history: {error}"))?;

        connection.execute_batch(
            "CREATE TABLE IF NOT EXISTS series_watched_before (
                series_key TEXT PRIMARY KEY,
                season INTEGER NOT NULL,
                episode INTEGER NOT NULL
            );"
        ).map_err(|error| format!("Could not prepare series watch status: {error}"))?;

        let extras_initialized: bool = connection.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='media_extras')", [], |row| row.get(0)).map_err(|error| error.to_string())?;
        connection.execute_batch("CREATE TABLE IF NOT EXISTS media_extras (media_id INTEGER PRIMARY KEY REFERENCES media_items(id) ON DELETE CASCADE)").map_err(|error| error.to_string())?;
        if !extras_initialized {
            let mut statement = connection.prepare("SELECT id, relative_path FROM media_items").map_err(|error| error.to_string())?;
            let rows = statement.query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))).map_err(|error| error.to_string())?;
            for row in rows {
                let (id, relative) = row.map_err(|error| error.to_string())?;
                if is_extra_media_path(Path::new(&relative)) {
                    connection.execute("INSERT OR IGNORE INTO media_extras(media_id) VALUES (?1)", [id]).map_err(|error| error.to_string())?;
                }
            }
        }
        // Persist identities separately from paths and row numbers. Moving a file
        // keeps its identity; deleting it cannot redirect an old player to a new row.
        connection.execute_batch(
            "CREATE TABLE IF NOT EXISTS media_identity (
                media_id INTEGER PRIMARY KEY REFERENCES media_items(id) ON DELETE CASCADE,
                uuid TEXT NOT NULL UNIQUE
             );
             CREATE TRIGGER IF NOT EXISTS media_identity_insert AFTER INSERT ON media_items BEGIN
                INSERT INTO media_identity(media_id, uuid) VALUES (NEW.id,
                    lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4' ||
                    substr(lower(hex(randomblob(2))),2) || '-' || substr('89ab',1 + (random() & 3),1) ||
                    substr(lower(hex(randomblob(2))),2) || '-' || lower(hex(randomblob(6))));
             END;
             INSERT INTO media_identity(media_id, uuid)
             SELECT id, lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4' ||
                    substr(lower(hex(randomblob(2))),2) || '-' || substr('89ab',1 + (random() & 3),1) ||
                    substr(lower(hex(randomblob(2))),2) || '-' || lower(hex(randomblob(6)))
             FROM media_items WHERE id NOT IN (SELECT media_id FROM media_identity);"
        ).map_err(|error| format!("Could not prepare media identities: {error}"))?;
        Ok(Self { connection })
    }

    pub fn playback_uuid(&self, media_id: i64) -> Result<String, String> {
        self.connection.query_row("SELECT uuid FROM media_identity WHERE media_id=?1", [media_id], |row| row.get(0))
            .map_err(|error| format!("Could not identify this library file: {error}"))
    }

    pub fn media_id_for_uuid(&self, uuid: &str) -> Result<Option<i64>, String> {
        self.connection.query_row("SELECT media_id FROM media_identity WHERE uuid=?1", [uuid], |row| row.get(0))
            .optional().map_err(|error| format!("Could not identify this library file: {error}"))
    }

    pub fn require_media_identity(&self, media_id: i64, uuid: Option<&str>) -> Result<(), String> {
        if let Some(uuid) = uuid {
            if self.media_id_for_uuid(uuid)? == Some(media_id) { return Ok(()); }
        }
        Err("This library item changed. Refresh the library before moving or deleting it.".into())
    }

    pub fn record_playback_activity(&mut self, media_id: i64) -> Result<(), String> {
        if media_id <= 0 {
            return Err("Playback history item is invalid.".to_owned());
        }
        let exists: bool = self
            .connection
            .query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM media_items AS items
                    JOIN media_files AS files
                      ON files.root_id = items.root_id AND files.relative_path = items.relative_path
                    JOIN library_roots AS roots
                      ON roots.id = files.root_id AND roots.current_generation = files.generation
                    WHERE items.id = ?1
                 )",
                [media_id],
                |row| row.get(0),
            )
            .map_err(|error| format!("Could not find the media item: {error}"))?;
        if !exists {
            return Err("This media item is no longer in the library.".to_owned());
        }
        self.connection
            .execute(
                "INSERT INTO playback_activity(media_id, updated_at)
                 VALUES (?1, ?2)
                 ON CONFLICT(media_id) DO UPDATE SET updated_at = excluded.updated_at",
                params![media_id, unix_time_millis()],
            )
            .map_err(|error| format!("Could not update playback history: {error}"))?;
        Ok(())
    }

    pub fn mark_previous_episodes_watched(&mut self, media_id: i64) -> Result<(), String> {
        let selected: Option<(String, String, String)> = self.connection.query_row(
            "SELECT files.display_name, items.relative_path,
                    CASE WHEN metadata.tmdb_id IS NOT NULL
                         THEN COALESCE(metadata.kind, '') || ':' || metadata.tmdb_id
                         ELSE 'series:' || items.local_key || ':' || COALESCE(items.local_year, '') END
             FROM media_items AS items
             JOIN media_files AS files ON files.root_id = items.root_id AND files.relative_path = items.relative_path
             JOIN library_roots AS roots ON roots.id = files.root_id AND files.generation = roots.current_generation
             LEFT JOIN media_metadata AS metadata ON metadata.media_id = items.id
            WHERE items.id = ?1 AND COALESCE(metadata.kind, items.local_kind) = 'series'
              AND NOT EXISTS (SELECT 1 FROM media_extras WHERE media_id = items.id)",
            [media_id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        ).optional().map_err(|error| format!("Could not identify the selected episode: {error}"))?;
        let Some((name, path, series_key)) = selected else { return Ok(()); };
        let (Some(season), Some(episode)) = episode_position(&name, &path) else { return Ok(()); };
        // Persist a boundary, including earlier episodes which have not been downloaded yet.
        // Rewatching an earlier episode must not undo previously inferred watched status.
        let transaction = self.connection.transaction().map_err(|error| error.to_string())?;
        transaction.execute(
            "INSERT INTO series_watched_before(series_key, season, episode) VALUES (?1, ?2, ?3)
             ON CONFLICT(series_key) DO UPDATE SET season = excluded.season, episode = excluded.episode
             WHERE excluded.season > season OR (excluded.season = season AND excluded.episode > episode)",
            params![series_key, season, episode]
        ).map_err(|error| format!("Could not save earlier episodes' watched status: {error}"))?;
        let earlier_ids = {
            let mut statement = transaction.prepare(
                "SELECT items.id, files.display_name, items.relative_path
                 FROM media_items AS items
                 JOIN media_files AS files ON files.root_id = items.root_id AND files.relative_path = items.relative_path
                 JOIN library_roots AS roots ON roots.id = files.root_id AND files.generation = roots.current_generation
                 LEFT JOIN media_metadata AS metadata ON metadata.media_id = items.id
                 WHERE CASE WHEN metadata.tmdb_id IS NOT NULL
                            THEN COALESCE(metadata.kind, '') || ':' || metadata.tmdb_id
                            ELSE 'series:' || items.local_key || ':' || COALESCE(items.local_year, '') END = ?1
                   AND NOT EXISTS (SELECT 1 FROM media_extras WHERE media_id = items.id)"
            ).map_err(|error| error.to_string())?;
            let rows = statement.query_map([&series_key], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?)))
                .map_err(|error| error.to_string())?;
            let mut ids = Vec::new();
            for row in rows {
                let (id, name, path) = row.map_err(|error| error.to_string())?;
                if let (Some(s), Some(e)) = episode_position(&name, &path) {
                    if (s, e) < (season, episode) { ids.push(id); }
                }
            }
            ids
        };
        for id in earlier_ids {
            transaction.execute("DELETE FROM playback_progress WHERE media_id = ?1", [id]).map_err(|error| error.to_string())?;
        }
        transaction.commit().map_err(|error| error.to_string())
    }

    pub fn playback_history(
        &self,
        requested_count: u32,
    ) -> Result<Vec<PlaybackHistoryItem>, String> {
        let count = requested_count.clamp(1, 48);
        let mut statement = self
            .connection
            .prepare(
                "WITH candidates AS (
                    SELECT activity.media_id AS id,
                           COALESCE(NULLIF(metadata.title, ''), items.local_title, files.display_name) AS title,
                           COALESCE(metadata.kind, items.local_kind) AS kind,
                           COALESCE(metadata.release_year, items.local_year) AS release_year,
                           metadata.overview, metadata.vote_average,
                           metadata.poster_url, metadata.backdrop_url, activity.updated_at,
                           CASE WHEN metadata.tmdb_id IS NOT NULL
                                THEN COALESCE(metadata.kind, '') || ':' || metadata.tmdb_id
                                WHEN items.local_kind = 'series'
                                THEN 'series:' || items.local_key || ':' || COALESCE(items.local_year, '')
                                ELSE 'file:' || items.id END AS group_key
                    FROM playback_activity AS activity
                    JOIN media_items AS items ON items.id = activity.media_id
                    JOIN media_files AS files
                      ON files.root_id = items.root_id AND files.relative_path = items.relative_path
                    JOIN library_roots AS roots
                      ON roots.id = files.root_id AND roots.current_generation = files.generation
                    LEFT JOIN media_metadata AS metadata ON metadata.media_id = items.id
                    WHERE NOT EXISTS (SELECT 1 FROM media_extras WHERE media_id = items.id)
                 ), ranked AS (
                    SELECT *, ROW_NUMBER() OVER (PARTITION BY group_key ORDER BY updated_at DESC) AS rank
                    FROM candidates
                 )
                 SELECT id, title, kind, release_year, overview, vote_average,
                        poster_url, backdrop_url, updated_at
                 FROM ranked WHERE rank = 1 ORDER BY updated_at DESC LIMIT ?1",
            )
            .map_err(|error| format!("Could not prepare playback history: {error}"))?;
        let rows = statement
            .query_map([count], |row| {
                let year: Option<i64> = row.get(3)?;
                Ok(PlaybackHistoryItem {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    kind: row.get(2)?,
                    year: year.and_then(|value| u16::try_from(value).ok()),
                    overview: row.get(4)?,
                    vote_average: row.get(5)?,
                    poster_url: row.get(6)?,
                    backdrop_url: row.get(7)?,
                    updated_at: row.get(8)?,
                })
            })
            .map_err(|error| format!("Could not read playback history: {error}"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("Could not read a playback history item: {error}"))
    }

    pub fn save_playback_progress(
        &mut self,
        media_id: i64,
        position_seconds: f64,
        duration_seconds: f64,
    ) -> Result<(), String> {
        if media_id <= 0
            || !position_seconds.is_finite()
            || !duration_seconds.is_finite()
            || duration_seconds <= 0.0
            || position_seconds < 0.0
        {
            return Err("Playback progress is invalid.".to_owned());
        }
        if position_seconds < 10.0 {
            // Opening a resume item can emit a brief zero-position update while
            // the player is restoring its saved seek. Keep the existing marker.
            return Ok(());
        }
        if position_seconds >= duration_seconds - 20.0
            || position_seconds / duration_seconds >= 0.95
        {
            self.connection
                .execute(
                    "DELETE FROM playback_progress WHERE media_id = ?1",
                    [media_id],
                )
                .map_err(|error| format!("Could not clear completed playback: {error}"))?;
            return Ok(());
        }
        let exists: bool = self
            .connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM media_items WHERE id = ?1)",
                [media_id],
                |row| row.get(0),
            )
            .map_err(|error| format!("Could not find the media item: {error}"))?;
        if !exists {
            return Err("This media item is no longer in the library.".to_owned());
        }
        self.connection
            .execute(
                "INSERT INTO playback_progress(media_id, position_seconds, duration_seconds, updated_at)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(media_id) DO UPDATE SET
                    position_seconds = excluded.position_seconds,
                    duration_seconds = excluded.duration_seconds,
                    updated_at = excluded.updated_at",
                params![media_id, position_seconds, duration_seconds, unix_time_millis()],
            )
            .map_err(|error| format!("Could not save playback progress: {error}"))?;
        Ok(())
    }

    pub fn playback_position(&self, media_id: i64) -> Result<Option<f64>, String> {
        self.connection
            .query_row(
                "SELECT position_seconds FROM playback_progress WHERE media_id = ?1",
                [media_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| format!("Could not read playback position: {error}"))
    }

    pub fn continue_watching(
        &self,
        requested_count: u32,
    ) -> Result<Vec<ContinueWatchingItem>, String> {
        let count = requested_count.clamp(1, 24);
        let mut statement = self
            .connection
            .prepare(&format!(
                "{GROUPED_CATALOG_CTE}, progress_rows AS (
                    SELECT items.id AS playback_id,
                           progress.position_seconds, progress.duration_seconds,
                           progress.updated_at,
                           CASE WHEN metadata.tmdb_id IS NOT NULL
                                THEN COALESCE(metadata.kind, '') || ':' || metadata.tmdb_id
                                WHEN items.local_kind = 'series'
                                THEN 'series:' || items.local_key || ':' || COALESCE(items.local_year, '')
                                ELSE 'file:' || items.id END AS group_key
                    FROM playback_progress AS progress
                    JOIN media_items AS items ON items.id = progress.media_id
                    JOIN media_files AS files
                      ON files.root_id = items.root_id AND files.relative_path = items.relative_path
                    JOIN library_roots AS roots
                      ON roots.id = files.root_id AND roots.current_generation = files.generation
                    LEFT JOIN media_metadata AS metadata ON metadata.media_id = items.id
                    WHERE progress.duration_seconds > 0
                      AND NOT EXISTS (SELECT 1 FROM media_extras WHERE media_id = items.id)
                 ), candidates AS (
                    SELECT grouped.id, progress_rows.playback_id, grouped.title,
                           grouped.kind, grouped.release_year, grouped.overview,
                           grouped.vote_average, grouped.poster_url, grouped.backdrop_url,
                           progress_rows.position_seconds, progress_rows.duration_seconds,
                           progress_rows.updated_at, progress_rows.group_key
                    FROM progress_rows
                    JOIN grouped ON grouped.group_key = progress_rows.group_key
                 ), ranked AS (
                    SELECT *, ROW_NUMBER() OVER (PARTITION BY group_key ORDER BY updated_at DESC) AS rank
                    FROM candidates
                 )
                 SELECT id, playback_id, title, kind, release_year, overview, vote_average,
                        poster_url, backdrop_url,
                        position_seconds, duration_seconds, updated_at
                 FROM ranked WHERE rank = 1 ORDER BY updated_at DESC LIMIT ?1"
            ))
            .map_err(|error| format!("Could not prepare playback history: {error}"))?;
        let rows = statement
            .query_map([count], |row| {
                Ok(ContinueWatchingItem {
                    id: row.get(0)?,
                    playback_id: row.get(1)?,
                    playback_uuid: self.playback_uuid(row.get(1)?) .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    title: row.get(2)?,
                    kind: row.get(3)?,
                    year: row.get::<_, Option<i64>>(4)?.and_then(|value| u16::try_from(value).ok()),
                    overview: row.get(5)?,
                    vote_average: row.get(6)?,
                    poster_url: row.get(7)?,
                    backdrop_url: row.get(8)?,
                    position_seconds: row.get(9)?,
                    duration_seconds: row.get(10)?,
                    updated_at: row.get(11)?,
                })
            })
            .map_err(|error| format!("Could not read playback history: {error}"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("Could not read a playback history item: {error}"))
    }

    pub fn subtitle_files(&self, media_id: i64) -> Result<Vec<PathBuf>, String> {
        let record: Option<(String, String)> = self
            .connection
            .query_row(
                "SELECT roots.canonical_path, items.relative_path
                 FROM media_items AS items
                 JOIN library_roots AS roots ON roots.id = items.root_id
                 JOIN media_files AS files
                   ON files.root_id = items.root_id AND files.relative_path = items.relative_path
                 WHERE items.id = ?1 AND files.generation = roots.current_generation",
                [media_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(|error| format!("Could not find the library item: {error}"))?;
        let Some((root, relative_path)) = record else {
            return Ok(Vec::new());
        };
        let root = PathBuf::from(root);
        let media_path = std::fs::canonicalize(root.join(relative_path))
            .map_err(|error| format!("Could not open the indexed media file: {error}"))?;
        if !media_path.starts_with(&root) || !media_path.is_file() {
            return Err(
                "The indexed media file is no longer inside its library folder.".to_owned(),
            );
        }
        crate::subtitles::discover_subtitle_files(&root, &media_path)
    }

    pub fn catalog_page(
        &mut self,
        offset: u32,
        requested_count: u32,
    ) -> Result<CatalogPage, String> {
        self.catalog_filtered_page(offset, requested_count, None, None, None)
    }

    pub fn catalog_filtered_page(
        &mut self,
        offset: u32,
        requested_count: u32,
        kind: Option<&str>,
        query: Option<&str>,
        sort: Option<&str>,
    ) -> Result<CatalogPage, String> {
        const MAX_PAGE_SIZE: u32 = 200;
        let count = requested_count.clamp(1, MAX_PAGE_SIZE);
        let kind = kind.filter(|value| matches!(*value, "movie" | "series"));
        let query = query.map(str::trim).filter(|value| !value.is_empty());
        let sort = sort.filter(|value| matches!(*value, "Title" | "Rating" | "Recently added"));
        let snapshot = self
            .connection
            .transaction()
            .map_err(|error| format!("Could not read the library catalog: {error}"))?;
        let count_query = format!(
            "{GROUPED_CATALOG_CTE}
             SELECT COUNT(*) FROM grouped
             WHERE (?1 IS NULL OR kind = ?1)
               AND (?2 IS NULL OR lower(title) LIKE '%' || lower(?2) || '%')"
        );
        let total: i64 = snapshot
            .query_row(&count_query, params![kind, query], |row| row.get(0))
            .map_err(|error| format!("Could not count library items: {error}"))?;
        let catalog_query = format!(
            "{GROUPED_CATALOG_CTE}
             SELECT id, title, extension, size_bytes, modified_at, kind, release_year,
                    overview, vote_average, poster_url, backdrop_url
             FROM grouped
             WHERE (?1 IS NULL OR kind = ?1)
               AND (?2 IS NULL OR lower(title) LIKE '%' || lower(?2) || '%')
             ORDER BY CASE WHEN ?3 = 'Rating' THEN vote_average END DESC,
                      CASE WHEN ?3 = 'Recently added' THEN modified_at END DESC,
                      title COLLATE NOCASE, id
             LIMIT ?4 OFFSET ?5"
        );
        let mut statement = snapshot
            .prepare(&catalog_query)
            .map_err(|error| format!("Could not prepare the library catalog: {error}"))?;
        let rows = statement
            .query_map(params![kind, query, sort, count, offset], |row| {
                let size_bytes: i64 = row.get(3)?;
                let year: Option<i64> = row.get(6)?;
                Ok(CatalogMedia {
                    id: row.get(0)?,
                    playback_uuid: String::new(),
                    title: row.get(1)?,
                    extension: row.get(2)?,
                    size_bytes: size_bytes.max(0) as u64,
                    modified_at: row.get(4)?,
                    kind: row.get(5)?,
                    year: year.and_then(|value| u16::try_from(value).ok()),
                    overview: row.get(7)?,
                    vote_average: row.get(8)?,
                    poster_url: row.get(9)?,
                    backdrop_url: row.get(10)?,
                })
            })
            .map_err(|error| format!("Could not read the library catalog: {error}"))?;
        let mut items = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("Could not read a library item: {error}"))?;
        drop(statement);
        snapshot
            .commit()
            .map_err(|error| format!("Could not finish reading the library catalog: {error}"))?;

        for item in &mut items { item.playback_uuid = self.playback_uuid(item.id)?; }

        Ok(CatalogPage {
            items,
            total: total.max(0) as u64,
            offset,
        })
    }

    pub fn catalog_detail(&self, media_id: i64) -> Result<Option<LocalTitleDetail>, String> {
        let media_query = format!(
            "{GROUPED_CATALOG_CTE}
             SELECT id, title, extension, size_bytes, modified_at, kind, release_year,
                    overview, vote_average, poster_url, backdrop_url, group_key
             FROM grouped WHERE id = ?1 LIMIT 1"
        );
        let selected: Option<(CatalogMedia, String)> = self
            .connection
            .query_row(&media_query, [media_id], |row| {
                let size_bytes: i64 = row.get(3)?;
                let year: Option<i64> = row.get(6)?;
                Ok((
                    CatalogMedia {
                        id: row.get(0)?,
                        playback_uuid: self.playback_uuid(row.get(0)?).map_err(|_| rusqlite::Error::InvalidQuery)?,
                        title: row.get(1)?,
                        extension: row.get(2)?,
                        size_bytes: size_bytes.max(0) as u64,
                        modified_at: row.get(4)?,
                        kind: row.get(5)?,
                        year: year.and_then(|value| u16::try_from(value).ok()),
                        overview: row.get(7)?,
                        vote_average: row.get(8)?,
                        poster_url: row.get(9)?,
                        backdrop_url: row.get(10)?,
                    },
                    row.get(11)?,
                ))
            })
            .optional()
            .map_err(|error| format!("Could not read this title's details: {error}"))?;
        let Some((media, group_key)) = selected else {
            let extra: Option<(String, String, String, i64, String)> = self.connection.query_row("SELECT files.display_name, files.extension, roots.canonical_path, files.size_bytes, files.relative_path FROM media_extras JOIN media_items AS items ON items.id=media_extras.media_id JOIN library_roots AS roots ON roots.id=items.root_id JOIN media_files AS files ON files.root_id=items.root_id AND files.relative_path=items.relative_path AND files.generation=roots.current_generation WHERE items.id=?1", [media_id], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?))).optional().map_err(|error| error.to_string())?;
            if let Some((name, extension, root, size, relative)) = extra {
                let Ok(canonical_root) = std::fs::canonicalize(&root) else { return Ok(None); };
                let Ok(path) = std::fs::canonicalize(Path::new(&root).join(relative)) else { return Ok(None); };
                if !path.starts_with(canonical_root) || !path.is_file() { return Ok(None); }
                return Ok(Some(LocalTitleDetail {
                    media: CatalogMedia { id:media_id, playback_uuid:self.playback_uuid(media_id)?, title:name.replace(['.', '_'], " "), extension, size_bytes:size.max(0) as u64, modified_at:None, kind:Some("movie".into()), year:None, overview:None, vote_average:None, poster_url:None, backdrop_url:None },
                    files:vec![LocalEpisodeFile { media_id, playback_uuid:self.playback_uuid(media_id)?, file_name:name, path:path.to_string_lossy().into_owned(), season:None, episode:None }],
                    extras:vec![], watched_before:None, tmdb_id:None,
                }));
            }
            return Ok(None);
        };

        let files_query =
            "SELECT items.id, files.display_name, roots.canonical_path, items.relative_path
             FROM media_items AS items
             JOIN library_roots AS roots ON roots.id = items.root_id
             JOIN media_files AS files
               ON files.root_id = items.root_id AND files.relative_path = items.relative_path
              AND files.generation = roots.current_generation
             LEFT JOIN media_metadata AS metadata ON metadata.media_id = items.id
             WHERE NOT EXISTS (SELECT 1 FROM media_extras WHERE media_id = items.id)
               AND CASE WHEN metadata.tmdb_id IS NOT NULL
                        THEN COALESCE(metadata.kind, '') || ':' || metadata.tmdb_id
                        WHEN items.local_kind = 'series'
                        THEN 'series:' || items.local_key || ':' || COALESCE(items.local_year, '')
                        ELSE 'file:' || items.id END = ?1
             ORDER BY files.relative_path COLLATE NOCASE";
        let mut statement = self
            .connection
            .prepare(files_query)
            .map_err(|error| format!("Could not prepare the episode list: {error}"))?;
        let rows = statement
            .query_map([&group_key], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .map_err(|error| format!("Could not read the episode list: {error}"))?;
        let mut files = Vec::new();
        for row in rows {
            let (episode_id, file_name, root, relative_path) =
                row.map_err(|error| format!("Could not read an episode: {error}"))?;
            let root = PathBuf::from(root);
            let Ok(canonical_root) = std::fs::canonicalize(&root) else {
                continue;
            };
            let Ok(path) = std::fs::canonicalize(root.join(&relative_path)) else {
                continue;
            };
            if !path.starts_with(&canonical_root) || !path.is_file() {
                continue;
            }
            let (season, episode) = episode_position(&file_name, &relative_path);
            files.push(LocalEpisodeFile {
                media_id: episode_id,
                playback_uuid: self.playback_uuid(episode_id)?,
                file_name,
                path: path.to_string_lossy().into_owned(),
                season,
                episode,
            });
        }
        let watched_before = self.connection.query_row(
            "SELECT season, episode FROM series_watched_before WHERE series_key = ?1",
            [&group_key], |row| Ok(EpisodePosition { season: row.get(0)?, episode: row.get(1)? })
        ).optional().map_err(|error| format!("Could not read series watch status: {error}"))?;
        let tmdb_id = self.tmdb_target(media.id)?.map(|(id, _)| id);
        let mut extras = Vec::new();
        let mut extra_statement = self.connection.prepare("SELECT items.id, files.display_name, roots.canonical_path, items.relative_path FROM media_extras JOIN media_items AS items ON items.id=media_extras.media_id JOIN library_roots AS roots ON roots.id=items.root_id JOIN media_files AS files ON files.root_id=items.root_id AND files.relative_path=items.relative_path AND files.generation=roots.current_generation ORDER BY files.relative_path COLLATE NOCASE").map_err(|error| error.to_string())?;
        let rows = extra_statement.query_map([], |row| Ok((row.get::<_, i64>(0)?,row.get::<_, String>(1)?,row.get::<_, String>(2)?,row.get::<_, String>(3)?))).map_err(|error| error.to_string())?;
        for row in rows {
            let (id, file_name, root, relative) = row.map_err(|error| error.to_string())?;
            let Ok(root) = std::fs::canonicalize(root) else { continue; };
            let mut base = root.clone();
            for component in Path::new(&relative).components() {
                if is_extra_media_folder(Path::new(component.as_os_str())) { break; }
                base.push(component);
            }
            if !files.iter().any(|file| Path::new(&file.path).starts_with(&base)) { continue; }
            // An Extras folder shared by a multi-title library cannot be assigned safely.
            if base == root {
                let main_count: i64 = self.connection.query_row("SELECT COUNT(*) FROM media_files AS files JOIN library_roots AS roots ON roots.id=files.root_id JOIN media_items AS items ON items.root_id=files.root_id AND items.relative_path=files.relative_path WHERE roots.canonical_path=?1 AND files.generation=roots.current_generation AND NOT EXISTS(SELECT 1 FROM media_extras WHERE media_id=items.id)", [root.to_string_lossy().as_ref()], |row| row.get(0)).map_err(|error| error.to_string())?;
                if main_count != 1 { continue; }
            }
            let Ok(path) = std::fs::canonicalize(root.join(relative)) else { continue; };
            if path.starts_with(&root) && path.is_file() {
                extras.push(LocalEpisodeFile { media_id:id, playback_uuid:self.playback_uuid(id)?, file_name, path:path.to_string_lossy().into_owned(), season:None, episode:None });
            }
        }
        Ok(Some(LocalTitleDetail { media, files, extras, watched_before, tmdb_id }))
    }

    pub fn tmdb_target(&self, media_id: i64) -> Result<Option<(u64, String)>, String> {
        let target: Option<(i64, String)> = self
            .connection
            .query_row(
                "SELECT metadata.tmdb_id, metadata.kind
             FROM media_metadata AS metadata
             JOIN media_items AS items ON items.id = metadata.media_id
             JOIN media_files AS files ON files.root_id = items.root_id
               AND files.relative_path = items.relative_path
             JOIN library_roots AS roots ON roots.id = files.root_id
             WHERE items.id = ?1 AND files.generation = roots.current_generation
               AND metadata.tmdb_id IS NOT NULL AND metadata.kind IN ('movie', 'series')",
                [media_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(|error| format!("Could not read this title's TMDb metadata: {error}"))?;
        Ok(target.and_then(|(id, kind)| u64::try_from(id).ok().map(|id| (id, kind))))
    }

    pub fn resolve_media_path(&self, media_id: i64) -> Result<Option<PathBuf>, String> {
        let record: Option<(String, String)> = self
            .connection
            .query_row(
                "SELECT roots.canonical_path, items.relative_path
                 FROM media_items AS items
                 JOIN library_roots AS roots ON roots.id = items.root_id
                 JOIN media_files AS files
                   ON files.root_id = items.root_id AND files.relative_path = items.relative_path
                 WHERE items.id = ?1 AND files.generation = roots.current_generation",
                [media_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(|error| format!("Could not find the library item: {error}"))?;
        let Some((root, relative_path)) = record else {
            return Ok(None);
        };
        let root = PathBuf::from(root);
        let file = std::fs::canonicalize(root.join(relative_path))
            .map_err(|error| format!("Could not open the indexed media file: {error}"))?;
        if !file.starts_with(&root) || !file.is_file() {
            return Err(
                "The indexed media file is no longer inside its library folder.".to_owned(),
            );
        }
        Ok(Some(file))
    }

    /// Move every indexed file belonging to a grouped title (including its Extras) into
    /// another configured library root while retaining media IDs and playback metadata.
    pub fn move_title(&mut self, media_id: i64, destination_root: &Path) -> Result<(), String> {
        let detail = self.catalog_detail(media_id)?.ok_or_else(|| "This title is no longer in the library.".to_owned())?;
        let destination_root = std::fs::canonicalize(destination_root)
            .map_err(|error| format!("Could not open the destination library folder: {error}"))?;
        if !destination_root.is_dir() {
            return Err("The destination must be an existing library folder.".into());
        }
        let registered: bool = self.connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM library_roots WHERE canonical_path = ?1)",
            [destination_root.to_string_lossy().as_ref()], |row| row.get(0)
        ).map_err(|error| format!("Could not verify the destination library folder: {error}"))?;
        if !registered { return Err("Choose a folder that is already included in your library.".into()); }
        let (target_root_id, target_generation): (i64, i64) = self.connection.query_row(
            "SELECT id, current_generation FROM library_roots WHERE canonical_path = ?1",
            [destination_root.to_string_lossy().as_ref()], |row| Ok((row.get(0)?, row.get(1)?))
        ).map_err(|error| format!("Could not locate the destination folder: {error}"))?;

        let mut ids: Vec<i64> = detail.files.iter().chain(detail.extras.iter()).map(|file| file.media_id).collect();
        ids.sort_unstable();
        ids.dedup();
        if ids.is_empty() { return Err("No indexed files were found for this title.".into()); }

        #[derive(Clone)]
        struct MoveRecord { id: i64, file_id: i64, source: PathBuf, destination: PathBuf, relative: String, display_name: String, extension: String, size: i64, modified: Option<i64>, generation: i64 }
        let mut records = Vec::with_capacity(ids.len());
        let mut planned_destinations = std::collections::HashSet::new();
        for id in ids {
            let row: Option<(String, String, i64, String, String, i64, Option<i64>)> = self.connection.query_row(
                "SELECT roots.canonical_path, items.relative_path, files.id, files.display_name, files.extension, files.size_bytes, files.modified_at
                 FROM media_items AS items JOIN library_roots AS roots ON roots.id = items.root_id
                 JOIN media_files AS files ON files.root_id = items.root_id AND files.relative_path = items.relative_path AND files.generation = roots.current_generation
                 WHERE items.id = ?1", [id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?))
            ).optional().map_err(|error| format!("Could not find an indexed file: {error}"))?;
            let Some((source_root, relative, file_id, display_name, extension, size, modified)) = row else {
                return Err("A file belonging to this title is no longer indexed. Refresh the library and try again.".into());
            };
            let relative_path = Path::new(&relative);
            if relative_path.as_os_str().is_empty() || relative_path.components().any(|part| !matches!(part, std::path::Component::Normal(_))) {
                return Err("The library contains an unsafe file path; move cancelled.".into());
            }
            let canonical_source_root = std::fs::canonicalize(&source_root).map_err(|e| format!("Could not open a source library folder: {e}"))?;
            let source = std::fs::canonicalize(canonical_source_root.join(relative_path)).map_err(|e| format!("Could not open an indexed file: {e}"))?;
            if !source.starts_with(&canonical_source_root) || !source.is_file() { return Err("A file is outside its registered library folder; move cancelled.".into()); }
            let destination = destination_root.join(relative_path);
            if !planned_destinations.insert(destination.clone()) {
                return Err(format!("More than one file has the same relative path ({}), so this title cannot be moved to that folder without overwriting media.", relative));
            }
            let occupied: bool = self.connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM media_items WHERE root_id = (SELECT id FROM library_roots WHERE canonical_path = ?1) AND relative_path = ?2 AND id != ?3)",
                params![destination_root.to_string_lossy().as_ref(), relative, id], |row| row.get(0)
            ).map_err(|error| format!("Could not check the destination path: {error}"))?;
            if occupied || (destination.exists() && destination != source) {
                return Err(format!("The destination already contains a file at {}.", relative));
            }
            records.push(MoveRecord { id, file_id, source, destination, relative, display_name, extension, size, modified, generation: target_generation });
        }

        let mut moved: Vec<(PathBuf, PathBuf)> = Vec::new();
        for record in &records {
            if record.source == record.destination { continue; }
            let parent = record.destination.parent().ok_or_else(|| "The destination path is invalid.".to_owned())?;
            if let Err(error) = std::fs::create_dir_all(parent) {
                rollback_moves(&moved);
                return Err(format!("Could not create the destination folder: {error}"));
            }
            let canonical_parent = match std::fs::canonicalize(parent) {
                Ok(path) if path.starts_with(&destination_root) => path,
                _ => { rollback_moves(&moved); return Err("The destination path leaves the selected library folder.".into()); }
            };
            let _ = canonical_parent;
            if let Err(error) = move_file(&record.source, &record.destination) {
                rollback_moves(&moved);
                return Err(format!("Could not move {}: {error}", record.display_name));
            }
            moved.push((record.source.clone(), record.destination.clone()));
        }

        let transaction = match self.connection.transaction() {
            Ok(transaction) => transaction,
            Err(error) => { rollback_moves(&moved); return Err(format!("Could not update the library index: {error}")); }
        };
        for record in &records {
            let result = (|| -> Result<(), rusqlite::Error> {
                transaction.execute("UPDATE media_items SET root_id = ?1, relative_path = ?2 WHERE id = ?3", params![target_root_id, record.relative, record.id])?;
                transaction.execute("UPDATE media_files SET root_id = ?1, relative_path = ?2, display_name = ?3, extension = ?4, size_bytes = ?5, modified_at = ?6, generation = ?7 WHERE id = ?8", params![target_root_id, record.relative, record.display_name, record.extension, record.size, record.modified, record.generation, record.file_id])?;
                Ok(())
            })();
            if let Err(error) = result {
                drop(transaction);
                rollback_moves(&moved);
                return Err(format!("Could not update the library index: {error}"));
            }
        }
        if let Err(error) = transaction.commit() {
            rollback_moves(&moved);
            return Err(format!("Could not finish updating the library index: {error}"));
        }
        Ok(())
    }

    /// Permanently remove the indexed files and their library metadata.
    pub fn permanently_delete_title(&mut self, media_id: i64) -> Result<(), String> {
        let detail = self.catalog_detail(media_id)?.ok_or_else(|| "This title is no longer in the library.".to_owned())?;
        let mut ids: Vec<i64> = detail.files.iter().chain(detail.extras.iter()).map(|file| file.media_id).collect();
        ids.sort_unstable();
        ids.dedup();
        if ids.is_empty() { return Err("No indexed files were found for this title.".into()); }
        let mut paths = Vec::with_capacity(ids.len());
        for id in &ids {
            let row: Option<(String, String)> = self.connection.query_row(
                "SELECT roots.canonical_path, items.relative_path FROM media_items AS items JOIN library_roots AS roots ON roots.id = items.root_id JOIN media_files AS files ON files.root_id = items.root_id AND files.relative_path = items.relative_path AND files.generation = roots.current_generation WHERE items.id = ?1",
                [id], |row| Ok((row.get(0)?, row.get(1)?))
            ).optional().map_err(|error| format!("Could not inspect a title file: {error}"))?;
            let Some((root, relative)) = row else { return Err("A file belonging to this title is no longer indexed; deletion cancelled.".into()); };
            let canonical_root = std::fs::canonicalize(root).map_err(|error| format!("Could not open a library folder: {error}"))?;
            let file = std::fs::canonicalize(canonical_root.join(relative)).map_err(|error| format!("Could not open a title file: {error}"))?;
            if !file.starts_with(&canonical_root) || !file.is_file() { return Err("A title file is outside its registered library folder; deletion cancelled.".into()); }
            paths.push(file);
        }
        for path in &paths {
            std::fs::remove_file(path).map_err(|error| format!("Could not permanently delete {}: {error}", path.file_name().unwrap_or_default().to_string_lossy()))?;
        }
        let transaction = self.connection.transaction().map_err(|error| format!("Could not update the library index: {error}"))?;
        for id in ids { transaction.execute("DELETE FROM media_items WHERE id = ?1", [id]).map_err(|error| format!("Could not remove a deleted file from the library index: {error}"))?; }
        transaction.commit().map_err(|error| format!("Could not finish removing the title from the library: {error}"))?;
        Ok(())
    }

    pub fn status(&self, is_scanning: bool) -> Result<LibraryStatus, String> {
        let root_count: i64 = self
            .connection
            .query_row("SELECT COUNT(*) FROM library_roots", [], |row| row.get(0))
            .map_err(|error| format!("Could not read local library roots: {error}"))?;
        let file_count: i64 = self
            .connection
            .query_row(
                "SELECT COUNT(*) FROM media_files AS files
                 JOIN library_roots AS roots ON roots.id = files.root_id
                 WHERE files.generation = roots.current_generation",
                [],
                |row| row.get(0),
            )
            .map_err(|error| format!("Could not read indexed media files: {error}"))?;
        let (matched_count, unmatched_count): (i64, i64) = self
            .connection
            .query_row(
                "SELECT COALESCE(SUM(CASE WHEN metadata.tmdb_id IS NOT NULL THEN 1 ELSE 0 END), 0),
                        COALESCE(SUM(CASE WHEN metadata.media_id IS NOT NULL AND metadata.tmdb_id IS NULL THEN 1 ELSE 0 END), 0)
                 FROM media_files AS files
                 JOIN library_roots AS roots ON roots.id = files.root_id
                 JOIN media_items AS items
                   ON items.root_id = files.root_id AND items.relative_path = files.relative_path
                 LEFT JOIN media_metadata AS metadata ON metadata.media_id = items.id
                 WHERE files.generation = roots.current_generation",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(|error| format!("Could not read media metadata status: {error}"))?;
        let last_scan_at = self
            .connection
            .query_row(
                "SELECT MAX(last_scanned_at) FROM library_roots",
                [],
                |row| row.get(0),
            )
            .map_err(|error| format!("Could not read the last library scan time: {error}"))?;

        let mut folder_statement = self.connection.prepare("SELECT canonical_path FROM library_roots ORDER BY id")
            .map_err(|error| format!("Could not read library folders: {error}"))?;
        let folders = folder_statement.query_map([], |row| row.get::<_, String>(0))
            .map_err(|error| format!("Could not read library folders: {error}"))?
            .collect::<Result<Vec<_>, _>>().map_err(|error| format!("Could not read library folders: {error}"))?;
        Ok(LibraryStatus {
            folders,
            root_count: root_count.max(0) as u64,
            file_count: file_count.max(0) as u64,
            matched_count: matched_count.max(0) as u64,
            unmatched_count: unmatched_count.max(0) as u64,
            pending_count: (file_count - matched_count - unmatched_count).max(0) as u64,
            last_scan_at,
            is_scanning,
        })
    }

    pub fn metadata_candidates(
        &self,
        requested_count: u32,
    ) -> Result<Vec<MetadataCandidate>, String> {
        let count = requested_count.clamp(1, 256);
        let mut statement = self
            .connection
            .prepare(
                "SELECT items.id, files.display_name, files.relative_path
                 FROM media_files AS files
                 JOIN library_roots AS roots ON roots.id = files.root_id
                 JOIN media_items AS items
                   ON items.root_id = files.root_id AND items.relative_path = files.relative_path
                 LEFT JOIN media_metadata AS metadata ON metadata.media_id = items.id
                 WHERE files.generation = roots.current_generation
                   AND NOT EXISTS (SELECT 1 FROM media_extras WHERE media_id = items.id)
                   AND (metadata.media_id IS NULL OR metadata.looked_up_at < ?1)
                 ORDER BY items.id LIMIT ?2",
            )
            .map_err(|error| format!("Could not prepare media metadata candidates: {error}"))?;
        let retry_before = unix_time().saturating_sub(30 * 24 * 60 * 60);
        let rows = statement
            .query_map(params![retry_before, count], |row| {
                Ok(MetadataCandidate {
                    media_id: row.get(0)?,
                    file_title: row.get(1)?,
                    relative_path: row.get(2)?,
                })
            })
            .map_err(|error| format!("Could not read media metadata candidates: {error}"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("Could not read a media metadata candidate: {error}"))
    }

    pub fn retry_unmatched_metadata(&mut self) -> Result<(), String> {
        self.connection
            .execute(
                "UPDATE media_metadata SET looked_up_at = 0
                 WHERE tmdb_id IS NULL OR poster_url IS NULL",
                [],
            )
            .map_err(|error| {
                format!("Could not queue incomplete artwork for metadata refresh: {error}")
            })?;
        Ok(())
    }

    pub fn save_metadata_lookups(&mut self, lookups: &[MetadataLookup]) -> Result<(), String> {
        let transaction = self
            .connection
            .transaction()
            .map_err(|error| format!("Could not save media metadata: {error}"))?;
        let looked_up_at = unix_time();
        {
            let mut statement = transaction
                .prepare(
                    "INSERT INTO media_metadata(
                        media_id, tmdb_id, kind, title, release_year, overview,
                        vote_average, poster_url, backdrop_url, looked_up_at
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                     ON CONFLICT(media_id) DO UPDATE SET
                        tmdb_id = excluded.tmdb_id,
                        kind = excluded.kind,
                        title = excluded.title,
                        release_year = excluded.release_year,
                        overview = excluded.overview,
                        vote_average = excluded.vote_average,
                        poster_url = excluded.poster_url,
                        backdrop_url = excluded.backdrop_url,
                        looked_up_at = excluded.looked_up_at",
                )
                .map_err(|error| format!("Could not prepare media metadata writes: {error}"))?;
            for lookup in lookups {
                for media_id in &lookup.media_ids {
                    let metadata = lookup.metadata.as_ref();
                    statement
                        .execute(params![
                            media_id,
                            metadata.map(|item| item.tmdb_id as i64),
                            metadata.map(|item| item.kind.as_str()),
                            metadata.map(|item| item.title.as_str()),
                            metadata.and_then(|item| item.year),
                            metadata.map(|item| item.overview.as_str()),
                            metadata.and_then(|item| item.vote_average),
                            metadata.and_then(|item| item.poster_url.as_deref()),
                            metadata.and_then(|item| item.backdrop_url.as_deref()),
                            looked_up_at,
                        ])
                        .map_err(|error| format!("Could not save media metadata: {error}"))?;
                    if metadata.is_some_and(|item| item.kind == "series") {
                        transaction.execute(
                            "INSERT INTO series_watched_before(series_key, season, episode)
                             SELECT 'series:' || metadata.tmdb_id, watched.season, watched.episode
                             FROM media_items AS items
                             JOIN media_metadata AS metadata ON metadata.media_id = items.id
                             JOIN series_watched_before AS watched
                               ON watched.series_key = 'series:' || items.local_key || ':' || COALESCE(items.local_year, '')
                             WHERE items.id = ?1
                             ON CONFLICT(series_key) DO UPDATE SET season = excluded.season, episode = excluded.episode
                             WHERE excluded.season > season OR (excluded.season = season AND excluded.episode > episode)",
                            [media_id]
                        ).map_err(|error| format!("Could not preserve series watch status: {error}"))?;
                    }
                }
            }
        }
        transaction
            .commit()
            .map_err(|error| format!("Could not finish saving media metadata: {error}"))
    }

    pub fn metadata_counts(&self) -> Result<(u64, u64, u64), String> {
        let status = self.status(false)?;
        Ok((
            status.matched_count,
            status.unmatched_count,
            status.pending_count,
        ))
    }

    pub fn scan_root(&mut self, requested_path: &str) -> Result<ScanSummary, String> {
        let requested_metadata = std::fs::symlink_metadata(requested_path)
            .map_err(|error| format!("Could not inspect the selected folder: {error}"))?;
        if requested_metadata.file_type().is_symlink() {
            return Err("Choose the folder itself, not a symbolic link.".to_owned());
        }
        let root = std::fs::canonicalize(requested_path)
            .map_err(|error| format!("Could not open the selected folder: {error}"))?;
        let root_metadata = std::fs::metadata(&root)
            .map_err(|error| format!("Could not read the selected folder: {error}"))?;
        if !root_metadata.is_dir() {
            return Err("Select a folder to scan.".to_owned());
        }

        let canonical_path = root.to_str().ok_or_else(|| {
            "The selected folder path cannot be represented on this system.".to_owned()
        })?;
        self.connection
            .execute(
                "INSERT INTO library_roots(canonical_path) VALUES (?1)
                 ON CONFLICT(canonical_path) DO NOTHING",
                [canonical_path],
            )
            .map_err(|error| format!("Could not add the selected folder: {error}"))?;
        let root_id: i64 = self
            .connection
            .query_row(
                "SELECT id FROM library_roots WHERE canonical_path = ?1",
                [canonical_path],
                |row| row.get(0),
            )
            .map_err(|error| format!("Could not find the selected library folder: {error}"))?;
        let current_generation: i64 = self
            .connection
            .query_row(
                "SELECT current_generation FROM library_roots WHERE id = ?1",
                [root_id],
                |row| row.get(0),
            )
            .map_err(|error| format!("Could not read the selected library folder: {error}"))?;
        let result = self.scan_generation(&root, root_id, current_generation);
        if result.is_err() {
            let staging_generation = current_generation + 1;
            let _ = self.connection.execute(
                "DELETE FROM media_files WHERE root_id = ?1 AND generation = ?2",
                params![root_id, staging_generation],
            );
            if current_generation == 0 {
                let _ = self.connection.execute(
                    "DELETE FROM library_roots WHERE id = ?1 AND current_generation = 0 AND last_scanned_at IS NULL",
                    [root_id],
                );
            }
        }
        result
    }

    /// Import only the completed files supplied by the torrent engine, without walking folders.
    pub fn import_completed_files(&mut self, download_root: &Path, paths: &[PathBuf]) -> Result<u64, String> {
        let download_root = std::fs::canonicalize(download_root).map_err(|error| format!("Could not open torrent downloads: {error}"))?;
        let roots: Vec<(i64, PathBuf, i64)> = {
            let mut statement = self.connection.prepare("SELECT id, canonical_path, current_generation FROM library_roots")
                .map_err(|error| error.to_string())?;
            let rows = statement.query_map([], |row| Ok((row.get(0)?, PathBuf::from(row.get::<_, String>(1)?), row.get(2)?)))
                .map_err(|error| error.to_string())?;
            rows.collect::<Result<_, _>>().map_err(|error| error.to_string())?
        };
        let mut imported = 0;
        for path in paths {
            if !is_video_file(path) { continue; }
            let path = std::fs::canonicalize(path).map_err(|error| format!("Could not open completed torrent file: {error}"))?;
            if !path.starts_with(&download_root) { return Err("Torrent file is outside the download folder.".into()); }
            let metadata = std::fs::metadata(&path).map_err(|error| error.to_string())?;
            if !metadata.is_file() { continue; }
            let (root_id, root, generation) = if let Some((id, root, generation)) = roots.iter()
                .filter(|(_, root, _)| path.starts_with(root)).max_by_key(|(_, root, _)| root.components().count()) {
                (*id, root.clone(), *generation)
            } else {
                let root = download_root.to_string_lossy();
                self.connection.execute("INSERT INTO library_roots(canonical_path) VALUES (?1) ON CONFLICT(canonical_path) DO NOTHING", [root.as_ref()]).map_err(|error| error.to_string())?;
                let (id, generation) = self.connection.query_row("SELECT id, current_generation FROM library_roots WHERE canonical_path = ?1", [root.as_ref()], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))).map_err(|error| error.to_string())?;
                (id, download_root.clone(), generation)
            };
            let relative_path = path.strip_prefix(&root).map_err(|error| error.to_string())?.to_string_lossy().replace('\\', "/");
            let file_name = path.file_stem().and_then(|value| value.to_str()).unwrap_or("Untitled").to_owned();
            let parsed = parse_media_name(&file_name, &relative_path);
            let local_title = parsed.as_ref().map(|name| name.query.clone()).unwrap_or_else(|| file_name.clone());
            let local_key = parsed.as_ref().map(|name| name.normalized.clone()).filter(|key| !key.is_empty()).unwrap_or_else(|| normalize_title(&local_title));
            let record = MediaFileRecord {
                local_kind: parsed.as_ref().map(|name| name.kind.clone()).unwrap_or_else(|| infer_media_kind(&file_name, &relative_path).to_owned()),
                local_year: parsed.and_then(|name| name.year), local_title, local_key, relative_path, display_name: file_name,
                extension: path.extension().and_then(|value| value.to_str()).unwrap_or_default().to_ascii_lowercase(),
                size_bytes: metadata.len().min(i64::MAX as u64) as i64,
                modified_at: metadata.modified().ok().and_then(|time| time.duration_since(UNIX_EPOCH).ok()).map(|duration| duration.as_secs().min(i64::MAX as u64) as i64),
            };
            write_batch(&mut self.connection, root_id, generation, &[record])?;
            imported += 1;
        }
        Ok(imported)
    }

    pub fn rescan_roots(&mut self) -> Result<LibraryScanSummary, String> {
        let roots = {
            let mut statement = self
                .connection
                .prepare("SELECT canonical_path FROM library_roots ORDER BY id")
                .map_err(|error| format!("Could not read library folders: {error}"))?;
            let rows = statement
                .query_map([], |row| row.get::<_, String>(0))
                .map_err(|error| format!("Could not read library folders: {error}"))?;
            rows.collect::<Result<Vec<_>, _>>()
                .map_err(|error| format!("Could not read a library folder: {error}"))?
        };

        if roots.is_empty() {
            return Err("Choose a folder before refreshing the library.".to_owned());
        }

        let root_count = roots.len() as u64;
        let mut file_count = 0_u64;
        for root in roots {
            file_count = file_count.saturating_add(self.scan_root(&root)?.file_count);
        }

        Ok(LibraryScanSummary {
            root_count,
            file_count,
            matched_count: 0,
            unmatched_count: 0,
            pending_count: file_count,
            metadata_error: None,
        })
    }

    fn scan_generation(
        &mut self,
        root: &Path,
        root_id: i64,
        current_generation: i64,
    ) -> Result<ScanSummary, String> {
        let generation = current_generation + 1;

        let mut pending = Vec::with_capacity(BATCH_SIZE);
        let mut file_count = 0_u64;
        for entry in WalkDir::new(&root)
            .follow_links(false)
            .max_depth(MAX_SCAN_DEPTH)
            .into_iter()
        {
            let entry = entry.map_err(|error| {
                format!("The scan could not read part of the selected folder: {error}")
            })?;
            if !entry.file_type().is_file() || !is_video_file(entry.path()) {
                continue;
            }

            file_count += 1;
            if file_count > MAX_FILES_PER_ROOT as u64 {
                return Err(format!(
                    "This folder contains more than {MAX_FILES_PER_ROOT} video files; the scan was stopped safely."
                ));
            }

            let relative_path = entry
                .path()
                .strip_prefix(&root)
                .map_err(|_| {
                    "A file was outside the selected folder; the scan was stopped.".to_owned()
                })?
                .to_string_lossy()
                .replace('\\', "/");
            let metadata = entry
                .metadata()
                .map_err(|error| format!("Could not read a media file: {error}"))?;
            let file_name = entry
                .path()
                .file_stem()
                .and_then(|value| value.to_str())
                .unwrap_or("Untitled")
                .to_owned();
            let parsed_name = parse_media_name(&file_name, &relative_path);
            let local_title = parsed_name
                .as_ref()
                .map(|item| item.query.clone())
                .unwrap_or_else(|| file_name.clone());
            let local_key = parsed_name
                .as_ref()
                .map(|item| item.normalized.clone())
                .filter(|key| !key.is_empty())
                .unwrap_or_else(|| normalize_title(&local_title));
            let local_kind = parsed_name
                .as_ref()
                .map(|item| item.kind.clone())
                .unwrap_or_else(|| infer_media_kind(&file_name, &relative_path).to_owned());
            let extension = entry
                .path()
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or_default()
                .to_ascii_lowercase();
            let modified_at = metadata
                .modified()
                .ok()
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                .map(|duration| duration.as_secs().min(i64::MAX as u64) as i64);

            pending.push(MediaFileRecord {
                relative_path,
                display_name: file_name,
                local_title,
                local_kind,
                local_year: parsed_name.and_then(|item| item.year),
                local_key,
                extension,
                size_bytes: metadata.len().min(i64::MAX as u64) as i64,
                modified_at,
            });

            if pending.len() >= BATCH_SIZE {
                write_batch(&mut self.connection, root_id, generation, &pending)?;
                pending.clear();
            }
        }

        if !pending.is_empty() {
            write_batch(&mut self.connection, root_id, generation, &pending)?;
        }

        let scanned_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs().min(i64::MAX as u64) as i64)
            .unwrap_or_default();
        let transaction = self
            .connection
            .transaction()
            .map_err(|error| format!("Could not finish the library scan: {error}"))?;
        transaction
            .execute(
                "UPDATE library_roots SET current_generation = ?1, last_scanned_at = ?2 WHERE id = ?3",
                params![generation, scanned_at, root_id],
            )
            .map_err(|error| format!("Could not publish the scan results: {error}"))?;
        transaction
            .execute(
                "DELETE FROM media_files WHERE root_id = ?1 AND generation != ?2",
                params![root_id, generation],
            )
            .map_err(|error| format!("Could not clean up previous scan records: {error}"))?;
        transaction
            .commit()
            .map_err(|error| format!("Could not commit the library scan: {error}"))?;

        let root_name = root
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
            .unwrap_or("Media folder")
            .to_owned();

        Ok(ScanSummary {
            root_name,
            file_count,
            matched_count: 0,
            unmatched_count: 0,
            pending_count: file_count,
            metadata_error: None,
        })
    }
}

fn unix_time() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs().min(i64::MAX as u64) as i64)
        .unwrap_or_default()
}

fn unix_time_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(i64::MAX as u128) as i64)
        .unwrap_or_default()
}

fn backfill_local_labels(transaction: &Transaction<'_>) -> Result<(), String> {
    let active_files = {
        let mut statement = transaction
            .prepare(
                "SELECT items.id, files.display_name, files.relative_path
                 FROM media_files AS files
                 JOIN library_roots AS roots ON roots.id = files.root_id
                 JOIN media_items AS items
                   ON items.root_id = files.root_id AND items.relative_path = files.relative_path
                 WHERE files.generation = roots.current_generation",
            )
            .map_err(|error| format!("Could not prepare local media labels: {error}"))?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(|error| format!("Could not read local media labels: {error}"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("Could not read a local media label: {error}"))?
    };
    let mut update = transaction
        .prepare(
            "UPDATE media_items
             SET local_title = ?1, local_kind = ?2, local_year = ?3, local_key = ?4
             WHERE id = ?5",
        )
        .map_err(|error| format!("Could not prepare local media labels: {error}"))?;
    for (media_id, display_name, relative_path) in active_files {
        let parsed = parse_media_name(&display_name, &relative_path);
        let local_title = parsed
            .as_ref()
            .map(|item| item.query.as_str())
            .filter(|title| !title.is_empty())
            .unwrap_or(&display_name);
        let local_key = parsed
            .as_ref()
            .map(|item| item.normalized.as_str())
            .filter(|key| !key.is_empty())
            .map(str::to_owned)
            .unwrap_or_else(|| normalize_title(local_title));
        let local_kind = parsed
            .as_ref()
            .map(|item| item.kind.as_str())
            .unwrap_or_else(|| infer_media_kind(&display_name, &relative_path));
        update
            .execute(params![
                local_title,
                local_kind,
                parsed.as_ref().and_then(|item| item.year),
                local_key,
                media_id,
            ])
            .map_err(|error| format!("Could not save local media labels: {error}"))?;
    }
    Ok(())
}

struct MediaFileRecord {
    relative_path: String,
    display_name: String,
    local_title: String,
    local_kind: String,
    local_year: Option<u16>,
    local_key: String,
    extension: String,
    size_bytes: i64,
    modified_at: Option<i64>,
}

fn write_batch(
    connection: &mut Connection,
    root_id: i64,
    generation: i64,
    records: &[MediaFileRecord],
) -> Result<(), String> {
    let transaction = connection
        .transaction()
        .map_err(|error| format!("Could not save scanned files: {error}"))?;
    write_records(&transaction, root_id, generation, records)?;
    transaction
        .commit()
        .map_err(|error| format!("Could not save scanned files: {error}"))
}

fn write_records(
    transaction: &Transaction<'_>,
    root_id: i64,
    generation: i64,
    records: &[MediaFileRecord],
) -> Result<(), String> {
    let mut identity_statement = transaction
        .prepare(
            "INSERT INTO media_items(root_id, relative_path, local_title, local_kind, local_year, local_key)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(root_id, relative_path) DO UPDATE SET
                local_title = excluded.local_title,
                local_kind = excluded.local_kind,
                local_year = excluded.local_year,
                local_key = excluded.local_key",
        )
        .map_err(|error| format!("Could not prepare media identities: {error}"))?;
    let mut statement = transaction
        .prepare(
            "INSERT INTO media_files(
                root_id, relative_path, display_name, extension, size_bytes, modified_at, generation
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(root_id, relative_path, generation) DO UPDATE SET
                display_name = excluded.display_name,
                extension = excluded.extension,
                size_bytes = excluded.size_bytes,
                modified_at = excluded.modified_at",
        )
        .map_err(|error| format!("Could not prepare media index records: {error}"))?;

    for record in records {
        identity_statement
            .execute(params![
                root_id,
                &record.relative_path,
                &record.local_title,
                &record.local_kind,
                record.local_year,
                &record.local_key,
            ])
            .map_err(|error| format!("Could not save a media identity: {error}"))?;
        if is_extra_media_path(Path::new(&record.relative_path)) {
            transaction.execute("INSERT OR IGNORE INTO media_extras(media_id) SELECT id FROM media_items WHERE root_id=?1 AND relative_path=?2", params![root_id, &record.relative_path]).map_err(|error| error.to_string())?;
        }
        statement
            .execute(params![
                root_id,
                &record.relative_path,
                &record.display_name,
                &record.extension,
                record.size_bytes,
                record.modified_at,
                generation
            ])
            .map_err(|error| format!("Could not save a media index record: {error}"))?;
    }

    Ok(())
}

fn is_extra_media_folder(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else { return false; };
    let normalized: String = name.chars().filter(|character| character.is_alphanumeric()).flat_map(char::to_lowercase).collect();
    matches!(normalized.as_str(), "extra" | "extras" | "featurette" | "featurettes"
        | "sample" | "samples" | "behindthescenes" | "bonus" | "bonusfeatures"
        | "specialfeatures" | "deletedscenes" | "interviews" | "trailers")
}

fn is_extra_media_path(relative_path: &Path) -> bool {
    relative_path.parent().is_some_and(|parent| parent.components().any(|part| is_extra_media_folder(Path::new(part.as_os_str()))))
}

fn is_video_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            let extension = extension.to_ascii_lowercase();
            VIDEO_EXTENSIONS.contains(&extension.as_str())
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::{LibraryStore, MediaMetadata, MetadataLookup};

    #[test]
    fn moving_a_title_to_another_library_root_keeps_its_identity_and_progress() {
        let temporary = tempfile::tempdir().unwrap();
        let source = temporary.path().join("source");
        let destination = temporary.path().join("destination");
        let episode = source.join("Example/Season 01/Example.S01E01.mkv");
        let extra = source.join("Example/Extras/Behind the scenes.mp4");
        std::fs::create_dir_all(episode.parent().unwrap()).unwrap();
        std::fs::create_dir_all(extra.parent().unwrap()).unwrap();
        std::fs::create_dir_all(&destination).unwrap();
        std::fs::write(&episode, b"episode").unwrap();
        std::fs::write(&extra, b"extra").unwrap();
        let mut store = LibraryStore::open(&temporary.path().join("library.sqlite3")).unwrap();
        store.scan_root(source.to_str().unwrap()).unwrap();
        store.import_completed_files(&source, &[extra]).unwrap();
        store.scan_root(destination.to_str().unwrap()).unwrap();
        let before = store.catalog_page(0, 10).unwrap().items.into_iter().find(|item| item.title == "Example").unwrap();
        store.record_playback_activity(before.id).unwrap();
        store.move_title(before.id, &destination).unwrap();
        assert!(!episode.exists());
        let moved_episode = destination.join("Example/Season 01/Example.S01E01.mkv");
        let moved_extra = destination.join("Example/Extras/Behind the scenes.mp4");
        assert!(moved_episode.is_file());
        assert!(moved_extra.is_file());
        let after = store.catalog_detail(before.id).unwrap().unwrap();
        assert_eq!(after.media.id, before.id);
        assert_eq!(after.media.playback_uuid, before.playback_uuid);
        assert_eq!(after.files[0].playback_uuid, before.playback_uuid);
        assert_eq!(store.media_id_for_uuid(&before.playback_uuid).unwrap(), Some(before.id));
        let canonical_destination = std::fs::canonicalize(&destination).unwrap();
        assert!(std::path::Path::new(&after.files[0].path).starts_with(&canonical_destination));
        assert!(std::path::Path::new(&after.extras[0].path).starts_with(&canonical_destination));
        assert_eq!(store.playback_history(10).unwrap()[0].id, before.id);
    }

    #[test]
    fn uuid_survives_reopen_rescan_and_migration_but_never_a_reused_numeric_id() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().join("media");
        let database = temporary.path().join("library.sqlite3");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("First.mp4"), b"video").unwrap();
        let mut store = LibraryStore::open(&database).unwrap();
        store.scan_root(root.to_str().unwrap()).unwrap();
        // Simulate an existing index from before the identity migration.
        store.connection.execute_batch("DROP TRIGGER media_identity_insert; DROP TABLE media_identity;").unwrap();
        drop(store);
        let mut store = LibraryStore::open(&database).unwrap();
        let first = store.catalog_page(0, 10).unwrap().items.remove(0);
        assert_eq!(first.playback_uuid.len(), 36);
        assert_eq!(&first.playback_uuid[14..15], "4");
        assert!("89ab".contains(&first.playback_uuid[19..20]));
        store.scan_root(root.to_str().unwrap()).unwrap();
        assert_eq!(store.catalog_page(0, 10).unwrap().items[0].playback_uuid, first.playback_uuid);
        drop(store);
        let store = LibraryStore::open(&database).unwrap();
        assert_eq!(store.playback_uuid(first.id).unwrap(), first.playback_uuid);
        // Even if SQLite reuses a number, the removed UUID cannot point at it.
        let root_id:i64 = store.connection.query_row("SELECT root_id FROM media_items WHERE id=?1", [first.id], |row| row.get(0)).unwrap();
        store.connection.execute("DELETE FROM media_items WHERE id=?1", [first.id]).unwrap();
        store.connection.execute("INSERT INTO media_items(id,root_id,relative_path) VALUES (?1,?2,'First.mp4')", rusqlite::params![first.id,root_id]).unwrap();
        assert_eq!(store.media_id_for_uuid(&first.playback_uuid).unwrap(), None);
        assert_ne!(store.playback_uuid(first.id).unwrap(), first.playback_uuid);
        assert!(store.require_media_identity(first.id, Some(&first.playback_uuid)).is_err());
        assert!(store.require_media_identity(first.id, None).is_err());
        assert!(store.require_media_identity(first.id, Some(&store.playback_uuid(first.id).unwrap())).is_ok());
    }

    #[test]
    fn episode_and_continue_watching_uuids_identify_each_file_and_each_computer() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().join("media");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("Example.S01E01.mp4"), b"video").unwrap();
        std::fs::write(root.join("Example.S01E02.mp4"), b"video").unwrap();
        let mut store = LibraryStore::open(&temporary.path().join("library.sqlite3")).unwrap();
        store.scan_root(root.to_str().unwrap()).unwrap();
        let id = store.catalog_page(0,10).unwrap().items[0].id;
        let detail = store.catalog_detail(id).unwrap().unwrap();
        assert_eq!(detail.files.len(), 2);
        assert_ne!(detail.files[0].playback_uuid, detail.files[1].playback_uuid);
        let second = &detail.files[1];
        store.save_playback_progress(second.media_id, 12.0, 120.0).unwrap();
        assert_eq!(store.continue_watching(10).unwrap()[0].playback_uuid, second.playback_uuid);
        let mut other = LibraryStore::open(&temporary.path().join("other-computer.sqlite3")).unwrap();
        other.scan_root(root.to_str().unwrap()).unwrap();
        assert_eq!(other.media_id_for_uuid(&second.playback_uuid).unwrap(), None);
    }

    #[test]
    fn permanent_title_deletion_removes_media_from_disk_and_catalog() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().join("media");
        let episode = root.join("Example/Season 01/Example.S01E01.mkv");
        let extra = root.join("Example/Extras/Behind the scenes.mp4");
        std::fs::create_dir_all(episode.parent().unwrap()).unwrap();
        std::fs::create_dir_all(extra.parent().unwrap()).unwrap();
        std::fs::write(&episode, b"episode").unwrap();
        std::fs::write(&extra, b"extra").unwrap();
        let mut store = LibraryStore::open(&temporary.path().join("library.sqlite3")).unwrap();
        store.scan_root(root.to_str().unwrap()).unwrap();
        store.import_completed_files(&root, &[extra.clone()]).unwrap();
        let id = store.catalog_page(0, 10).unwrap().items[0].id;
        store.permanently_delete_title(id).unwrap();
        assert!(!episode.exists());
        assert!(!extra.exists());
        assert!(store.catalog_detail(id).unwrap().is_none());
        assert_eq!(store.catalog_page(0, 10).unwrap().total, 0);
    }

    #[test]
    fn extras_are_excluded_from_scans_and_completed_torrent_imports() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().join("media");
        let episode = root.join("Example/Season 01/Example.S01E01.mkv");
        let extra = root.join("Example/Featurettes/Making.Of.mkv");
        let film = root.join("Extraordinary/Extraordinary.mp4");
        for path in [&episode, &extra, &film] {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, b"video").unwrap();
        }
        let mut store = LibraryStore::open(&temporary.path().join("library.sqlite3")).unwrap();
        let summary = store.scan_root(root.to_str().unwrap()).unwrap();
        assert_eq!(summary.file_count, 3);
        assert_eq!(store.catalog_page(0, 10).unwrap().total, 2);
        assert_eq!(store.import_completed_files(&root, &[extra]).unwrap(), 1);
        assert_eq!(store.catalog_page(0, 10).unwrap().total, 2);
        let page = store.catalog_page(0, 10).unwrap();
        let series = page.items.iter().find(|item| item.title == "Example").unwrap();
        let detail = store.catalog_detail(series.id).unwrap().unwrap();
        assert_eq!(detail.files.len(), 1);
        assert_eq!(detail.extras.len(), 1);
        assert!(store.catalog_detail(detail.extras[0].media_id).unwrap().is_some());
        assert!(!store.metadata_candidates(100).unwrap().iter().any(|candidate| candidate.media_id == detail.extras[0].media_id));
        assert!(!super::is_extra_media_path(std::path::Path::new("Extras (2005)/Season 01/Extras.S01E01.mkv")));
        assert!(super::is_extra_media_path(std::path::Path::new("Example/BEHIND-THE-SCENES/clip.mp4")));
    }

    #[test]
    fn extra_media_never_becomes_a_resume_source_or_recommendation_seed() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().join("media");
        let film = root.join("Film.mp4");
        let bonus = root.join("Extras/Film behind the scenes.mp4");
        let trailer = root.join("Extras/Trailer.mp4");
        for path in [&film, &bonus, &trailer] {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, b"video").unwrap();
        }
        let mut store = LibraryStore::open(&temporary.path().join("library.sqlite3")).unwrap();
        store.scan_root(root.to_str().unwrap()).unwrap();
        let film_id = store.catalog_page(0, 10).unwrap().items[0].id;
        let item_id = |relative: &str| -> i64 {
            store.connection.query_row("SELECT id FROM media_items WHERE relative_path=?1", [relative], |row| row.get(0)).unwrap()
        };
        let bonus_id = item_id("Extras/Film behind the scenes.mp4");
        let trailer_id = item_id("Extras/Trailer.mp4");
        let metadata = |tmdb_id, title: &str| MediaMetadata {
            tmdb_id, kind: "movie".to_owned(), title: title.to_owned(), year: Some(2024),
            overview: String::new(), vote_average: None, poster_url: Some("poster".to_owned()), backdrop_url: None,
        };
        store.save_metadata_lookups(&[
            MetadataLookup { media_ids: vec![film_id, bonus_id], metadata: Some(metadata(987, "Film")) },
            MetadataLookup { media_ids: vec![trailer_id], metadata: Some(metadata(654, "Trailer")) },
        ]).unwrap();
        assert!(store.recommendation_profile().unwrap().iter().any(|(id, _, _, _)| *id == 987));
        assert!(!store.recommendation_profile().unwrap().iter().any(|(id, _, _, _)| *id == 654));

        store.save_playback_progress(film_id, 120.0, 600.0).unwrap();
        store.record_playback_activity(film_id).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(3));
        store.save_playback_progress(bonus_id, 240.0, 600.0).unwrap();
        store.record_playback_activity(bonus_id).unwrap();

        let resume = store.continue_watching(10).unwrap();
        assert_eq!(resume.len(), 1);
        assert_eq!(resume[0].playback_id, film_id);
        assert_eq!(resume[0].position_seconds, 120.0);
        let history = store.playback_history(10).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].id, film_id);
    }

    #[test]
    fn playing_a_series_extra_does_not_mark_episodes_watched() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().join("media");
        let first = root.join("Show.S01E01.mp4");
        let second = root.join("Show.S01E02.mp4");
        let extra = root.join("Extras/Show.S01E03.mp4");
        for path in [&first, &second, &extra] {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, b"video").unwrap();
        }
        let mut store = LibraryStore::open(&temporary.path().join("library.sqlite3")).unwrap();
        store.scan_root(root.to_str().unwrap()).unwrap();
        let series_id = store.catalog_page(0, 10).unwrap().items[0].id;
        let id_for = |relative: &str| -> i64 {
            store.connection.query_row("SELECT id FROM media_items WHERE relative_path=?1", [relative], |row| row.get(0)).unwrap()
        };
        let first_id = id_for("Show.S01E01.mp4");
        let second_id = id_for("Show.S01E02.mp4");
        let extra_id = id_for("Extras/Show.S01E03.mp4");
        let metadata = MediaMetadata {
            tmdb_id: 1234, kind: "series".to_owned(), title: "Show".to_owned(), year: Some(2024),
            overview: String::new(), vote_average: None, poster_url: Some("poster".to_owned()), backdrop_url: None,
        };
        store.save_metadata_lookups(&[MetadataLookup { media_ids: vec![first_id, second_id, extra_id], metadata: Some(metadata) }]).unwrap();
        store.save_playback_progress(first_id, 120.0, 600.0).unwrap();
        store.save_playback_progress(second_id, 180.0, 600.0).unwrap();

        store.mark_previous_episodes_watched(extra_id).unwrap();

        assert_eq!(store.playback_position(first_id).unwrap(), Some(120.0));
        assert_eq!(store.playback_position(second_id).unwrap(), Some(180.0));
        assert!(store.catalog_detail(series_id).unwrap().unwrap().watched_before.is_none());
    }

    #[test]
    fn refreshing_removes_previously_indexed_extras_without_deleting_files() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().join("media");
        let clip = root.join("Extras/clip.mp4");
        std::fs::create_dir_all(clip.parent().unwrap()).unwrap();
        std::fs::write(&clip, b"video").unwrap();
        let mut store = LibraryStore::open(&temporary.path().join("library.sqlite3")).unwrap();
        // Emulate a previously indexed extra using an explicitly selected root.
        store.scan_root(clip.parent().unwrap().to_str().unwrap()).unwrap();
        assert_eq!(store.catalog_page(0, 10).unwrap().total, 1);
        let root_id: i64 = store.connection.query_row("SELECT id FROM library_roots", [], |row| row.get(0)).unwrap();
        let canonical_root = std::fs::canonicalize(&root).unwrap();
        store.connection.execute("UPDATE library_roots SET canonical_path=?1 WHERE id=?2", rusqlite::params![canonical_root.to_str().unwrap(), root_id]).unwrap();
        store.connection.execute("UPDATE media_files SET relative_path='Extras/clip.mp4'", []).unwrap();
        store.connection.execute("UPDATE media_items SET relative_path='Extras/clip.mp4'", []).unwrap();
        store.scan_root(root.to_str().unwrap()).unwrap();
        assert_eq!(store.catalog_page(0, 10).unwrap().total, 0);
        assert!(clip.exists());
    }

    #[test]
    fn torrent_import_indexes_only_supplied_files_and_preserves_existing_identity() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().join("downloads");
        std::fs::create_dir(&root).unwrap();
        let first = root.join("First.mp4");
        std::fs::write(&first, b"video").unwrap();
        let mut store = LibraryStore::open(&temporary.path().join("library.sqlite3")).unwrap();
        store.import_completed_files(&root, &[first.clone()]).unwrap();
        let id = store.catalog_page(0, 10).unwrap().items[0].id;
        store.record_playback_activity(id).unwrap();
        std::fs::write(root.join("Unfinished.mkv"), b"partial").unwrap();
        store.import_completed_files(&root, &[first]).unwrap();
        let page = store.catalog_page(0, 10).unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].id, id);
        assert_eq!(store.playback_history(10).unwrap().len(), 1);
        let outside = temporary.path().join("Outside.mp4");
        std::fs::write(&outside, b"video").unwrap();
        assert!(store.import_completed_files(&root, &[outside]).is_err());
        assert_eq!(store.catalog_page(0, 10).unwrap().total, 1);
    }

    #[test]
    fn torrent_import_reuses_existing_parent_root_without_rescanning() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().join("media");
        let downloads = root.join("downloads");
        std::fs::create_dir_all(&downloads).unwrap();
        std::fs::write(root.join("Existing.mp4"), b"video").unwrap();
        let mut store = LibraryStore::open(&temporary.path().join("library.sqlite3")).unwrap();
        store.scan_root(root.to_str().unwrap()).unwrap();
        let completed = downloads.join("New.mkv");
        std::fs::write(&completed, b"video").unwrap();
        store.import_completed_files(&downloads, &[completed.clone()]).unwrap();
        store.import_completed_files(&downloads, &[completed]).unwrap();
        assert_eq!(store.status(false).unwrap().root_count, 1);
        assert_eq!(store.catalog_page(0, 10).unwrap().total, 2);
    }
    use rusqlite::Connection;

    #[test]
    fn catalog_ids_survive_rescans_and_removed_files_disappear() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let root = temporary.path().join("videos");
        std::fs::create_dir(&root).expect("media directory");
        std::fs::write(root.join("First.mp4"), b"video").expect("first media file");

        let mut store =
            LibraryStore::open(&temporary.path().join("library.sqlite3")).expect("library");
        store
            .scan_root(root.to_str().expect("root path"))
            .expect("first scan");
        let first_page = store.catalog_page(0, 10).expect("first catalog page");
        assert_eq!(first_page.total, 1);
        let first_id = first_page.items[0].id;
        assert_eq!(first_page.items[0].title, "First");
        assert!(store
            .resolve_media_path(first_id)
            .expect("resolved media")
            .is_some());

        std::fs::write(root.join("Second.mkv"), b"video").expect("second media file");
        store
            .scan_root(root.to_str().expect("root path"))
            .expect("second scan");
        let second_page = store.catalog_page(0, 10).expect("second catalog page");
        assert_eq!(second_page.total, 2);
        assert_eq!(second_page.items[0].id, first_id);
        assert_eq!(
            store.catalog_page(1, 1).expect("paged catalog").items.len(),
            1
        );

        std::fs::remove_file(root.join("First.mp4")).expect("remove media file");
        store
            .scan_root(root.to_str().expect("root path"))
            .expect("third scan");
        let last_page = store.catalog_page(0, 10).expect("last catalog page");
        assert_eq!(last_page.total, 1);
        assert_eq!(last_page.items[0].title, "Second");
        assert!(store
            .resolve_media_path(first_id)
            .expect("removed media lookup")
            .is_none());
    }

    #[test]
    fn existing_index_is_migrated_without_changing_media_identity() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let root = temporary.path().join("videos");
        std::fs::create_dir(&root).expect("media directory");
        std::fs::write(root.join("Existing.mp4"), b"video").expect("media file");
        let canonical_root = std::fs::canonicalize(&root).expect("canonical root");
        let database = temporary.path().join("legacy.sqlite3");
        let connection = Connection::open(&database).expect("legacy database");
        connection
            .execute_batch(
                "CREATE TABLE library_roots (
                    id INTEGER PRIMARY KEY,
                    canonical_path TEXT NOT NULL UNIQUE,
                    current_generation INTEGER NOT NULL DEFAULT 0,
                    last_scanned_at INTEGER
                 );
                 CREATE TABLE media_files (
                    id INTEGER PRIMARY KEY,
                    root_id INTEGER NOT NULL REFERENCES library_roots(id) ON DELETE CASCADE,
                    relative_path TEXT NOT NULL,
                    display_name TEXT NOT NULL,
                    extension TEXT NOT NULL,
                    size_bytes INTEGER NOT NULL,
                    modified_at INTEGER,
                    generation INTEGER NOT NULL,
                    UNIQUE(root_id, relative_path, generation)
                 );",
            )
            .expect("legacy schema");
        connection
            .execute(
                "INSERT INTO library_roots(id, canonical_path, current_generation) VALUES (1, ?1, 1)",
                [canonical_root.to_str().expect("root path")],
            )
            .expect("legacy root");
        connection
            .execute(
                "INSERT INTO media_files(root_id, relative_path, display_name, extension,
                                         size_bytes, generation)
                 VALUES (1, 'Existing.mp4', 'Existing', 'mp4', 5, 1)",
                [],
            )
            .expect("legacy file");
        drop(connection);

        let mut store = LibraryStore::open(&database).expect("migrated library");
        let first_id = store.catalog_page(0, 10).expect("migrated catalog").items[0].id;
        store
            .scan_root(root.to_str().expect("root path"))
            .expect("rescan migrated root");
        assert_eq!(
            store.catalog_page(0, 10).expect("rescanned catalog").items[0].id,
            first_id
        );
    }

    #[test]
    fn version_three_index_migration_reclassifies_episodes_and_retries_unmatched_metadata() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let root = temporary.path().join("videos");
        std::fs::create_dir_all(root.join("Quiet Show (2022)").join("Season 1"))
            .expect("series directory");
        std::fs::write(
            root.join("Quiet Show (2022)")
                .join("Season 1")
                .join("S01E01.mkv"),
            b"episode",
        )
        .expect("episode file");
        std::fs::write(root.join("Unknown Film (2019).mp4"), b"movie").expect("movie file");
        let canonical_root = std::fs::canonicalize(&root).expect("canonical root");
        let database = temporary.path().join("version-two.sqlite3");
        let connection = Connection::open(&database).expect("version two database");
        connection
            .execute_batch(
                "CREATE TABLE library_roots (
                    id INTEGER PRIMARY KEY, canonical_path TEXT NOT NULL UNIQUE,
                    current_generation INTEGER NOT NULL DEFAULT 0, last_scanned_at INTEGER
                 );
                 CREATE TABLE media_files (
                    id INTEGER PRIMARY KEY, root_id INTEGER NOT NULL, relative_path TEXT NOT NULL,
                    display_name TEXT NOT NULL, extension TEXT NOT NULL, size_bytes INTEGER NOT NULL,
                    modified_at INTEGER, generation INTEGER NOT NULL,
                    UNIQUE(root_id, relative_path, generation)
                 );
                 CREATE TABLE media_items (
                    id INTEGER PRIMARY KEY, root_id INTEGER NOT NULL, relative_path TEXT NOT NULL,
                    local_title TEXT, local_kind TEXT, local_year INTEGER, local_key TEXT,
                    UNIQUE(root_id, relative_path)
                 );
                 CREATE TABLE media_metadata (
                    media_id INTEGER PRIMARY KEY, tmdb_id INTEGER, kind TEXT, title TEXT,
                    release_year INTEGER, overview TEXT, vote_average REAL, poster_url TEXT,
                    backdrop_url TEXT, looked_up_at INTEGER NOT NULL
                 );
                 PRAGMA user_version = 3;",
            )
            .expect("version two schema");
        connection
            .execute(
                "INSERT INTO library_roots(id, canonical_path, current_generation) VALUES (1, ?1, 1)",
                [canonical_root.to_str().expect("root path")],
            )
            .expect("library root");
        for (id, relative_path, display_name, extension) in [
            (
                1_i64,
                "Quiet Show (2022)/Season 1/S01E01.mkv",
                "S01E01",
                "mkv",
            ),
            (
                2_i64,
                "Unknown Film (2019).mp4",
                "Unknown Film (2019)",
                "mp4",
            ),
        ] {
            connection
                .execute(
                    "INSERT INTO media_items(id, root_id, relative_path, local_title, local_kind, local_key)
                     VALUES (?1, 1, ?2, ?3, 'movie', 'stale-label')",
                    rusqlite::params![id, relative_path, display_name],
                )
                .expect("existing media record");
            connection
                .execute(
                    "INSERT INTO media_files(id, root_id, relative_path, display_name, extension,
                                              size_bytes, generation)
                     VALUES (?1, 1, ?2, ?3, ?4, 10, 1)",
                    rusqlite::params![id, relative_path, display_name, extension],
                )
                .expect("existing media file");
            connection
                .execute(
                    "INSERT INTO media_metadata(media_id, looked_up_at) VALUES (?1, 9999999999)",
                    [id],
                )
                .expect("existing metadata lookup");
        }
        drop(connection);

        let mut store = LibraryStore::open(&database).expect("migrated library");
        let catalog = store.catalog_page(0, 10).expect("migrated local catalog");
        assert_eq!(catalog.total, 2);
        assert!(catalog
            .items
            .iter()
            .any(|item| { item.title == "Quiet Show" && item.kind.as_deref() == Some("series") }));
        assert!(catalog
            .items
            .iter()
            .any(|item| { item.title == "Unknown Film" && item.kind.as_deref() == Some("movie") }));
        assert_eq!(
            store
                .metadata_candidates(10)
                .expect("unmatched metadata candidates")
                .len(),
            2,
            "the migration should make old unmatched files eligible for another lookup"
        );
    }

    #[test]
    fn rescan_refreshes_all_registered_folders_without_duplicate_items() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let first_root = temporary.path().join("first");
        let second_root = temporary.path().join("second");
        std::fs::create_dir(&first_root).expect("first media directory");
        std::fs::create_dir(&second_root).expect("second media directory");
        std::fs::write(first_root.join("First.mp4"), b"video").expect("first video");
        std::fs::write(second_root.join("Second.mkv"), b"video").expect("second video");

        let mut store =
            LibraryStore::open(&temporary.path().join("library.sqlite3")).expect("library");
        store
            .scan_root(first_root.to_str().expect("first path"))
            .expect("scan first folder");
        store
            .scan_root(second_root.to_str().expect("second path"))
            .expect("scan second folder");
        std::fs::write(first_root.join("Added.mp4"), b"video").expect("added video");

        let summary = store.rescan_roots().expect("rescan all folders");
        let page = store.catalog_page(0, 20).expect("catalog");

        assert_eq!(summary.root_count, 2);
        assert_eq!(store.status(false).unwrap().folders, vec![
            std::fs::canonicalize(&first_root).unwrap().to_string_lossy().into_owned(),
            std::fs::canonicalize(&second_root).unwrap().to_string_lossy().into_owned(),
        ]);
        assert_eq!(summary.file_count, 3);
        assert_eq!(page.total, 3);
        assert_eq!(page.items.len(), 3);
    }

    #[test]
    fn rescan_requires_an_existing_library_folder() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let mut store =
            LibraryStore::open(&temporary.path().join("library.sqlite3")).expect("library");

        assert_eq!(
            store.rescan_roots().expect_err("no folders configured"),
            "Choose a folder before refreshing the library."
        );
    }

    #[test]
    fn earlier_episodes_watch_boundary_persists_and_isolates_series() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().join("videos");
        for (show, names) in [
            ("Quiet Show (2022)", vec!["S01E01.mkv", "S01E03.mkv", "S02E01.mkv"]),
            ("Other Show (2022)", vec!["S01E01.mkv"]),
        ] {
            std::fs::create_dir_all(root.join(show)).unwrap();
            for name in names { std::fs::write(root.join(show).join(name), b"video").unwrap(); }
        }
        let db = temporary.path().join("library.sqlite3");
        let mut store = LibraryStore::open(&db).unwrap();
        store.scan_root(root.to_str().unwrap()).unwrap();
        let page = store.catalog_page(0, 20).unwrap();
        let series_id = page.items.iter().find(|item| item.title == "Quiet Show").unwrap().id;
        let other_id = page.items.iter().find(|item| item.title == "Other Show").unwrap().id;
        let detail = store.catalog_detail(series_id).unwrap().unwrap();
        let first = detail.files.iter().find(|file| file.episode == Some(1) && file.season == Some(1)).unwrap().media_id;
        let third = detail.files.iter().find(|file| file.episode == Some(3)).unwrap().media_id;
        let next_season = detail.files.iter().find(|file| file.season == Some(2)).unwrap().media_id;
        store.save_playback_progress(first, 120.0, 600.0).unwrap();
        store.save_playback_progress(third, 120.0, 600.0).unwrap();
        store.save_playback_progress(other_id, 120.0, 600.0).unwrap();
        store.mark_previous_episodes_watched(third).unwrap();
        assert_eq!(store.playback_position(first).unwrap(), None);
        assert_eq!(store.playback_position(third).unwrap(), Some(120.0));
        assert_eq!(store.playback_position(other_id).unwrap(), Some(120.0));
        assert!(store.catalog_detail(other_id).unwrap().unwrap().watched_before.is_none());
        let boundary = store.catalog_detail(series_id).unwrap().unwrap().watched_before.unwrap();
        assert_eq!((boundary.season, boundary.episode), (1, 3));
        store.mark_previous_episodes_watched(next_season).unwrap();
        store.mark_previous_episodes_watched(first).unwrap();
        drop(store);
        let mut store = LibraryStore::open(&db).unwrap();
        std::fs::write(root.join("Quiet Show (2022)").join("S01E02.mkv"), b"new download").unwrap();
        store.scan_root(root.to_str().unwrap()).unwrap();
        let boundary = store.catalog_detail(series_id).unwrap().unwrap().watched_before.unwrap();
        assert_eq!((boundary.season, boundary.episode), (2, 1));
        let candidates = store.metadata_candidates(20).unwrap();
        store.save_metadata_lookups(&[MetadataLookup {
            media_ids: candidates.iter().filter(|item| item.relative_path.contains("Quiet Show")).map(|item| item.media_id).collect(),
            metadata: Some(MediaMetadata { tmdb_id: 1234, kind: "series".to_owned(), title: "Quiet Show".to_owned(), year: Some(2022),
                overview: String::new(), vote_average: None, poster_url: None, backdrop_url: None }),
        }]).unwrap();
        let boundary = store.catalog_detail(series_id).unwrap().unwrap().watched_before.unwrap();
        assert_eq!((boundary.season, boundary.episode), (2, 1));
    }

    #[test]
    fn catalog_groups_matched_series_files_and_filters_metadata() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let root = temporary.path().join("videos");
        std::fs::create_dir_all(root.join("Quiet Show").join("Season 1"))
            .expect("series directory");
        std::fs::write(
            root.join("Quiet Show").join("Season 1").join("S01E01.mkv"),
            b"one",
        )
        .expect("episode one");
        std::fs::write(
            root.join("Quiet Show").join("Season 1").join("S01E02.mkv"),
            b"two",
        )
        .expect("episode two");

        let mut store =
            LibraryStore::open(&temporary.path().join("library.sqlite3")).expect("library");
        store
            .scan_root(root.to_str().expect("root path"))
            .expect("scan series");
        let files = store
            .metadata_candidates(10)
            .expect("unmatched episode files");
        assert_eq!(files.len(), 2);

        store
            .save_metadata_lookups(&[MetadataLookup {
                media_ids: files.iter().map(|item| item.media_id).collect(),
                metadata: Some(MediaMetadata {
                    tmdb_id: 1234,
                    kind: "series".to_owned(),
                    title: "Quiet Show".to_owned(),
                    year: Some(2022),
                    overview: "A short overview.".to_owned(),
                    vote_average: Some(8.1),
                    poster_url: Some("https://image.tmdb.org/t/p/w342/show.jpg".to_owned()),
                    backdrop_url: None,
                }),
            }])
            .expect("save metadata");

        let catalog = store.catalog_page(0, 10).expect("grouped catalog");
        assert_eq!(catalog.total, 1);
        assert_eq!(catalog.items[0].title, "Quiet Show");
        assert_eq!(catalog.items[0].kind.as_deref(), Some("series"));
        assert_eq!(catalog.items[0].year, Some(2022));
        let detail = store
            .catalog_detail(catalog.items[0].id)
            .expect("series details")
            .expect("series detail record");
        assert_eq!(detail.media.title, "Quiet Show");
        assert_eq!(detail.files.len(), 2);
        assert_eq!(
            (detail.files[0].season, detail.files[0].episode),
            (Some(1), Some(1))
        );
        assert_eq!(
            (detail.files[1].season, detail.files[1].episode),
            (Some(1), Some(2))
        );
        assert_eq!(
            catalog.items[0].poster_url.as_deref(),
            Some("https://image.tmdb.org/t/p/w342/show.jpg")
        );
        assert_eq!(
            store
                .catalog_filtered_page(0, 10, Some("movie"), None, None)
                .expect("movie filter")
                .total,
            0
        );
        assert_eq!(
            store
                .catalog_filtered_page(0, 10, Some("series"), Some("Quiet"), Some("Title"))
                .expect("series search")
                .total,
            1
        );
        assert_eq!(store.metadata_counts().expect("metadata counts"), (2, 0, 0));
    }

    #[test]
    fn catalog_shows_unmatched_movies_and_groups_unmatched_series_episodes() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let root = temporary.path().join("videos");
        let season = root.join("Quiet Show (2022)").join("Season 1");
        std::fs::create_dir_all(&season).expect("series directory");
        std::fs::write(season.join("S01E01.mkv"), b"one").expect("episode one");
        std::fs::write(season.join("S01E02.mkv"), b"two").expect("episode two");
        std::fs::write(root.join("Unknown Film (2019).mp4"), b"movie").expect("movie");

        let mut store =
            LibraryStore::open(&temporary.path().join("library.sqlite3")).expect("library");
        store
            .scan_root(root.to_str().expect("root path"))
            .expect("scan media");

        let catalog = store.catalog_page(0, 10).expect("local catalog");
        assert_eq!(
            catalog.total, 2,
            "two episodes should form one series entry"
        );
        let series = catalog
            .items
            .iter()
            .find(|item| item.kind.as_deref() == Some("series"))
            .expect("unmatched series entry");
        assert_eq!(series.title, "Quiet Show");
        assert_eq!(series.year, Some(2022));
        let series_detail = store
            .catalog_detail(series.id)
            .expect("local series details")
            .expect("local series detail record");
        assert_eq!(series_detail.files.len(), 2);
        let movie = catalog
            .items
            .iter()
            .find(|item| item.kind.as_deref() == Some("movie"))
            .expect("unmatched movie entry");
        assert_eq!(movie.title, "Unknown Film");
        assert_eq!(movie.year, Some(2019));
        let movie_detail = store
            .catalog_detail(movie.id)
            .expect("local movie details")
            .expect("local movie detail record");
        assert_eq!(movie_detail.files.len(), 1);
        assert_eq!(
            store
                .catalog_filtered_page(0, 10, Some("series"), None, None)
                .expect("series filter")
                .total,
            1
        );
        assert_eq!(
            store
                .catalog_filtered_page(0, 10, Some("movie"), None, None)
                .expect("movie filter")
                .total,
            1
        );
    }

    #[test]
    fn playback_progress_persists_for_home_and_completed_items_disappear() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let root = temporary.path().join("videos");
        std::fs::create_dir(&root).expect("media directory");
        std::fs::write(root.join("First Film (2024).mp4"), b"video").expect("video file");
        let mut store =
            LibraryStore::open(&temporary.path().join("library.sqlite3")).expect("library");
        store
            .scan_root(root.to_str().expect("root path"))
            .expect("scan");
        let media = store.catalog_page(0, 10).expect("catalog").items.remove(0);

        store
            .save_metadata_lookups(&[MetadataLookup {
                media_ids: vec![media.id],
                metadata: Some(MediaMetadata {
                    tmdb_id: 987,
                    kind: "movie".to_owned(),
                    title: "First Film".to_owned(),
                    year: Some(2024),
                    overview: "A test synopsis.".to_owned(),
                    vote_average: Some(7.8),
                    poster_url: Some("https://image.tmdb.org/poster.jpg".to_owned()),
                    backdrop_url: Some("https://image.tmdb.org/backdrop.jpg".to_owned()),
                }),
            }])
            .expect("save playback metadata");

        store
            .record_playback_activity(media.id)
            .expect("record playback");
        let history = store.playback_history(12).expect("playback history");
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].id, media.id);
        assert_eq!(history[0].title, "First Film");
        assert_eq!(history[0].overview.as_deref(), Some("A test synopsis."));
        assert_eq!(history[0].vote_average, Some(7.8));

        store
            .save_playback_progress(media.id, 120.0, 600.0)
            .expect("save progress");
        assert_eq!(
            store.playback_position(media.id).expect("resume position"),
            Some(120.0)
        );
        let resume = store.continue_watching(12).expect("home resume list");
        assert_eq!(resume.len(), 1);
        assert_eq!(resume[0].id, media.id);
        assert_eq!(resume[0].position_seconds, 120.0);
        assert_eq!(resume[0].title, "First Film");
        assert_eq!(resume[0].overview.as_deref(), Some("A test synopsis."));
        assert_eq!(resume[0].vote_average, Some(7.8));

        store
            .save_playback_progress(media.id, 0.0, 600.0)
            .expect("ignore the player's initial position while it restores resume");
        assert_eq!(store.playback_position(media.id).expect("preserved resume"), Some(120.0));
        assert_eq!(store.continue_watching(12).expect("preserved home resume").len(), 1);

        store
            .save_playback_progress(media.id, 590.0, 600.0)
            .expect("complete playback");
        assert!(store
            .continue_watching(12)
            .expect("completed home list")
            .is_empty());
        assert_eq!(
            store.playback_position(media.id).expect("cleared resume"),
            None
        );
        assert_eq!(
            store
                .playback_history(12)
                .expect("completed playback history")
                .len(),
            1
        );
    }
}
