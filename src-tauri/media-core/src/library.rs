//! Bounded local media discovery and the persistent SQLite index.
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OptionalExtension, Transaction};
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogMedia {
    pub id: i64,
    pub title: String,
    pub extension: String,
    pub size_bytes: u64,
    pub modified_at: Option<i64>,
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
                 );",
            )
            .map_err(|error| format!("Could not initialize the local library database: {error}"))?;

        let schema_version: i64 = connection
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
        }

        Ok(Self { connection })
    }

    pub fn catalog_page(
        &mut self,
        offset: u32,
        requested_count: u32,
    ) -> Result<CatalogPage, String> {
        const MAX_PAGE_SIZE: u32 = 200;
        let count = requested_count.clamp(1, MAX_PAGE_SIZE);
        let snapshot = self
            .connection
            .transaction()
            .map_err(|error| format!("Could not read the library catalog: {error}"))?;
        let total: i64 = snapshot
            .query_row(
                "SELECT COUNT(*) FROM media_files AS files
                 JOIN library_roots AS roots ON roots.id = files.root_id
                 WHERE files.generation = roots.current_generation",
                [],
                |row| row.get(0),
            )
            .map_err(|error| format!("Could not count library items: {error}"))?;
        let mut statement = snapshot
            .prepare(
                "SELECT items.id, files.display_name, files.extension,
                        files.size_bytes, files.modified_at
                 FROM media_files AS files
                 JOIN library_roots AS roots ON roots.id = files.root_id
                 JOIN media_items AS items
                   ON items.root_id = files.root_id AND items.relative_path = files.relative_path
                 WHERE files.generation = roots.current_generation
                 ORDER BY items.id
                 LIMIT ?1 OFFSET ?2",
            )
            .map_err(|error| format!("Could not prepare the library catalog: {error}"))?;
        let rows = statement
            .query_map(params![count, offset], |row| {
                let size_bytes: i64 = row.get(3)?;
                Ok(CatalogMedia {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    extension: row.get(2)?,
                    size_bytes: size_bytes.max(0) as u64,
                    modified_at: row.get(4)?,
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
    use super::LibraryStore;
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
}
