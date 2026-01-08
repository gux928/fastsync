use crate::Result;
use crate::scanner::{Manifest, Scanner};
use crate::transport::ssh::SshConnection;
use crate::protocol::{Request, Response};
use crate::delta::block_level::{FileSignature, FileDelta};
use ssh2::Channel;
use std::path::Path;
use std::io::{Read, Write};
use tracing::debug;

pub struct AgentRemote {
    channel: Channel,
}

impl AgentRemote {
    pub fn new(conn: &SshConnection, remote_cmd: &str) -> Result<Self> {
        let mut channel = conn.open_channel()?;
        
        channel.exec(remote_cmd)
             .map_err(|e| crate::FastSyncError::RemoteCommand(format!("Failed to exec agent: {}", e)))?;
        
        // Handshake
        let mut agent = Self { channel };
        agent.handshake()?;
        
        Ok(agent)
    }

    fn handshake(&mut self) -> Result<()> {
        self.send_request(Request::Hello { version: 1 })?;
        match self.read_response()? {
            Response::Hello { version } => {
                debug!("Remote agent version: {}", version);
                Ok(())
            },
            resp => Err(crate::FastSyncError::Protocol(format!("Unexpected handshake response: {:?}", resp))),
        }
    }

    fn send_request(&mut self, req: Request) -> Result<()> {
        let data = bincode::serialize(&req)
            .map_err(|e| crate::FastSyncError::Protocol(format!("Serialize error: {}", e)))?;
        
        let len = data.len() as u32;
        self.channel.write_all(&len.to_be_bytes()).map_err(crate::FastSyncError::Io)?;
        self.channel.write_all(&data).map_err(crate::FastSyncError::Io)?;
        self.channel.flush().map_err(crate::FastSyncError::Io)?;
        Ok(())
    }

    fn read_response(&mut self) -> Result<Response> {
        let mut len_buf = [0u8; 4];
        self.channel.read_exact(&mut len_buf).map_err(crate::FastSyncError::Io)?;
        let len = u32::from_be_bytes(len_buf) as usize;

        let mut buf = vec![0u8; len];
        self.channel.read_exact(&mut buf).map_err(crate::FastSyncError::Io)?;

        let resp = bincode::deserialize(&buf)
            .map_err(|e| crate::FastSyncError::Protocol(format!("Deserialize error: {}", e)))?;
        
        if let Response::Error { message } = &resp {
            return Err(crate::FastSyncError::RemoteCommand(message.clone()));
        }
        
        Ok(resp)
    }

    pub fn get_signature(&mut self, path: &str, block_size: usize) -> Result<FileSignature> {
        self.send_request(Request::GetSignature { path: path.to_string(), block_size })?;
        match self.read_response()? {
            Response::Signature(sig) => Ok(sig),
            resp => Err(crate::FastSyncError::Protocol(format!("Unexpected response for GetSignature: {:?}", resp))),
        }
    }

    pub fn apply_delta(&mut self, path: &str, delta: FileDelta) -> Result<()> {
        self.send_request(Request::ApplyDelta { path: path.to_string(), delta })?;
        match self.read_response()? {
            Response::Ok => Ok(()),
            resp => Err(crate::FastSyncError::Protocol(format!("Unexpected response for ApplyDelta: {:?}", resp))),
        }
    }
}

impl Scanner for AgentRemote {
    fn scan(&mut self, path: &Path) -> Result<Manifest> {
        self.send_request(Request::GetManifest { path: path.to_string_lossy().to_string() })?;
        match self.read_response()? {
            Response::Manifest(m) => Ok(m),
            resp => Err(crate::FastSyncError::Protocol(format!("Unexpected response for GetManifest: {:?}", resp))),
        }
    }
}
