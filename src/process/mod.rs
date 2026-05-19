use crate::config::ConfigManager;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use tokio::sync::RwLock;

pub type AgentStates = Arc<RwLock<HashMap<String, AgentState>>>;
type ChildProcesses = Arc<tokio::sync::Mutex<HashMap<String, Child>>>;

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
    children: ChildProcesses,
    config: Arc<ConfigManager>,
    microclaw_binary: String,
}

impl ProcessManager {
    pub fn new(config: Arc<ConfigManager>, microclaw_binary: String) -> Self {
        ProcessManager {
            states: Arc::new(RwLock::new(HashMap::new())),
            children: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            config,
            microclaw_binary,
        }
    }

    pub fn states(&self) -> AgentStates {
        self.states.clone()
    }

    pub async fn start_agent(&self, slug: &str) -> Result<AgentState> {
        tracing::info!("start_agent: loading meta for '{}'", slug);
        let meta = self.config.load_meta(slug)?;
        tracing::info!("start_agent: meta loaded, enabled={} auto_start={}", meta.enabled, meta.auto_start);
        if !meta.enabled {
            anyhow::bail!("Agent '{}' is disabled", slug);
        }

        let agent_dir = self.config.agent_dir(slug);
        let config_path = self.config.agent_config_path(slug);
        let log_file = self.config.agent_logs_dir(slug).join("microclaw.log");

        self.config.ensure_agent_dirs(slug)?;

        tracing::info!("start_agent: preparing config for '{}'", slug);
        // Inject the correct web_port into the config
        let config_content = self.config.load_microclaw_config(slug)?;
        let config_content = inject_port(&config_content, meta.port);
        self.config.save_microclaw_config(slug, &config_content)?;

        tracing::info!("start_agent: spawning '{}' binary={} config={}", slug, self.microclaw_binary, config_path.display());
        let mut cmd = Command::new(&self.microclaw_binary);
        cmd.arg("start")
            .env("MICROCLAW_CONFIG", config_path.to_str().unwrap_or_default())
            .current_dir(&agent_dir);

        if let Ok(log_file) = std::fs::File::create(&log_file) {
            let log_clone = log_file.try_clone()?;
            cmd.stdout(log_file).stderr(log_clone);
        } else {
            cmd.stdout(Stdio::null()).stderr(Stdio::null());
        }

        let child = cmd.spawn().with_context(|| {
            format!("Failed to start microclaw for agent '{}'", slug)
        })?;
        let pid = child.id();

        // Store child process so it doesn't get killed when Child is dropped
        self.children
            .lock()
            .await
            .insert(slug.to_string(), child);

        let state = AgentState {
            slug: slug.to_string(),
            port: meta.port,
            status: ProcessStatus::Running,
            pid: Some(pid),
        };

        self.states
            .write()
            .await
            .insert(slug.to_string(), state.clone());

        tracing::info!("Started agent '{}' (pid={}, port={})", slug, pid, meta.port);
        Ok(state)
    }

    pub async fn stop_agent(&self, slug: &str) -> Result<AgentState> {
        // Kill the process by PID regardless of state tracking
        if let Some(state) = self.states.read().await.get(slug) {
            if let Some(pid) = state.pid {
                let pid = pid as i32;
                #[cfg(unix)]
                unsafe {
                    libc::kill(pid, libc::SIGTERM);
                }
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                #[cfg(unix)]
                unsafe {
                    libc::kill(pid, libc::SIGKILL);
                }
            }
        }

        // Remove child from tracking
        self.children.lock().await.remove(slug);

        self.states
            .write()
            .await
            .insert(
                slug.to_string(),
                AgentState {
                    slug: slug.to_string(),
                    port: 0,
                    status: ProcessStatus::Stopped,
                    pid: None,
                },
            );

        tracing::info!("Stopped agent '{}'", slug);
        Ok(AgentState {
            slug: slug.to_string(),
            port: 0,
            status: ProcessStatus::Stopped,
            pid: None,
        })
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
    for line in &mut lines {
        let trimmed = line.trim_start();
        if trimmed.starts_with("web_port:") && !trimmed.starts_with('#') {
            *line = format!("web_port: {}", port);
            break;
        }
    }
    lines.join("\n")
}

pub fn find_microclaw_binary() -> Result<String> {
    let home = std::env::var("HOME").unwrap_or_default();
    for candidate in &[
        "/usr/local/bin/microclaw",
        &format!("{}/.local/bin/microclaw", home),
        &format!("{}/.cargo/bin/microclaw", home),
        "./microclaw",
    ] {
        if std::path::Path::new(candidate).exists() {
            tracing::info!("Found microclaw at {}", candidate);
            return Ok(candidate.to_string());
        }
    }
    anyhow::bail!(
        "microclaw binary not found. Install it with 'ownify-desk bootstrap'"
    )
}
