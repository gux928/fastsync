use crate::scanner::{Manifest, FileEntry};
use std::collections::HashMap;

#[derive(Debug)]
pub enum SyncAction {
    Upload(FileEntry),
    Delete(String),
}

pub fn compute_diff(local: &Manifest, remote: &Manifest, delete: bool) -> Vec<SyncAction> {
    let mut actions = Vec::new();
    let remote_map: HashMap<&str, &FileEntry> = remote.entries.iter()
        .map(|e| (e.path.as_str(), e))
        .collect();

    for local_entry in &local.entries {
        match remote_map.get(local_entry.path.as_str()) {
            Some(remote_entry) => {
                let needs_update = if local_entry.is_dir != remote_entry.is_dir {
                    // Type changed (file <-> dir), treat as update (upload will overwrite?)
                    // If remote is dir and local is file, we might need to remove remote dir first?
                    // For MVP, just upload. scp handles this?
                    // If remote is file and local is dir, scp needs recursive?
                    // Our FileEntry includes directories.
                    // If it is a directory, size might be 4096.
                    true
                } else if local_entry.is_dir {
                    // Directory exists on both side. Do nothing.
                    false
                } else if local_entry.size != remote_entry.size {
                    true
                } else if local_entry.mtime > remote_entry.mtime {
                    true
                } else {
                    false
                };
                
                if needs_update {
                    actions.push(SyncAction::Upload(local_entry.clone()));
                }
            },
            None => {
                actions.push(SyncAction::Upload(local_entry.clone()));
            }
        }
    }
    
    if delete {
         let local_map: HashMap<&str, &FileEntry> = local.entries.iter()
            .map(|e| (e.path.as_str(), e))
            .collect();
            
         for remote_entry in &remote.entries {
             if !local_map.contains_key(remote_entry.path.as_str()) {
                 actions.push(SyncAction::Delete(remote_entry.path.clone()));
             }
         }
    }
    
    actions
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::Manifest;

    #[test]
    fn test_compute_diff() {
        let local = Manifest {
            generated_at: 0,
            root_path: ".".into(),
            entries: vec![
                FileEntry { path: "updated.txt".into(), size: 10, mtime: 100, mode: 0, is_dir: false },
                FileEntry { path: "new.txt".into(), size: 20, mtime: 200, mode: 0, is_dir: false },
                FileEntry { path: "same.txt".into(), size: 30, mtime: 300, mode: 0, is_dir: false },
            ]
        };
        
        let remote = Manifest {
            generated_at: 0,
            root_path: ".".into(),
            entries: vec![
                FileEntry { path: "updated.txt".into(), size: 10, mtime: 90, mode: 0, is_dir: false }, 
                FileEntry { path: "same.txt".into(), size: 30, mtime: 300, mode: 0, is_dir: false },
                FileEntry { path: "deleted.txt".into(), size: 40, mtime: 400, mode: 0, is_dir: false },
            ]
        };
        
        // Test without delete
        let actions = compute_diff(&local, &remote, false);
        // updated.txt: local(100) > remote(90) -> Upload
        // new.txt: new -> Upload
        // same.txt: same -> Skip
        // deleted.txt: ignored
        assert_eq!(actions.len(), 2); 
        
        // Test with delete
        let actions = compute_diff(&local, &remote, true);
        // + Delete deleted.txt
        assert_eq!(actions.len(), 3);
    }
}
