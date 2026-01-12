use crate::Result;
use crate::config::Args;
use crate::transport::ssh::{SshConfig, SshConnection};
use crate::transport::Transport;
use crate::scanner::{Scanner, LocalScanner, Manifest};
use crate::remote::agentless::AgentlessRemote;
use crate::remote::agent::AgentRemote;
use crate::delta::block_level::{compute_delta, DEFAULT_BLOCK_SIZE};
use crate::delta::file_level::{compute_diff, SyncAction};
use std::path::Path;
use tracing::{info, error, debug};
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use std::sync::{Arc, Mutex};

pub struct SyncEngine {
    args: Args,
}

impl SyncEngine {
    pub fn new(args: Args) -> Self {
        Self { args }
    }

    pub fn run(&self) -> Result<()> {
        // 1. Connect
        let destination = self.args.destination.as_ref().expect("Destination required in client mode");
        let (user, host, remote_path) = parse_destination(destination)
            .ok_or_else(|| crate::FastSyncError::Config("Invalid destination format. Expected user@host:path".into()))?;
        let is_windows_remote = is_windows_remote_path(remote_path);

        info!("Connecting to {}@{}...", user, host);
        let ssh_config = SshConfig {
            host: host.to_string(),
            port: self.args.port,
            user: user.to_string(),
            key_path: self.args.identity.clone(),
        };
        
        let conn = Arc::new(SshConnection::connect(&ssh_config)?);
        info!("Connected.");

        // 2. Scan Local
        let source_path = self.args.source.as_ref().expect("Source required in client mode");
        info!("Scanning local directory: {:?}", source_path);
        let mut local_scanner = LocalScanner::new(self.args.exclude.clone());
        let local_manifest = local_scanner.scan(source_path)?;
        info!("Found {} local items.", local_manifest.entries.len());

        // 3. Scan Remote
        info!("Scanning remote directory: {}", remote_path);
        
        if !self.args.dry_run {
             conn.create_dir_all(Path::new(remote_path))?;
        }

        let remote_manifest: Manifest;

        if self.args.block_level {
             info!("Starting remote agent (scan)...");
             let mut agent = match AgentRemote::new(&conn, "fastsync --server") {
                 Ok(a) => a,
                 Err(e) => {
                     error!("Failed to start remote agent. Make sure 'fastsync' is installed on remote and in PATH. Error: {}", e);
                     return Err(e);
                 }
             };
             
             match agent.scan(Path::new(remote_path)) {
                 Ok(m) => remote_manifest = m,
                 Err(e) => return Err(e),
             }
        } else {
            let mut remote_scanner = AgentlessRemote::new(conn.as_ref());
            remote_manifest = match remote_scanner.scan(Path::new(remote_path)) {
                Ok(m) => m,
                Err(e) => {
                    if self.args.dry_run {
                        crate::scanner::Manifest {
                            generated_at: 0,
                            root_path: remote_path.to_string(),
                            entries: vec![],
                        }
                    } else {
                        return Err(e);
                    }
                }
            };
        }
        
        info!("Found {} remote items.", remote_manifest.entries.len());

        // 4. Compute Diff
        info!("Computing differences...");
        let actions = compute_diff(&local_manifest, &remote_manifest, self.args.delete);
        info!("Found {} actions to perform.", actions.len());
        
        if self.args.dry_run {
            for action in actions {
                match action {
                    SyncAction::Upload(entry) => println!("UPLOAD: {}", entry.path),
                    SyncAction::Delete(path) => println!("DELETE: {}", path),
                }
            }
            return Ok(());
        }

        // 5. Apply
        let mut uploads = Vec::new();
        let mut deletes = Vec::new();

        for action in actions {
            match action {
                SyncAction::Upload(entry) => uploads.push(entry),
                SyncAction::Delete(path) => deletes.push(path),
            }
        }
        
        if !deletes.is_empty() {
             info!("Deleting {} files/dirs...", deletes.len());
             for path in deletes {
                 let remote_file_path = Path::new(remote_path).join(path);
                 let remote_file_str = remote_file_path.to_string_lossy();
                 if is_windows_remote {
                     let ps_path = escape_powershell_literal(&remote_file_str);
                     let cmd = format!(
                         "powershell -NoProfile -NonInteractive -Command \"Remove-Item -LiteralPath '{}' -Force -Recurse -ErrorAction Stop\"",
                         ps_path
                     );
                     conn.exec(&cmd)?;
                 } else {
                     let sh_path = escape_posix_literal(&remote_file_str);
                     let cmd = format!("rm -rf -- '{}'", sh_path);
                     conn.exec(&cmd)?;
                 }
             }
        }
        
        if uploads.is_empty() {
             info!("Sync completed (no uploads).");
             return Ok(());
        }

        let errors = Arc::new(Mutex::new(Vec::new()));
        let pb = if self.args.progress {
            let pb = ProgressBar::new(uploads.len() as u64);
            pb.set_style(ProgressStyle::default_bar().template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) {msg}").unwrap());
            Some(pb)
        } else { None };

        let remote_path_base = Path::new(remote_path);
        let source_base = source_path;

        if self.args.block_level {
            info!("Syncing with Block-Level incremental (Parallel)...");
            
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(self.args.parallel)
                .build()
                .map_err(|e| crate::FastSyncError::Config(format!("Failed to build thread pool: {}", e)))?;
                
            let ssh_config = ssh_config.clone();
            let agent_pool = Arc::new(Mutex::new(Vec::new()));
            
            pool.install(|| {
                uploads.par_iter().for_each(|entry| {
                     let mut ctx = {
                         let mut pool = agent_pool.lock().unwrap();
                         pool.pop()
                     };
                     
                     if ctx.is_none() {
                         match SshConnection::connect(&ssh_config) {
                             Ok(c) => ctx = Some(c),
                             Err(e) => {
                                 error!("Failed to connect: {}", e);
                                 errors.lock().unwrap().push(format!("{}: Connect failed", entry.path));
                                 return;
                             }
                         }
                     }
                     
                     let context_conn = ctx.unwrap();
                     
                     let result = (|| -> Result<()> {
                         let mut agent = AgentRemote::new(&context_conn, "fastsync --server")?;
                         
                         let local_file_path = source_base.join(&entry.path);
                         let remote_file_path = remote_path_base.join(&entry.path);
                         let remote_path_str = remote_file_path.to_string_lossy().to_string();

                         if entry.is_dir {
                             debug!("Creating remote directory: {}", remote_path_str);
                             context_conn.create_dir_all(Path::new(&remote_path_str))?;
                         } else {
                            if let Some(pb) = &pb { pb.set_message(format!("Syncing file {}", entry.path)); }
                            
                            if let Some(parent) = remote_file_path.parent() {
                                context_conn.create_dir_all(parent)?;
                            }

                            let sig = agent.get_signature(&remote_path_str, DEFAULT_BLOCK_SIZE)
                                .unwrap_or_else(|_| {
                                     crate::delta::block_level::FileSignature {
                                         blocks: vec![],
                                         block_size: DEFAULT_BLOCK_SIZE,
                                         file_size: 0,
                                     }
                                });
                            
                            let local_data = std::fs::read(&local_file_path).map_err(crate::FastSyncError::Io)?;
                            let delta = compute_delta(&local_data, &sig);
                            
                            agent.apply_delta(&remote_path_str, delta)?;
                            
                            if is_windows_remote {
                                let ps_path = escape_powershell_literal(&remote_path_str);
                                let cmd = format!(
                                    "powershell -NoProfile -NonInteractive -Command \"(Get-Item -LiteralPath '{}').LastWriteTimeUtc = [DateTimeOffset]::FromUnixTimeSeconds({}).UtcDateTime\"",
                                    ps_path,
                                    entry.mtime
                                );
                                context_conn.exec(&cmd).ok();
                            } else {
                                let sh_path = escape_posix_literal(&remote_path_str);
                                let cmd = format!("touch -d @{} '{}'", entry.mtime, sh_path);
                                context_conn.exec(&cmd).ok();
                            }
                         }
                         Ok(())
                     })();

                     if let Err(e) = result {
                         error!("Sync error for {}: {}", entry.path, e);
                         errors.lock().unwrap().push(format!("{}: {}", entry.path, e));
                     } else {
                         agent_pool.lock().unwrap().push(context_conn);
                     }
                     
                     if let Some(pb) = &pb { pb.inc(1); }
                });
            });
        } else {
            // Parallel Uploads (File Level)
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(self.args.parallel)
                .build()
                .map_err(|e| crate::FastSyncError::Config(format!("Failed to build thread pool: {}", e)))?;
                
            pool.install(|| {
                uploads.par_iter().for_each(|entry| {
                    let local_file_path = source_base.join(&entry.path);
                    let remote_file_path = remote_path_base.join(&entry.path);
                    
                    let result = (|| -> Result<()> {
                        if entry.is_dir {
                             conn.create_dir_all(&remote_file_path)?;
                        } else {
                             if let Some(pb) = &pb {
                                 pb.set_message(format!("Uploading {}", entry.path));
                             }
                             if let Some(parent) = remote_file_path.parent() {
                                 conn.create_dir_all(parent)?;
                             }
                             conn.upload_file(&local_file_path, &remote_file_path)?;
                             let remote_path_str = remote_file_path.to_string_lossy();
                             if is_windows_remote {
                                 let ps_path = escape_powershell_literal(&remote_path_str);
                                 let cmd = format!(
                                     "powershell -NoProfile -NonInteractive -Command \"(Get-Item -LiteralPath '{}').LastWriteTimeUtc = [DateTimeOffset]::FromUnixTimeSeconds({}).UtcDateTime\"",
                                     ps_path,
                                     entry.mtime
                                 );
                                 conn.exec(&cmd).ok();
                             } else {
                                 let sh_path = escape_posix_literal(&remote_path_str);
                                 let cmd = format!(
                                     "touch -d @{} '{}' && chmod {:o} '{}'", 
                                     entry.mtime,
                                     sh_path,
                                     entry.mode & 0o777,
                                     sh_path
                                 );
                                 conn.exec(&cmd).ok();
                             }
                        }
                        Ok(())
                    })();
                    
                    if let Err(e) = result {
                        error!("Sync error for {}: {}", entry.path, e);
                        errors.lock().unwrap().push(format!("{}: {}", entry.path, e));
                    }
                    if let Some(pb) = &pb { pb.inc(1); }
                });
            });
        }

        if let Some(pb) = &pb {
            pb.finish_with_message("Done");
        }
        
        let final_errors = errors.lock().unwrap();
        if !final_errors.is_empty() {
            error!("Encoutered {} errors during sync.", final_errors.len());
            return Err(crate::FastSyncError::Io(std::io::Error::new(std::io::ErrorKind::Other, "Sync completed with errors")));
        }

        info!("Sync completed successfully.");
        Ok(())
    }
}

fn parse_destination(dest: &str) -> Option<(&str, &str, &str)> {
    let parts: Vec<&str> = dest.splitn(2, ':').collect();
    if parts.len() != 2 { return None; }
    let remote_path = parts[1];
    
    let user_host: Vec<&str> = parts[0].splitn(2, '@').collect();
    if user_host.len() != 2 { return None; }
    let user = user_host[0];
    let host = user_host[1];
    
    Some((user, host, remote_path))
}

fn is_windows_remote_path(remote_path: &str) -> bool {
    let bytes = remote_path.as_bytes();
    bytes.len() > 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic()
}

fn escape_posix_literal(value: &str) -> String {
    value.replace('\'', "'\\''")
}

fn escape_powershell_literal(value: &str) -> String {
    value.replace('\'', "''")
}
