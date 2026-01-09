use clap::Parser;
use fastsync::config::Args;
use fastsync::engine::SyncEngine;
use fastsync::server::Server;
use tracing::{error, info, warn, Level};
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

    let (bin_name, asset_name) = if cfg!(windows) {
        ("fastsync.exe", "fastsync-windows-x64.zip")
    } else {
        ("fastsync", "fastsync-linux-x64.tar.gz")
    };

    match self_update::backends::github::Update::configure()
        .repo_owner("gux928")
        .repo_name("fastsync")
        .bin_name(bin_name)
        .show_download_progress(true)
        .current_version(env!("CARGO_PKG_VERSION"))
        .target(asset_name)
        .build()?
        .update()
    {
        Ok(status) => {
            if status.updated() {
                info!("Update successful! New version: {}", status.version());
            } else {
                info!("Already up to date.");
            }
            Ok(())
        }
        Err(err) => {
            warn!("GitHub update failed, trying mirror fallback: {}", err);
            update_from_mirrors(bin_name, asset_name)?;
            info!("Update successful via mirror.");
            Ok(())
        }
    }
}

fn update_from_mirrors(bin_name: &str, asset_name: &str) -> anyhow::Result<()> {
    let urls = mirror_asset_urls(asset_name);
    let tmp_dir = self_update::TempDir::new()?;
    let tmp_archive_path = tmp_dir.path().join(asset_name);

    let mut last_err: Option<anyhow::Error> = None;
    for url in urls {
        let mut tmp_archive = std::fs::File::create(&tmp_archive_path)?;
        let mut download = self_update::Download::from_url(&url);
        download.show_progress(true);
        match download.download_to(&mut tmp_archive) {
            Ok(_) => {
                let bin_path = tmp_dir.path().join(bin_name);
                self_update::Extract::from_source(&tmp_archive_path)
                    .extract_file(tmp_dir.path(), bin_name)?;
                self_update::self_replace::self_replace(bin_path)?;
                return Ok(());
            }
            Err(err) => {
                last_err = Some(err.into());
                continue;
            }
        }
    }

    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("All mirrors failed")))
}

fn mirror_asset_urls(asset_name: &str) -> Vec<String> {
    let base = format!(
        "https://github.com/{}/{}/releases/latest/download/{}",
        "gux928", "fastsync", asset_name
    );
    vec![
        format!(
            "https://dl.fastsync.190hao.cn/releases/latest/download/{}",
            asset_name
        ),
        base.clone(),
        format!("https://ghproxy.com/{}", base),
        format!(
            "https://download.fastgit.org/{}/{}/releases/latest/download/{}",
            "gux928", "fastsync", asset_name
        ),
    ]
}
