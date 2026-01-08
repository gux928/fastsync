use crate::Result;
use crate::scanner::{Manifest, FileEntry, Scanner};
use crate::transport::Transport;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct AgentlessRemote<'a> {
    conn: &'a dyn Transport,
}

impl<'a> AgentlessRemote<'a> {
    pub fn new(conn: &'a dyn Transport) -> Self {
        Self { conn }
    }

    fn scan_recursive(&self, root_path: &Path, current_rel: &Path, entries: &mut Vec<FileEntry>) -> Result<()> {
        let current_abs = if current_rel == Path::new("") {
            root_path.to_path_buf()
        } else {
            root_path.join(current_rel)
        };

        let dir_entries = self.conn.list_dir(&current_abs)?;

        for entry in dir_entries {
            let rel_path = if current_rel == Path::new("") {
                PathBuf::from(&entry.path)
            } else {
                current_rel.join(&entry.path)
            };
            
            // Normalize to forward slashes for the manifest
            let rel_path_str = rel_path.to_string_lossy().to_string().replace('\\', "/");

            let mut full_entry = entry.clone();
            full_entry.path = rel_path_str;
            entries.push(full_entry.clone());

            if full_entry.is_dir {
                // Recursively scan subdirectories
                // Note: We might want to handle errors gracefully (e.g. permission denied) 
                // but for now we propagate.
                // Avoid following symlinks infinitely? 
                // SFTP usually returns file attributes. If is_dir is true, it's a directory.
                // We trust the loop isn't infinite unless the FS structure is.
                self.scan_recursive(root_path, &rel_path, entries)?;
            }
        }
        Ok(())
    }
}

impl<'a> Scanner for AgentlessRemote<'a> {
    fn scan(&mut self, path: &Path) -> Result<Manifest> {
        let path_str = path.to_string_lossy().to_string();
        let mut entries = Vec::new();
        
        self.scan_recursive(path, Path::new(""), &mut entries)?;
        
        Ok(Manifest {
            generated_at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64,
            root_path: path_str,
            entries,
        })
    }
}
