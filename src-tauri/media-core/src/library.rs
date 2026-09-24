//! Bounded local media discovery and the persistent SQLite index.
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, Transaction};
use serde::Serialize;
use walkdir::WalkDir;

const BATCH_SIZE: usize = 256;
const MAX_SCAN_DEPTH: usize = 64;
const MAX_FILES_PER_ROOT: usize = 1_000_000;

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
    pub last_scan_at: Option<i64>,
    pub is_scanning: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanSummary {
    pub root_name: String,
    pub file_count: u64,
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

        let connection = Connection::open(path)
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
                    ON media_files(root_id, generation);",
            )
            .map_err(|error| format!("Could not initialize the local library database: {error}"))?;

        Ok(Self { connection })
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
            last_scan_at,
            is_scanning,
        })
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
        })
    }
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
