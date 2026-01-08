use fastsync::transport::Transport;
use fastsync::remote::agentless::AgentlessRemote;
use fastsync::scanner::{Scanner, FileEntry};
use fastsync::Result;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::collections::HashMap;

struct MockTransport {
    exec_responses: Mutex<Vec<(String, String)>>, 
    dir_entries: Mutex<HashMap<PathBuf, Vec<FileEntry>>>,
}

impl MockTransport {
    fn new() -> Self {
        Self { 
            exec_responses: Mutex::new(Vec::new()),
            dir_entries: Mutex::new(HashMap::new()),
        }
    }
    
    fn add_response(&self, cmd: &str, response: &str) {
        self.exec_responses.lock().unwrap().push((cmd.to_string(), response.to_string()));
    }
    
    fn add_dir_entry(&self, dir: &Path, entry: FileEntry) {
        self.dir_entries.lock().unwrap().entry(dir.to_path_buf()).or_default().push(entry);
    }
}

impl Transport for MockTransport {
    fn exec(&self, command: &str) -> Result<String> {
        let responses = self.exec_responses.lock().unwrap();
        if let Some(pos) = responses.iter().position(|(c, _)| command.contains(c)) {
             return Ok(responses[pos].1.clone());
        }
        
        Ok("".to_string())
    }
    
    fn upload_file(&self, _local: &Path, _remote: &Path) -> Result<()> {
        Ok(())
    }
    
    fn list_dir(&self, path: &Path) -> Result<Vec<FileEntry>> {
        let entries = self.dir_entries.lock().unwrap();
        Ok(entries.get(path).cloned().unwrap_or_default())
    }

    fn create_dir_all(&self, _path: &Path) -> Result<()> {
        Ok(())
    }
}

#[test]
fn test_remote_scan_recursive() {
    let transport = MockTransport::new();
    let root = Path::new("/remote");
    
    // Structure:
    // /remote
    //   |- file.txt
    //   |- subdir/
    //       |- deep.txt
    
    // Entries in /remote
    transport.add_dir_entry(root, FileEntry {
        path: "file.txt".into(), size: 100, mtime: 1000, mode: 0o644, is_dir: false 
    });
    transport.add_dir_entry(root, FileEntry {
        path: "subdir".into(), size: 0, mtime: 1000, mode: 0o755, is_dir: true
    });
    
    // Entries in /remote/subdir
    transport.add_dir_entry(&root.join("subdir"), FileEntry {
        path: "deep.txt".into(), size: 50, mtime: 1000, mode: 0o644, is_dir: false
    });
    
    let mut remote = AgentlessRemote::new(&transport);
    let manifest = remote.scan(root).expect("Scan failed");
    
    assert_eq!(manifest.entries.len(), 3);
    
    let file = manifest.entries.iter().find(|e| e.path == "file.txt").unwrap();
    assert_eq!(file.size, 100);
    
    let subdir = manifest.entries.iter().find(|e| e.path == "subdir").unwrap();
    assert!(subdir.is_dir);
    
    let deep = manifest.entries.iter().find(|e| e.path == "subdir/deep.txt").unwrap();
    assert_eq!(deep.size, 50);
}
