use serde::{Deserialize, Serialize};
use std::path::Path;
use crate::Result;

pub mod local;

pub use local::LocalScanner;

/// File metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    /// Relative path (using / as separator)
    pub path: String,
    /// File size (bytes)
    pub size: u64,
    /// Modification time (Unix timestamp, seconds)
    pub mtime: i64,
    /// File permissions (Unix mode, e.g., 0o644)
    pub mode: u32,
    /// Is directory
    pub is_dir: bool,
}

/// Directory manifest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    /// Manifest generation time
    pub generated_at: i64,
    /// Root path
    pub root_path: String,
    /// List of files
    pub entries: Vec<FileEntry>,
}

/// Scanner trait
pub trait Scanner {
    /// Scan directory and return manifest
    fn scan(&mut self, path: &Path) -> Result<Manifest>;
}
