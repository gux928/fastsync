use serde::{Deserialize, Serialize};
use crate::scanner::Manifest;
use crate::delta::block_level::{FileSignature, FileDelta};

#[derive(Debug, Serialize, Deserialize)]
pub enum Request {
    /// Handshake / Check capability
    Hello { version: u32 },
    
    /// Get file list from remote
    GetManifest { path: String },
    
    /// Get block signatures for a file (for delta calculation)
    GetSignature { path: String, block_size: usize },
    
    /// Apply delta to a file (patching)
    ApplyDelta { path: String, delta: FileDelta },
    
    /// Create directory
    MkDir { path: String, mode: u32 },

    /// Set file metadata (mtime/permissions) after transfer
    SetMetadata { path: String, mtime: i64, mode: u32 },
    
    /// Delete file/dir
    Delete { path: String },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Response {
    /// Generic Ack
    Ok,
    
    /// Handshake Ack
    Hello { version: u32 },
    
    /// Return Manifest
    Manifest(Manifest),
    
    /// Return Signature
    Signature(FileSignature),
    
    /// Error occurred
    Error { message: String },
}
