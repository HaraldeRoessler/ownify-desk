mod api;
mod config;
mod process;

use clap::{Parser, Subcommand};
use std::sync::Arc;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

#[derive(Parser)]
#[command(name = "ownify-desk")]
#[command(about = "Local multi-agent management dashboard for ownify microclaw", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the dashboard and managed agents
    Start {
        /// Dashboard port (default: 9090)
        #[arg(long, default_value = "9090")]
        port: u16,

        /// Path to microclaw binary
        #[arg(long)]
        microclaw_binary: Option<String>,
    },

    /// Auto-install the microclaw agent binary from GitHub releases
    Bootstrap {
        /// Specific version to install (default: latest)
        #[arg(long)]
        version: Option<String>,
    },

    /// Show status of all agents
    Status,

    /// Initialize the data directory
    Init,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "ownify_desk=info".into()))
        .init();

    let cli = Cli::parse();

    let desk_dir = dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".ownify-desk");

    let config_manager = Arc::new(config::ConfigManager::new(desk_dir.clone()));
    config_manager.ensure_dirs()?;

    match cli.command {
        Commands::Init => {
            config_manager.ensure_dirs()?;
            tracing::info!("Initialized ownify-desk at {}", desk_dir.display());
            Ok(())
        }
        Commands::Bootstrap { version } => {
            bootstrap_microclaw(version).await?;
            Ok(())
        }
        Commands::Status => {
            let agents = config_manager.list_agents()?;
            if agents.is_empty() {
                println!("No agents configured.");
                println!(
                    "Start the dashboard with 'ownify-desk start' to create your first agent."
                );
            } else {
                println!("Agents:");
                for agent in agents {
                    let status = if agent.enabled {
                        "enabled"
                    } else {
                        "disabled"
                    };
                    println!(
                        "  {:<20} port {:>5}  {}  {}",
                        agent.slug, agent.port, status, agent.display_name
                    );
                }
            }
            Ok(())
        }
        Commands::Start {
            port,
            microclaw_binary,
        } => {
            let microclaw_path = match microclaw_binary {
                Some(p) => p,
                None => process::find_microclaw_binary()?,
            };

            let process_manager = Arc::new(process::ProcessManager::new(
                config_manager.clone(),
                microclaw_path,
            ));

            let state = Arc::new(api::AppState {
                config: config_manager,
                process: process_manager,
                agent_cookies: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            });

            let app = api::router(state);

            let addr = format!("127.0.0.1:{}", port);
            tracing::info!("ownify-desk dashboard starting at http://{}", addr);
            tracing::info!("Data directory: {}", desk_dir.display());

            let listener = tokio::net::TcpListener::bind(&addr).await?;
            axum::serve(listener, app).await?;
            Ok(())
        }
    }
}

async fn bootstrap_microclaw(version: Option<String>) -> anyhow::Result<()> {
    let install_dir = dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".local")
        .join("bin");
    std::fs::create_dir_all(&install_dir)?;

    let target_path = install_dir.join("microclaw");

    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;

    let platform = match os {
        "macos" => "apple-darwin",
        "linux" => "unknown-linux-gnu",
        _ => anyhow::bail!("Unsupported OS: {}. ownify-desk runs on macOS and Linux.", os),
    };

    let arch_name = match arch {
        "aarch64" => "aarch64",
        "x86_64" => "x86_64",
        _ => anyhow::bail!("Unsupported architecture: {}", arch),
    };

    let version = version.unwrap_or_else(|| "latest".to_string());
    let tag = if version == "latest" {
        let client = reqwest::Client::new();
        let resp = client
            .get("https://api.github.com/repos/HaraldeRoessler/ownify-microclaw/releases/latest")
            .header("User-Agent", "ownify-desk")
            .header("Accept", "application/vnd.github+json")
            .send()
            .await?;

        let status = resp.status();
        let text = resp.text().await?;

        if !status.is_success() {
            anyhow::bail!(
                "GitHub API returned {}: {}",
                status,
                &text[..text.len().min(200)]
            );
        }

        let json: serde_json::Value = serde_json::from_str(&text)?;
        json["tag_name"].as_str().unwrap_or("v0.1.52").to_string()
    } else {
        format!("v{}", version)
    };

    let asset = format!(
        "microclaw-{}-{}-{}.tar.gz",
        tag, arch_name, platform
    );
    let url = format!(
        "https://github.com/HaraldeRoessler/ownify-microclaw/releases/download/{}/{}",
        tag, asset
    );

    tracing::info!("Downloading microclaw {} from {}", tag, url);
    
    let response = reqwest::get(&url).await?;
    if !response.status().is_success() {
        anyhow::bail!(
            "Could not download {}\n\nNo prebuilt binary for {}/{}.\n\
             Build from source:\n  git clone https://github.com/HaraldeRoessler/ownify-microclaw.git\n  \
             cd ownify-microclaw && cargo build --release\n  \
             cp target/release/microclaw {}",
            asset, os, arch, target_path.display()
        );
    }

    let bytes = response.bytes().await?;
    let cursor = std::io::Cursor::new(bytes);
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(cursor));
    archive.unpack(&install_dir)?;

    // Make executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&target_path)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&target_path, perms)?;
    }

    tracing::info!("✓ microclaw {} installed to {}", tag, target_path.display());
    tracing::info!("");
    tracing::info!("Make sure {} is in your PATH:", install_dir.display());
    tracing::info!("  export PATH=\"{}:$PATH\"", install_dir.display());
    tracing::info!("");
    tracing::info!("Now start the dashboard:");
    tracing::info!("  ownify-desk start");

    Ok(())
}
