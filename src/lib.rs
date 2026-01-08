pub mod config;
pub mod scanner;
pub mod delta;
pub mod transport;
pub mod remote;
pub mod apply;
pub mod util;
pub mod error;
pub mod engine;
pub mod protocol;
pub mod server;

pub use error::FastSyncError;
pub type Result<T> = std::result::Result<T, FastSyncError>;
