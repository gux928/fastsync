use crate::Result;
use crate::transport::Transport;
use crate::scanner::FileEntry;
use ssh2::{Session, Sftp};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::io::Read;

#[derive(Clone)]
pub struct SshConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub key_path: Option<PathBuf>,
}

pub struct SshConnection {
    session: Session,
    _tcp: TcpStream,
}

impl Transport for SshConnection {
    fn exec(&self, command: &str) -> Result<String> {
        let mut channel = self.session.channel_session()
            .map_err(|e| crate::FastSyncError::SshConnection(format!("Channel open failed: {}", e)))?;
        channel.exec(command)
            .map_err(|e| crate::FastSyncError::RemoteCommand(format!("Exec failed: {}", e)))?;
        
        let mut s = String::new();
        channel.read_to_string(&mut s)
            .map_err(crate::FastSyncError::Io)?;
            
        channel.wait_close().ok(); 
        let exit_status = channel.exit_status().unwrap_or(0);
        
        if exit_status != 0 {
             return Err(crate::FastSyncError::RemoteCommand(format!("Command '{}' exited with code {}. Output: {}", command, exit_status, s)));
        }
        
        Ok(s)
    }

    fn upload_file(&self, local: &Path, remote: &Path) -> Result<()> {
        let mut local_file = std::fs::File::open(local).map_err(crate::FastSyncError::Io)?;
        let sftp = self.sftp()?;
        
        let mut remote_file = sftp.create(remote)
            .map_err(|e| crate::FastSyncError::SshConnection(format!("Remote file create failed {:?}: {}", remote, e)))?;
            
        std::io::copy(&mut local_file, &mut remote_file).map_err(crate::FastSyncError::Io)?;
        
        Ok(())
    }

    fn list_dir(&self, path: &Path) -> Result<Vec<FileEntry>> {
        let sftp = self.sftp()?;
        let items = sftp.readdir(path)
            .map_err(|e| crate::FastSyncError::SshConnection(format!("SFTP readdir failed for {:?}: {}", path, e)))?;

        let mut entries = Vec::new();
        for (pb, stat) in items {
            let file_name = pb.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if file_name == "." || file_name == ".." || file_name.is_empty() {
                continue;
            }

            // PathBuf from ssh2 might be absolute or relative depending on readdir implementation, 
            // usually it is just the filename or path relative to the searched dir?
            // Actually ssh2 readdir usually returns the full path provided + filename.
            // But we want just the filename here to let the caller handle full paths?
            // Wait, FileEntry usually needs relative path from root. 
            // Transport::list_dir returns entries for *that* dir. 
            // Let's store just the filename in path for now, or let the caller reconstruct?
            // The `FileEntry` struct expects `path` to be "Relative path (using / as separator)".
            // BUT, `list_dir` is generic. It should probably return just the name, or the path relative to the `path` argument.
            // `readdir` returns the path.
            // Let's rely on `file_name` for the entry path, and let the recursive scanner prepend the parent path.

            entries.push(FileEntry {
                path: file_name.to_string(), 
                size: stat.size.unwrap_or(0),
                mtime: stat.mtime.unwrap_or(0) as i64,
                mode: stat.perm.unwrap_or(0),
                is_dir: stat.is_dir(),
            });
        }
        Ok(entries)
    }

    fn create_dir_all(&self, path: &Path) -> Result<()> {
        let sftp = self.sftp()?;
        // Naive implementation: Try to create, if fail, try parent. 
        // Better: Iterate components.
        // Note: paths might be absolute.
        
        let mut stack = Vec::new();
        let mut p = path;
        while let Some(parent) = p.parent() {
            stack.push(p);
            p = parent;
        }
        stack.push(p); // Push root or first component? 
        // Path::components might be better but handling absolute paths on remote (Windows D:/) is tricky via Path.
        
        // Let's rely on the fact that we usually create directories that are subdirs of our target.
        // But here we need generic support.
        
        // Alternative: Use SFTP to stat. If missing, create.
        // Iterate from root is hard if we don't know the root format (C:\ vs /).
        // Let's try to create from the top down? No, we need to know where to start.
        
        // Let's just try to create. If it fails, assume it exists? No.
        // Actually, we can just split by components.
        
        // Problem: `std::path::Path` logic depends on local OS. 
        // If Linux client connects to Windows, Path might not parse `D:\` correctly as root.
        // But `D:/dist` is parsed as `D:` (prefix?)
        
        // Workaround for MVP:
        // Attempt to create the full path. If it fails, try parent. 
        // Recursion?
        
        self.create_dir_recursive(&sftp, path)
    }
}

