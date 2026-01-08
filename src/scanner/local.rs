use crate::scanner::{Manifest, FileEntry, Scanner};
use crate::Result;
use ignore::WalkBuilder;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(unix)]
use std::os::unix::fs::MetadataExt;

pub struct LocalScanner {
    excludes: Vec<String>,
}

impl LocalScanner {
    pub fn new(excludes: Vec<String>) -> Self {
        Self { excludes }
    }
}

impl Scanner for LocalScanner {
    fn scan(&mut self, path: &Path) -> Result<Manifest> {
        let mut entries = Vec::new();
        // canonicalize can fail on windows for some paths or behave weirdly with UNC.
        // But for now it's fine.
        let root = path.canonicalize()?;
        
        // Configure WalkBuilder
        let mut builder = WalkBuilder::new(&root);
        builder.hidden(false); 
        builder.git_ignore(true);
        builder.follow_links(false); // <--- 重要：不跟随符号链接，防止误判
        
        // Add custom overrides
        if !self.excludes.is_empty() {
             let mut overrides = ignore::overrides::OverrideBuilder::new(&root);
             for pattern in &self.excludes {
                 // To ignore a pattern, we add it prefixed with "!" in OverrideBuilder
                 overrides.add(&format!("!{}", pattern)).map_err(|e| crate::FastSyncError::Config(e.to_string()))?;
             }
             if let Ok(ov) = overrides.build() {
                 builder.overrides(ov);
             }
        }

        for result in builder.build() {
            match result {
                Ok(entry) => {
                     let p = entry.path();
                     if p == root { continue; } 
                     
                     let relative_path = match p.strip_prefix(&root) {
                         Ok(rp) => rp,
                         Err(_) => continue,
                     };
                     
                     let path_str = relative_path.to_string_lossy().to_string().replace('\\', "/");

                     // Skip if we can't get metadata (e.g. broken symlink or permission)
                     let metadata = match entry.metadata() {
                         Ok(m) => m,
                         Err(e) => {
                             tracing::warn!("Failed to get metadata for {:?}: {}", p, e);
                             continue;
                         }
                     };
                     
                     let (mtime, mode) = get_metadata_platform(&metadata);

                     entries.push(FileEntry {
                         path: path_str,
                         size: metadata.len(),
                         mtime,
                         mode,
                         is_dir: metadata.is_dir(),
                     });
                }
                Err(err) => {
                    tracing::warn!("Scan error: {}", err);
                }
            }
        }

        Ok(Manifest {
            generated_at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64,
            root_path: root.to_string_lossy().to_string(),
            entries,
        })
    }
}

#[cfg(unix)]
fn get_metadata_platform(metadata: &std::fs::Metadata) -> (i64, u32) {
    (metadata.mtime(), metadata.mode())
}

#[cfg(not(unix))]
fn get_metadata_platform(metadata: &std::fs::Metadata) -> (i64, u32) {
    let mtime = metadata.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    
    // Synthesize mode for non-unix
    let mode = if metadata.is_dir() { 0o755 } else { 0o644 };
    (mtime, mode)
}
