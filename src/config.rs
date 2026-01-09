use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Source path (local directory)
    #[arg(value_name = "SOURCE", required_unless_present_any = ["server", "update"])]
    pub source: Option<PathBuf>,

    /// Destination path (user@host:path or local path)
    #[arg(value_name = "DESTINATION", required_unless_present_any = ["server", "update"])]
    pub destination: Option<String>,

    /// Exclude patterns (gitignore style)
    #[arg(short, long)]
    pub exclude: Vec<String>,

    /// Delete extraneous files from destination dirs
    #[arg(long, default_value_t = false)]
    pub delete: bool,

    /// Perform a trial run with no changes made
    #[arg(short = 'n', long, default_value_t = false)]
    pub dry_run: bool,

    /// Show progress during transfer
    #[arg(short = 'P', long, default_value_t = true)]
    pub progress: bool,

    /// Compress file data during the transfer
    #[arg(short = 'z', long, default_value_t = false)]
    pub compress: bool,

    /// Number of parallel transfers
    #[arg(short = 'j', long, default_value_t = 4)]
    pub parallel: usize,

    /// Identity file for SSH
    #[arg(short = 'i', long)]
    pub identity: Option<PathBuf>,

    /// Port for SSH
    #[arg(short = 'p', long, default_value_t = 22)]
    pub port: u16,

    /// Suppress non-error messages
    #[arg(short, long, default_value_t = false)]
    pub quiet: bool,

    /// Increase verbosity
    #[arg(short, long, default_value_t = false)]
    pub verbose: bool,

    /// Enable block-level incremental sync (requires agent on remote)
    #[arg(short = 'b', long, default_value_t = false)]
    pub block_level: bool,

    /// Skip based on checksum, not mod-time & size
    #[arg(short = 'c', long, default_value_t = false)]
    pub checksum: bool,

    /// Run in server mode (Agent mode)
    #[arg(long, default_value_t = false, hide = true)]
    pub server: bool,

    /// Update the tool to the latest version
    #[arg(long)]
    pub update: bool,
}
