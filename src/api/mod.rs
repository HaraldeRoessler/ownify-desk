use crate::config::{is_valid_slug, AgentMeta, ConfigManager};
use crate::process::{ProcessManager, ProcessStatus};
use axum::{
    extract::{Path, State},
    response::IntoResponse,
    routing::{delete, get, post, put},
    Json, Router,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_http::cors::{Any, CorsLayer};

pub struct AppState {
    pub config: Arc<ConfigManager>,
    pub process: Arc<ProcessManager>,
    pub agent_cookies: Arc<Mutex<HashMap<String, String>>>, // session cookies per slug
}

pub mod proxy;

pub fn router(state: Arc<AppState>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let api = Router::new()
        // Agent lifecycle
        .route("/api/agents/:slug/start", post(start_agent))
        .route("/api/agents/:slug/stop", post(stop_agent))
        .route("/api/agents/:slug/restart", post(restart_agent))
        // Agent config
        .route("/api/agents/:slug/config", get(get_config))
        .route("/api/agents/:slug/config", put(save_config))
        // Agent logs
        .route("/api/agents/:slug/logs", get(get_logs))
        // Agent CRUD
        .route("/api/agents", get(list_agents))
        .route("/api/agents", post(create_agent))
        .route("/api/agents/:slug", get(get_agent))
        .route("/api/agents/:slug", put(update_agent))
        .route("/api/agents/:slug", delete(delete_agent))
        // Dashboard
        .route("/api/dashboard", get(get_dashboard))
        .route("/api/status", get(get_status))
        // Proxy routes to running microclaw agents
        .route("/api/proxy/:slug/health", get(proxy::proxy_health))
        .route("/api/proxy/:slug/config", get(proxy::proxy_get_config))
        .route("/api/proxy/:slug/config/self_check", get(proxy::proxy_config_self_check))
        .route("/api/proxy/:slug/sessions", get(proxy::proxy_sessions))
        .route("/api/proxy/:slug/history", get(proxy::proxy_history))
        .route("/api/proxy/:slug/audit", get(proxy::proxy_audit))
        .route("/api/proxy/:slug/usage", get(proxy::proxy_usage))
        .route("/api/proxy/:slug/skills", get(proxy::proxy_skills))
        .route("/api/proxy/:slug/metrics", get(proxy::proxy_metrics))
        .route("/api/proxy/:slug/metrics/history", get(proxy::proxy_metrics_history))
        .route("/api/proxy/:slug/memory_observability", get(proxy::proxy_memory_observability))
        .route("/api/proxy/:slug/auth/status", get(proxy::proxy_auth_status))
        .route("/api/proxy/:slug/a2a/agent_card", get(proxy::proxy_a2a_agent_card))
        .layer(cors)
        .with_state(state);

    // Serve static web UI files (everything outside /api)
    api.fallback_service(tower_http::services::ServeDir::new("web/dist"))
}

// ---------- Types ----------

#[derive(Debug, Serialize)]
struct ApiResponse<T: Serialize> {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

impl<T: Serialize> ApiResponse<T> {
    fn success(data: T) -> Self {
        ApiResponse {
            ok: true,
            data: Some(data),
            error: None,
        }
    }

    fn error(msg: impl Into<String>) -> Self {
        ApiResponse {
            ok: false,
            data: None,
            error: Some(msg.into()),
        }
    }
}

#[derive(Debug, Deserialize)]
struct CreateAgentRequest {
    slug: String,
    display_name: String,
    #[serde(default)]
    description: String,
    #[serde(default = "default_true")]
    auto_start: bool,
    #[serde(default = "default_true")]
    enabled: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Serialize)]
struct AgentDetail {
    meta: AgentMeta,
    status: ProcessStatus,
    pid: Option<u32>,
}

#[derive(Debug, Serialize)]
struct AgentListItem {
    slug: String,
    display_name: String,
    description: String,
    port: u16,
    status: ProcessStatus,
    enabled: bool,
    auto_start: bool,
}

#[derive(Debug, Serialize)]
struct DashboardData {
    agents: Vec<AgentListItem>,
    total: usize,
    running: usize,
    stopped: usize,
}

// ---------- Agent CRUD ----------

async fn list_agents(
    State(state): State<Arc<AppState>>,
) -> Json<ApiResponse<Vec<AgentListItem>>> {
    let agents = match state.config.list_agents() {
        Ok(a) => a,
        Err(e) => return Json(ApiResponse::error(e.to_string())),
    };

    let mut items = Vec::new();
    let running = state.process.all_states().await;
    for m in agents {
        let status = running
            .iter()
            .find(|s| s.slug == m.slug)
            .map(|s| s.status.clone())
            .unwrap_or(ProcessStatus::Stopped);
        items.push(AgentListItem {
            slug: m.slug,
            display_name: m.display_name,
            description: m.description,
            port: m.port,
            status,
            enabled: m.enabled,
            auto_start: m.auto_start,
        });
    }

    Json(ApiResponse::success(items))
}

async fn create_agent(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateAgentRequest>,
) -> Json<ApiResponse<AgentMeta>> {
    let slug = req.slug.to_lowercase().trim().to_string();

    if !is_valid_slug(&slug) {
        return Json(ApiResponse::error(
            "Invalid slug: use lowercase letters, numbers, and hyphens (max 32 chars)",
        ));
    }

    // Check for duplicates
    if let Ok(agents) = state.config.list_agents() {
        if agents.iter().any(|a| a.slug == slug) {
            return Json(ApiResponse::error(format!(
                "Agent '{}' already exists",
                slug
            )));
        }
    }

    let port = match state.config.next_port() {
        Ok(p) => p,
        Err(e) => return Json(ApiResponse::error(e.to_string())),
    };

    let now = Utc::now().to_rfc3339();
    let meta = AgentMeta {
        slug: slug.clone(),
        display_name: req.display_name,
        description: req.description,
        port,
        auto_start: req.auto_start,
        enabled: req.enabled,
        created_at: now.clone(),
        updated_at: now,
    };

    if let Err(e) = state.config.save_meta(&meta) {
        return Json(ApiResponse::error(e.to_string()));
    }

    // Generate default config
    if let Err(e) = state.config.save_microclaw_config(&slug, &crate::config::generate_default_config(&slug)) {
        return Json(ApiResponse::error(e.to_string()));
    }

    Json(ApiResponse::success(meta))
}

async fn get_agent(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
) -> Json<ApiResponse<AgentDetail>> {
    let meta = match state.config.load_meta(&slug) {
        Ok(m) => m,
        Err(_) => return Json(ApiResponse::error("Agent not found")),
    };

    let status = state
        .process
        .get_state(&slug)
        .await
        .map(|s| (s.status, s.pid))
        .unwrap_or((ProcessStatus::Stopped, None));

    Json(ApiResponse::success(AgentDetail {
        meta,
        status: status.0,
        pid: status.1,
    }))
}

async fn update_agent(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
    Json(updates): Json<serde_json::Value>,
) -> Json<ApiResponse<AgentMeta>> {
    let mut meta = match state.config.load_meta(&slug) {
        Ok(m) => m,
        Err(_) => return Json(ApiResponse::error("Agent not found")),
    };

    if let Some(name) = updates.get("display_name").and_then(|v| v.as_str()) {
        meta.display_name = name.to_string();
    }
    if let Some(desc) = updates.get("description").and_then(|v| v.as_str()) {
        meta.description = desc.to_string();
    }
    if let Some(port) = updates.get("port").and_then(|v| v.as_u64()) {
        meta.port = port as u16;
    }
    if let Some(v) = updates.get("auto_start").and_then(|v| v.as_bool()) {
        meta.auto_start = v;
    }
    if let Some(v) = updates.get("enabled").and_then(|v| v.as_bool()) {
        meta.enabled = v;
    }
    meta.updated_at = Utc::now().to_rfc3339();

    if let Err(e) = state.config.save_meta(&meta) {
        return Json(ApiResponse::error(e.to_string()));
    }

    Json(ApiResponse::success(meta))
}

async fn delete_agent(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
) -> Json<ApiResponse<String>> {
    let process = state.process.clone();
    let config = state.config.clone();
    let slug_owned = slug.clone();

    // Stop + delete in background
    tokio::spawn(async move {
        let _ = process.stop_agent(&slug_owned).await;
        let _ = config.delete_agent(&slug_owned);
        tracing::info!("Agent '{}' deleted", slug_owned);
    });

    Json(ApiResponse::success(format!("Agent '{}' deleting", slug)))
}

// ---------- Agent Lifecycle ----------

async fn start_agent(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
) -> Json<ApiResponse<serde_json::Value>> {
    // If the meta has auto_start disabled, enable it
    if let Ok(mut meta) = state.config.load_meta(&slug) {
        if !meta.auto_start {
            meta.auto_start = true;
            let _ = state.config.save_meta(&meta);
        }
    }

    let process = state.process.clone();
    let slug_owned = slug.clone();

    // Spawn on a separate task so the HTTP handler returns immediately
    tokio::spawn(async move {
        match process.start_agent(&slug_owned).await {
            Ok(_) => tracing::info!("Agent '{}' started", slug_owned),
            Err(e) => tracing::error!("Failed to start agent '{}': {}", slug_owned, e),
        }
    });

    Json(ApiResponse::success(serde_json::json!({
        "slug": slug,
        "status": "starting",
    })))
}

async fn stop_agent(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
) -> Json<ApiResponse<serde_json::Value>> {
    let process = state.process.clone();
    let slug_owned = slug.clone();
    tokio::spawn(async move {
        match process.stop_agent(&slug_owned).await {
            Ok(_) => tracing::info!("Agent '{}' stopped", slug_owned),
            Err(e) => tracing::error!("Failed to stop '{}': {}", slug_owned, e),
        }
    });
    Json(ApiResponse::success(serde_json::json!({
        "slug": slug,
        "status": "stopping",
    })))
}

async fn restart_agent(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
) -> Json<ApiResponse<serde_json::Value>> {
    let process = state.process.clone();
    let slug_owned = slug.clone();
    tokio::spawn(async move {
        let _ = process.stop_agent(&slug_owned).await;
        match process.start_agent(&slug_owned).await {
            Ok(_) => tracing::info!("Agent '{}' restarted", slug_owned),
            Err(e) => tracing::error!("Failed to restart '{}': {}", slug_owned, e),
        }
    });
    Json(ApiResponse::success(serde_json::json!({
        "slug": slug,
        "status": "restarting",
    })))
}

// ---------- Config ----------

async fn get_config(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
) -> Json<ApiResponse<serde_json::Value>> {
    match state.config.load_microclaw_config(&slug) {
        Ok(content) => Json(ApiResponse::success(serde_json::json!({
            "slug": slug,
            "content": content,
        }))),
        Err(e) => Json(ApiResponse::error(e.to_string())),
    }
}

#[derive(Debug, Deserialize)]
struct SaveConfigRequest {
    content: String,
}

async fn save_config(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
    Json(req): Json<SaveConfigRequest>,
) -> Json<ApiResponse<String>> {
    match state.config.save_microclaw_config(&slug, &req.content) {
        Ok(()) => Json(ApiResponse::success(format!(
            "Config for '{}' saved",
            slug
        ))),
        Err(e) => Json(ApiResponse::error(e.to_string())),
    }
}

// ---------- Logs ----------

async fn get_logs(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
) -> impl IntoResponse {
    let log_path = state.config.agent_logs_dir(&slug).join("microclaw.log");
    match std::fs::read_to_string(&log_path) {
        Ok(content) => {
            let lines: Vec<&str> = content.lines().rev().take(200).collect();
            let content: String = lines.into_iter().rev().collect::<Vec<_>>().join("\n");
            Json(ApiResponse::success(serde_json::json!({
                "slug": slug,
                "lines": content,
            })))
            .into_response()
        }
        Err(_) => Json(ApiResponse::<()>::error("No logs found")).into_response(),
    }
}

// ---------- Dashboard ----------

async fn get_dashboard(
    State(state): State<Arc<AppState>>,
) -> Json<ApiResponse<DashboardData>> {
    let agents = match state.config.list_agents() {
        Ok(a) => a,
        Err(e) => return Json(ApiResponse::error(e.to_string())),
    };

    let mut running = 0;
    let mut stopped = 0;
    let mut items = Vec::new();

    for m in agents {
        let status = state
            .process
            .get_state(&m.slug)
            .await
            .map(|s| s.status)
            .unwrap_or(ProcessStatus::Stopped);

        match status {
            ProcessStatus::Running | ProcessStatus::Starting => running += 1,
            _ => stopped += 1,
        }

        items.push(AgentListItem {
            slug: m.slug.clone(),
            display_name: m.display_name,
            description: m.description,
            port: m.port,
            status,
            enabled: m.enabled,
            auto_start: m.auto_start,
        });
    }

    Json(ApiResponse::success(DashboardData {
        total: items.len(),
        running,
        stopped,
        agents: items,
    }))
}

// ---------- Status ----------

async fn get_status(
    State(state): State<Arc<AppState>>,
) -> Json<ApiResponse<serde_json::Value>> {
    let agents = state.config.list_agents().unwrap_or_default();
    let mut running = 0;
    for a in &agents {
        if let Some(s) = state.process.get_state(&a.slug).await {
            if s.status == ProcessStatus::Running {
                running += 1;
            }
        }
    }
    Json(ApiResponse::success(serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "binary": std::env::current_exe().ok().map(|p| p.to_string_lossy().to_string()),
        "dashboard_url": format!("http://127.0.0.1:9090"),
        "agents": agents.len(),
        "running": running,
    })))
}
