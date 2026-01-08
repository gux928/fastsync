use crate::Result;
use crate::scanner::FileEntry;
use std::path::Path;

pub mod ssh;

pub trait Transport {
    fn exec(&self, command: &str) -> Result<String>;
    fn upload_file(&self, local: &Path, remote: &Path) -> Result<()>;
    /// List entries in a remote directory. Returns file metadata.
    fn list_dir(&self, path: &Path) -> Result<Vec<FileEntry>>;
    /// Recursively create a directory.
    fn create_dir_all(&self, path: &Path) -> Result<()>;
}