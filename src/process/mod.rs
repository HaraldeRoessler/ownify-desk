use crate::config::ConfigManager;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::RwLock;

pub type AgentStates = Arc<RwLock<HashMap<String, AgentState>>>;

#[derive(Debug, Clone)]
pub struct AgentState {
    pub slug: String,
    pub port: u16,
    pub status: ProcessStatus,
    pub pid: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProcessStatus {
    Running,
    Stopped,
    Crashed,
    Starting,
}

pub struct ProcessManager {
    states: AgentStates,
    config: Arc<ConfigManager>,
    microclaw_binary: String,
}

impl ProcessManager {
    pub fn new(config: Arc<ConfigManager>, microclaw_binary: String) -> Self {
        ProcessManager {
            states: Arc::new(RwLock::new(HashMap::new())),
            config,
            microclaw_binary,
        }
    }

    pub fn states(&self) -> AgentStates {
        self.states.clone()
    }

    pub async fn start_agent(&self, slug: &str) -> Result<AgentState> {
        let meta = self.config.load_meta(slug)?;
        if !meta.enabled {
            anyhow::bail!("Agent '{}' is disabled", slug);
        }
        if !meta.auto_start {
            anyhow::bail!(
                "Agent '{}' has auto_start disabled — enable it in the dashboard first",
                slug
            );
        }

        let agent_dir = self.config.agent_dir(slug);
        let config_path = self.config.agent_config_path(slug);
        let _data_dir = self.config.agent_data_dir(slug);
        let log_file = self.config.agent_logs_dir(slug).join("microclaw.log");

        self.config.ensure_agent_dirs(slug)?;

        // Inject the correct web_port into the config
        let mut config_content = self.config.load_microclaw_config(slug)?;
        config_content = inject_port(&config_content, meta.port);
        self.config.save_microclaw_config(slug, &config_content)?;

        // Build microclaw arguments
        let mut cmd = Command::new(&self.microclaw_binary);
        cmd.arg("start")
            .env("MICROCLAW_CONFIG", &config_path)
            .current_dir(&agent_dir);

        if let Ok(log_file) = std::fs::File::create(&log_file) {
            let log_stdout = log_file.try_clone()?;
            let log_stderr = log_file.try_clone()?;
            cmd.stdout(log_stdout).stderr(log_stderr);
        }

        let child = cmd
            .spawn()
            .with_context(|| format!("Failed to start microclaw for agent '{}'", slug))?;
        let pid = child.id();

        // Store the running child in state (we let it run detached)
        // In a production version we'd spawn a monitor task for it.
        let state = AgentState {
            slug: slug.to_string(),
            port: meta.port,
            status: ProcessStatus::Running,
            pid,
        };

        self.states
            .write()
            .await
            .insert(slug.to_string(), state.clone());

        tracing::info!("Started agent '{}' (pid={:?}, port={})", slug, pid, meta.port);
        Ok(state)
    }

    pub async fn stop_agent(&self, slug: &str) -> Result<AgentState> {
        let mut states = self.states.write().await;
        let state = states
            .get_mut(slug)
            .ok_or_else(|| anyhow::anyhow!("Agent '{}' not running", slug))?;

        // Try graceful shutdown via SIGTERM
        if let Some(pid) = state.pid {
            let pid = pid as i32;
            #[cfg(unix)]
            {
                unsafe { libc::kill(pid, libc::SIGTERM) };
                // Give it 5 seconds, then SIGKILL
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                unsafe { libc::kill(pid, libc::SIGKILL) };
            }
        }

        state.status = ProcessStatus::Stopped;
        state.pid = None;
        tracing::info!("Stopped agent '{}'", slug);
        Ok(state.clone())
    }

    pub async fn get_state(&self, slug: &str) -> Option<AgentState> {
        self.states.read().await.get(slug).cloned()
    }

    pub async fn all_states(&self) -> Vec<AgentState> {
        self.states
            .read()
            .await
            .values()
            .cloned()
            .collect()
    }
}

fn inject_port(config: &str, port: u16) -> String {
    let mut lines: Vec<String> = config.lines().map(|l| l.to_string()).collect();
    let mut found = false;
    for line in &mut lines {
        if line.trim_start().starts_with("web_port:") {
            *line = format!("web_port: {}", port);
            found = true;
            break;
        }
    }
    if !found {
        lines.push(format!("web_port: {}", port));
    }
    lines.join("\n")
}

pub fn find_microclaw_binary() -> Result<String> {
    for candidate in &[
        "/usr/local/bin/microclaw",
        &format!("{}/.local/bin/microclaw", std::env::var("HOME").unwrap_or_default()),
        &format!("{}/.cargo/bin/microclaw", std::env::var("HOME").unwrap_or_default()),
        "./microclaw",
    ] {
        if std::path::Path::new(candidate).exists() {
            tracing::info!("Found microclaw at {}", candidate);
            return Ok(candidate.to_string());
        }
    }
    anyhow::bail!(
        "microclaw binary not found. Install it from https://github.com/HaraldeRoessler/ownify-microclaw"
    )
}
