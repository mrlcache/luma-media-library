//! Bounded local media discovery and the persistent SQLite index.
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::Serialize;
use walkdir::WalkDir;

const BATCH_SIZE: usize = 256;
const MAX_SCAN_DEPTH: usize = 64;
const MAX_FILES_PER_ROOT: usize = 1_000_000;
const GROUPED_CATALOG_CTE: &str = "WITH current_items AS (
    SELECT items.id, files.display_name, files.extension, files.size_bytes,
           files.modified_at, metadata.kind, metadata.release_year,
           metadata.title AS metadata_title, metadata.overview,
           metadata.vote_average, metadata.poster_url, metadata.backdrop_url,
           metadata.tmdb_id
    FROM media_files AS files
    JOIN library_roots AS roots ON roots.id = files.root_id
    JOIN media_items AS items
      ON items.root_id = files.root_id AND items.relative_path = files.relative_path
    LEFT JOIN media_metadata AS metadata ON metadata.media_id = items.id
    WHERE files.generation = roots.current_generation
), grouped AS (
    SELECT MIN(id) AS id,
           COALESCE(MAX(NULLIF(metadata_title, '')), MIN(display_name)) AS title,
           MIN(extension) AS extension,
           SUM(size_bytes) AS size_bytes,
           MAX(modified_at) AS modified_at,
           MAX(kind) AS kind,
           MAX(release_year) AS release_year,
           MAX(overview) AS overview,
           MAX(vote_average) AS vote_average,
           MAX(poster_url) AS poster_url,
           MAX(backdrop_url) AS backdrop_url
    FROM current_items
    GROUP BY CASE WHEN tmdb_id IS NOT NULL
         THEN kind || ':' || tmdb_id
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

pub struct LibraryStore {
    connection: Connection,
}

impl LibraryStore {
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
        }

        Ok(Self { connection })
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
            .query_row(&count_query, params![kind, query],
                |row| row.get(0),
            )
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
        let items = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("Could not read a library item: {error}"))?;
        drop(statement);
        snapshot
            .commit()
            .map_err(|error| format!("Could not finish reading the library catalog: {error}"))?;

        Ok(CatalogPage {
            items,
            total: total.max(0) as u64,
            offset,
        })
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

        Ok(LibraryStatus {
            root_count: root_count.max(0) as u64,
            file_count: file_count.max(0) as u64,
            matched_count: matched_count.max(0) as u64,
            unmatched_count: unmatched_count.max(0) as u64,
            pending_count: (file_count - matched_count - unmatched_count).max(0) as u64,
            last_scan_at,
            is_scanning,
        })
    }

    pub fn metadata_candidates(&self, requested_count: u32) -> Result<Vec<MetadataCandidate>, String> {
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
                }
            }
        }
        transaction
            .commit()
            .map_err(|error| format!("Could not finish saving media metadata: {error}"))
    }

    pub fn metadata_counts(&self) -> Result<(u64, u64, u64), String> {
        let status = self.status(false)?;
        Ok((status.matched_count, status.unmatched_count, status.pending_count))
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

struct MediaFileRecord {
    relative_path: String,
    display_name: String,
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
            "INSERT INTO media_items(root_id, relative_path) VALUES (?1, ?2)
             ON CONFLICT(root_id, relative_path) DO NOTHING",
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
            .execute(params![root_id, &record.relative_path])
            .map_err(|error| format!("Could not save a media identity: {error}"))?;
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
    fn catalog_groups_matched_series_files_and_filters_metadata() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let root = temporary.path().join("videos");
        std::fs::create_dir_all(root.join("Quiet Show").join("Season 1"))
            .expect("series directory");
        std::fs::write(root.join("Quiet Show").join("Season 1").join("S01E01.mkv"), b"one")
            .expect("episode one");
        std::fs::write(root.join("Quiet Show").join("Season 1").join("S01E02.mkv"), b"two")
            .expect("episode two");

        let mut store =
            LibraryStore::open(&temporary.path().join("library.sqlite3")).expect("library");
        store
            .scan_root(root.to_str().expect("root path"))
            .expect("scan series");
        let files = store.catalog_page(0, 10).expect("initial catalog").items;
        assert_eq!(files.len(), 2);

        store
            .save_metadata_lookups(&[MetadataLookup {
                media_ids: files.iter().map(|item| item.id).collect(),
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
        assert_eq!(catalog.items[0].poster_url.as_deref(), Some("https://image.tmdb.org/t/p/w342/show.jpg"));
        assert_eq!(store.catalog_filtered_page(0, 10, Some("movie"), None, None).expect("movie filter").total, 0);
        assert_eq!(store.catalog_filtered_page(0, 10, Some("series"), Some("Quiet"), Some("Title")).expect("series search").total, 1);
        assert_eq!(store.metadata_counts().expect("metadata counts"), (2, 0, 0));
    }
}
