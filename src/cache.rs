use redb::ReadableDatabase;
use redb::{Database, TableDefinition};
use memmap2::MmapOptions;
use blake3::Hasher;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::os::unix::fs::MetadataExt;
use std::io::Result;


const TABLE: TableDefinition<&str, (u64, u64, [u8; 32], &str)> = TableDefinition::new("metadata");
// Schema: Key = File Path
// Value = (Last_Modified (timestamp), Size, BLAKE3_Hash, Cached_Image_Path)

#[derive(Debug, PartialEq)]
pub enum FileState {
    Unchanged(PathBuf), // Path to cached PNG
    Modified(PathBuf),  // Path to new generated/cached PNG
    New(PathBuf),       // Path to new generated/cached PNG
}

pub struct CacheLayer {
    db: Database,
    cache_dir: PathBuf,
}

impl CacheLayer {
    pub fn new(cache_dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&cache_dir)?;
        let db_path = cache_dir.join("metadata.redb");

        // Attempt to open or create the database
        let db = if db_path.exists() {
            Database::open(&db_path)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?
        } else {
            let db = Database::create(&db_path)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
            let write_txn = db.begin_write()
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
            write_txn.open_table(TABLE)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
            write_txn.commit()
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
            db
        };

        Ok(Self { db, cache_dir })
    }

    fn hash_file(file_path: &Path) -> Result<[u8; 32]> {
        let file = File::open(file_path)?;

        // Handle empty files safely
        let metadata = file.metadata()?;
        if metadata.len() == 0 {
            let hash = blake3::hash(&[]);
            return Ok(hash.into());
        }

        let mmap = unsafe { MmapOptions::new().map(&file)? };

        // Use parallel hashing via blake3/rayon if file is large enough (e.g., > 128KB)
        // Note: blake3 updates in parallel automatically via Rayon when update_rayon is used
        let mut hasher = Hasher::new();
        hasher.update_rayon(&mmap);

        Ok(hasher.finalize().into())
    }

    pub fn check_file(&self, file_path: &Path) -> Result<(FileState, [u8; 32], u64, u64)> {
        let path_str = file_path.to_string_lossy().to_string();
        let metadata = std::fs::metadata(file_path)?;

        let mtime = metadata.mtime() as u64;
        let size = metadata.len();

        let read_txn = self.db.begin_read()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
        let table = read_txn.open_table(TABLE)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

        let mut cached_entry = None;
        if let Ok(Some(entry)) = table.get(path_str.as_str()) {
            let (c_mtime, c_size, c_hash, c_img_path) = entry.value();
            cached_entry = Some((c_mtime, c_size, c_hash, c_img_path.to_string()));
        }
        drop(read_txn);

        if let Some((c_mtime, c_size, c_hash, c_img_path)) = cached_entry {
            // Cache Hit: Metadata matches exactly
            if mtime == c_mtime && size == c_size {
                return Ok((FileState::Unchanged(PathBuf::from(c_img_path)), c_hash, mtime, size));
            }

            // Cache Miss (Modified): Metadata changed. Hash it.
            let new_hash = Self::hash_file(file_path)?;

            // Generate a deterministic image path based on the new hash
            let hex_hash = hex::encode(new_hash);
            let new_img_path = self.cache_dir.join(format!("{}.png", hex_hash));

            if new_hash != c_hash {
                // Return Modified state to trigger alert in UI/CLI
                Ok((FileState::Modified(new_img_path), new_hash, mtime, size))
            } else {
                // Edge case: mtime changed but hash is identical (e.g., touch file)
                Ok((FileState::Unchanged(new_img_path), new_hash, mtime, size))
            }
        } else {
            // Cache Miss (New): Hash file and create new entry
            let new_hash = Self::hash_file(file_path)?;
            let hex_hash = hex::encode(new_hash);
            let new_img_path = self.cache_dir.join(format!("{}.png", hex_hash));
            Ok((FileState::New(new_img_path), new_hash, mtime, size))
        }
    }

    pub fn update_cache(&self, file_path: &Path, mtime: u64, size: u64, hash: [u8; 32], img_path: &Path) -> Result<()> {
        let path_str = file_path.to_string_lossy().to_string();
        let img_path_str = img_path.to_string_lossy().to_string();

        let write_txn = self.db.begin_write()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

        {
            let mut table = write_txn.open_table(TABLE)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

            table.insert(path_str.as_str(), (mtime, size, hash, img_path_str.as_str()))
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
        }

        write_txn.commit()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

        Ok(())
    }
}
