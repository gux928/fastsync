use thiserror::Error;
use std::path::PathBuf;

#[derive(Debug, Error)]
pub enum FastSyncError {
    #[error("SSH connection failed: {0}")]
    SshConnection(String),
    
    #[error("Authentication failed: {0}")]
    Authentication(String),
    
    #[error("File not found: {0}")]
    FileNotFound(PathBuf),
    
    #[error("Permission denied: {0}")]
    PermissionDenied(PathBuf),
    
    #[error("Remote command failed: {0}")]
    RemoteCommand(String),
    
    #[error("Protocol error: {0}")]
    Protocol(String),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Checksum mismatch for {path}")]
    ChecksumMismatch { path: PathBuf },

    #[error("Config error: {0}")]
    Config(String),
    
    #[error("WalkDir error: {0}")]
    WalkDir(#[from] walkdir::Error),
    
    #[error("Pattern error: {0}")]
    Pattern(#[from] globset::Error),
}
