use clap::Parser;
use fastsync::config::Args;
use fastsync::engine::SyncEngine;
use fastsync::server::Server;
use tracing::{error, info, Level};
use tracing_subscriber::FmtSubscriber;

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // Initialize logging
    let log_level = if args.verbose {
        Level::DEBUG
    } else if args.quiet {
        Level::ERROR
    } else {
        Level::INFO
    };

    // If server mode, log to stderr only? Or file? 
    // Standard stdout is used for protocol.
    // So logging MUST go to stderr.
    // FmtSubscriber defaults to stdout? No, it defaults to stdout.
    // We need to change it to stderr for server mode.
    
    let writer = if args.server {
        std::io::stderr
    } else {
        std::io::stderr // Actually CLI tools usually log to stderr to avoid corrupting pipes.
    };

    let subscriber = FmtSubscriber::builder()
        .with_max_level(log_level)
        .with_writer(writer)
        .finish();

    tracing::subscriber::set_global_default(subscriber)
        .expect("setting default subscriber failed");

    if args.server {
        info!("Starting server mode...");
        let server = Server::new();
        if let Err(e) = server.run() {
            error!("Server error: {}", e);
            std::process::exit(1);
        }
        return Ok(());
    }

    if args.update {
        handle_update()?;
        return Ok(());
    }

    // Check if source exists
    if let Some(source) = &args.source {
        if !source.exists() {
            error!("Source path does not exist: {:?}", source);
            std::process::exit(1);
        }
    } else {
        // Should be handled by clap, but safe check
        if !args.server {
             error!("Source path required");
             std::process::exit(1);
        }
    }

    let engine = SyncEngine::new(args);
    if let Err(e) = engine.run() {
        error!("Sync failed: {}", e);
        std::process::exit(1);
    }

    Ok(())
}

fn handle_update() -> anyhow::Result<()> {
    info!("Checking for updates...");
    
    // Choose the target name based on OS
    let bin_name = if cfg!(windows) {
        "fastsync-windows-amd64.exe"
    } else {
        "fastsync-linux-amd64"
    };

    let status = self_update::backends::github::Update::configure()
        .repo_owner("gux928")
        .repo_name("tool-rrsyc")
        .bin_name("fastsync")
        .show_download_progress(true)
        .current_version(env!("CARGO_PKG_VERSION"))
        .target(bin_name)
        .build()?
        .update()?;

    if status.updated() {
        info!("Update successful! New version: {}", status.version());
    } else {
        info!("Already up to date.");
    }

    Ok(())
}

