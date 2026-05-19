use crate::api::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use std::sync::Arc;

async fn agent_url(state: &AppState, slug: &str, path: &str) -> Result<String, String> {
    let meta = state
        .config
        .load_meta(slug)
        .map_err(|_| format!("Agent '{}' not found", slug))?;
    Ok(format!("http://127.0.0.1:{}/api/{}", meta.port, path))
}

/// Login and return cookie. Cached in state.agent_cookies.
async fn ensure_auth(state: &AppState, slug: &str) -> Result<String, String> {
    // Check cached cookie
    {
        let cookies = state.agent_cookies.lock().await;
        if let Some(cookie) = cookies.get(slug) {
            // Quick verify
            let url = agent_url(state, slug, "auth/status").await?;
            let client = reqwest::Client::new();
            let resp = client
                .get(&url)
                .header("cookie", cookie.as_str())
                .timeout(std::time::Duration::from_secs(5))
                .send()
                .await;
            if let Ok(r) = resp {
                if let Ok(status) = r.json::<serde_json::Value>().await {
                    if status.get("authenticated").and_then(|v| v.as_bool()).unwrap_or(false) {
                        return Ok(cookie.clone());
                    }
                }
            }
        }
    }

    // Login
    let url = agent_url(state, slug, "auth/login").await?;
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| format!("Client build: {}", e))?;

    let resp = client
        .post(&url)
        .json(&serde_json::json!({"password": "helloworld"}))
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
        .map_err(|e| format!("Login failed: {}", e))?;

    let cookie = resp
        .headers()
        .get("set-cookie")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    if cookie.is_empty() {
        return Err("Login failed - no session cookie received".to_string());
    }

    // Cache it
    state.agent_cookies.lock().await.insert(slug.to_string(), cookie.clone());
    Ok(cookie)
}

async fn fetch_json(state: &AppState, slug: &str, path: &str) -> Result<serde_json::Value, String> {
    let cookie = ensure_auth(state, slug).await?;
    let url = agent_url(state, slug, path).await?;
    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .header("cookie", &cookie)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| format!("Connection failed: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("Agent returned {}: {}", status, &text[..text.len().min(200)]));
    }

    resp.json::<serde_json::Value>()
        .await
        .map_err(|e| format!("Parse failed: {}", e))
}

pub fn proxy_ok(data: serde_json::Value) -> (StatusCode, Json<serde_json::Value>) {
    (StatusCode::OK, Json(serde_json::json!({"ok":true,"data":data})))
}

pub fn proxy_err(msg: impl Into<String>) -> (StatusCode, Json<serde_json::Value>) {
    (StatusCode::BAD_GATEWAY, Json(serde_json::json!({"ok":false,"error":msg.into()})))
}

pub fn proxy_not_found(msg: impl Into<String>) -> (StatusCode, Json<serde_json::Value>) {
    (StatusCode::NOT_FOUND, Json(serde_json::json!({"ok":false,"error":msg.into()})))
}

// ---------- Endpoint handlers ----------

/// GET /api/proxy/:slug/health
pub async fn proxy_health(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
) -> impl IntoResponse {
    let url = match agent_url(&state, &slug, "health").await {
        Ok(u) => u,
        Err(e) => return proxy_not_found(e),
    };
    let client = reqwest::Client::new();
    match client.get(&url).timeout(std::time::Duration::from_secs(5)).send().await {
        Ok(resp) => match resp.json::<serde_json::Value>().await {
            Ok(data) => proxy_ok(data),
            Err(e) => proxy_err(e.to_string()),
        },
        Err(e) => proxy_err(format!("Agent unreachable: {}", e)),
    }
}

/// GET /api/proxy/:slug/config (from local filesystem)
pub async fn proxy_get_config(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
) -> impl IntoResponse {
    let meta = match state.config.load_meta(&slug) {
        Ok(m) => m,
        Err(_) => return proxy_not_found("Agent not found"),
    };
    match state.config.load_microclaw_config(&slug) {
        Ok(content) => proxy_ok(serde_json::json!({"slug":slug,"port":meta.port,"content":content})),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"ok":false,"error":e.to_string()}))),
    }
}

/// GET /api/proxy/:slug/config/self_check
pub async fn proxy_config_self_check(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
) -> impl IntoResponse {
    match fetch_json(&state, &slug, "config/self_check").await {
        Ok(data) => proxy_ok(data),
        Err(e) => proxy_err(e),
    }
}

/// GET /api/proxy/:slug/sessions
pub async fn proxy_sessions(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
) -> impl IntoResponse {
    match fetch_json(&state, &slug, "sessions").await {
        Ok(data) => proxy_ok(data),
        Err(e) => proxy_err(e),
    }
}

/// GET /api/proxy/:slug/history
pub async fn proxy_history(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
) -> impl IntoResponse {
    match fetch_json(&state, &slug, "history").await {
        Ok(data) => proxy_ok(data),
        Err(e) => proxy_err(e),
    }
}

/// GET /api/proxy/:slug/audit
pub async fn proxy_audit(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
) -> impl IntoResponse {
    match fetch_json(&state, &slug, "audit").await {
        Ok(data) => proxy_ok(data),
        Err(e) => proxy_err(e),
    }
}

/// GET /api/proxy/:slug/usage
pub async fn proxy_usage(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
) -> impl IntoResponse {
    match fetch_json(&state, &slug, "usage").await {
        Ok(data) => proxy_ok(data),
        Err(e) => proxy_err(e),
    }
}

/// GET /api/proxy/:slug/skills
pub async fn proxy_skills(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
) -> impl IntoResponse {
    match fetch_json(&state, &slug, "skills").await {
        Ok(data) => proxy_ok(data),
        Err(e) => proxy_err(e),
    }
}

/// GET /api/proxy/:slug/metrics
pub async fn proxy_metrics(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
) -> impl IntoResponse {
    match fetch_json(&state, &slug, "metrics/summary").await {
        Ok(data) => proxy_ok(data),
        Err(e) => proxy_err(e),
    }
}

/// GET /api/proxy/:slug/metrics/history
pub async fn proxy_metrics_history(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
) -> impl IntoResponse {
    match fetch_json(&state, &slug, "metrics/history").await {
        Ok(data) => proxy_ok(data),
        Err(e) => proxy_err(e),
    }
}

/// GET /api/proxy/:slug/memory_observability
pub async fn proxy_memory_observability(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
) -> impl IntoResponse {
    match fetch_json(&state, &slug, "memory_observability").await {
        Ok(data) => proxy_ok(data),
        Err(e) => proxy_err(e),
    }
}

/// GET /api/proxy/:slug/auth/status
pub async fn proxy_auth_status(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
) -> impl IntoResponse {
    match fetch_json(&state, &slug, "auth/status").await {
        Ok(data) => proxy_ok(data),
        Err(e) => proxy_err(e),
    }
}

/// GET /api/proxy/:slug/a2a/agent_card
pub async fn proxy_a2a_agent_card(
    State(state): State<Arc<AppState>>,
    Path(slug): Path<String>,
) -> impl IntoResponse {
    match fetch_json(&state, &slug, "a2a/agent-card").await {
        Ok(data) => proxy_ok(data),
        Err(e) => proxy_err(e),
    }
}