impl SshConnection {
    fn create_dir_recursive(&self, sftp: &Sftp, path: &Path) -> Result<()> {
        // Check if exists
        if sftp.stat(path).is_ok() {
            return Ok(());
        }
        
        // Try to create parent first
        if let Some(parent) = path.parent() {
            // Avoid infinite recursion if parent is same as path (root)
            if parent != path && parent.as_os_str().len() > 0 {
                self.create_dir_recursive(sftp, parent)?;
            }
        }
        
        // Create current
        // Mode 0o755 is standard for dirs
        match sftp.mkdir(path, 0o755) {
            Ok(_) => Ok(()),
            Err(e) => {
                // Check again if it exists (race condition or root drive)
                if sftp.stat(path).is_ok() {
                    Ok(())
                } else {
                    // Ignore error for drive letters or roots?
                    // e.g. mkdir("D:") might fail.
                    // We assume that if stat failed and mkdir failed, it's an error.
                    // BUT, `stat("D:")` should pass.
                    Err(crate::FastSyncError::SshConnection(format!("Failed to create dir {:?}: {}", path, e)))
                }
            }
        }
    }

    pub fn connect(config: &SshConfig) -> Result<Self> {
        let tcp = TcpStream::connect((config.host.as_str(), config.port))
            .map_err(|e| crate::FastSyncError::SshConnection(format!("Failed to connect to {}:{}: {}", config.host, config.port, e)))?;
        
        let mut session = Session::new()
             .map_err(|e| crate::FastSyncError::SshConnection(e.to_string()))?;
        
        session.set_tcp_stream(tcp.try_clone().map_err(crate::FastSyncError::Io)?);
        session.handshake()
             .map_err(|e| crate::FastSyncError::SshConnection(format!("Handshake failed: {}", e)))?;
             
        if let Some(key) = &config.key_path {
             session.userauth_pubkey_file(&config.user, None, key, None)
                 .map_err(|e| crate::FastSyncError::Authentication(format!("Key auth failed: {}", e)))?;
        } else {
             // 1. Try Agent
             if session.userauth_agent(&config.user).is_err() || !session.authenticated() {
                 // 2. Agent failed, try default keys
                 let home = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).unwrap_or_default();
                 let default_keys = vec![
                     PathBuf::from(&home).join(".ssh/id_rsa"),
                     PathBuf::from(&home).join(".ssh/id_ed25519"),
                 ];
                 
                 for key in default_keys {
                     if key.exists() {
                         // Try auth with this key
                         if session.userauth_pubkey_file(&config.user, None, &key, None).is_ok() {
                             if session.authenticated() {
                                 break;
                             }
                         }
                     }
                 }
             }
        }
        
        if !session.authenticated() {
            return Err(crate::FastSyncError::Authentication("Authentication failed (Agent and default keys tried)".into()));
        }

        Ok(Self { session, _tcp: tcp })
    }
    
    pub fn sftp(&self) -> Result<Sftp> {
        self.session.sftp().map_err(|e| crate::FastSyncError::SshConnection(format!("SFTP init failed: {}", e)))
    }

    pub fn open_channel(&self) -> Result<ssh2::Channel> {
        self.session.channel_session()
             .map_err(|e| crate::FastSyncError::SshConnection(format!("Channel open failed: {}", e)))
    }
}

