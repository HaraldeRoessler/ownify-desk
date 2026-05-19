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
